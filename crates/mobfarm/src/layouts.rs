//! Layout generators (`mctas mobfarm gen <layout>`): each writes a design text with its caption.
//! Coordinates: the villagers' bed heads at y = 0, z = 0, north is -z.

use mctas::design::{Design, EntitySpec};
use std::collections::BTreeMap;

/// A design under construction.
pub struct Builder {
    pub d: Design,
}

impl Builder {
    pub fn new(name: &str) -> Builder {
        Builder { d: Design { name: name.to_string(), meta: BTreeMap::new(), ports: BTreeMap::new(), blocks: BTreeMap::new(), entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() } }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, s: &str) {
        let s = if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") };
        self.d.blocks.insert([x, y, z], s);
    }

    pub fn entity(&mut self, kind: &str, pos: [f64; 3], nbt: Option<&str>) {
        self.d.entities.push(EntitySpec { kind: format!("minecraft:{kind}"), pos, nbt: nbt.map(|s| s.to_string()) });
    }

    /// A villager on the head of its bed (facing north) in a closed 1-cell pod at (x, 0, 0): the
    /// foot capped so it cannot stand there, glass walls, an open trapdoor across the pod's
    /// north face at y+1 (its eyes see out over and under it, it cannot pass), a roof of leaves.
    pub fn pod(&mut self, x: i32) {
        self.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
        self.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
        for z in 0..2 {
            self.set(x, -1, z, "glass");
        }
        self.set(x, 1, 1, "glass");
        self.set(x, 2, 1, "glass");
        for w in [x - 1, x + 1] {
            self.set(w, 1, 0, "glass");
            self.set(w, 2, 0, "glass");
        }
        self.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
        for dx in -1..=1 {
            for z in 0..2 {
                self.set(x + dx, 3, z, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
            }
        }
        self.entity("villager", [x as f64 + 0.5, 0.5625, 0.5], None);
    }

    /// [`Builder::pod`] in day-1 materials (vjeux 13:09: no glass, leaves or stone on day 1): a
    /// cobblestone floor and walls (the villager sees out over its north trapdoor only), and a
    /// roof of bottom cobblestone slabs (a golem's spawn fails on a half top, as on leaves).
    pub fn pod_day1(&mut self, x: i32) {
        self.set(x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
        self.set(x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
        for z in 0..2 {
            self.set(x, -1, z, STONE);
        }
        self.set(x, 1, 1, STONE);
        self.set(x, 2, 1, STONE);
        for w in [x - 1, x + 1] {
            self.set(w, 1, 0, STONE);
            self.set(w, 2, 0, STONE);
        }
        self.set(x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
        for dx in -1..=1 {
            for z in 0..2 {
                self.set(x + dx, 3, z, CAP);
            }
        }
        self.entity("villager", [x as f64 + 0.5, 0.5625, 0.5], None);
    }

    pub fn text(&self, caption: &[&str]) -> String {
        let mut s = String::new();
        for l in caption {
            s.push_str("# ");
            s.push_str(l);
            s.push('\n');
        }
        s.push_str(&self.d.to_text());
        s
    }
}

// (roles, designs/farms/roles.tsv: a structural, floor or fluid-holding block is cobblestone, the
// cheapest at every stage's prices; mining stone without silk touch gives cobblestone)
const STONE: &str = "cobblestone";
const GLASS: &str = "glass";
/// A day-1 cap: a bottom cobblestone slab (no golem spawns on its half top; 3 cobblestone make 6).
const CAP: &str = "cobblestone_slab[type=bottom,waterlogged=false]";
const LAVA: &str = "lava[level=0]";
const SHUT: &str = "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]";

/// The zombie of every layout: a boat on a slab 7 north of the pods, a roof of leaves over its
/// eyes, and the trapdoor in front of it that hides it when powered.
fn zombie(b: &mut Builder) {
    b.set(2, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(2, 2, -7, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    b.set(2, 1, -6, SHUT);
}

fn wall_sign(b: &mut Builder, x: i32, y: i32, z: i32, facing: &str) {
    b.set(x, y, z, &format!("oak_wall_sign[facing={facing},waterlogged=false]"));
}

/// The farm's single chest (at its `port output`: the judge reads it, the download holds it).
pub const CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";

/// A design text edited, its leading comment block (the caption) kept: `chest` puts the farm's
/// single chest at its `port output`, `how` sets its `how:` line.
pub fn edit(text: &str, chest: bool, how: Option<&str>) -> anyhow::Result<String> {
    edit_meta(text, chest, how, &[])
}

/// [`edit`], and each `key=value` of `meta` set as a meta line (`sample=village-edge`: judge a
/// void design in that terrain sample; an empty value removes the line).
pub fn edit_meta(text: &str, chest: bool, how: Option<&str>, meta: &[String]) -> anyhow::Result<String> {
    let mut d = Design::parse(text)?;
    for kv in meta {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("--meta {kv:?}: not key=value"))?;
        if v.trim().is_empty() {
            d.meta.remove(k.trim());
        } else {
            d.meta.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    if chest {
        let out = *d.ports.get("output").ok_or_else(|| anyhow::anyhow!("no `port output` for the chest"))?;
        match d.blocks.get(&out) {
            None => {
                d.blocks.insert(out, CHEST.to_string());
            }
            Some(s) if s.trim_start_matches("minecraft:").starts_with("chest[") => {}
            Some(s) => anyhow::bail!("port output {out:?} holds {s}"),
        }
    }
    if let Some(h) = how {
        d.meta.insert("how".to_string(), h.to_string());
    }
    // (farmopt / ironspec Nov 13 01:08: every `#` line kept, not only the leading caption)
    Ok(keep_comments(text, &d.to_text()).0)
}

/// (ironspec Nov 13 23:53: V judged with a re-laid part; G1 Nov 14 00:40b P1 / P4) A composed part swapped
/// in a design's cells: every cell `old` holds, moved by `off`, is cleared where the design still holds
/// old's block there; a cell holding another block is another owner's by the compose's precedence (kept and
/// named); a cell the design doesn't hold at all is ABSENT (named): the old part must be found, at least one
/// cell cleared and every non-air cell either cleared or kept, else the swap is refused (a missing or
/// misaligned OLD). Then every cell `new` holds is set (an air cell of `new` clears its cell only when the
/// cell is empty or was the old part's: another owner's block there is a clash), and a cell the design
/// still holds with a different block is a clash: an error naming every one. Every changed cell printed,
/// `x,y,z: before -> after`. Returns the lines and how many cells of the design changed.
pub fn swap_part(blocks: &mut BTreeMap<[i32; 3], String>, old: &BTreeMap<[i32; 3], String>, new: &BTreeMap<[i32; 3], String>, off: [i32; 3]) -> Result<(Vec<String>, usize), String> {
    let before = blocks.clone();
    let mv = |p: &[i32; 3]| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
    let air = |s: &str| s == "minecraft:air";
    let (mut cleared, mut kept, mut absent) = (Vec::new(), Vec::new(), Vec::new());
    for (p, s) in old.iter().filter(|(_, s)| !air(s)) {
        let q = mv(p);
        match blocks.get(&q) {
            Some(b) if b == s => {
                blocks.remove(&q);
                cleared.push(q);
            }
            Some(b) => kept.push(format!("{},{},{} {}", q[0], q[1], q[2], b.trim_start_matches("minecraft:"))),
            None => absent.push(format!("{},{},{} ({})", q[0], q[1], q[2], s.trim_start_matches("minecraft:"))),
        }
    }
    let n_old = old.values().filter(|s| !air(s)).count();
    if cleared.is_empty() || cleared.len() + kept.len() != n_old {
        return Err(format!(
            "the old part is not in the design at offset {},{},{}: of its {n_old} cell(s), {} cleared, {} held by another owner, {} ABSENT{}",
            off[0],
            off[1],
            off[2],
            cleared.len(),
            kept.len(),
            absent.len(),
            if absent.is_empty() { String::new() } else { format!(": {}", absent.iter().take(12).cloned().collect::<Vec<_>>().join(", ")) }
        ));
    }
    let mut clashes = Vec::new();
    for (p, s) in new {
        let q = mv(p);
        let cur = blocks.get(&q).cloned();
        match cur {
            Some(b) if air(s) && !cleared.contains(&q) => clashes.push(format!("{},{},{} holds {} (another owner's; the new part clears it)", q[0], q[1], q[2], b.trim_start_matches("minecraft:"))),
            Some(b) if !air(s) && b != *s => clashes.push(format!("{},{},{} holds {} (the new part's {})", q[0], q[1], q[2], b.trim_start_matches("minecraft:"), s.trim_start_matches("minecraft:"))),
            _ if air(s) => {
                blocks.remove(&q);
            }
            _ => {
                blocks.insert(q, s.clone());
            }
        }
    }
    if !clashes.is_empty() {
        return Err(format!("{} clash(es) with cells another owner holds: {}", clashes.len(), clashes.join("; ")));
    }
    let mut changes = Vec::new();
    let short = |s: Option<&String>| s.map(|s| s.trim_start_matches("minecraft:").to_string()).unwrap_or_else(|| "air".to_string());
    let keys: std::collections::BTreeSet<[i32; 3]> = before.keys().chain(blocks.keys()).copied().collect();
    for q in keys {
        let (a, b) = (before.get(&q), blocks.get(&q));
        if a != b {
            changes.push(format!("  {},{},{}: {} -> {}", q[0], q[1], q[2], short(a), short(b)));
        }
    }
    let mut lines = vec![format!(
        "swap: the old part's {n_old} cell(s) at offset {},{},{}: {} cleared, {} held by another owner (kept){}; the new part's {} cell(s) set; {} cell(s) of the design changed:",
        off[0],
        off[1],
        off[2],
        cleared.len(),
        kept.len(),
        if kept.is_empty() { String::new() } else { format!(": {}", kept.join(", ")) },
        new.len(),
        changes.len()
    )];
    let n = changes.len();
    lines.extend(changes);
    Ok((lines, n))
}

/// The declaration keys a swap moves with its part (declkeys' comment and header keys).
fn decl_key(line: &str) -> Option<(bool, &'static str, &str)> {
    let mut keys: Vec<(&'static str, bool)> = crate::declkeys::COMMENT_KEYS.iter().map(|k| (*k, true)).chain(crate::declkeys::EITHER_KEYS.iter().map(|k| (*k, true))).chain(crate::declkeys::META_KEYS.iter().map(|k| (*k, false))).chain(crate::declkeys::EITHER_KEYS.iter().map(|k| (*k, false))).collect();
    keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let t = line.trim_start();
    if let Some(c) = t.strip_prefix('#') {
        let c = c.trim_start();
        for (k, comment) in &keys {
            if *comment && let Some(body) = c.strip_prefix(k).and_then(|r| r.strip_prefix(':')) {
                return Some((true, k, body));
            }
        }
        return None;
    }
    for (k, comment) in &keys {
        if !*comment && let Some(body) = t.strip_prefix(k).and_then(|r| r.strip_prefix(':')) {
            return Some((false, k, body));
        }
    }
    None
}

/// A declaration body's entries (`pushscan::header_entries`' split: a `;` part beginning `cells=` starts
/// an entry).
fn decl_entries(body: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    for part in body.split(';') {
        let head = part.trim_start();
        let starts = head.split_once('=').is_some_and(|(c, _)| !c.trim().is_empty() && c.split(['+', ',', '>']).all(|t| t.trim().parse::<i32>().is_ok()));
        if starts && !cur.trim().is_empty() {
            v.push(cur.trim().to_string());
            cur = String::new();
        }
        if !cur.is_empty() {
            cur.push(';');
        }
        cur.push_str(part);
    }
    if !cur.trim().is_empty() {
        v.push(cur.trim().to_string());
    }
    v
}

/// The cells an entry names: every `x,y,z` before its first `=` (`+` and `>` separate them).
fn entry_cells(entry: &str) -> Vec<[i32; 3]> {
    let key = entry.split_once('=').map(|(k, _)| k).unwrap_or("");
    key.split(['+', '>']).filter_map(|t| {
        let v: Vec<i32> = t.trim().split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    }).collect()
}

/// The entry with its key cells moved by `off`.
fn shift_entry(entry: &str, off: [i32; 3]) -> String {
    let Some((key, rest)) = entry.split_once('=') else { return entry.to_string() };
    let mut out = String::new();
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        let v: Option<Vec<i32>> = tok.trim().split(',').map(|n| n.trim().parse::<i32>().ok()).collect();
        match v {
            Some(v) if v.len() == 3 => out.push_str(&format!("{},{},{}", v[0] + off[0], v[1] + off[1], v[2] + off[2])),
            _ => out.push_str(tok),
        }
        tok.clear();
    };
    for ch in key.chars() {
        if ch == '+' || ch == '>' {
            flush(&mut tok, &mut out);
            out.push(ch);
        } else {
            tok.push(ch);
        }
    }
    flush(&mut tok, &mut out);
    format!("{out}={rest}")
}

/// (G1 Nov 14 00:40b P3) A part's declarations swapped with its cells: every declaration entry of `text`
/// whose cells all lie in the old part's footprint `old_cells` (in the design's frame) is dropped, by
/// name; an entry with only some of its cells there is refused; every declaration entry of `new_text` is
/// carried with its cells moved by `off` and "(carried from the swapped part, moved by dx,dy,dz)" (a header
/// key's entries joined to the design's line of that key, or a new line before `palette:`). Returns the
/// text and the lines printed.
pub fn swap_declarations(text: &str, new_text: &str, old_cells: &std::collections::BTreeSet<[i32; 3]>, off: [i32; 3]) -> Result<(String, Vec<String>), String> {
    let mut report = Vec::new();
    let mut carried: Vec<(bool, &'static str, String)> = Vec::new();
    for l in new_text.lines() {
        if let Some((comment, k, body)) = decl_key(l) {
            for e in decl_entries(body) {
                let s = shift_entry(&e, off);
                report.push(format!("  carried {}{k}: {s}", if comment { "# " } else { "" }));
                carried.push((comment, k, format!("{s} (carried from the swapped part, moved by {},{},{})", off[0], off[1], off[2])));
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut header_done: Vec<&'static str> = Vec::new();
    for l in text.lines() {
        let Some((comment, k, body)) = decl_key(l) else {
            if l.trim_start().starts_with("palette:") {
                for (c, key, e) in carried.iter().filter(|(c, key, _)| *c || !header_done.contains(key)) {
                    if *c {
                        out.push(format!("# {key}: {e}"));
                    }
                }
                let mut heads: Vec<&'static str> = carried.iter().filter(|(c, key, _)| !*c && !header_done.contains(key)).map(|(_, key, _)| *key).collect();
                heads.dedup();
                for key in heads {
                    let es: Vec<&str> = carried.iter().filter(|(c, k2, _)| !*c && *k2 == key).map(|(_, _, e)| e.as_str()).collect();
                    out.push(format!("{key}: {}", es.join("; ")));
                    header_done.push(key);
                }
                carried.retain(|(c, _, _)| !*c);
            }
            out.push(l.to_string());
            continue;
        };
        let mut keep = Vec::new();
        for e in decl_entries(body) {
            let cells = entry_cells(&e);
            let inside = cells.iter().filter(|c| old_cells.contains(*c)).count();
            if inside > 0 && inside < cells.len() {
                return Err(format!("the declaration `{k}: {e}` names cells both in the old part's footprint and outside it: split it by hand"));
            }
            if !cells.is_empty() && inside == cells.len() {
                report.push(format!("  dropped {}{k}: {e}", if comment { "# " } else { "" }));
            } else {
                keep.push(e);
            }
        }
        if !comment && !header_done.contains(&k) {
            for (_, _, e) in carried.iter().filter(|(c, k2, _)| !*c && *k2 == k) {
                keep.push(e.clone());
            }
            header_done.push(k);
        }
        if !keep.is_empty() {
            out.push(if comment { format!("# {k}: {}", keep.join("; ")) } else { format!("{k}: {}", keep.join("; ")) });
        }
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Ok((s, report))
}

/// (ironspec Nov 13 23:53, G1 Nov 14 00:40b P1-P4) `mobfarm edit --swap-part`: each (OLD's text, NEW's text,
/// offset) swapped into `original`'s cells (`swap_part`: OLD found in full, every changed cell printed, no
/// clash, no no-op) and declarations (`swap_declarations`), the `#` lines kept; the composed text read back:
/// its cells equal the composed cells, every NEW cell present with its block (moved), every OLD-only cell
/// gone unless another owner held it. Returns the text and the lines printed.
pub fn edit_swap(original: &str, swaps: &[(String, String, [i32; 3])]) -> anyhow::Result<(String, Vec<String>)> {
    let mut d = Design::parse(original)?;
    let mut lines = Vec::new();
    let mut parts = Vec::new();
    for (old_text, new_text, off) in swaps {
        let (old_d, new_d) = (Design::parse(old_text)?, Design::parse(new_text)?);
        let (l, n) = swap_part(&mut d.blocks, &old_d.blocks, &new_d.blocks, *off).map_err(|e| anyhow::anyhow!("--swap-part at {},{},{}: {e}", off[0], off[1], off[2]))?;
        anyhow::ensure!(n > 0, "NO-OP --swap-part at {},{},{}: no cell changes", off[0], off[1], off[2]);
        lines.extend(l);
        parts.push((old_d, new_d, *off));
    }
    let (mut out, kept, moved) = keep_comments(original, &d.to_text());
    let before = original.lines().filter(|l| l.starts_with('#')).count();
    anyhow::ensure!(kept + moved == before, "edit: {before} `#` lines read, {} written", kept + moved);
    for ((old_d, _, off), (_, new_text, _)) in parts.iter().zip(swaps) {
        let old_cells: std::collections::BTreeSet<[i32; 3]> = old_d.blocks.iter().filter(|(_, s)| s.as_str() != "minecraft:air").map(|(p, _)| [p[0] + off[0], p[1] + off[1], p[2] + off[2]]).collect();
        let (o, rep) = swap_declarations(&out, new_text, &old_cells, *off).map_err(|e| anyhow::anyhow!(e))?;
        lines.push(format!("declarations: {} dropped (the old part's), {} carried (the new part's, moved by {},{},{})", rep.iter().filter(|l| l.starts_with("  dropped")).count(), rep.iter().filter(|l| l.starts_with("  carried")).count(), off[0], off[1], off[2]));
        lines.extend(rep);
        out = o;
    }
    let back = Design::parse(&out)?;
    anyhow::ensure!(back.blocks == d.blocks, "swap read-back: the composed text's cells differ from the composed cells");
    for (old_d, new_d, off) in &parts {
        let mv = |p: &[i32; 3]| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        for (p, s) in new_d.blocks.iter().filter(|(_, s)| s.as_str() != "minecraft:air") {
            anyhow::ensure!(back.blocks.get(&mv(p)) == Some(s), "swap read-back: the new part's {},{},{} ({s}) is missing at {:?}", p[0], p[1], p[2], mv(p));
        }
        for (p, s) in old_d.blocks.iter().filter(|(q, s)| s.as_str() != "minecraft:air" && !new_d.blocks.contains_key(*q)) {
            anyhow::ensure!(back.blocks.get(&mv(p)) != Some(s), "swap read-back: the old part's {},{},{} ({s}) is still at {:?}", p[0], p[1], p[2], mv(p));
        }
    }
    lines.push(format!("swap read-back: {} cell(s), every new part cell present, every old-only cell gone", back.blocks.len()));
    Ok((out, lines))
}

#[cfg(test)]
mod swap_part_tests {
    /// (ironspec Nov 13 23:53, G1 Nov 14 00:40b P1 / P4) A part swapped: its old cells cleared where the
    /// design holds them, the new ones set, every changed cell printed; a cell another owner holds is kept
    /// and named; an old part not found in full (an offset off by one) is refused naming its absent cells;
    /// a new cell on another owner's block, or a new part's air over it, is a clash.
    #[test]
    fn swap_clears_sets_keeps_and_clashes() {
        let m = |v: &[([i32; 3], &str)]| v.iter().map(|(p, s)| (*p, format!("minecraft:{s}"))).collect::<std::collections::BTreeMap<_, _>>();
        let old = m(&[([0, 0, 0], "stone"), ([1, 0, 0], "glass"), ([2, 0, 0], "glass")]);
        let new = m(&[([0, 0, 0], "stone"), ([3, 0, 0], "redstone_lamp[lit=false]")]);
        // (the design at offset 10,0,0: the old part's cells, but 12,0,0 holds another owner's dirt)
        let mut d = m(&[([10, 0, 0], "stone"), ([11, 0, 0], "glass"), ([12, 0, 0], "dirt"), ([20, 0, 0], "sand")]);
        let (l, n) = super::swap_part(&mut d, &old, &new, [10, 0, 0]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(n, 2, "11 removed, 13 added: {l:?}");
        assert!(!d.contains_key(&[11, 0, 0]) && d.get(&[13, 0, 0]).is_some_and(|s| s.contains("lamp")) && d.get(&[12, 0, 0]).is_some_and(|s| s.contains("dirt")) && d.contains_key(&[20, 0, 0]), "{d:?}");
        assert!(l[0].contains("2 cleared, 1 held by another owner (kept): 12,0,0 dirt"), "{l:?}");
        assert!(l.iter().any(|x| x == "  11,0,0: glass -> air") && l.iter().any(|x| x == "  13,0,0: air -> redstone_lamp[lit=false]"), "every changed cell printed: {l:?}");
        // (P1) the offset off by one: the old part's cells are not there: refused, its absent cells named
        let mut d1 = m(&[([10, 0, 0], "stone"), ([11, 0, 0], "glass"), ([12, 0, 0], "glass")]);
        let e1 = super::swap_part(&mut d1, &old, &new, [11, 0, 0]).err().unwrap_or_default();
        assert!(e1.contains("the old part is not in the design at offset 11,0,0") && e1.contains("ABSENT") && e1.contains("13,0,0 (glass)"), "off by one: {e1}");
        // a new cell on another owner's block: a clash
        let new2 = m(&[([0, 0, 0], "stone"), ([5, 0, 0], "glass")]);
        let mut d2 = m(&[([10, 0, 0], "stone"), ([11, 0, 0], "glass"), ([12, 0, 0], "glass"), ([15, 0, 0], "sand")]);
        let e = super::swap_part(&mut d2, &old, &new2, [10, 0, 0]).err().unwrap_or_default();
        assert!(e.contains("1 clash") && e.contains("15,0,0 holds sand"), "{e}");
        // (P4) the new part's air over another owner's block: a clash, never a silent removal
        let new3 = m(&[([0, 0, 0], "stone"), ([5, 0, 0], "air")]);
        let mut d3 = m(&[([10, 0, 0], "stone"), ([11, 0, 0], "glass"), ([12, 0, 0], "glass"), ([15, 0, 0], "sand")]);
        let e3 = super::swap_part(&mut d3, &old, &new3, [10, 0, 0]).err().unwrap_or_default();
        assert!(e3.contains("15,0,0 holds sand (another owner's; the new part clears it)"), "{e3}");
    }

    /// (G1 Nov 14 00:40b P2 / P3) `edit_swap` end to end on a design text: the old part's declarations (a
    /// `# switches:` entry and a `riderless:` header entry on its cell) dropped by name, the new part's
    /// carried moved by the offset, another owner's kept; the composed text read back (its cells, the new
    /// part present, the old-only cell gone).
    #[test]
    fn edit_swap_moves_declarations_and_reads_back() {
        let design = "name: t\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\n# switches: 1,0,0=the old part's lever; 2,0,0=another owner's lever\nriderless: 1,0,0=the old part's seat\npalette:\n  s minecraft:stone\n  g minecraft:glass\nlayer 0:\n  sgs\n";
        let old = "name: old\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\n# switches: 0,0,0=the old part's lever\npalette:\n  g minecraft:glass\nlayer 0:\n  g\n";
        let new = "name: new\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\n# switches: 0,0,1=the new part's lever\nriderless: 0,0,1=the new part's seat\npalette:\n  l minecraft:redstone_lamp[lit=false]\nlayer 0:\n  .\n  l\n";
        let (out, lines) = super::edit_swap(design, &[(old.to_string(), new.to_string(), [1, 0, 0])]).unwrap_or_else(|e| panic!("{e}"));
        assert!(lines.iter().any(|l| l == "  dropped # switches: 1,0,0=the old part's lever") && lines.iter().any(|l| l == "  dropped riderless: 1,0,0=the old part's seat"), "the old declarations dropped: {lines:?}");
        assert!(lines.iter().any(|l| l == "  carried # switches: 1,0,1=the new part's lever") && lines.iter().any(|l| l == "  carried riderless: 1,0,1=the new part's seat"), "the new ones carried: {lines:?}");
        assert!(out.contains("# switches: 2,0,0=another owner's lever") && !out.contains("1,0,0=the old part's") && out.contains("riderless: 1,0,1=the new part's seat (carried from the swapped part, moved by 1,0,0)"), "{out}");
        let back = mctas::design::Design::parse(&out).unwrap();
        assert!(back.blocks.get(&[1, 0, 1]).is_some_and(|s| s.contains("lamp")) && !back.blocks.contains_key(&[1, 0, 0]) && back.blocks.get(&[2, 0, 0]).is_some_and(|s| s.contains("stone")), "{:?}", back.blocks);
        assert!(lines.iter().any(|l| l.starts_with("swap read-back:")), "{lines:?}");
    }
}

/// (9fc3155f, farmopt / ironspec Nov 13 01:08: `mobfarm edit` dropped every `#` line of a design it
/// rewrote, so a variant lost its `# switches:` / `# update-only:` declarations) The regenerated
/// text `new` with every `#` line of `orig` put back: each run of comment lines after the line it
/// followed in `orig` (that line's same occurrence in `new`; the leading caption at the top); a
/// run whose line is gone from `new` goes after the caption, counted as moved. Returns the text,
/// the comment lines kept in place and those moved.
pub fn keep_comments(orig: &str, new: &str) -> (String, usize, usize) {
    // (each comment run: the non-comment line before it and that line's occurrence index)
    let mut runs: Vec<(Option<(String, usize)>, Vec<&str>)> = Vec::new();
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut anchor: Option<(String, usize)> = None;
    let mut in_run = false;
    for l in orig.lines() {
        if l.starts_with('#') {
            if !in_run {
                runs.push((anchor.clone(), Vec::new()));
                in_run = true;
            }
            runs.last_mut().expect("a run").1.push(l);
        } else {
            in_run = false;
            let k = seen.entry(l).or_insert(0);
            anchor = Some((l.to_string(), *k));
            *k += 1;
        }
    }
    let new_lines: Vec<&str> = new.lines().filter(|l| !l.starts_with('#')).collect();
    // (where each anchor sits in `new`: its occurrence index)
    let mut at: std::collections::HashMap<(String, usize), usize> = std::collections::HashMap::new();
    let mut seen_new: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, l) in new_lines.iter().enumerate() {
        let k = seen_new.entry(l).or_insert(0);
        at.insert((l.to_string(), *k), i);
        *k += 1;
    }
    let mut top: Vec<&str> = Vec::new();
    let mut after: std::collections::HashMap<usize, Vec<&str>> = std::collections::HashMap::new();
    let (mut kept, mut moved) = (0usize, 0usize);
    let mut moved_lines: Vec<&str> = Vec::new();
    for (a, lines) in &runs {
        match a {
            None => {
                kept += lines.len();
                top.extend(lines.iter().copied());
            }
            Some(key) => match at.get(key) {
                Some(i) => {
                    kept += lines.len();
                    after.entry(*i).or_default().extend(lines.iter().copied());
                }
                None => {
                    moved += lines.len();
                    moved_lines.extend(lines.iter().copied());
                }
            },
        }
    }
    let mut s = String::new();
    for l in top.iter().chain(moved_lines.iter()) {
        s.push_str(l);
        s.push('\n');
    }
    for (i, l) in new_lines.iter().enumerate() {
        s.push_str(l);
        s.push('\n');
        if let Some(v) = after.get(&i) {
            for c in v {
                s.push_str(c);
                s.push('\n');
            }
        }
    }
    (s, kept, moved)
}

/// (ironspec / G1 Nov 14 07:31r, part 58) A design text's palette, in its file order: (char, state) from the
/// `palette:` block's indented lines.
fn palette_of(text: &str) -> Vec<(char, String)> {
    let mut out = Vec::new();
    let mut inp = false;
    for l in text.lines() {
        if l.trim_end() == "palette:" {
            inp = true;
            continue;
        }
        if inp {
            if !l.starts_with("  ") || l.trim().is_empty() {
                break;
            }
            let mut it = l[2..].chars();
            if let Some(c) = it.next() {
                let state = it.as_str().trim().to_string();
                if !state.is_empty() {
                    out.push((c, state));
                }
            }
        }
    }
    out
}

/// (ironspec / G1 Nov 14 07:31r, part 58) The re-serialized text `generated` with the ORIGINAL file's palette
/// characters kept: each state the original names keeps its character (in the original's order), a new state takes
/// a character the original doesn't use, and every layer row is re-lettered to match, so an edit leaves every row of
/// a layer it doesn't touch byte for byte as it was (the plain re-serialization re-letters the whole palette).
pub fn stable_palette(original: &str, generated: &str) -> String {
    let orig = palette_of(original);
    let gen_p = palette_of(generated);
    let orig_by_state: BTreeMap<&str, char> = orig.iter().map(|(c, s)| (s.as_str(), *c)).collect();
    let mut used: std::collections::BTreeSet<char> = orig.iter().map(|(c, _)| *c).collect();
    let pool: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$%&*+=?!<>^~/\\|;,'`\"(){}"
        .chars()
        .chain(('À'..='ÿ').filter(|c| c.is_alphabetic()))
        .chain(('Α'..='ω').filter(|c| c.is_alphabetic()))
        .chain(('Ѐ'..='ӿ').filter(|c| c.is_alphabetic()))
        .collect();
    let mut remap: BTreeMap<char, char> = BTreeMap::new();
    for (gc, s) in &gen_p {
        let to = match orig_by_state.get(s.as_str()) {
            Some(c) => *c,
            None => {
                let c = pool.iter().copied().find(|c| !used.contains(c)).unwrap_or(*gc);
                used.insert(c);
                c
            }
        };
        remap.insert(*gc, to);
    }
    // (the palette block: the original's order for the states still used, then the new ones)
    let gen_states: std::collections::BTreeSet<&str> = gen_p.iter().map(|(_, s)| s.as_str()).collect();
    let mut pal_lines: Vec<String> = orig.iter().filter(|(_, s)| gen_states.contains(s.as_str())).map(|(c, s)| format!("  {c} {s}")).collect();
    for (gc, s) in &gen_p {
        if !orig_by_state.contains_key(s.as_str()) {
            pal_lines.push(format!("  {} {s}", remap[gc]));
        }
    }
    let mut out = String::new();
    let mut in_pal = false;
    let mut in_layer = false;
    let mut pal_done = false;
    // (b8949c96 07:49, part 58) the head (every line before `origin:` / `palette:`: name, meta, ports, entities) in the
    // ORIGINAL file's order: the re-serialization sorts the meta; a line the original didn't hold goes after
    let head_end = |t: &str| t.lines().position(|l| l.starts_with("origin:") || l.trim_end() == "palette:").unwrap_or(0);
    let orig_head: Vec<&str> = original.lines().take(head_end(original)).filter(|l| !l.starts_with('#')).collect();
    let gen_head: Vec<&str> = generated.lines().take(head_end(generated)).collect();
    let mut ordered: Vec<&str> = Vec::new();
    let mut pending: Vec<&str> = gen_head.clone();
    for l in &orig_head {
        if let Some(i) = pending.iter().position(|g| g == l) {
            ordered.push(pending.remove(i));
        }
    }
    ordered.extend(pending);
    for l in &ordered {
        out.push_str(l);
        out.push('\n');
    }
    for l in generated.lines().skip(gen_head.len()) {
        let t = l.trim_end();
        if t == "palette:" {
            in_pal = true;
            out.push_str(l);
            out.push('\n');
            continue;
        }
        if in_pal {
            if l.starts_with("  ") && !l.trim().is_empty() {
                if !pal_done {
                    for p in &pal_lines {
                        out.push_str(p);
                        out.push('\n');
                    }
                    pal_done = true;
                }
                continue;
            }
            in_pal = false;
        }
        if !l.starts_with(' ') && !l.trim().is_empty() {
            in_layer = (t.starts_with("layer ") || t.starts_with("nether layer ")) && t.ends_with(':');
            out.push_str(l);
            out.push('\n');
            continue;
        }
        if in_layer && l.starts_with("  ") {
            out.push_str("  ");
            out.extend(l[2..].chars().map(|c| *remap.get(&c).unwrap_or(&c)));
            out.push('\n');
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    if !generated.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

/// (ironspec 08:00r, part 58) The head lines (before `origin:` / `palette:`, `#` lines aside) of `original` that `out`
/// still holds but in another order: a gratuitous re-ordering (the re-serializer sorts the meta) fails by name.
pub fn reordered_head(original: &str, out: &str) -> Vec<String> {
    let head = |t: &str| -> Vec<String> {
        let end = t.lines().position(|l| l.starts_with("origin:") || l.trim_end() == "palette:").unwrap_or(0);
        t.lines().take(end).filter(|l| !l.starts_with('#')).map(|l| l.to_string()).collect()
    };
    let (a, b) = (head(original), head(out));
    let kept: Vec<&String> = a.iter().filter(|l| b.contains(l)).collect();
    let in_out: Vec<&String> = b.iter().filter(|l| a.contains(l)).collect();
    if kept == in_out {
        Vec::new()
    } else {
        kept.iter().zip(in_out.iter()).filter(|(x, y)| x != y).map(|(x, _)| (*x).clone()).collect()
    }
}

/// (ironspec / G1 Nov 14 07:31r, part 58) `mobfarm edit --set`'s text edit, as one function the CLI and the tests
/// share: each `x,y,z=state` applied (an edit that leaves its cell as it was is refused, ironspec Nov 13 04:07), the
/// design re-serialized with the ORIGINAL palette letters kept ([`stable_palette`]), every `#` line put back after the
/// line it followed ([`keep_comments`]), and two guards that FAIL the edit by name: the `#` lines' count, and every
/// other line the edit doesn't touch kept byte for byte ([`lost_lines`]). Returns the new text and the lines to print.
pub fn edit_set_text(original: &str, sets: &[String]) -> anyhow::Result<(String, Vec<String>)> {
    let mut d = Design::parse(original)?;
    let mut log = Vec::new();
    let mut ys: Vec<i32> = Vec::new();
    for s in sets {
        let (c, state) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--set x,y,z=state, not {s:?}"))?;
        let v: Vec<i32> = c.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("--set {s}: {e}"))?;
        anyhow::ensure!(v.len() == 3, "--set x,y,z=state, not {s:?}");
        let state = state.trim();
        let full = if state.contains(':') { state.to_string() } else { format!("minecraft:{state}") };
        let before = d.blocks.get(&[v[0], v[1], v[2]]).cloned().unwrap_or_else(|| "minecraft:air".to_string());
        anyhow::ensure!(before != full, "NO-OP --set {},{},{}: the design already holds {before} (an override that doesn't change its cell fails, ironspec Nov 13 04:07)", v[0], v[1], v[2]);
        log.push(format!("set {},{},{}: {} -> {}", v[0], v[1], v[2], before.trim_start_matches("minecraft:"), full.trim_start_matches("minecraft:")));
        if full == "minecraft:air" {
            d.blocks.remove(&[v[0], v[1], v[2]]);
        } else {
            d.blocks.insert([v[0], v[1], v[2]], full);
        }
        ys.push(v[1]);
    }
    let stable = stable_palette(original, &d.to_text());
    let (out, kept, moved) = keep_comments(original, &stable);
    let before = original.lines().filter(|l| l.starts_with('#')).count();
    anyhow::ensure!(kept + moved == before, "edit: {before} `#` lines read, {} written", kept + moved);
    log.push(format!("header: {before} `#` lines before, {} after ({kept} each after the line it followed, {moved} moved up: the line before them changed)", out.lines().filter(|l| l.starts_with('#')).count()));
    let lost = lost_lines(original, &out, &ys);
    anyhow::ensure!(lost.is_empty(), "edit: {} line(s) the edit doesn't change would not be carried (nothing silently dropped, ironspec Nov 14 07:31r): {}", lost.len(), lost.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>().join("; "));
    let re = reordered_head(original, &out);
    anyhow::ensure!(re.is_empty(), "edit: {} head line(s) re-ordered (an edit changes only the lines its cells require, ironspec Nov 14 08:00r): {}", re.len(), re.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>().join("; "));
    let changed = {
        let mut have: BTreeMap<&str, usize> = BTreeMap::new();
        for l in original.lines() {
            *have.entry(l).or_insert(0) += 1;
        }
        out.lines()
            .filter(|l| match have.get_mut(l) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    false
                }
                _ => true,
            })
            .count()
    };
    log.push(format!("lines written that the file didn't hold: {changed} (the palette's new entries and the rows of the layers the cells sit in); every other line kept byte for byte"));
    Ok((out, log))
}

/// (ironspec / G1 Nov 14 07:31r, part 58) The `#` lines of `original` that `out` no longer holds (a multiset), but the
/// declaration lines a part swap rewrites (`# <key>:` for a key of [`crate::declkeys::COMMENT_KEYS`]): the meta and
/// swap paths' guard.
pub fn lost_comment_lines(original: &str, out: &str, allow_declarations: bool) -> Vec<String> {
    let mut have: BTreeMap<&str, usize> = BTreeMap::new();
    for l in out.lines().filter(|l| l.starts_with('#')) {
        *have.entry(l).or_insert(0) += 1;
    }
    let decl = |l: &str| {
        let k = l.trim_start_matches('#').trim_start();
        crate::declkeys::COMMENT_KEYS.iter().any(|key| k.starts_with(&format!("{key}:")))
    };
    original
        .lines()
        .filter(|l| l.starts_with('#'))
        .filter(|l| match have.get_mut(*l) {
            Some(n) if *n > 0 => {
                *n -= 1;
                false
            }
            _ => !(allow_declarations && decl(l)),
        })
        .map(|l| l.to_string())
        .collect()
}

#[cfg(test)]
mod edit_integrity_tests {
    /// (ironspec / G1 Nov 14 07:31r, part 58) A canonical file with a `#` line of every kind (before the first line,
    /// between meta lines, a declaration, the day-time line, before the palette, between layers, at the end): a
    /// `--set` keeps every `#` line byte for byte and every line of the layers it doesn't touch; setting the cell back
    /// round-trips the file BYTE FOR BYTE; a lost line fails by name; the meta path keeps every `#` line.
    #[test]
    fn edit_round_trips_byte_for_byte() {
        let mut d = mctas::design::Design::parse("name: t\norigin: 0 0 0\npalette:\n  A minecraft:stone\n  B minecraft:glass\nlayer 0:\n  AAA\n  ABA\nlayer 1:\n  B\n").unwrap();
        d.meta.insert("circuit".to_string(), "mob-farm iron_ingot".to_string());
        d.meta.insert("zeta".to_string(), "last".to_string());
        let canon = d.to_text();
        let mut lines: Vec<String> = canon.lines().map(|l| l.to_string()).collect();
        // (b8949c96 07:49) the meta out of the re-serializer's sorted order: "zeta" before "circuit"
        let zi = lines.iter().position(|l| l.starts_with("zeta:")).unwrap();
        let zl = lines.remove(zi);
        let ci = lines.iter().position(|l| l.starts_with("circuit:")).unwrap();
        lines.insert(ci, zl);
        let at = |lines: &Vec<String>, p: &str| lines.iter().position(|l| l.starts_with(p)).unwrap();
        let i = at(&lines, "palette:");
        lines.insert(i, "# before the palette".to_string());
        let j = at(&lines, "layer 1:");
        lines.insert(j, "# between layers".to_string());
        lines.insert(1, "# shape: 0,1,0=north_south (a declaration)".to_string());
        lines.insert(1, "# day-time: 18000 (the save's day time)".to_string());
        lines.insert(0, "# the file's first line".to_string());
        lines.push("# the last line".to_string());
        let original = lines.join("\n") + "\n";
        let (out, log) = super::edit_set_text(&original, &["1,1,0=minecraft:redstone_block".to_string()]).unwrap();
        let hashes = |t: &str| t.lines().filter(|l| l.starts_with('#')).map(|l| l.to_string()).collect::<Vec<_>>();
        assert_eq!(hashes(&out), hashes(&original), "every `#` line kept, in order: {out}");
        let rows_of = |t: &str, y: &str| -> Vec<String> {
            let mut on = false;
            let mut v = Vec::new();
            for l in t.lines() {
                if !l.starts_with(' ') && !l.trim().is_empty() {
                    on = l.trim_end() == format!("layer {y}:");
                } else if on && l.starts_with("  ") {
                    v.push(l.to_string());
                }
            }
            v
        };
        assert!(!rows_of(&original, "0").is_empty() && rows_of(&out, "0") == rows_of(&original, "0"), "layer 0 untouched byte for byte: {out}");
        assert!(log.iter().any(|l| l.starts_with("lines written that the file didn't hold: ")), "{log:?}");
        let (back, _) = super::edit_set_text(&out, &["1,1,0=minecraft:air".to_string()]).unwrap();
        assert_eq!(back, original, "set and set back: byte for byte");
        // a line the edit doesn't touch, missing from an output, is named
        let missing = out.replacen("circuit: mob-farm iron_ingot\n", "", 1);
        let lost = super::lost_lines(&original, &missing, &[1]);
        assert!(lost == vec!["circuit: mob-farm iron_ingot".to_string()], "{lost:?}");
        // the meta path keeps every `#` line
        let meta = super::edit_meta(&original, false, None, &["note=x".to_string()]).unwrap();
        assert!(super::lost_comment_lines(&original, &meta, false).is_empty(), "the meta path: {meta}");
        // the plain re-serialization (the ctl27 path) is caught naming its 6 dropped `#` lines
        let dropped = super::lost_comment_lines(&original, &d.to_text(), false);
        assert!(dropped.len() == 6 && dropped.iter().any(|l| l.starts_with("# day-time:")), "{dropped:?}");
        // (ironspec 08:00r) gratuitous changes fail by name: the plain re-serialization re-orders the head ("zeta" back
        // after "circuit") and re-letters the palette when the letters differ
        let plain = super::keep_comments(&original, &d.to_text()).0;
        let re = super::reordered_head(&original, &plain);
        assert!(!re.is_empty() && re.iter().any(|l| l.starts_with("zeta:") || l.starts_with("circuit:")), "a sorted head: {re:?} in {plain}");
        let relettered = original.replace("  G minecraft:glass", "  Q minecraft:glass");
        let lost_pal = super::lost_lines(&original, &relettered, &[]);
        assert!(lost_pal == vec!["  G minecraft:glass".to_string()], "a re-lettered palette line: {lost_pal:?}");
    }
}
/// holds (as a multiset), but those it may change: the palette block's lines, the rows and headers of the layers
/// at `layers` (the y's its cells touch), and, when the edit moves the design's origin, every layer row. An empty
/// result: every other line kept byte for byte.
pub fn lost_lines(original: &str, out: &str, layers: &[i32]) -> Vec<String> {
    let origin = |t: &str| t.lines().find(|l| l.starts_with("origin:")).map(|l| l.to_string());
    let moved = origin(original) != origin(out);
    // (ironspec 08:00r, part 58) a palette line may change only when its state leaves the design: a state the output
    // still holds keeps its line byte for byte (a re-encoded palette fails by name)
    let out_states: std::collections::BTreeSet<String> = palette_of(out).into_iter().map(|(_, s)| s).collect();
    let mut have: BTreeMap<&str, usize> = BTreeMap::new();
    for l in out.lines() {
        *have.entry(l).or_insert(0) += 1;
    }
    let mut lost = Vec::new();
    let mut in_pal = false;
    let mut layer: Option<i32> = None;
    for l in original.lines() {
        let t = l.trim_end();
        let mut may_change = false;
        if t == "palette:" {
            in_pal = true;
        } else if in_pal && l.starts_with("  ") && !l.trim().is_empty() {
            let state: String = l[2..].chars().skip(1).collect::<String>().trim().to_string();
            may_change = !out_states.contains(&state);
        } else if !l.starts_with(' ') && !l.trim().is_empty() {
            in_pal = false;
            layer = t.strip_prefix("layer ").or_else(|| t.strip_prefix("nether layer ")).and_then(|r| r.strip_suffix(':')).and_then(|y| y.trim().parse::<i32>().ok());
            if layer.is_some_and(|y| layers.contains(&y)) || (moved && t.starts_with("origin:")) {
                may_change = true;
            }
        } else if layer.is_some() && (l.starts_with("  ") || l.trim().is_empty()) && (moved || layer.is_some_and(|y| layers.contains(&y))) {
            may_change = true;
        }
        if may_change {
            continue;
        }
        match have.get_mut(l) {
            Some(n) if *n > 0 => *n -= 1,
            _ => lost.push(l.to_string()),
        }
    }
    lost
}

#[cfg(test)]
mod keep_comments_tests {
    /// (ironspec Nov 13 01:08) An edit round-trip keeps every `#` line byte for byte, the leading
    /// caption on top and each interleaved run after the line it followed; a run whose line is gone
    /// moves up after the caption, counted.
    #[test]
    fn every_comment_line_kept() {
        let orig = "# caption 1\n# caption 2\nname: x\norigin: 0 0 0\n# switch: 1,2,3=a lever (its reason)\n# update-only: 4,5,6=a detector\nwaker-places: 1,1,1=w\n# one-way: 7,8,9=a step\npalette:\n  a minecraft:glass\n";
        let regenerated = "name: x\norigin: 0 0 0\nwaker-places: 1,1,1=w\npalette:\n  a minecraft:glass\n  b minecraft:stone\n";
        let (out, kept, moved) = super::keep_comments(orig, regenerated);
        assert_eq!((kept, moved), (5, 0));
        let comments = |t: &str| t.lines().filter(|l| l.starts_with('#')).map(str::to_string).collect::<Vec<_>>();
        assert_eq!(comments(&out), comments(orig));
        assert!(out.contains("origin: 0 0 0\n# switch: 1,2,3=a lever (its reason)\n# update-only: 4,5,6=a detector\nwaker-places:"), "{out}");
        assert!(out.contains("waker-places: 1,1,1=w\n# one-way: 7,8,9=a step\npalette:"), "{out}");
        // (the line a run followed changed: the run moves up, nothing dropped)
        let changed = regenerated.replace("waker-places: 1,1,1=w", "waker-places: 2,2,2=w");
        let (out2, kept2, moved2) = super::keep_comments(orig, &changed);
        assert_eq!((kept2, moved2), (4, 1));
        assert_eq!(comments(&out2).len(), 5);
    }
}

/// A design text with its tier on its `circuit:` line (`circuit: mob-farm iron_ingot middle`):
/// the published kind (one site page per tier).
pub fn set_tier(text: &str, tier: &str) -> anyhow::Result<String> {
    let tiers = circuit::mobfarm::TIERS;
    if !tiers.iter().any(|(t, _)| *t == tier) {
        anyhow::bail!("no tier {tier:?} ({})", tiers.iter().map(|(t, _)| *t).collect::<Vec<_>>().join(", "));
    }
    let mut found = false;
    let out: Vec<String> = text
        .lines()
        .map(|l| match l.strip_prefix("circuit: mob-farm ") {
            Some(rest) if !found => {
                found = true;
                let item = rest.split_whitespace().next().unwrap_or("iron_ingot");
                format!("circuit: mob-farm {item} {tier}")
            }
            _ => l.to_string(),
        })
        .collect();
    if !found {
        anyhow::bail!("no `circuit: mob-farm` line to give the tier to");
    }
    Ok(out.join("\n") + "\n")
}

/// The dusk pulse: the zombie's trapdoor opens for about 46 gt when night falls, then shuts for
/// the rest of the night, so the villagers fall asleep once (`LAST_SLEPT`) and wake up scared
/// again. Two inverted daylight detectors switch on together at nightfall: D2 (through a dust
/// into B1) turns torch T1 off at once, D reaches K0 through 6 repeaters (46 gt); torch T0 under
/// the trapdoor is lit while K0 is unpowered: between the two. At dawn T1 relights before the
/// repeaters go dark: no pulse. No iron; 6 quartz.
fn dusk_pulse(b: &mut Builder) {
    b.set(2, -1, -6, STONE); // K0
    b.set(2, 0, -6, "redstone_torch[lit=false]"); // T0, under the trapdoor
    b.set(2, -3, -6, STONE); // B1
    b.set(2, -2, -6, "redstone_torch[lit=true]"); // T1, under K0
    b.set(2, -3, -8, "daylight_detector[inverted=true,power=0]"); // D2
    b.set(2, -4, -7, STONE);
    b.set(2, -3, -7, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    for x in 3..=8 {
        b.set(x, -2, -6, STONE);
        b.set(x, -1, -6, "repeater[delay=4,facing=east,locked=false,powered=false]");
    }
    b.set(9, -1, -6, "daylight_detector[inverted=true,power=0]"); // D
}

/// L2, the middle ground: L1's pods and zombie, the dusk pulse instead of a hide all night, a 4x4
/// pad around a 2x2 hole (water from its corners), a shaft that drops each golem out of every
/// villager's follow range (feet at -22), and L1's kill chamber at its bottom with 2 hoppers.
pub fn l2(n: i32, fy: i32, villagers: i32) -> String {
    let mut b = Builder::new(&format!("iron-middle-{n}"));
    if villagers == 4 {
        front4(&mut b);
    } else {
        for x in [0, 2, 4] {
            b.pod(x);
        }
        zombie(&mut b);
        dusk_pulse(&mut b);
    }
    let k = -22;
    let h = n / 2;
    let (x0, x1, z0, z1) = (3 - h, 2 + h, 5 - h, 4 + h);
    let hole = |x: i32, z: i32| (2..=3).contains(&x) && (4..=5).contains(&z);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if !hole(x, z) {
                b.set(x, fy, z, STONE);
            }
        }
    }
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        b.set(x, fy + 1, z, "water[level=0]");
    }
    for y in (fy + 1)..=(fy + 2) {
        for x in (x0 - 1)..=(x1 + 1) {
            for z in [z0 - 1, z1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
        for z in z0..=z1 {
            for x in [x0 - 1, x1 + 1] {
                b.set(x, y, z, GLASS);
            }
        }
    }
    for z in 4..=5 {
        wall_sign(&mut b, 2, fy, z, "east");
        wall_sign(&mut b, 3, fy, z, "west");
    }
    // glass caps the shaft walls the pad does not cover (no spawn spot on their tops)
    for x in 1..=4 {
        for z in 1..=6 {
            if !(x0..=x1).contains(&x) || !(z0..=z1).contains(&z) {
                b.set(x, fy, z, GLASS);
            }
        }
    }
    // the shaft (x 2-3, z 4-5) down to feet level k, the chamber north of it (z 2-3)
    for y in (k - 1)..fy {
        for x in 1..=4 {
            for z in 1..=6 {
                let shaft = (2..=3).contains(&x) && (4..=5).contains(&z) && y >= k;
                let chamber = (2..=3).contains(&x) && (2..=3).contains(&z) && (k..=k + 2).contains(&y);
                let inner = (2..=3).contains(&x) && (2..=5).contains(&z);
                let wall = !inner && (x == 1 || x == 4 || z == 1 || z == 6);
                if !shaft && !chamber && (inner || wall) {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    for z in 2..=3 {
        wall_sign(&mut b, 2, k + 1, z, "east");
        wall_sign(&mut b, 3, k + 1, z, "west");
        b.set(2, k + 2, z, LAVA);
        b.set(3, k + 2, z, LAVA);
    }
    wall_sign(&mut b, 2, k + 2, 4, "east");
    wall_sign(&mut b, 3, k + 2, 4, "west");
    b.set(2, k, 5, "water[level=0]");
    b.set(3, k, 5, "water[level=0]");
    b.set(2, k - 1, 2, "hopper[enabled=true,facing=east]");
    b.set(3, k - 1, 2, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [3, k - 2, 2]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L2, the middle iron farm: three villagers on their beds in closed 1-cell pods under leaves, a",
        "zombie in a boat 7 blocks north of them in view. At nightfall two inverted daylight detectors open",
        "the trapdoor in front of the zombie for 46 game ticks: the villagers fall asleep, then wake up",
        "scared for the rest of the night (and the farm restarts by itself after a missed night). Golems",
        "spawn on a 4x4 pad around a 2x2 hole (water from its corners, wall signs in the hole stop it) and",
        "fall 18 blocks, out of every villager's sight, to water that pushes them under a lava blade held",
        "by signs, a ceiling right above it, over two hoppers.",
    ])
}

/// L1, the obvious design (the bar): three villagers in 1-cell pods, the zombie in a boat, an
/// inverted daylight detector on the trapdoor in front of it (it hides the zombie all night, so
/// the villagers sleep through every night); a 6 x 5 pad 4 below the villagers whose water flows
/// one way (sources along its far edge) into a 2-deep trench along its near edge (wall signs
/// hold the water over the trench to its wall: the current never stops over a floor); the trench
/// down to water that carries the golems to a lava blade held by signs, a ceiling right above it,
/// 2 hoppers into one chest.
pub fn l1() -> String {
    let mut b = Builder::new("iron-obvious");
    for x in [0, 2, 4] {
        b.pod(x);
    }
    zombie(&mut b);
    b.set(2, 2, -6, "daylight_detector[inverted=true,power=0]");
    let (x0, x1) = (-1, 4);
    // the pad (x 0..3, z 4..8) is one block narrower than the trench on each side: a golem spawned
    // 0.2 into the inner rim meets no wall on its way into the trench, and the outer rim stops it
    // walking out through the inner one (a box ignores the shapes it already overlaps)
    for x in (x0 + 1)..=(x1 - 1) {
        for z in 4..=8 {
            b.set(x, -4, z, STONE);
        }
        b.set(x, -3, 8, "water[level=0]");
    }
    for x in x0..=x1 {
        wall_sign(&mut b, x, -4, 3, "north");
        wall_sign(&mut b, x, -4, 2, "south");
    }
    // the rims: open fence gates beside and behind the pad hold its water (no fluid enters a fence
    // gate) with no collision box, so a golem spawned 0.2 into them meets nothing; glass 2 high
    // (golems step 1 and jump about 1.25) one block further out stops them; the north row holds
    // the trench's signs
    for x in x0..=x1 {
        b.set(x, -4, 9, STONE);
        b.set(x, -3, 9, GATE_N);
    }
    for z in 4..=8 {
        for x in [x0, x1] {
            b.set(x, -4, z, STONE);
            b.set(x, -3, z, GATE_E);
        }
    }
    for y in -4..=-2 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, 1, GLASS);
            if y > -4 {
                b.set(x, y, 10, GLASS);
            }
        }
        if y > -4 {
            for z in 2..=10 {
                b.set(x0 - 1, y, z, GLASS);
                b.set(x1 + 1, y, z, GLASS);
            }
        }
    }
    // the trench (x -1..4, z 2..3) down to feet level -9; at its bottom water flows east into the
    // kill chamber (x 5..6): lava at head height (-7) held by signs at -8 and at its mouth (x 4), a
    // ceiling right above it (-6) so a golem that jumps in the lava cannot float up with its drops
    for y in -10..=-5 {
        for x in (x0 - 1)..=7 {
            for z in 1..=4 {
                let trench = (x0..=x1).contains(&x) && (2..=3).contains(&z) && y >= -9;
                let chamber = (5..=6).contains(&x) && (2..=3).contains(&z) && (-9..=-7).contains(&y);
                let inner = (2..=3).contains(&z) && (x0..=6).contains(&x);
                if !trench && !chamber && (inner || z == 1 || z == 4 || x == x0 - 1 || x == 7) {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    // glass caps the walls the pad does not cover (no spawn spot on their tops)
    for z in 1..=4 {
        b.set(x0 - 1, -4, z, GLASS);
        for x in (x1 + 1)..=7 {
            b.set(x, -4, z, GLASS);
        }
    }
    for x in 5..=6 {
        wall_sign(&mut b, x, -8, 2, "south");
        wall_sign(&mut b, x, -8, 3, "north");
        b.set(x, -7, 2, LAVA);
        b.set(x, -7, 3, LAVA);
    }
    wall_sign(&mut b, 4, -7, 2, "south");
    wall_sign(&mut b, 4, -7, 3, "north");
    b.set(x0, -9, 2, "water[level=0]");
    b.set(x0, -9, 3, "water[level=0]");
    b.set(6, -10, 3, "hopper[enabled=true,facing=north]");
    b.set(6, -10, 2, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [6, -11, 2]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L1, the obvious iron farm (the bar): three villagers on their beds in closed 1-cell pods under",
        "leaves, a zombie in a boat 7 blocks north of them in view. An inverted daylight detector opens the",
        "trapdoor in front of the zombie at nightfall, so the villagers sleep through every night (and the",
        "farm restarts by itself after a missed night); it stops in rain (clear weather is not judged).",
        "Golems spawn on a 6x5 pad 4 below the villagers; water from its far edge carries them into a trench",
        "along its near edge (wall signs hold the water over it); they fall to water that pushes them under",
        "a lava blade held by signs, with a ceiling right above it, over two hoppers, in view of the villagers.",
    ])
}

/// L3, the day-1 farm: three villagers in 1-cell pods, a zombie in a boat 7 blocks north, a row of
/// `pits` 1x1 spawn floors west of the zombie (x = -pits..-1, z = -6) where golems spawn with a
/// lava source beside their head on both sides and burn where they stand. One tripwire string
/// runs through every pit at golem knee height: a golem in any pit powers the hook under the
/// sight-blocking trapdoor in front of the zombie, so the villagers calm down (and sleep at
/// night) while it burns. A water channel under the string carries the drops east to one hopper.
pub fn l3(pits: i32) -> String {
    let mut b = Builder::new(&format!("iron-day1-{pits}"));
    for x in [0, 2, 4] {
        b.pod_day1(x);
    }
    // the zombie: a boat on a slab, a slab roof over its eyes (no sky: it never burns)
    b.set(2, 0, -7, CAP);
    b.set(2, 2, -7, CAP);
    b.entity("oak_boat", [2.5, 0.5, -6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    // the sight blocker, opened by the hook under it; a closed trapdoor west of it stops golems
    b.set(2, 1, -6, SHUT);
    b.set(2, 0, -6, "tripwire_hook[attached=true,facing=west,powered=false]");
    b.set(3, 0, -6, STONE);
    b.set(1, 1, -6, SHUT);
    // the string from the east hook to the west hook
    let west = -pits - 1;
    for x in (west + 1)..=1 {
        b.set(x, 0, -6, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
    }
    b.set(west, 0, -6, "tripwire_hook[attached=true,facing=east,powered=false]");
    b.set(west - 1, 0, -6, STONE);
    b.set(west - 1, 1, -6, STONE);
    b.set(west, 1, -6, STONE);
    b.set(west, -1, -6, STONE);
    // the pits (day-1 materials, vjeux 13:09: cobblestone for glass; a bottom cobblestone slab
    // holds a lava source and caps it: a golem's spawn fails on its half top)
    for px in (west + 1)..=-1 {
        b.set(px, -2, -6, STONE);
        b.set(px, 1, -6, SHUT);
        // over the pit's trapdoor ceiling, a cobblestone with a torch on it (glass before): no
        // golem spawns on the trapdoor or the cobblestone (a block is above each), and the
        // golem search's scan passes the torch and goes on down to the pit
        b.set(px, 2, -6, STONE);
        b.set(px, 3, -6, "torch");
        for z in [-7, -5] {
            b.set(px, -1, z, STONE);
            b.set(px, 0, z, STONE);
            b.set(px, 1, z, LAVA);
            b.set(px, 2, z, CAP);
        }
        b.set(px, 1, -8, CAP);
        b.set(px, 1, -4, CAP);
    }
    for x in [west, 0] {
        for z in [-7, -5] {
            b.set(x, 1, z, CAP);
            b.set(x, 0, z, STONE);
            b.set(x, -1, z, STONE);
        }
    }
    // the channel: a source at the west pit, dry at x = 0 over the hopper
    b.set(west + 1, -1, -6, "water[level=0]");
    b.set(0, -2, -6, "hopper[enabled=true,facing=down]");
    b.set(0, 1, -6, STONE);
    b.set(1, -1, -6, STONE);
    b.d.ports.insert("output".into(), [0, -3, -6]);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // every other top a golem could spawn on gets its cap by its role (`parts::cap_spawns`: a torch
    // over a pit's column, so the search scans on down to the pit, else a slab)
    let _ = crate::parts::cap_spawns(&mut b, &|p| p[2] == -6 && p[0] > west && p[0] < 0 && p[1] == -1);
    b.text(&[
        &format!("L3, the day-1 iron farm ({pits} spawn pits): three villagers on their beds in closed 1-cell pods under"),
        "leaves, a zombie in a boat 7 blocks north of them in view of their standing and sleeping eyes.",
        "Golems spawn in a row of 1x1 pits west of the zombie, a lava source beside their head on both",
        "sides, and burn where they stand. One tripwire runs through every pit at knee height: a golem in",
        "any pit powers the hook under the trapdoor in front of the zombie, the trapdoor opens and hides it,",
        "and the villagers calm down (at night they fall asleep) while the golem burns. A water channel",
        "under the string carries the iron to one hopper. Hide the zombie by hand on the first night: the",
        "villagers need a recent sleep to summon the first golem.",
    ])
}

/// The 4-villager front of L4: pods at x = 0, 2, 4, 6; the zombie in a boat at x 3.5, 7 north;
/// three trapdoors at z = -5 (1.5 from its eyes: every ray crosses them) opened by the dust on
/// the blocks above them; the dusk pulse drives that dust (D2 into B1 turns T1 off at once, D
/// reaches K0 through 6 repeaters: T0 on K0 is lit for about 46 gt at nightfall).
fn front4(b: &mut Builder) {
    for x in [0, 2, 4, 6] {
        b.pod(x);
    }
    b.set(3, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    b.set(3, 2, -7, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [3.5, 0.5, -6.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    for x in 2..=4 {
        b.set(x, 1, -5, "oak_trapdoor[facing=south,half=bottom,open=false,powered=false,waterlogged=false]");
        b.set(x, 2, -5, STONE);
        let (e, w) = (if x < 4 { "side" } else { "side" }, if x > 2 { "side" } else { "none" });
        b.set(x, 3, -5, &format!("redstone_wire[east={e},north=none,power=0,south=none,west={w}]"));
    }
    b.set(5, 2, -5, STONE); // K0
    b.set(5, 3, -5, "redstone_torch[lit=false]"); // T0 feeds the dust
    b.set(5, 0, -5, STONE); // B1
    b.set(5, 1, -5, "redstone_torch[lit=true]"); // T1 under K0
    b.set(5, -1, -4, STONE);
    b.set(5, 0, -4, "redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    b.set(5, 0, -3, "daylight_detector[inverted=true,power=0]"); // D2
    for x in 6..=11 {
        b.set(x, 1, -5, STONE);
        b.set(x, 2, -5, "repeater[delay=4,facing=east,locked=false,powered=false]");
    }
    b.set(12, 2, -5, "daylight_detector[inverted=true,power=0]"); // D
}

const GATE_N: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]";
const GATE_E: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]";

/// The platforms of L4 at water level wl, units at x 0, 3, 6 (each 2 wide), in two banks facing
/// the drop column (z 4-7). North bank: source row z 1 (water on bottom slabs: no spawn there),
/// spawn rows 2 and 3, edge row 4 (water over open fence gates: it holds the water, not a golem),
/// stop row 5 (open fence gates: no fluid enters one, a golem walks through). South bank: source
/// row 9, spawn row 8, edge row 7, stop row 6 (spawn rows reach z 8, the villagers' square). A
/// golem leaves the floor still in the edge row's water, so it never stands on land at the drop
/// (where a walk against the current would step it back up), and falls dry past the stop row.
/// Glass walls between the units hold the water; at the edge rows, fence gates do (a golem
/// spawned 0.2 into a wall's column meets no wall ahead).
fn banks4(b: &mut Builder, wl: i32) {
    let fl = wl - 1;
    for sx in [0, 3, 6] {
        for x in sx..=sx + 1 {
            for (src, rows, edge, stop, back) in [(1, &[2, 3][..], 4, 5, 0), (9, &[8][..], 7, 6, 10)] {
                b.set(x, fl, src, "stone_slab[type=bottom,waterlogged=false]");
                b.set(x, wl, src, "water[level=0]");
                for &z in rows {
                    b.set(x, fl, z, STONE);
                }
                b.set(x, fl, edge, GATE_N);
                b.set(x, wl, stop, GATE_N);
                b.set(x, wl, back, GLASS);
                b.set(x, wl + 1, back, GLASS);
            }
        }
    }
    for x in [-1, 2, 5, 8] {
        for z in (0..=3).chain(8..=10) {
            b.set(x, fl, z, STONE);
            b.set(x, wl, z, GLASS);
            b.set(x, wl + 1, z, GLASS);
        }
        b.set(x, wl, 4, GATE_E);
        b.set(x, wl, 7, GATE_E);
    }
}

/// The drop column of L4 (x -1..8, z 4-7) from the platforms down to feet level k, and its kill:
/// water from a row of sources in a niche of the south wall (z 8) pushes every golem north into
/// the chamber (x -1..8, z 2-3): lava at head height held by open fence gates below it and at its
/// mouth, a ceiling right above it, hoppers under its north row.
fn column4(b: &mut Builder, wl: i32, k: i32) {
    let fl = wl - 1;
    for y in (k - 1)..=(wl + 1) {
        for x in -2..=9 {
            if y < fl {
                for z in [3, 8] {
                    let chamber = z == 3 && (-1..=8).contains(&x) && (k..=k + 2).contains(&y);
                    let niche = z == 8 && (-1..=8).contains(&x) && y == k;
                    if !chamber && !niche {
                        b.set(x, y, z, STONE);
                    }
                }
            }
            if x == -2 || x == 9 || y == k - 1 {
                // glass where it rises above the platforms' floor (no spawn on its top)
                let m = if y >= fl { GLASS } else { STONE };
                for z in 4..=7 {
                    b.set(x, y, z, m);
                }
            }
        }
    }
    for x in -1..=8 {
        b.set(x, k, 8, "water[level=0]");
        b.set(x, k, 9, STONE);
        for y in k..=k + 3 {
            b.set(x, y, 1, STONE);
        }
        b.set(x, k + 3, 2, STONE);
        b.set(x, k + 3, 3, STONE);
        for z in 2..=3 {
            b.set(x, k + 1, z, GATE_E);
            b.set(x, k + 2, z, LAVA);
        }
        b.set(x, k + 2, 4, GATE_N);
        b.set(x, k - 1, 3, STONE);
        b.set(x, k - 1, 2, if x < 8 { "hopper[enabled=true,facing=east]" } else { "hopper[enabled=true,facing=down]" });
    }
    for z in 2..=3 {
        for y in (k - 1)..=(k + 3) {
            b.set(-2, y, z, STONE);
            b.set(9, y, z, STONE);
        }
    }
    b.d.ports.insert("output".into(), [8, k - 2, 2]);
}

/// L4, the fast module: 4 villagers (only 3 must agree, so one sighting does not stretch a
/// cycle), the dusk pulse, two banks of short platforms whose open fence gates drop each golem
/// dry within about 2 blocks of its spawn, out of every villager's range, and one kill chamber.
pub fn l4() -> String {
    let mut b = Builder::new("iron-fast");
    front4(&mut b);
    let (wl, k) = (-6, -22);
    banks4(&mut b, wl);
    column4(&mut b, wl, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b.text(&[
        "L4, the fast iron farm module: four villagers on their beds in closed 1-cell pods under leaves",
        "(only three must agree, so a golem one of them sees does not stretch the cycle) and a zombie in a",
        "boat 7 blocks north. At nightfall two inverted daylight detectors open the three trapdoors in front",
        "of the zombie for 46 game ticks: the villagers fall asleep, then wake up scared for the rest of the",
        "night. Golems spawn on 2-wide platforms in two banks facing a drop: water pushes them at most 2.2",
        "blocks through open fence gates (no water enters a fence gate) and they fall dry, out of every",
        "villager's sight, to water that pushes them under a lava blade with a ceiling, over two hoppers.",
    ])
}

/// The dry hole of L2d: a 6x6 pad (x 0..5, z 2..7, water level wl) whose 4x4 middle is the hole:
/// its outer ring holds water over open fence gates (the water stays, a golem sinks through), its
/// 2x2 center has open fence gates at the water level (dry). Water from the pad's corners pushes
/// each golem off the stone while it is still in water (no step back onto land) and on into the
/// dry center, where it falls freely down a 4x4 shaft.
pub(crate) fn dry_hole(b: &mut Builder, n: i32, wl: i32, k: i32) {
    dry_hole_walls(b, n, wl, k, wl - 1);
}

/// [`dry_hole`] with the shaft walled only below `top` (M2-open: in the void a golem leaving the
/// dry center falls straight, so only the drop's bottom layers need walls).
pub fn dry_hole_walls(b: &mut Builder, n: i32, wl: i32, k: i32, top: i32) {
    let fl = wl - 1;
    let ring = |x: i32, z: i32| (1..=4).contains(&x) && (3..=6).contains(&z);
    let center = |x: i32, z: i32| (2..=3).contains(&x) && (4..=5).contains(&z);
    let h = n / 2;
    let (x0, x1, z0, z1) = (3 - h, 2 + h, 5 - h, 4 + h);
    for x in x0..=x1 {
        for z in z0..=z1 {
            if center(x, z) {
                b.set(x, wl, z, GATE_N);
            } else if ring(x, z) {
                b.set(x, fl, z, GATE_N);
            } else {
                b.set(x, fl, z, STONE);
            }
        }
    }
    for (x, z) in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)] {
        b.set(x, wl, z, "water[level=0]");
    }
    // the pad's walls: they hold its water and keep golems on it (fluid holder and walk blocker);
    // a wall top a golem could spawn on gets its cap from the judge's spot check (parts::cap_spawns)
    for y in wl..=wl + 1 {
        for x in (x0 - 1)..=(x1 + 1) {
            b.set(x, y, z0 - 1, STONE);
            b.set(x, y, z1 + 1, STONE);
        }
        for z in z0..=z1 {
            b.set(x0 - 1, y, z, STONE);
            b.set(x1 + 1, y, z, STONE);
        }
    }
    // the shaft (x 1..4, z 3..6) down to feet level k
    for y in (k - 1)..fl.min(top) {
        for x in 0..=5 {
            for z in 2..=7 {
                let inside = ring(x, z) && y >= k;
                let mouth = (1..=4).contains(&x) && z == 2 && (k..=k + 2).contains(&y);
                if !inside && !mouth {
                    b.set(x, y, z, STONE);
                }
            }
        }
    }
    // the bottom: sources in its two south corners only; the levels then fall toward the north
    // row's middle, so every golem is pushed north and toward x 2..3, into the chamber
    for (x, z) in [(1, 6), (4, 6)] {
        b.set(x, k, z, "water[level=0]");
    }
}

/// The kill chamber of L2d north of its shaft's bottom, as wide as the shaft (x 1..4, z 1..2,
/// feet k), so no golem meets a corner: lava at head height held by open fence gates below it and
/// at its mouth (z 3), a ceiling right above it. The water enters from the shaft's south corners,
/// so its levels fall toward the middle columns: drops drift to the 2 hoppers under x 2..3.
pub(crate) fn chamber2(b: &mut Builder, k: i32) {
    for x in 1..=4 {
        for y in k..=k + 3 {
            b.set(x, y, 0, STONE);
        }
        b.set(x, k + 3, 1, STONE);
        b.set(x, k + 3, 2, STONE);
        for z in 1..=2 {
            b.set(x, k + 1, z, GATE_E);
            b.set(x, k + 2, z, LAVA);
        }
        b.set(x, k + 2, 3, GATE_N);
        b.set(x, k - 1, 2, STONE);
        if x == 1 || x == 4 {
            b.set(x, k - 1, 1, STONE);
        }
    }
    for z in 0..=2 {
        for y in (k - 1)..=(k + 3) {
            b.set(0, y, z, STONE);
            b.set(5, y, z, STONE);
        }
    }
    b.set(2, k - 1, 1, "hopper[enabled=true,facing=east]");
    b.set(3, k - 1, 1, "hopper[enabled=true,facing=down]");
    b.d.ports.insert("output".into(), [3, k - 2, 1]);
}

/// L2d, the middle ground with a dry drop: L2's front (3 villagers, or L4's 4), the dusk pulse,
/// the dry hole (golems fall out of every villager's range without sinking through water) and a
/// 2-hopper chamber: 13 iron.
fn l2d_module(villagers: i32, n: i32) -> Builder {
    let mut b = Builder::new(&format!("iron-middle-dry-{villagers}-{n}"));
    if villagers == 4 {
        front4(&mut b);
    } else {
        for x in [0, 2, 4] {
            b.pod(x);
        }
        zombie(&mut b);
        dusk_pulse(&mut b);
    }
    let (wl, k) = (-6, -22);
    dry_hole(&mut b, n, wl, k);
    chamber2(&mut b, k);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("restart".into(), "yes".into());
    b
}

fn l2d_caption(villagers: i32, n: i32) -> Vec<String> {
    let front = if villagers == 4 {
        "four villagers on their beds in closed 1-cell pods under leaves (only three must agree, so a golem one of\nthem sees does not stretch the cycle) and a zombie in a boat 7 blocks north of them in view. At nightfall\ntwo inverted daylight detectors open the three trapdoors in front of the zombie for 46 game ticks"
    } else {
        "three villagers on their beds in closed 1-cell pods under leaves and a zombie in a boat 7 blocks north of\nthem in view. At nightfall two inverted daylight detectors open the trapdoor in front of the zombie for 46\ngame ticks"
    };
    format!(
        "{front}: the villagers fall asleep, then wake up scared for the rest of the night (the farm restarts by\nitself after a missed night). Golems spawn on a {n}x{n} pad; water from its corners pushes them onto the hole's\nring (water over open fence gates: it holds the water, not a golem) and into its dry 2x2 center (open fence\ngates stop the water), and they fall 16 blocks out of every villager's sight to water that pushes them under\na lava blade held by fence gates, a ceiling right above it, over two hoppers."
    )
    .lines()
    .map(|l| l.to_string())
    .collect()
}

/// L2d, the dry-drop module: L2's front (3 villagers, or L4's 4), the dusk pulse, the dry hole in
/// an n x n pad (golems fall out of every villager's range without sinking through water) and a
/// 2-hopper chamber: 13 iron.
pub fn l2d(villagers: i32, n: i32) -> String {
    let b = l2d_module(villagers, n);
    let mut cap = vec![format!("L2d, the dry-drop iron farm module:")];
    cap.extend(l2d_caption(villagers, n));
    let cap: Vec<&str> = cap.iter().map(|s| s.as_str()).collect();
    b.text(&cap)
}

/// L5, the scaled farm: g x g L2d modules at a 27-block pitch (no golem of one module is ever in
/// another module's villagers' follow range, 26.9 at most); each module keeps its own villagers,
/// zombie, dusk pulse and kill. The first module's chest is the output; the others' are blocks.
pub fn l5(villagers: i32, n: i32, g: i32) -> String {
    let m = l2d_module(villagers, n).d;
    let mut b = Builder::new(&format!("iron-scaled-{g}x{g}"));
    b.d.meta = m.meta.clone();
    let out = m.ports["output"];
    for i in 0..g {
        for j in 0..g {
            let (dx, dz) = (27 * i, 27 * j);
            for (p, s) in &m.blocks {
                b.d.blocks.insert([p[0] + dx, p[1], p[2] + dz], s.clone());
            }
            for e in &m.entities {
                b.d.entities.push(EntitySpec { kind: e.kind.clone(), pos: [e.pos[0] + dx as f64, e.pos[1], e.pos[2] + dz as f64], nbt: e.nbt.clone() });
            }
            if i == 0 && j == 0 {
                b.d.ports.insert("output".into(), out);
            } else {
                b.set(out[0] + dx, out[1], out[2] + dz, "chest[facing=north,type=single,waterlogged=false]");
            }
        }
    }
    let mut cap = vec![format!("L5, the scaled iron farm: {g} x {g} L2d modules at a 27-block pitch, each one:")];
    cap.extend(l2d_caption(villagers, n));
    let cap: Vec<&str> = cap.iter().map(|s| s.as_str()).collect();
    b.text(&cap)
}
