//! A bench row counts toward a bar only when the bench's cells match the design of record's (ironspec,
//! Nov 12 2026 21:43, adopted as a general rule): every part bench prints its cells against the design
//! of record (record7, or the part's package file) over its world box; each differing cell is listed as
//! a declared STAND-IN (with its seven-property guard) or a declared CANDIDATE (the row then counts for
//! that candidate, not the module); its rows count toward the module's bars only when the UNDECLARED
//! diff is 0 and every stand-in's guard passes.
//!
//! [`compare`] (cells) and [`compare_world`] (a bench's world over its box) make the diff,
//! [`RecordMatch::line`] the printed line and [`RecordMatch::counts`] the verdict. A stand-in's guard
//! ([`Guard`]): golem, path, late-path effects and sight are filled from the two blocks' intrinsic
//! properties ([`intrinsic`]) where the owner declared nothing; update reach, power reach and survival
//! depend on the stand-in's context and come from the owner's bench (a missing or failing verdict leaves
//! the stand-in unguarded, and its rows don't count). A bench's emitted design file declares its
//! stand-ins, their owner-checked verdicts and its candidates in header lines ([`declared_from_header`]);
//! `mctas mobfarm record-match` runs the compare on any emitted file.

use std::collections::{BTreeMap, BTreeSet};

pub type Cell = [i32; 3];

/// The seven properties a stand-in passes (the brief's next7, ironspec Nov 12 evening): golem
/// (legacy-solid, full top face), path (full-block collision, solid render), late-path effects, update
/// reach, power reach within the built input's, survival on every laid world and write, and sight (every
/// stand-in cell blocks or passes every rider -> scarer and rider -> rider ray exactly as the built cell
/// does).
pub const PROPERTIES: [&str; 7] = ["golem", "path", "late-path", "update reach", "power reach", "survival", "sight"];

/// One property's verdict: passes (how it was checked) or fails (why).
pub type Verdict = Result<String, String>;

/// A stand-in's verdicts, one per [`PROPERTIES`] entry (None: not checked).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guard(pub [Option<Verdict>; 7]);

impl Guard {
    /// Every property checked and passing.
    pub fn passes(&self) -> bool {
        self.0.iter().all(|v| matches!(v, Some(Ok(_))))
    }

    /// The properties not checked or failing, each named (`update reach unchecked`, `sight: why`).
    pub fn gaps(&self) -> Vec<String> {
        PROPERTIES
            .iter()
            .zip(self.0.iter())
            .filter_map(|(p, v)| match v {
                Some(Ok(_)) => None,
                Some(Err(why)) => Some(format!("{p} FAILS: {why}")),
                None => Some(format!("{p} unchecked")),
            })
            .collect()
    }

    /// Set a property's verdict by name (`update reach` or `update`, `power reach` or `power`,
    /// `late-path`, …).
    pub fn set(&mut self, name: &str, v: Verdict) -> Result<(), String> {
        let n = name.trim();
        let k = PROPERTIES.iter().position(|p| *p == n || p.split(' ').next() == Some(n)).ok_or(format!("{n:?} is none of the seven properties ({})", PROPERTIES.join(", ")))?;
        self.0[k] = Some(v);
        Ok(())
    }
}

/// A declared stand-in cell: why it stands in, and its verdicts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StandIn {
    pub why: String,
    pub guard: Guard,
}

/// A bench's declarations: its stand-in cells and its candidate cells (the candidate's name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Declared {
    pub standins: BTreeMap<Cell, StandIn>,
    pub candidates: BTreeMap<Cell, String>,
}

/// How a differing cell is classed.
#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    /// A declared stand-in; `guarded`: its seven verdicts all pass.
    StandIn { guarded: bool },
    /// A declared candidate's cell.
    Candidate(String),
    Undeclared,
}

/// One differing cell: the bench's state, the record's (both canonical; `minecraft:air` where absent),
/// its class and, for a stand-in, its guard's gaps.
#[derive(Clone, Debug)]
pub struct Diff {
    pub cell: Cell,
    pub bench: String,
    pub record: String,
    pub class: Class,
    pub gaps: Vec<String>,
}

/// The compare's result.
#[derive(Clone, Debug)]
pub struct RecordMatch {
    pub record: String,
    pub lo: Cell,
    pub hi: Cell,
    /// The record's frame + `offset` = the bench's.
    pub offset: Cell,
    pub compared: usize,
    pub diffs: Vec<Diff>,
    /// Declared cells that don't differ (a stale declaration: printed, not a failure) or lie outside the box.
    pub stale: Vec<Cell>,
}

/// Which bars the bench's rows count toward.
#[derive(Clone, Debug, PartialEq)]
pub enum Counts {
    /// The module's (0 undeclared, every stand-in guarded, no candidate).
    Module,
    /// Only these candidates' (0 undeclared, every stand-in guarded).
    Candidates(Vec<String>),
    /// None: why.
    No(String),
}

impl RecordMatch {
    pub fn undeclared(&self) -> Vec<&Diff> {
        self.diffs.iter().filter(|d| d.class == Class::Undeclared).collect()
    }

    pub fn unguarded(&self) -> Vec<&Diff> {
        self.diffs.iter().filter(|d| d.class == Class::StandIn { guarded: false }).collect()
    }

    pub fn candidates(&self) -> Vec<String> {
        let c: BTreeSet<String> = self.diffs.iter().filter_map(|d| if let Class::Candidate(n) = &d.class { Some(n.clone()) } else { None }).collect();
        c.into_iter().collect()
    }

    pub fn counts(&self) -> Counts {
        let (u, g) = (self.undeclared().len(), self.unguarded().len());
        if u > 0 || g > 0 {
            return Counts::No(format!("{u} undeclared cell(s), {g} unguarded stand-in(s)"));
        }
        let c = self.candidates();
        if c.is_empty() { Counts::Module } else { Counts::Candidates(c) }
    }

    /// The line every part bench prints (ironspec 21:43).
    pub fn line(&self) -> String {
        let cell = |c: &Cell| format!("({},{},{})", c[0], c[1], c[2]);
        let short = |s: &str| s.trim_start_matches("minecraft:").to_string();
        let st: Vec<&Diff> = self.diffs.iter().filter(|d| matches!(d.class, Class::StandIn { .. })).collect();
        let ug = self.unguarded();
        let cand: Vec<&Diff> = self.diffs.iter().filter(|d| matches!(d.class, Class::Candidate(_))).collect();
        let und = self.undeclared();
        let first = |v: &[&Diff], f: &dyn Fn(&Diff) -> String| -> String {
            let mut s: Vec<String> = v.iter().take(12).map(|d| f(d)).collect();
            if v.len() > 12 {
                s.push(format!("… {} more", v.len() - 12));
            }
            s.join("; ")
        };
        format!(
            "cells against the design of record {} (its frame + ({},{},{})) over the box {}..={}: {} compared, {} the same; {} stand-in(s){}; {} candidate cell(s){}; {} UNDECLARED{}{} -> {}",
            self.record,
            self.offset[0],
            self.offset[1],
            self.offset[2],
            cell(&self.lo),
            cell(&self.hi),
            self.compared,
            self.compared - self.diffs.len(),
            st.len(),
            if st.is_empty() {
                String::new()
            } else if ug.is_empty() {
                " (every one guarded: its seven verdicts pass)".to_string()
            } else {
                format!(" ({} UNGUARDED: {})", ug.len(), first(&ug, &|d: &Diff| format!("{} {} for {}: {}", cell(&d.cell), short(&d.bench), short(&d.record), d.gaps.join(", "))))
            },
            cand.len(),
            if cand.is_empty() { String::new() } else { format!(" ({})", self.candidates().join(", ")) },
            und.len(),
            if und.is_empty() { String::new() } else { format!(": {}", first(&und, &|d: &Diff| format!("{} bench {} vs record {}", cell(&d.cell), short(&d.bench), short(&d.record)))) },
            if self.stale.is_empty() { String::new() } else { format!("; declared but the same or outside the box (stale): {}", self.stale.iter().map(cell).collect::<Vec<_>>().join(", ")) },
            match self.counts() {
                Counts::Module => "the rows count toward the module's bars".to_string(),
                Counts::Candidates(c) => format!("the rows count toward the candidate(s) {} only, not the module", c.join(", ")),
                Counts::No(why) => format!("the rows DON'T count toward any bar ({why})"),
            }
        )
    }
}

/// A block state's canonical text (redsim's parse and print: every property, in its order); every air
/// (`air`, `cave_air`, `void_air`, absent) reads `minecraft:air`; a state redsim can't parse is kept as
/// written with its properties sorted.
pub fn canon(s: &str) -> String {
    let s = s.trim();
    let id = s.split('[').next().unwrap_or("");
    let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
    if s.is_empty() || matches!(id.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
        return "minecraft:air".to_string();
    }
    match redsim::block::Block::parse(s) {
        Ok(b) if !b.is_air() => b.to_string(),
        Ok(_) => "minecraft:air".to_string(),
        Err(_) => {
            let mut props: Vec<&str> = s.split_once('[').map(|(_, p)| p.trim_end_matches(']').split(',').map(str::trim).filter(|p| !p.is_empty()).collect()).unwrap_or_default();
            props.sort();
            if props.is_empty() { id } else { format!("{id}[{}]", props.join(",")) }
        }
    }
}

/// golem / path / late-path / sight from the two blocks' intrinsic properties: the stand-in passes each
/// exactly when it answers it as the built block does (golem: `legacy_golem_can_spawn_on` under the
/// built cell above; path: full-block collision and solid render; late-path: the friction and the
/// collision boxes a walker meets; sight: the collision boxes, a mob's sight ray's colliders). Update
/// reach, power reach and survival stay unchecked (None): they depend on the stand-in's neighbours and
/// drive, which only its bench knows.
pub fn intrinsic(standin: &str, built: &str, above: &str) -> [Option<Verdict>; 7] {
    use redsim::block::Block;
    let parse = |s: &str| Block::parse(&canon(s)).unwrap_or(Block::Air);
    let (a, b, up) = (parse(standin), parse(built), parse(above));
    let same = |what: &str, x: String, y: String| -> Verdict { if x == y { Ok(format!("{what} {x}, as built")) } else { Err(format!("{what} {x} vs the built cell's {y}")) } };
    let boxes = |s: &Block| redsim::behavior::collision_boxes(s);
    let full = |s: &Block| boxes(s) == vec![[0, 16, 0, 16, 0, 16]];
    let golem = |s: &Block| redsim::entity::mob::blocks::legacy_golem_can_spawn_on(s, &up);
    let render = |s: &Block| redsim::entity::mob::ai::goal::stroll::is_solid_render(s);
    let fr = |s: &Block| redsim::entity::mob::blocks::friction(s);
    let mut v: [Option<Verdict>; 7] = Default::default();
    v[0] = Some(same("a golem spawn on it", golem(&a).to_string(), golem(&b).to_string()));
    v[1] = Some(same("full collision / solid render", format!("{}/{}", full(&a), render(&a)), format!("{}/{}", full(&b), render(&b))));
    v[2] = Some(same("friction / collision boxes", format!("{}/{:?}", fr(&a), boxes(&a)), format!("{}/{:?}", fr(&b), boxes(&b))));
    v[6] = Some(same("the collision boxes (sight's colliders)", format!("{:?}", boxes(&a)), format!("{:?}", boxes(&b))));
    v
}

/// The compare: `bench`'s cells (its frame; a cell it lacks is air) against `record`'s, each record cell
/// shifted by `offset` into the bench's frame, over lo..=hi; `declared` classes each differing cell (a
/// stand-in's guard: its declared verdicts, with golem / path / late-path / sight filled from
/// [`intrinsic`] where it declared none).
pub fn compare(bench: &BTreeMap<Cell, String>, record: &BTreeMap<Cell, String>, offset: Cell, lo: Cell, hi: Cell, declared: &Declared, record_name: &str) -> RecordMatch {
    let inside = |c: &Cell| (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]);
    let rec: BTreeMap<Cell, String> = record.iter().map(|(c, s)| ([c[0] + offset[0], c[1] + offset[1], c[2] + offset[2]], canon(s))).filter(|(c, _)| inside(c)).collect();
    let get = |m: &BTreeMap<Cell, String>, c: &Cell, canonical: bool| m.get(c).map(|s| if canonical { s.clone() } else { canon(s) }).unwrap_or_else(|| "minecraft:air".to_string());
    let mut diffs = Vec::new();
    let mut compared = 0usize;
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let c = [x, y, z];
                compared += 1;
                let (b, r) = (get(bench, &c, false), get(&rec, &c, true));
                if b == r {
                    continue;
                }
                let (class, gaps) = if let Some(si) = declared.standins.get(&c) {
                    let mut g = si.guard.clone();
                    let above = get(&rec, &[x, y + 1, z], true);
                    for (k, v) in intrinsic(&b, &r, &above).into_iter().enumerate() {
                        if g.0[k].is_none() {
                            g.0[k] = v;
                        }
                    }
                    (Class::StandIn { guarded: g.passes() }, g.gaps())
                } else if let Some(n) = declared.candidates.get(&c) {
                    (Class::Candidate(n.clone()), Vec::new())
                } else {
                    (Class::Undeclared, Vec::new())
                };
                diffs.push(Diff { cell: c, bench: b, record: r, class, gaps });
            }
        }
    }
    let differs: BTreeSet<Cell> = diffs.iter().map(|d| d.cell).collect();
    let stale: Vec<Cell> = declared.standins.keys().chain(declared.candidates.keys()).filter(|c| !differs.contains(*c)).copied().collect::<BTreeSet<_>>().into_iter().collect();
    RecordMatch { record: record_name.to_string(), lo, hi, offset, compared, diffs, stale }
}

/// [`compare`] over a bench's world: every cell of its box (`World::min`, `World::size`).
pub fn compare_world(w: &redsim::world::World, record: &BTreeMap<Cell, String>, offset: Cell, declared: &Declared, record_name: &str) -> RecordMatch {
    let lo = [w.min.x, w.min.y, w.min.z];
    let hi = [w.min.x + w.size[0] - 1, w.min.y + w.size[1] - 1, w.min.z + w.size[2] - 1];
    let mut bench: BTreeMap<Cell, String> = BTreeMap::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let s = w.get(redsim::pos::BlockPos::new(x, y, z));
                if !s.is_air() {
                    bench.insert([x, y, z], s.to_string());
                }
            }
        }
    }
    compare(&bench, record, offset, lo, hi, declared, record_name)
}

/// Split `s` at `sep` outside parentheses.
fn split_outside(s: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if ch == sep && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// `x,y,z` at the start of `s`, and the rest.
fn cell_and_rest(s: &str) -> Result<(Cell, String), String> {
    let s = s.trim();
    let mut parts = s.splitn(4, |c: char| c == ',' || c.is_whitespace() || c == ':');
    let mut c = [0i32; 3];
    for v in c.iter_mut() {
        let t = parts.next().ok_or(format!("{s:?}: x,y,z"))?;
        *v = t.trim().parse::<i32>().map_err(|e| format!("{s:?}: x,y,z ({e})"))?;
    }
    Ok((c, parts.next().unwrap_or("").trim().trim_start_matches(':').trim().to_string()))
}

/// A bench file's declarations, from its header lines:
/// - `# stand-in: x,y,z[ (why)]; …` (dfb89499's `--out` format);
/// - `# stand-in-guard: x,y,z: PROPERTY=ok[ (how)] | PROPERTY=fail (why), …; …` (the owner-checked
///   verdicts: update reach, power reach, survival, or any of the seven);
/// - `# candidate: NAME: x,y,z; …`.
pub fn declared_from_header(text: &str) -> Result<Declared, String> {
    let mut d = Declared::default();
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("# stand-in:") {
            for e in split_outside(rest, ';') {
                let (c, why) = cell_and_rest(&e)?;
                let why = why.trim().trim_start_matches('(').trim_end_matches(')').trim().to_string();
                d.standins.entry(c).or_default().why = why;
            }
        } else if let Some(rest) = l.strip_prefix("# stand-in-guard:") {
            for e in split_outside(rest, ';') {
                let (c, verdicts) = cell_and_rest(&e)?;
                let si = d.standins.entry(c).or_default();
                for v in split_outside(&verdicts, ',') {
                    let (p, val) = v.split_once('=').ok_or(format!("stand-in-guard {v:?}: PROPERTY=ok|fail"))?;
                    let val = val.trim();
                    let note = val.split_once('(').map(|(_, n)| n.trim_end_matches(')').trim().to_string()).unwrap_or_default();
                    let verdict = if val.starts_with("ok") {
                        Ok(if note.is_empty() { "declared by its bench".to_string() } else { note })
                    } else if val.starts_with("fail") {
                        Err(if note.is_empty() { "declared failing by its bench".to_string() } else { note })
                    } else {
                        return Err(format!("stand-in-guard {v:?}: ok or fail"));
                    };
                    si.guard.set(p, verdict)?;
                }
            }
        } else if let Some(rest) = l.strip_prefix("# candidate:") {
            let (name, cells) = rest.split_once(':').ok_or(format!("candidate {rest:?}: NAME: x,y,z; …"))?;
            for e in split_outside(cells, ';') {
                let (c, _) = cell_and_rest(&e)?;
                d.candidates.insert(c, name.trim().to_string());
            }
        }
    }
    Ok(d)
}

/// A bench file's world box from a `# world box: x0,y0,z0..x1,y1,z1` header line, if any.
pub fn world_box_from_header(text: &str) -> Option<(Cell, Cell)> {
    let l = text.lines().find_map(|l| l.trim().strip_prefix("# world box:"))?;
    let (a, b) = l.split_once("..")?;
    let p = |s: &str| -> Option<Cell> {
        let v: Vec<i32> = s.trim().trim_start_matches('=').split(',').map(|x| x.trim().parse::<i32>().ok()).collect::<Option<Vec<_>>>()?;
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    Some((p(a)?, p(b)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(v: &[(Cell, &str)]) -> BTreeMap<Cell, String> {
        v.iter().map(|(c, s)| (*c, s.to_string())).collect()
    }

    /// The record: a little stretch (stone floor, an ascending D at (1,1,0), a curve's inner corner air
    /// at (2,1,1), steps on glass at (3,1,0)); each of tonight's three misses (ironspec 21:43: the flat-D
    /// S-bend, (C)'s inner-corner wall, R1's stand-in below the steps) reads as a nonzero UNDECLARED diff
    /// on a bench that has it; declared as a stand-in it still doesn't count until its guard's owner-checked
    /// verdicts pass; declared as a candidate it counts for the candidate only; the record's own cells count
    /// toward the module.
    #[test]
    fn the_three_misses_are_undeclared() {
        let mut rec = vec![([0, 0, 0], "minecraft:stone"), ([1, 0, 0], "minecraft:stone"), ([2, 0, 0], "minecraft:stone"), ([3, 0, 0], "minecraft:smooth_stone")];
        rec.push(([1, 1, 0], "minecraft:rail[shape=ascending_east,waterlogged=false]"));
        rec.push(([3, 1, 0], "minecraft:glass"));
        let record = cells(&rec);
        let (lo, hi) = ([0, 0, 0], [3, 2, 1]);
        let none = Declared::default();
        let m = compare(&record, &record, [0, 0, 0], lo, hi, &none, "rec");
        assert_eq!((m.compared, m.diffs.len(), m.counts()), (24, 0, Counts::Module), "{}", m.line());
        assert!(m.line().ends_with("-> the rows count toward the module's bars"), "{}", m.line());
        // (1) the flat-D S-bend: the bench's D flat where the record's ascends
        let mut flat = record.clone();
        flat.insert([1, 1, 0], "minecraft:rail[shape=east_west,waterlogged=false]".into());
        let m1 = compare(&flat, &record, [0, 0, 0], lo, hi, &none, "rec");
        assert_eq!(m1.undeclared().len(), 1, "{}", m1.line());
        assert!(matches!(m1.counts(), Counts::No(_)) && m1.line().contains("(1,1,0) bench rail[shape=east_west,waterlogged=false] vs record rail[shape=ascending_east"), "{}", m1.line());
        // (2) (C)'s inner-corner wall: glass where the record has air
        let mut wall = record.clone();
        wall.insert([2, 1, 1], "minecraft:glass".into());
        let m2 = compare(&wall, &record, [0, 0, 0], lo, hi, &none, "rec");
        assert!(m2.undeclared().len() == 1 && m2.line().contains("(2,1,1) bench glass vs record air"), "{}", m2.line());
        // (3) R1's stand-in below the steps: glass where the record has smooth stone
        let mut r1 = record.clone();
        r1.insert([3, 0, 0], "minecraft:glass".into());
        let m3 = compare(&r1, &record, [0, 0, 0], lo, hi, &none, "rec");
        assert!(m3.undeclared().len() == 1 && m3.line().contains("(3,0,0) bench glass vs record smooth_stone"), "{}", m3.line());
        // (the same cell declared a stand-in: its intrinsic verdicts filled (glass vs smooth stone: golem the
        // same here, the glass above blocking a spawn on either; path fails, glass not solid render), the
        // owner's three unchecked: unguarded, the rows don't count)
        let mut st = Declared::default();
        st.standins.insert([3, 0, 0], StandIn { why: "the old supports".into(), guard: Guard::default() });
        let m4 = compare(&r1, &record, [0, 0, 0], lo, hi, &st, "rec");
        assert_eq!((m4.undeclared().len(), m4.unguarded().len()), (0, 1), "{}", m4.line());
        assert!(m4.line().contains("UNGUARDED") && m4.line().contains("update reach unchecked") && m4.line().contains("path FAILS") && !m4.line().contains("golem FAILS"), "{}", m4.line());
        // (a stand-in equal in every intrinsic and declared passing on the owner's three: guarded; the rows
        // count toward the module)
        let mut lev = record.clone();
        lev.insert([0, 1, 1], "minecraft:stone_button[face=floor,facing=north,powered=false]".into());
        let mut g = Guard::default();
        for p in ["update", "power", "survival"] {
            g.set(p, Ok("its bench's check".into())).unwrap();
        }
        let mut ok = Declared::default();
        ok.standins.insert([0, 1, 1], StandIn { why: "a drive's stand-in".into(), guard: g });
        let m5 = compare(&lev, &record, [0, 0, 0], lo, hi, &ok, "rec");
        assert_eq!((m5.unguarded().len(), m5.counts()), (0, Counts::Module), "{}", m5.line());
        // (a candidate: counts for it, not the module)
        let mut c = Declared::default();
        c.candidates.insert([2, 1, 1], "wet-descent-2".into());
        let m6 = compare(&wall, &record, [0, 0, 0], lo, hi, &c, "rec");
        assert_eq!(m6.counts(), Counts::Candidates(vec!["wet-descent-2".into()]), "{}", m6.line());
        // (a stale declaration printed, not failing; an offset frame)
        let m7 = compare(&record, &record, [0, 0, 0], lo, hi, &c, "rec");
        assert!(m7.counts() == Counts::Module && m7.line().contains("(stale): (2,1,1)"), "{}", m7.line());
        let shifted: BTreeMap<Cell, String> = record.iter().map(|(k, v)| ([k[0] + 6, k[1], k[2] + 6], v.clone())).collect();
        let m8 = compare(&shifted, &record, [6, 0, 6], [6, 0, 6], [9, 2, 7], &none, "rec");
        assert_eq!(m8.counts(), Counts::Module, "{}", m8.line());
    }

    /// The header lines: stand-ins (dfb89499's), their owner-checked verdicts, candidates, the world box.
    #[test]
    fn reads_the_header() {
        let t = "# stand-in: 6,-13,-3 (the old step support, glass); 5,-14,-3\n# stand-in-guard: 6,-13,-3: update reach=ok (its six neighbours: the line's dusts), power=fail (powers the queue rail), survival=ok\n# candidate: wet-2: 8,-25,-4; 8,-25,-6\n# world box: -10,-30,-12..20,10,12\nname: x\n";
        let d = declared_from_header(t).unwrap();
        assert_eq!(d.standins.len(), 2);
        let s = &d.standins[&[6, -13, -3]];
        assert_eq!(s.why, "the old step support, glass");
        assert_eq!(s.guard.0[3], Some(Ok("its six neighbours: the line's dusts".to_string())));
        assert_eq!(s.guard.0[4], Some(Err("powers the queue rail".to_string())));
        assert_eq!(s.guard.0[5], Some(Ok("declared by its bench".to_string())));
        assert_eq!(d.candidates.get(&[8, -25, -6]).map(String::as_str), Some("wet-2"));
        assert_eq!(world_box_from_header(t), Some(([-10, -30, -12], [20, 10, 12])));
        assert!(declared_from_header("# stand-in-guard: 1,2,3: colour=ok\n").is_err());
        assert_eq!(canon("cave_air"), "minecraft:air");
        assert_eq!(canon("minecraft:rail[waterlogged=false,shape=east_west]"), canon("minecraft:rail[shape=east_west,waterlogged=false]"));
    }
}
