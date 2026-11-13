//! A bench row counts toward a bar only when the bench's cells match the design of record's (ironspec,
//! Nov 12 2026 21:43, adopted as a general rule; 22:37's specifics): every part bench prints its cells
//! against the design of record (record7, or the part's package file) over its world box (required, never
//! inferred: the bench's box + a 2-cell margin by default, covering update and power reach); each
//! differing cell is listed as a declared STAND-IN (with its seven-property guard) or a declared CANDIDATE
//! (the row then counts for that candidate, not the module); its rows count toward the module's bars only
//! when the UNDECLARED diff is 0 and every stand-in's guard passes. The design's persistent entities (the
//! scarers and their boats or posts, the queue zombie) are compared too, by type and position, the carts
//! and riders a bench spawns being declared bench inputs: a row whose bars involve mob behaviour counts
//! only with 0 undeclared entity differences as well.
//!
//! [`compare`] (cells and entities) and [`compare_world`] (a bench's world over its box) make the diff,
//! [`RecordMatch::line`] the printed line, [`RecordMatch::counts`] (block bars) and
//! [`RecordMatch::counts_mob`] (bars that involve mob behaviour) the verdicts. A stand-in's guard
//! ([`Guard`]) is the ONE shared implementation of the seven properties (ironspec 22:37: 58f770cc's bench
//! guard, onechunk's f1q2-run guard and this helper all call [`guard_intrinsics`]): golem (two roles),
//! path (the full PathType classification), late-path effects and sight come from the two blocks; update
//! reach, power reach and survival depend on the stand-in's context and come from the owner's bench (a
//! missing or failing verdict leaves the stand-in unguarded, and its rows don't count). A bench's emitted
//! design file declares its box, stand-ins, their owner-checked verdicts, candidates and bench-input
//! entities in header lines ([`declared_from_header`], [`world_box_from_header`]); `mctas mobfarm
//! record-match` runs the compare on any emitted file.

use redsim::block::Block;
use std::collections::{BTreeMap, BTreeSet};

pub type Cell = [i32; 3];

/// The seven properties a stand-in passes (the brief's next7, ironspec Nov 12 evening; 22:37): golem (the
/// cell as a floor and as a golem's spawn space), path (the full PathType classification), late-path
/// effects, update reach, power reach within the built input's, survival on every laid world and write,
/// and sight (every stand-in cell blocks or passes every rider -> scarer and rider -> rider ray exactly as
/// the built cell does).
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

    /// The properties not checked or failing, each named (`update reach unchecked`, `sight FAILS: why`).
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

/// THE shared implementation of the stand-in guard's block-intrinsic properties (ironspec 22:37: one
/// definition, owned by cc767b78; 58f770cc's bench guard, onechunk's f1q2-run guard and the record-match
/// helper all call it): `standin` passes each property exactly when it answers it as the `built` block
/// does, `above` and `below` being the built cells over and under it.
/// - golem, both roles: the cell as a FLOOR (`legacy_golem_can_spawn_on(cell, above)`: not an excluded
///   floor, solid, air or a liquid over it), and as a golem's SPAWN SPACE over the built cell below: its
///   feet cell (`legacy_golem_can_spawn_on(below, cell)`: air or a liquid over a solid floor), each of its
///   cells a valid empty spawn block (`NaturalSpawner.isValidEmptySpawnBlock`, which
///   `IronGolem.checkSpawnObstruction` checks for its feet and the two cells above: no full collision, not
///   a signal source, no fluid, no rail (`PREVENT_MOB_SPAWNING_INSIDE`), not dangerous to it (fire, lava,
///   magma, cactus, powder snow: redsim's path types for them)) and its box clear (no collision box);
/// - path: the full PathType classification (`WalkNodeEvaluator.getPathTypeFromState`, redsim's
///   `path_type::from_state`, `isPathfindable(LAND)` inside it), which a neighbouring node's evaluation
///   reads too (its floor, its fire-or-damaging neighbours), with full collision and solid render (a walk
///   target's solidity);
/// - late-path effects: the friction a walker meets and the collision boxes it stands on or pushes against;
/// - sight: the collision boxes (a mob's sight ray clips against colliders).
///
/// Update reach, power reach and survival stay unchecked (None): they depend on the stand-in's neighbours
/// and drive, which only its bench knows.
pub fn guard_intrinsics(standin: &Block, built: &Block, above: &Block, below: &Block) -> [Option<Verdict>; 7] {
    use redsim::entity::mob::ai::path::path_type::{self, PathType};
    let same = |what: &str, x: String, y: String| -> Verdict { if x == y { Ok(format!("{what} {x}, as built")) } else { Err(format!("{what} {x} vs the built cell's {y}")) } };
    let boxes = |s: &Block| redsim::behavior::collision_boxes(s);
    let full = |s: &Block| boxes(s) == vec![[0, 16, 0, 16, 0, 16]];
    let render = |s: &Block| redsim::entity::mob::ai::goal::stroll::is_solid_render(s);
    let fr = |s: &Block| redsim::entity::mob::blocks::friction(s);
    let signal = |s: &Block| redsim::behavior::is_signal_source(&redsim::world::World::new(redsim::pos::BlockPos::new(0, 0, 0), [1, 1, 1]), s);
    let valid_empty = |s: &Block| {
        let pt = path_type::from_state(s);
        !full(s) && !signal(s) && redsim::fluid::fluid_state_of(s).is_empty() && pt != PathType::Rail && !matches!(pt, PathType::Fire | PathType::Lava | PathType::Damaging | PathType::PowderSnow)
    };
    let golem = |s: &Block| {
        format!(
            "floor {} / spawn space: feet {}, a valid empty spawn block {}, its box clear {}",
            redsim::entity::mob::blocks::legacy_golem_can_spawn_on(s, above),
            redsim::entity::mob::blocks::legacy_golem_can_spawn_on(below, s),
            valid_empty(s),
            boxes(s).is_empty()
        )
    };
    let path = |s: &Block| format!("type {:?} / pathfindable {} / full collision {} / solid render {}", path_type::from_state(s), path_type::pathfindable_land(s), full(s), render(s));
    let mut v: [Option<Verdict>; 7] = Default::default();
    v[0] = Some(same("golem", golem(standin), golem(built)));
    v[1] = Some(same("path", path(standin), path(built)));
    v[2] = Some(same("friction / collision boxes", format!("{}/{:?}", fr(standin), boxes(standin)), format!("{}/{:?}", fr(built), boxes(built))));
    v[6] = Some(same("the collision boxes (sight's colliders)", format!("{:?}", boxes(standin)), format!("{:?}", boxes(built))));
    v
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
    match Block::parse(s) {
        Ok(b) if !b.is_air() => b.to_string(),
        Ok(_) => "minecraft:air".to_string(),
        Err(_) => {
            let mut props: Vec<&str> = s.split_once('[').map(|(_, p)| p.trim_end_matches(']').split(',').map(str::trim).filter(|p| !p.is_empty()).collect()).unwrap_or_default();
            props.sort();
            if props.is_empty() { id } else { format!("{id}[{}]", props.join(",")) }
        }
    }
}

/// [`guard_intrinsics`] on block-state texts. A state redsim can't parse fails every intrinsic verdict
/// ("unparsable state", ironspec 23:13 (2b): nothing silently dropped), so two unparsable cells never pass.
pub fn intrinsic(standin: &str, built: &str, above: &str, below: &str) -> [Option<Verdict>; 7] {
    let parse = |s: &str| Block::parse(&canon(s));
    match (parse(standin), parse(built), parse(above), parse(below)) {
        (Ok(a), Ok(b), Ok(up), Ok(down)) => guard_intrinsics(&a, &b, &up, &down),
        (a, b, up, down) => {
            let bad: Vec<String> = [("the stand-in", standin, a.is_err()), ("the built cell", built, b.is_err()), ("the cell above", above, up.is_err()), ("the cell below", below, down.is_err())]
                .into_iter()
                .filter(|(_, _, e)| *e)
                .map(|(what, s, _)| format!("{what} {s}"))
                .collect();
            let why = format!("unparsable state: {}", bad.join(", "));
            let mut v: [Option<Verdict>; 7] = Default::default();
            for k in [0, 1, 2, 6] {
                v[k] = Some(Err(why.clone()));
            }
            v
        }
    }
}

/// A declared stand-in cell: why it stands in, and its verdicts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StandIn {
    pub why: String,
    pub guard: Guard,
}

/// One entity of a bench or a record: its type (`minecraft:zombie`) and position.
#[derive(Clone, Debug, PartialEq)]
pub struct Ent {
    pub kind: String,
    pub pos: [f64; 3],
}

/// A bench's declarations: its stand-in cells, its candidate cells (the candidate's name) and the
/// entities it spawns itself (bench inputs: its carts and riders, with why).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Declared {
    pub standins: BTreeMap<Cell, StandIn>,
    pub candidates: BTreeMap<Cell, String>,
    pub bench_entities: Vec<(Ent, String)>,
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

/// One entity difference: a record entity the bench lacks (`missing`), or a bench entity the record lacks
/// (declared a bench input or not).
#[derive(Clone, Debug, PartialEq)]
pub struct EntDiff {
    pub ent: Ent,
    pub missing: bool,
    pub declared: Option<String>,
}

/// The compare's result.
#[derive(Clone, Debug)]
pub struct RecordMatch {
    pub record: String,
    pub lo: Cell,
    pub hi: Cell,
    /// Where the box came from (the bench's header, --box, the world's box; with the margin).
    pub box_source: String,
    /// The record's frame + `offset` = the bench's.
    pub offset: Cell,
    pub compared: usize,
    pub diffs: Vec<Diff>,
    /// Declared cells that don't differ (a stale declaration: printed, not a failure) or lie outside the box.
    pub stale: Vec<Cell>,
    pub entities_compared: usize,
    pub entity_diffs: Vec<EntDiff>,
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

    /// The entity differences no declaration covers.
    pub fn undeclared_entities(&self) -> Vec<&EntDiff> {
        self.entity_diffs.iter().filter(|d| d.declared.is_none()).collect()
    }

    /// The block bars' verdict.
    pub fn counts(&self) -> Counts {
        let (u, g) = (self.undeclared().len(), self.unguarded().len());
        if u > 0 || g > 0 {
            return Counts::No(format!("{u} undeclared cell(s), {g} unguarded stand-in(s)"));
        }
        let c = self.candidates();
        if c.is_empty() { Counts::Module } else { Counts::Candidates(c) }
    }

    /// The verdict for bars that involve mob behaviour (walk targets, sight, golem spots, panic): the block
    /// verdict, and 0 undeclared entity differences.
    pub fn counts_mob(&self) -> Counts {
        let ue = self.undeclared_entities().len();
        match self.counts() {
            Counts::No(why) if ue > 0 => Counts::No(format!("{why}, {ue} undeclared entity difference(s)")),
            Counts::No(why) => Counts::No(why),
            _ if ue > 0 => Counts::No(format!("{ue} undeclared entity difference(s)")),
            c => c,
        }
    }

    /// The line every part bench prints (ironspec 21:43 / 22:37).
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
        let ent = |e: &Ent| format!("{} at ({:.3},{:.3},{:.3})", short(&e.kind), e.pos[0], e.pos[1], e.pos[2]);
        let ue = self.undeclared_entities();
        let de: Vec<&EntDiff> = self.entity_diffs.iter().filter(|d| d.declared.is_some()).collect();
        let verdict = |c: Counts| match c {
            Counts::Module => "the module's".to_string(),
            Counts::Candidates(c) => format!("the candidate(s) {} only, not the module", c.join(", ")),
            Counts::No(why) => format!("NONE ({why})"),
        };
        format!(
            "cells against the design of record {} (its frame + ({},{},{})) over the box {}..={} ({}): {} compared, {} the same; {} stand-in(s){}; {} candidate cell(s){}; {} UNDECLARED{}{}; entities: {} compared, {} bench input(s) declared{}, {} UNDECLARED{} -> block bars: {}; bars involving mob behaviour: {}",
            self.record,
            self.offset[0],
            self.offset[1],
            self.offset[2],
            cell(&self.lo),
            cell(&self.hi),
            self.box_source,
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
            self.entities_compared,
            de.len(),
            if de.is_empty() { String::new() } else { format!(" ({})", de.iter().map(|d| ent(&d.ent)).collect::<Vec<_>>().join(", ")) },
            ue.len(),
            if ue.is_empty() { String::new() } else { format!(": {}", ue.iter().take(12).map(|d| format!("{} {}", if d.missing { "the bench lacks" } else { "the record lacks" }, ent(&d.ent))).collect::<Vec<_>>().join("; ")) },
            verdict(self.counts()),
            verdict(self.counts_mob())
        )
    }
}

/// The compare: `bench`'s cells (its frame; a cell it lacks is air) and entities against `record`'s, each
/// record cell and entity shifted by `offset` into the bench's frame, over lo..=hi (an entity by the cell
/// it stands in); `declared` classes each differing cell (a stand-in's guard: its declared verdicts, with
/// golem / path / late-path / sight from [`guard_intrinsics`] where it declared none) and covers the
/// bench's own entities. Entities match by type and position (within 1e-3).
#[allow(clippy::too_many_arguments)]
pub fn compare(bench: &BTreeMap<Cell, String>, bench_ents: &[Ent], record: &BTreeMap<Cell, String>, record_ents: &[Ent], offset: Cell, lo: Cell, hi: Cell, box_source: &str, declared: &Declared, record_name: &str) -> RecordMatch {
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
                    let (above, below) = (get(&rec, &[x, y + 1, z], true), get(&rec, &[x, y - 1, z], true));
                    for (k, v) in intrinsic(&b, &r, &above, &below).into_iter().enumerate() {
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
    // (entities: the record's shifted into the bench's frame, both inside the box, matched one to one)
    let in_box = |p: &[f64; 3]| inside(&[p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
    let canon_kind = |k: &str| if k.contains(':') { k.to_string() } else { format!("minecraft:{k}") };
    let close = |a: &Ent, b: &Ent| canon_kind(&a.kind) == canon_kind(&b.kind) && (0..3).all(|k| (a.pos[k] - b.pos[k]).abs() < 1e-3);
    let recs: Vec<Ent> = record_ents.iter().map(|e| Ent { kind: canon_kind(&e.kind), pos: [e.pos[0] + offset[0] as f64, e.pos[1] + offset[1] as f64, e.pos[2] + offset[2] as f64] }).filter(|e| in_box(&e.pos)).collect();
    let benches: Vec<Ent> = bench_ents.iter().map(|e| Ent { kind: canon_kind(&e.kind), pos: e.pos }).filter(|e| in_box(&e.pos)).collect();
    let mut used = vec![false; benches.len()];
    let mut entity_diffs = Vec::new();
    for r in &recs {
        match (0..benches.len()).find(|&k| !used[k] && close(r, &benches[k])) {
            Some(k) => used[k] = true,
            None => entity_diffs.push(EntDiff { ent: r.clone(), missing: true, declared: None }),
        }
    }
    for (k, b) in benches.iter().enumerate() {
        if !used[k] {
            let why = declared.bench_entities.iter().find(|(d, _)| close(d, b)).map(|(_, w)| w.clone());
            entity_diffs.push(EntDiff { ent: b.clone(), missing: false, declared: why });
        }
    }
    RecordMatch { record: record_name.to_string(), lo, hi, box_source: box_source.to_string(), offset, compared, diffs, stale, entities_compared: recs.len().max(benches.len()), entity_diffs }
}

/// [`compare`] over a bench's world: every cell of its box (`World::min`, `World::size`) widened by
/// `margin` (cells outside the world read as air), and its live entities.
pub fn compare_world(w: &redsim::world::World, margin: i32, record: &BTreeMap<Cell, String>, record_ents: &[Ent], offset: Cell, declared: &Declared, record_name: &str) -> RecordMatch {
    let lo = [w.min.x - margin, w.min.y - margin, w.min.z - margin];
    let hi = [w.min.x + w.size[0] - 1 + margin, w.min.y + w.size[1] - 1 + margin, w.min.z + w.size[2] - 1 + margin];
    let mut bench: BTreeMap<Cell, String> = BTreeMap::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = redsim::pos::BlockPos::new(x, y, z);
                if !w.contains(p) {
                    continue;
                }
                let s = w.get(p);
                if !s.is_air() {
                    bench.insert([x, y, z], s.to_string());
                }
            }
        }
    }
    let ents: Vec<Ent> = w.entities.list.iter().filter(|e| !e.removed).map(|e| Ent { kind: redsim::exact::entity_frame(w, e).ty, pos: [e.pos.x, e.pos.y, e.pos.z] }).collect();
    compare(&bench, &ents, record, record_ents, offset, lo, hi, &format!("the bench's world box + a {margin}-cell margin"), declared, record_name)
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
/// - `# candidate: NAME: x,y,z; …`;
/// - `# bench-entity: KIND x y z[ (why)]; …` (an entity the bench spawns itself: a cart, a rider).
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
        } else if let Some(rest) = l.strip_prefix("# bench-entity:") {
            for e in split_outside(rest, ';') {
                let (main, why) = match e.split_once('(') {
                    Some((m, w)) => (m.trim().to_string(), w.trim_end_matches(')').trim().to_string()),
                    None => (e.clone(), String::new()),
                };
                let w: Vec<&str> = main.split_whitespace().collect();
                if w.len() != 4 {
                    return Err(format!("bench-entity {e:?}: KIND x y z[ (why)]"));
                }
                let n = |s: &str| s.parse::<f64>().map_err(|x| format!("bench-entity {e:?}: {x}"));
                d.bench_entities.push((Ent { kind: w[0].to_string(), pos: [n(w[1])?, n(w[2])?, n(w[3])?] }, why));
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
        let cmp = |b: &BTreeMap<Cell, String>, d: &Declared| compare(b, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", d, "rec");
        let m = cmp(&record, &none);
        assert_eq!((m.compared, m.diffs.len(), m.counts(), m.counts_mob()), (24, 0, Counts::Module, Counts::Module), "{}", m.line());
        assert!(m.line().ends_with("-> block bars: the module's; bars involving mob behaviour: the module's"), "{}", m.line());
        // (1) the flat-D S-bend: the bench's D flat where the record's ascends
        let mut flat = record.clone();
        flat.insert([1, 1, 0], "minecraft:rail[shape=east_west,waterlogged=false]".into());
        let m1 = cmp(&flat, &none);
        assert_eq!(m1.undeclared().len(), 1, "{}", m1.line());
        assert!(matches!(m1.counts(), Counts::No(_)) && m1.line().contains("(1,1,0) bench rail[shape=east_west,waterlogged=false] vs record rail[shape=ascending_east"), "{}", m1.line());
        // (2) (C)'s inner-corner wall: glass where the record has air
        let mut wall = record.clone();
        wall.insert([2, 1, 1], "minecraft:glass".into());
        let m2 = cmp(&wall, &none);
        assert!(m2.undeclared().len() == 1 && m2.line().contains("(2,1,1) bench glass vs record air"), "{}", m2.line());
        // (3) R1's stand-in below the steps: glass where the record has smooth stone
        let mut r1 = record.clone();
        r1.insert([3, 0, 0], "minecraft:glass".into());
        let m3 = cmp(&r1, &none);
        assert!(m3.undeclared().len() == 1 && m3.line().contains("(3,0,0) bench glass vs record smooth_stone"), "{}", m3.line());
        // (the same cell declared a stand-in: its intrinsic verdicts filled (glass vs smooth stone: path fails,
        // glass not solid render), the owner's three unchecked: unguarded, the rows don't count)
        let mut st = Declared::default();
        st.standins.insert([3, 0, 0], StandIn { why: "the old supports".into(), guard: Guard::default() });
        let m4 = cmp(&r1, &st);
        assert_eq!((m4.undeclared().len(), m4.unguarded().len()), (0, 1), "{}", m4.line());
        assert!(m4.line().contains("UNGUARDED") && m4.line().contains("update reach unchecked") && m4.line().contains("path FAILS"), "{}", m4.line());
        // (a stand-in equal in every intrinsic and declared passing on the owner's three: guarded; the rows
        // count toward the module: smooth stone for a stone floor)
        let mut lev = record.clone();
        lev.insert([2, 0, 0], "minecraft:smooth_stone".into());
        let mut g = Guard::default();
        for p in ["update", "power", "survival"] {
            g.set(p, Ok("its bench's check".into())).unwrap();
        }
        let mut ok = Declared::default();
        ok.standins.insert([2, 0, 0], StandIn { why: "a floor's stand-in".into(), guard: g.clone() });
        let m5 = cmp(&lev, &ok);
        assert_eq!((m5.unguarded().len(), m5.counts()), (0, Counts::Module), "{}", m5.line());
        // (a candidate: counts for it, not the module)
        let mut c = Declared::default();
        c.candidates.insert([2, 1, 1], "wet-descent-2".into());
        let m6 = cmp(&wall, &c);
        assert_eq!(m6.counts(), Counts::Candidates(vec!["wet-descent-2".into()]), "{}", m6.line());
        // (a stale declaration printed, not failing; an offset frame)
        let m7 = cmp(&record, &c);
        assert!(m7.counts() == Counts::Module && m7.line().contains("(stale): (2,1,1)"), "{}", m7.line());
        let shifted: BTreeMap<Cell, String> = record.iter().map(|(k, v)| ([k[0] + 6, k[1], k[2] + 6], v.clone())).collect();
        let m8 = compare(&shifted, &[], &record, &[], [6, 0, 6], [6, 0, 6], [9, 2, 7], "the test's box", &none, "rec");
        assert_eq!(m8.counts(), Counts::Module, "{}", m8.line());
    }

    /// The golem property's two roles (ironspec 22:37), each with its positive control: a lever standing in
    /// for air over a solid floor FAILS the spawn-space role (no air over the floor, and a signal source);
    /// air standing in for a stone floor under air FAILS the floor role; a button, a rail or water for air
    /// over air each fail the spawn-space role through the valid-empty-block test (a signal source, a rail,
    /// a fluid); glass for air fails it through its box; smooth stone for stone passes.
    #[test]
    fn golem_roles() {
        let v = |s: &str, b: &str, above: &str, below: &str| intrinsic(s, b, above, below)[0].clone().unwrap();
        let lever = "minecraft:lever[face=floor,facing=north,powered=false]";
        let e = v(lever, "air", "air", "stone").unwrap_err();
        assert!(e.contains("feet false, a valid empty spawn block false"), "a lever for air over a solid floor fails the spawn-space role: {e}");
        assert!(v("air", "stone", "air", "stone").unwrap_err().contains("floor false"), "air for a stone floor fails the floor role");
        for s in ["minecraft:stone_button[face=floor,facing=north,powered=false]", "minecraft:rail[shape=north_south,waterlogged=false]", "minecraft:water[level=0]"] {
            let e = v(s, "air", "air", "air").unwrap_err();
            assert!(e.contains("a valid empty spawn block false"), "{s} for air: {e}");
        }
        assert!(v("glass", "air", "air", "air").unwrap_err().contains("its box clear false"), "glass for air: its box in a golem's cells");
        assert!(v("smooth_stone", "stone", "air", "stone").is_ok());
        // (nothing silently dropped (ironspec 23:13 (2b)): an unparsable state fails every intrinsic verdict,
        // so two unparsable cells never pass)
        let bad = intrinsic("minecraft:no_such_block[x=1]", "minecraft:no_such_block[x=1]", "air", "stone");
        for k in [0, 1, 2, 6] {
            assert!(bad[k].clone().unwrap().unwrap_err().contains("unparsable state: the stand-in minecraft:no_such_block[x=1], the built cell minecraft:no_such_block[x=1]"), "{:?}", bad[k]);
        }
        assert!(bad[3].is_none() && bad[4].is_none() && bad[5].is_none(), "the owner's three stay unchecked");
    }

    /// The path property is the full PathType classification (ironspec 22:37): one block per path type
    /// redsim's `from_state` gives, each standing in for every other: passes only against itself.
    #[test]
    fn path_types() {
        let blocks = [
            "minecraft:air",
            "minecraft:stone",
            "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]",
            "minecraft:powder_snow",
            "minecraft:cactus[age=0]",
            "minecraft:honey_block",
            "minecraft:lava[level=0]",
            "minecraft:magma_block",
            "minecraft:oak_door[facing=north,half=lower,hinge=left,open=true,powered=false]",
            "minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
            "minecraft:oak_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
            "minecraft:rail[shape=north_south,waterlogged=false]",
            "minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]",
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
            "minecraft:water[level=0]",
        ];
        let parsed: Vec<Block> = blocks.iter().map(|s| Block::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))).collect();
        let types: BTreeSet<String> = parsed.iter().map(|b| format!("{:?}", redsim::entity::mob::ai::path::path_type::from_state(b))).collect();
        assert_eq!(types.len(), blocks.len(), "one block per path type: {types:?}");
        for (i, a) in blocks.iter().enumerate() {
            for (j, b) in blocks.iter().enumerate() {
                let p = intrinsic(a, b, "air", "stone")[1].clone().unwrap();
                assert_eq!(p.is_ok(), i == j, "{a} for {b}: {p:?}");
            }
        }
    }

    /// Entities (ironspec 22:37 (b)): the record's persistent ones compared by type and position; a bench
    /// lacking one, adding an undeclared one or moving one is an undeclared difference that keeps its rows
    /// off the bars involving mob behaviour (the block bars stand); a declared bench input (its cart) is not.
    #[test]
    fn entities_compared() {
        let record = cells(&[([0, 0, 0], "minecraft:stone")]);
        let zombie = Ent { kind: "minecraft:zombie".into(), pos: [0.5, 1.0, 0.5] };
        let cart = Ent { kind: "minecraft:minecart".into(), pos: [1.5, 1.0, 0.5] };
        let (lo, hi) = ([0, 0, 0], [2, 2, 1]);
        let mut d = Declared::default();
        let m = compare(&record, &[zombie.clone()], &record, &[zombie.clone()], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert_eq!((m.entity_diffs.len(), m.counts_mob()), (0, Counts::Module));
        let lack = compare(&record, &[], &record, &[zombie.clone()], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert!(lack.counts() == Counts::Module && matches!(lack.counts_mob(), Counts::No(_)) && lack.line().contains("the bench lacks zombie at (0.500,1.000,0.500)"), "{}", lack.line());
        let moved = compare(&record, &[Ent { pos: [0.5, 1.0, 0.7], ..zombie.clone() }], &record, &[zombie.clone()], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert_eq!(moved.undeclared_entities().len(), 2, "{}", moved.line());
        let extra = compare(&record, &[zombie.clone(), cart.clone()], &record, &[zombie.clone()], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert!(matches!(extra.counts_mob(), Counts::No(_)), "{}", extra.line());
        d.bench_entities.push((cart.clone(), "the bench's own cart".into()));
        let declared = compare(&record, &[zombie.clone(), cart.clone()], &record, &[zombie.clone()], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert_eq!(declared.counts_mob(), Counts::Module, "{}", declared.line());
        assert!(declared.line().contains("1 bench input(s) declared (minecart at (1.500,1.000,0.500))"), "{}", declared.line());
    }

    /// The header lines: stand-ins (dfb89499's), their owner-checked verdicts, candidates, bench-input
    /// entities, the world box.
    #[test]
    fn reads_the_header() {
        let t = "# stand-in: 6,-13,-3 (the old step support, glass); 5,-14,-3\n# stand-in-guard: 6,-13,-3: update reach=ok (its six neighbours: the line's dusts), power=fail (powers the queue rail), survival=ok\n# candidate: wet-2: 8,-25,-4; 8,-25,-6\n# bench-entity: minecraft:minecart 1.5 0.0625 0.5 (the batch cart); minecraft:villager 1.5 0.4 0.5\n# world box: -10,-30,-12..20,10,12\nname: x\n";
        let d = declared_from_header(t).unwrap();
        assert_eq!(d.standins.len(), 2);
        let s = &d.standins[&[6, -13, -3]];
        assert_eq!(s.why, "the old step support, glass");
        assert_eq!(s.guard.0[3], Some(Ok("its six neighbours: the line's dusts".to_string())));
        assert_eq!(s.guard.0[4], Some(Err("powers the queue rail".to_string())));
        assert_eq!(s.guard.0[5], Some(Ok("declared by its bench".to_string())));
        assert_eq!(d.candidates.get(&[8, -25, -6]).map(String::as_str), Some("wet-2"));
        assert_eq!(d.bench_entities.len(), 2);
        assert_eq!(d.bench_entities[0], (Ent { kind: "minecraft:minecart".into(), pos: [1.5, 0.0625, 0.5] }, "the batch cart".to_string()));
        assert_eq!(world_box_from_header(t), Some(([-10, -30, -12], [20, 10, 12])));
        assert!(declared_from_header("# stand-in-guard: 1,2,3: colour=ok\n").is_err());
        assert!(declared_from_header("# bench-entity: minecraft:zombie 1 2\n").is_err());
        assert_eq!(canon("cave_air"), "minecraft:air");
        assert_eq!(canon("minecraft:rail[waterlogged=false,shape=east_west]"), canon("minecraft:rail[shape=east_west,waterlogged=false]"));
    }
}
