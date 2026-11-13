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
#[derive(Clone, Debug, PartialEq)]
pub struct Reserve {
    pub name: String,
    pub b: (P, P),
}

impl Reserve {
    pub fn parse(s: &str) -> Result<Reserve, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("--reserve {s:?}: NAME=x0,y0,z0,x1,y1,z1"))?;
        let n = ints(rest)?;
        if n.len() != 6 {
            return Err(format!("--reserve {s:?}: NAME=x0,y0,z0,x1,y1,z1"));
        }
        Ok(Reserve { name: name.trim().to_string(), b: ([n[0].min(n[3]), n[1].min(n[4]), n[2].min(n[5])], [n[0].max(n[3]), n[1].max(n[4]), n[2].max(n[5])]) })
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
    let mut out = Vec::new();
    for (i, a) in parts.iter().enumerate() {
        for b in &parts[i + 1..] {
            let both: Vec<&P> = a.placed.blocks.keys().filter(|p| b.placed.blocks.contains_key(*p)).collect();
            if !both.is_empty() {
                out.push(format!("{} and {}: {} cells, e.g. {}", a.spec.name, b.spec.name, both.len(), both.iter().take(4).map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" ")));
            }
        }
    }
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
    let mut parts: Vec<Part> = specs.iter().map(load_part).collect::<Result<_, _>>()?;
    let mut cleared = 0usize;
    for p in parts.iter_mut().filter(|p| !p.spec.relay) {
        for b in clear {
            let n = p.placed.blocks.len();
            p.placed.blocks.retain(|c, _| !inside(c, b));
            p.placed.block_entities.retain(|c, _| !inside(c, b));
            p.placed.entities.retain(|e| !inside(&e.cell(), b));
            p.placed.ports.retain(|_, c| !inside(c, b));
            p.declared.retain(|x| !inside(&x.cell, b));
            cleared += n - p.placed.blocks.len();
        }
    }
    parts.extend(extra);
    let mut s = format!("column x {}..{} z {}..{} (frame cells), chunk lines at x = {} + 16k, z = {} + 16k\n", f.x0, f.x1, f.z0, f.z1, f.phase_x, f.phase_z);
    for b in clear {
        s.push_str(&format!("cleared {} (file parts' blocks taken out of {} first: {cleared} blocks in all the boxes)\n", fmt_box(b), "the box"));
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
    if cl.is_empty() {
        s.push_str("clashes: none (no cell held by two parts, no reserved box over another's)\n");
    } else {
        for c in &cl {
            s.push_str(&format!("CLASH {c}\n"));
        }
    }
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
    Ok((s, d, bed_ok))
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
    let source = |s: &str| {
        let n = s.trim_start_matches("minecraft:");
        let n = n.split('[').next().unwrap_or(n);
        matches!(n, "detector_rail" | "redstone_wire" | "redstone_torch" | "redstone_wall_torch" | "repeater" | "comparator" | "observer" | "lever" | "tripwire_hook" | "daylight_detector" | "target" | "trapped_chest" | "lectern" | "sculk_sensor" | "calibrated_sculk_sensor" | "lightning_rod" | "redstone_block" | "jukebox")
            || n.ends_with("_lightning_rod")
            || n.ends_with("_button")
            || n.ends_with("_pressure_plate")
    };
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
    if !committed.nether_blocks.is_empty() || !composed.nether_blocks.is_empty() {
        let nl = committed.nether_blocks.iter().filter(|(p, b)| composed.nether_blocks.get(*p) != Some(*b)).count();
        let nm = composed.nether_blocks.iter().filter(|(p, _)| !committed.nether_blocks.contains_key(*p)).count();
        s.push_str(&format!("Nether part: {} cells in the file, {} declared: {nl} leftover or different, {nm} missing\n", committed.nether_blocks.len(), composed.nether_blocks.len()));
    }
    let ok = kept_ok && leftover.is_empty() && missing.is_empty() && differ.is_empty() && ent_left.is_empty() && ent_missing.is_empty();
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
    // stop's box (x -6..4, z -6..5, the bed head's y - 2 to + 5) a villager can stand in, its feet
    // on top of the cell's own collision shape or on the full cube under an empty cell, 1.95 of
    // head room free above them; labelled by cell like the rails, so the declared waker places
    // match by cell
    let stop_box = d.ports.get("home").map(|h| ([-6, h[1] - 2, -6], [4, h[1] + 5, 5]));
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
                }
            }
        }
    }
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
                None => in_sight.push(row),
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
        if let Some((d, place, p)) = points.iter().filter(|(_, p)| in_box(p)).map(|(place, p)| (((p.x - z.x).powi(2) + (p.y - z.y).powi(2) + (p.z - z.z).powi(2)).sqrt(), place, p)).min_by(|a, b| a.0.total_cmp(&b.0)) {
            let pass = d >= 6.05;
            stop_ok &= pass;
            stop_rows.push(format!("the zombie at ({:.2},{:.2},{:.2}): {d:.2} from the nearest stop position {place} ({:.2},{:.2},{:.2}) ({})", z.x, z.y, z.z, p.x, p.y, p.z, if pass { "ok" } else { "FAIL" }));
        }
    }
    let ok = in_sight.is_empty() && stop_ok;
    let s = format!(
        "the 6.05 flee bar, sight-gated ({} rider positions every 0.1 along every rail and both lifts ({} riderless rail cells declared and left out), and {} on-foot positions in the stop's box, the feet on the cell's own collision shape or on the top of the collider below an empty cell when it reaches the cell ({} standable cells declared unreachable and left out; {} cells holding liquid left out; the cells whose own shape reaches its top without a full cube, stood on from the cell above: {}), {} zombies; the eye at the cart (or the feet) + 1.62, the zombie's at its feet + 1.74; hidden: a full-collider block on the eye-to-eye ray; {} declared waker places): IN SIGHT within 6.05 {}{}; the waker's declared places in sight within 6.05 {}{}; hidden within 6.05 {}{}; clear {}; every other zombie at least 6.05 from every stop position (sight or not): {}; the landings on a chest (G1): {}; the pre-wake state (the open hatch's plates, hiding the zombie until F shuts them) is not a full-collider test: it rests on tick-exact's official A6 scene (NEAREST_HOSTILE never set with the hatch open); {}\n",
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
    // by the land world's propagation, its floor and its box: `hostile::spawn_parts`)
    let kind = redsim::entity::mob::natural::kind_of("minecraft:creeper");
    let starts: Vec<(BlockPos, f64)> = reach
        .cells
        .iter()
        .filter(|(c, _)| !rail_cells.contains(c) && w.block_light(*c).unwrap_or(0) == 0 && kind.is_none_or(|k| {
            let (_, placement, free) = hostile::spawn_parts(&w, k, *c);
            placement && free
        }))
        .copied()
        .collect();
    // (the light at every spawn space beside a target: an air cell face-adjacent to it with a
    // floor below)
    let mut spaces: Vec<(BlockPos, i32)> = Vec::new();
    for t in &targets {
        for q in [BlockPos::new(t.x + 1, t.y, t.z), BlockPos::new(t.x - 1, t.y, t.z), BlockPos::new(t.x, t.y, t.z + 1), BlockPos::new(t.x, t.y, t.z - 1), BlockPos::new(t.x, t.y + 1, t.z)] {
            let air = w.get(q).to_string().trim_start_matches("minecraft:").starts_with("air");
            let floor = !w.get(q.below()).to_string().trim_start_matches("minecraft:").starts_with("air");
            if air && floor && !spaces.iter().any(|(p, _)| *p == q) {
                spaces.push((q, w.block_light(q).unwrap_or(-1)));
            }
        }
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
    let mut d = Design { name: "blocks".into(), ..Default::default() };
    for (c, s) in blocks {
        d.blocks.insert(*c, if s.contains(':') { s.clone() } else { format!("minecraft:{s}") });
    }
    let spec = PartSpec { name: "blocks".into(), file: PathBuf::from("(--block)"), base: None, frame: [0; 3], tier: [0; 3], keep: None, relay: true, nether: false };
    Part { spec, committed: d.clone(), placed: d, positions: Vec::new(), declared: Vec::new(), base_changed: Vec::new() }
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
        // a clash is reported, and a reserved box over a part's blocks
        let specs2 = vec![PartSpec::parse(&format!("a={}", f1.display())).unwrap(), PartSpec::parse(&format!("b={}", f1.display())).unwrap()];
        let (rep2, _) = run(&specs2, Vec::new(), &[], &[Reserve::parse("fold=0,-1,0,1,0,0").unwrap(), Reserve::parse("run=1,-1,0,1,-1,0").unwrap()], &[], &f, "sk", false).unwrap();
        assert!(rep2.contains("CLASH a and b: 6 cells"), "{rep2}");
        assert!(rep2.contains("CLASH fold (reserved) and a: 2 of a's cells"), "{rep2}");
        assert!(rep2.contains("CLASH fold and run (both reserved)"), "{rep2}");
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
        assert!(rep5.contains("3 blocks in all the boxes"), "{rep5}");
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
