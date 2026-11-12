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

/// One part of the column: `NAME=FILE[@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]`.
#[derive(Clone, Debug, PartialEq)]
pub struct PartSpec {
    pub name: String,
    pub file: PathBuf,
    /// The move from the file's frame into the column's (`@`).
    pub frame: P,
    /// The move from the part's committed place in the column's frame to its tier (`^`).
    pub tier: P,
    /// The cells kept, in the file's frame (`#`, inclusive); none: the whole file.
    pub keep: Option<(P, P)>,
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
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("--part {s:?}: NAME=FILE[@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]"))?;
        let marks = ['@', '^', '#'];
        let cut = rest.find(marks).unwrap_or(rest.len());
        let mut p = PartSpec { name: name.trim().to_string(), file: PathBuf::from(&rest[..cut]), frame: [0; 3], tier: [0; 3], keep: None };
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
    Ok(m)
}

pub fn load_part(spec: &PartSpec) -> Result<Part, String> {
    let text = std::fs::read_to_string(&spec.file).map_err(|e| format!("{}: {e}", spec.file.display()))?;
    let d = Design::parse(&text).map_err(|e| format!("{}: {e}", spec.file.display()))?;
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
    Ok(Part { spec: spec.clone(), committed, placed, positions, declared })
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
/// own path) the bed-radius report after the tiers, its verdict third.
#[allow(clippy::too_many_arguments)]
pub fn run_bed(specs: &[PartSpec], extra: Vec<Part>, clear: &[(P, P)], reserves: &[Reserve], ports: &[(String, P)], f: &Frame, name: &str, with_plan: bool, bed: Option<(P, &[String])>) -> Result<(String, Design, bool), String> {
    let mut parts: Vec<Part> = specs.iter().map(load_part).collect::<Result<_, _>>()?;
    let mut cleared = 0usize;
    for p in &mut parts {
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
    let mut track_designs: Vec<Design> = tracks.iter().map(|(name, _, _)| Design { name: format!("track:{name}"), ..Default::default() }).collect();
    for (c, shape) in &rails {
        track_designs[owner[c]].blocks.insert(*c, format!("minecraft:rail[shape={shape},waterlogged=false]"));
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
        let spec = PartSpec { name: d.name.clone(), file: PathBuf::from(file), frame: [0; 3], tier: [0; 3], keep: None };
        Part { spec, committed: d.clone(), placed: d, positions: Vec::new(), declared: Vec::new() }
    };
    let mut out: Vec<Part> = track_designs.into_iter().filter(|t| !t.blocks.is_empty()).map(|t| part(t, "(sketch track)")).collect();
    if !d.blocks.is_empty() || !d.entities.is_empty() {
        out.push(part(d, "(sketch carts, posts)"));
    }
    Ok(out)
}

/// A rail block's shape, from its state string (`minecraft:rail[shape=north_east,...]`, powered,
/// detector and activator rails alike); None for anything else.
fn rail_shape_of(state: &str) -> Option<&str> {
    let base = state.strip_prefix("minecraft:").unwrap_or(state);
    if !(base.starts_with("rail[") || base.starts_with("powered_rail[") || base.starts_with("detector_rail[") || base.starts_with("activator_rail[")) {
        return None;
    }
    let i = base.find("shape=")? + 6;
    let rest = &base[i..];
    Some(&rest[..rest.find([',', ']']).unwrap_or(rest.len())])
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!((q.frame, q.tier, q.keep), ([0; 3], [0; 3], None));
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
