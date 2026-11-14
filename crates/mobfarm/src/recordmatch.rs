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
/// Update reach and power reach stay unchecked (None): they depend on the stand-in's neighbours and drive,
/// which only its bench knows. Survival needs the world: [`guard_survival`] (every static and bench runs it
/// on every stand-in, ironspec 01:07 (d); `settle::stand_in_survival` settles a copy of a bench's world and
/// runs it).
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

/// The SURVIVAL property (ironspec 01:07 (d)): a stand-in at `p` must survive where it stands in `w` (the
/// laid world: `behavior::can_survive`, and a rail that wouldn't pop at its first update) and stay as laid
/// through the settle (`settled`: the same world at its fixed point, entities held, as settle-check settles
/// it), unless `held` names the real driver and the controller phase its bench holds it in (a `# held:
/// x,y,z=DRIVER, PHASE` line).
pub fn guard_survival(w: &redsim::world::World, settled: &redsim::world::World, p: redsim::pos::BlockPos, held: Option<&str>) -> Verdict {
    let b = w.get(p);
    if !redsim::behavior::can_survive(w, &b, p) || redsim::rails::would_pop(w, p) {
        return Err(format!("{b} can't survive at {},{},{}", p.x, p.y, p.z));
    }
    let after = settled.get(p);
    if after != b {
        return match held {
            Some(h) => Ok(format!("survives; the settle changes it ({b} -> {after}), held: {h}")),
            None => Err(format!("the settle changes it: {b} -> {after} (no `# held:` line names its real driver and controller phase)")),
        };
    }
    Ok("survives, unchanged by the settle".into())
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

/// (ironspec Nov 14 08:41a (2), part 65) Each `# stand-in-guard:` verdict a bench declares, with the evidence its note
/// cites: a resolvable reference (an artifact a/<digits>, a test <fn name>, a recording <path> md5 <hex>), or why not.
/// Returns (line, resolvable) pairs; record-match fails the unresolved ones under `--require-evidence`.
pub fn guard_evidence(d: &Declared) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for (c, si) in &d.standins {
        for (p, v) in PROPERTIES.iter().zip(si.guard.0.iter()) {
            let Some(v) = v else { continue };
            let (verdict, note) = match v {
                Ok(n) => ("ok", n.as_str()),
                Err(n) => ("fail", n.as_str()),
            };
            match crate::fleecheck::evidence_ref(note) {
                Ok(kind) => out.push((format!("stand-in-guard ({},{},{}) {p}={verdict}: evidence {note:?} ({kind})", c[0], c[1], c[2]), true)),
                Err(why) => out.push((format!("stand-in-guard ({},{},{}) {p}={verdict}: evidence {why}", c[0], c[1], c[2]), false)),
            }
        }
    }
    out
}

/// One entity of a bench or a record: its type (`minecraft:zombie`), position, and the types of its
/// passengers (sorted; a passenger is compared as part of its vehicle, never as an entity of its own:
/// ironspec 03:29).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Ent {
    pub kind: String,
    pub pos: [f64; 3],
    pub passengers: Vec<String>,
}

/// An entity type, canonical: `minecraft:` prefixed, redsim's names mapped to the game's (a boat redsim
/// calls `boat` / `chest_boat` is the record's `oak_boat` / `oak_chest_boat`, ironspec 03:29's alias).
pub fn canon_kind(k: &str) -> String {
    let k = if k.contains(':') { k.to_string() } else { format!("minecraft:{k}") };
    match k.as_str() {
        "minecraft:boat" => "minecraft:oak_boat".to_string(),
        "minecraft:chest_boat" => "minecraft:oak_chest_boat".to_string(),
        _ => k,
    }
}

/// The passengers' types in an entity's NBT (`Passengers:[{id:"minecraft:villager",…},…]`), the direct
/// passengers only, canonical and sorted.
pub fn passengers_of_nbt(nbt: &str) -> Result<Vec<String>, String> {
    let Some(i) = nbt.find("Passengers:[") else { return Ok(Vec::new()) };
    let body = &nbt[i + "Passengers:[".len()..];
    // (the list's direct elements: depth 1 inside the list's brackets; a quoted string's braces and
    // brackets (a CustomName) skipped, its escapes honoured; unbalanced input refused, never guessed)
    let (mut depth, mut out, mut start, mut quote, mut escaped, mut closed) = (0i32, Vec::new(), None, None::<char>, false, false);
    let mut elements: Vec<(usize, usize)> = Vec::new();
    for (j, ch) in body.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '[' if depth > 0 || ch == '{' => {
                if depth == 0 {
                    start = Some(j);
                }
                depth += 1;
            }
            '}' | ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if ch != '}' {
                        return Err(format!("Passengers: an element closes with `{ch}`: {nbt}"));
                    }
                    if let Some(s) = start.take() {
                        elements.push((s, j));
                    }
                }
            }
            ']' => {
                closed = true;
                break;
            }
            '}' => return Err(format!("Passengers: an unbalanced `}}`: {nbt}")),
            _ => {}
        }
    }
    if quote.is_some() || depth != 0 || !closed {
        return Err(format!("Passengers: unbalanced braces, brackets or quotes: {nbt}"));
    }
    for (s, e) in elements {
        let el = &body[s..=e];
        // (its own id: the first `id:` at its top level, outside quotes)
        let (mut d, mut q, mut esc, mut id) = (0i32, None::<char>, false, None);
        for (k, c) in el.char_indices() {
            if let Some(qc) = q {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == qc {
                    q = None;
                }
                continue;
            }
            match c {
                '"' | '\'' => q = Some(c),
                '{' | '[' => d += 1,
                '}' | ']' => d -= 1,
                _ => {}
            }
            if d == 1 && el[k..].starts_with("id:") && matches!(el[..k].chars().last(), Some('{') | Some(',') | Some(' ')) {
                let v = el[k + 3..].trim_start().trim_start_matches('"');
                id = Some(v.split(|c: char| c == '"' || c == ',' || c == '}').next().unwrap_or("").to_string());
                break;
            }
        }
        match id {
            Some(id) if !id.is_empty() => out.push(canon_kind(&id)),
            _ => return Err(format!("Passengers: an element without its id: {el}")),
        }
    }
    out.sort();
    Ok(out)
}

/// A bench's declarations: its stand-in cells, its candidate cells (the candidate's name) and the
/// entities it spawns itself (bench inputs: its carts and riders, with why).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Declared {
    pub standins: BTreeMap<Cell, StandIn>,
    pub candidates: BTreeMap<Cell, String>,
    pub bench_entities: Vec<(Ent, String)>,
    /// `# out-of-scope: CELLS=REASON (ruling)` (cc767b78 for 7b90c3f1's `--record` rows): cells a ruling puts
    /// out of the compare's scope, each with its reason. They stand only where the record's state is one
    /// redsim can't model (a nether portal): declared on a modelled cell they're refused.
    pub out_of_scope: BTreeMap<Cell, String>,
    /// `# bench-removed: KIND x y z (why)` (ironspec 03:29): a record entity the bench's harness removes on
    /// purpose (58f770cc's lanes' riding villagers, so they don't claim the stop's bed): a record entity of
    /// KIND at x y z, or a passenger of KIND riding the record's vehicle at x y z, taken out of the compare
    /// and printed as declared.
    pub removed: Vec<(String, [f64; 3], String)>,
}

/// How a differing cell is classed.
#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    /// A declared stand-in; `guarded`: its seven verdicts all pass.
    StandIn { guarded: bool },
    /// A declared candidate's cell.
    Candidate(String),
    /// Declared out of scope, the record's state one redsim can't model: printed with its reason, counted
    /// apart, not a failure.
    OutOfScope(String),
    /// Declared out of scope on a cell whose record state redsim models: refused (nothing modelled hides
    /// behind the declaration).
    OutOfScopeRefused(String),
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
        let refused = self.diffs.iter().filter(|d| matches!(d.class, Class::OutOfScopeRefused(_))).count();
        if u > 0 || g > 0 || refused > 0 || !self.stale.is_empty() {
            let mut why = format!("{u} undeclared cell(s), {g} unguarded stand-in(s)");
            if refused > 0 {
                why.push_str(&format!(", {refused} out-of-scope declared on a modelled cell"));
            }
            if !self.stale.is_empty() {
                why.push_str(&format!(", {} stale declaration(s) (a declared cell that doesn't differ or lies outside the box: a no-op declaration fails)", self.stale.len()));
            }
            return Counts::No(why);
        }
        let c = self.candidates();
        if c.is_empty() { Counts::Module } else { Counts::Candidates(c) }
    }

    /// The cells declared out of scope (record states redsim can't model), with their reasons.
    pub fn out_of_scope(&self) -> Vec<&Diff> {
        self.diffs.iter().filter(|d| matches!(d.class, Class::OutOfScope(_))).collect()
    }

    /// The verdict for bars that involve mob behaviour (walk targets, sight, golem spots, panic): the block
    /// verdict, 0 undeclared entity differences, and no stale `# bench-removed:` (a removal matching nothing
    /// is a no-op declaration: it fails).
    pub fn counts_mob(&self) -> Counts {
        let ue = self.undeclared_entities().len();
        let stale = self.entity_diffs.iter().filter(|d| d.declared.as_deref().is_some_and(|w| w.starts_with("STALE"))).count();
        let mut extra = Vec::new();
        if ue > 0 {
            extra.push(format!("{ue} undeclared entity difference(s)"));
        }
        if stale > 0 {
            extra.push(format!("{stale} stale `# bench-removed:` (no record entity there: a no-op declaration fails)"));
        }
        match self.counts() {
            Counts::No(why) if extra.is_empty() => Counts::No(why),
            Counts::No(why) => Counts::No(format!("{why}, {}", extra.join(", "))),
            _ if !extra.is_empty() => Counts::No(extra.join(", ")),
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
        let ent = |e: &Ent| {
            format!(
                "{} at ({:.3},{:.3},{:.3}){}",
                short(&e.kind),
                e.pos[0],
                e.pos[1],
                e.pos[2],
                if e.passengers.is_empty() { String::new() } else { format!(" carrying {}", e.passengers.iter().map(|p| short(p)).collect::<Vec<_>>().join(", ")) }
            )
        };
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
            if und.is_empty() { String::new() } else { format!(": {}", first(&und, &|d: &Diff| format!("{} bench {} vs record {}{}", cell(&d.cell), short(&d.bench), short(&d.record), if Block::parse(&d.record).is_err() { " (unmodelled: redsim can't parse it)" } else { "" }))) },
            {
                let oos = self.out_of_scope();
                let refused: Vec<&Diff> = self.diffs.iter().filter(|d| matches!(d.class, Class::OutOfScopeRefused(_))).collect();
                let reason = |d: &Diff| match &d.class {
                    Class::OutOfScope(w) | Class::OutOfScopeRefused(w) => w.clone(),
                    _ => String::new(),
                };
                let mut s = if self.stale.is_empty() { String::new() } else { format!("; declared but the same or outside the box (STALE, failing): {}", self.stale.iter().map(cell).collect::<Vec<_>>().join(", ")) };
                if !oos.is_empty() {
                    s.push_str(&format!("; {} out of scope (record states redsim can't model, declared): {}", oos.len(), first(&oos, &|d: &Diff| format!("{} record {} / bench {} ({})", cell(&d.cell), short(&d.record), short(&d.bench), reason(d)))));
                }
                if !refused.is_empty() {
                    s.push_str(&format!("; {} out-of-scope declarations REFUSED (a modelled cell): {}", refused.len(), first(&refused, &|d: &Diff| format!("{} bench {} vs record {} ({})", cell(&d.cell), short(&d.bench), short(&d.record), reason(d)))));
                }
                s
            },
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
                let (class, gaps) = if let Some(why) = declared.out_of_scope.get(&c) {
                    // (out of scope only where redsim can't model the record's state)
                    if Block::parse(&r).is_err() { (Class::OutOfScope(why.clone()), Vec::new()) } else { (Class::OutOfScopeRefused(why.clone()), Vec::new()) }
                } else if let Some(si) = declared.standins.get(&c) {
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
    // (a declaration on a cell that doesn't differ, or outside the box, is a no-op: it fails the bench's rows
    // (ironspec 05:13 real (2b), the 04:07 override rule's spirit): stand-ins, candidates and out-of-scope
    // cells alike)
    let stale: Vec<Cell> = declared.standins.keys().chain(declared.candidates.keys()).chain(declared.out_of_scope.keys()).filter(|c| !differs.contains(*c)).copied().collect::<BTreeSet<_>>().into_iter().collect();
    // (entities: the record's shifted into the bench's frame, both inside the box, matched one to one by type,
    // position (within 1e-3) and passengers; the harness's declared removals taken out of the record first)
    let in_box = |p: &[f64; 3]| inside(&[p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
    let near = |a: &[f64; 3], b: &[f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
    let close = |a: &Ent, b: &Ent| canon_kind(&a.kind) == canon_kind(&b.kind) && near(&a.pos, &b.pos) && a.passengers == b.passengers;
    let canon_ent = |e: &Ent, d: [f64; 3]| -> Ent {
        let mut p: Vec<String> = e.passengers.iter().map(|k| canon_kind(k)).collect();
        p.sort();
        Ent { kind: canon_kind(&e.kind), pos: [e.pos[0] + d[0], e.pos[1] + d[1], e.pos[2] + d[2]], passengers: p }
    };
    let mut recs: Vec<Ent> = record_ents.iter().map(|e| canon_ent(e, [offset[0] as f64, offset[1] as f64, offset[2] as f64])).filter(|e| in_box(&e.pos)).collect();
    let benches: Vec<Ent> = bench_ents.iter().map(|e| canon_ent(e, [0.0; 3])).filter(|e| in_box(&e.pos)).collect();
    let mut entity_diffs = Vec::new();
    for (kind, pos, why) in &declared.removed {
        let kind = canon_kind(kind);
        let note = Some(format!("removed by the harness: {why}"));
        if let Some(k) = recs.iter().position(|r| r.kind == kind && near(&r.pos, pos)) {
            let r = recs.remove(k);
            entity_diffs.push(EntDiff { ent: r, missing: true, declared: note });
        } else if let Some(r) = recs.iter_mut().find(|r| near(&r.pos, pos) && r.passengers.contains(&kind)) {
            let at = r.passengers.iter().position(|p| *p == kind).expect("found");
            r.passengers.remove(at);
            entity_diffs.push(EntDiff { ent: Ent { kind: kind.clone(), pos: *pos, passengers: Vec::new() }, missing: true, declared: Some(format!("removed by the harness, a passenger of the {} there: {why}", r.kind)) });
        } else {
            entity_diffs.push(EntDiff { ent: Ent { kind: kind.clone(), pos: *pos, passengers: Vec::new() }, missing: false, declared: Some(format!("STALE `# bench-removed:` (no such record entity or passenger): {why}")) });
        }
    }
    let mut used = vec![false; benches.len()];
    for r in &recs {
        match (0..benches.len()).find(|&k| !used[k] && close(r, &benches[k])) {
            Some(k) => used[k] = true,
            None => entity_diffs.push(EntDiff { ent: r.clone(), missing: true, declared: None }),
        }
    }
    for (k, b) in benches.iter().enumerate() {
        if !used[k] {
            let why = declared.bench_entities.iter().find(|(d, _)| close(&canon_ent(d, [0.0; 3]), b)).map(|(_, w)| w.clone());
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
    // (the live entities with no vehicle, each with its passengers' types (a passenger is compared as part
    // of its vehicle); a boat by its own item, the game's entity type: `oak_boat`, not redsim's `boat`)
    let kind_of = |e: &redsim::entity::Entity| -> String {
        match &e.kind {
            redsim::entity::EntityKind::Boat(b) => canon_kind(b.item),
            _ => canon_kind(&redsim::exact::entity_frame(w, e).ty),
        }
    };
    let ents: Vec<Ent> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed && e.vehicle.is_none())
        .map(|e| {
            let mut p: Vec<String> = e.passengers.iter().filter_map(|id| redsim::entity::index_of(w, *id)).map(|i| kind_of(&w.entities.list[i])).collect();
            p.sort();
            Ent { kind: kind_of(e), pos: [e.pos.x, e.pos.y, e.pos.z], passengers: p }
        })
        .collect();
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
    // (the shared near-miss check over every tool's declaration keys, ironspec 06:21 (c))
    crate::declkeys::check(text)?;
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
        } else if let Some(rest) = l.strip_prefix("# out-of-scope:") {
            // (`CELLS=REASON (ruling)`: CELLS `x,y,z` entries joined by `+`, each coordinate a value or a
            // range `a..b`)
            for e in split_outside(rest, ';') {
                let (cells, why) = e.split_once('=').ok_or(format!("out-of-scope {e:?}: CELLS=REASON (ruling)"))?;
                let why = why.trim().to_string();
                if why.is_empty() {
                    return Err(format!("out-of-scope {e:?}: a declaration names its reason and ruling"));
                }
                for entry in cells.split('+').map(str::trim).filter(|s| !s.is_empty()) {
                    let parts: Vec<&str> = entry.split(',').map(str::trim).collect();
                    if parts.len() != 3 {
                        return Err(format!("out-of-scope {entry:?}: x,y,z (each a value or a..b)"));
                    }
                    let range = |s: &str| -> Result<(i32, i32), String> {
                        let num = |t: &str| t.trim().replace('\u{2212}', "-").parse::<i32>().map_err(|x| format!("out-of-scope {entry:?}: {x}"));
                        match s.split_once("..") {
                            Some((a, b)) => Ok((num(a)?, num(b)?)),
                            None => Ok((num(s)?, num(s)?)),
                        }
                    };
                    let (rx, ry, rz) = (range(parts[0])?, range(parts[1])?, range(parts[2])?);
                    for x in rx.0.min(rx.1)..=rx.0.max(rx.1) {
                        for y in ry.0.min(ry.1)..=ry.0.max(ry.1) {
                            for z in rz.0.min(rz.1)..=rz.0.max(rz.1) {
                                d.out_of_scope.insert([x, y, z], why.clone());
                            }
                        }
                    }
                }
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
                d.bench_entities.push((Ent { kind: w[0].to_string(), pos: [n(w[1])?, n(w[2])?, n(w[3])?], passengers: Vec::new() }, why));
            }
        } else if let Some(rest) = l.strip_prefix("# bench-removed:") {
            for e in split_outside(rest, ';') {
                let (main, why) = match e.split_once('(') {
                    Some((m, w)) => (m.trim().to_string(), w.trim_end_matches(')').trim().to_string()),
                    None => (e.clone(), String::new()),
                };
                let w: Vec<&str> = main.split_whitespace().collect();
                if w.len() != 4 || why.is_empty() {
                    return Err(format!("bench-removed {e:?}: KIND x y z (why)"));
                }
                let n = |s: &str| s.parse::<f64>().map_err(|x| format!("bench-removed {e:?}: {x}"));
                d.removed.push((w[0].to_string(), [n(w[1])?, n(w[2])?, n(w[3])?], why));
            }
        }
    }
    // (one declaration per cell, ironspec 05:13 real (2c): out of scope and a stand-in or candidate at once is
    // refused, since out of scope would skip the stand-in's guard)
    let both: Vec<String> = d.out_of_scope.keys().filter(|c| d.standins.contains_key(*c) || d.candidates.contains_key(*c)).map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect();
    if !both.is_empty() {
        return Err(format!("cells declared both out of scope and as a stand-in or candidate (one declaration per cell): {}", both.join(", ")));
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
        // (a stale declaration printed AND failing (ironspec 05:13 real (2b)): a candidate on a cell that doesn't
        // differ; a stale stand-in and a stale out-of-scope cell likewise; an offset frame)
        let m7 = cmp(&record, &c);
        assert!(matches!(m7.counts(), Counts::No(ref w) if w.contains("1 stale declaration")) && matches!(m7.counts_mob(), Counts::No(_)) && m7.line().contains("(STALE, failing): (2,1,1)"), "{}", m7.line());
        let mut st2 = Declared::default();
        st2.standins.insert([0, 0, 0], StandIn { why: "a floor's stand-in".into(), guard: g.clone() });
        assert!(matches!(cmp(&record, &st2).counts(), Counts::No(_)), "a stale stand-in fails");
        let mut oos = Declared::default();
        oos.out_of_scope.insert([0, 1, 0], "nothing differs here".into());
        let m7o = cmp(&record, &oos);
        assert!(matches!(m7o.counts(), Counts::No(_)) && m7o.line().contains("(STALE, failing): (0,1,0)"), "a stale out-of-scope cell fails: {}", m7o.line());
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
        let zombie = Ent { kind: "minecraft:zombie".into(), pos: [0.5, 1.0, 0.5], passengers: Vec::new() };
        let cart = Ent { kind: "minecraft:minecart".into(), pos: [1.5, 1.0, 0.5], passengers: Vec::new() };
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

    /// ironspec 03:29, the entity gap 58f770cc hit on record7e: redsim's `boat` is the record's `oak_boat`
    /// (the alias, and compare_world reads a boat's own item); a passenger is compared as part of its
    /// vehicle (the record's cart carrying a villager matches the bench's cart with its villager riding, and
    /// doesn't match an empty cart); `# bench-removed:` takes a record entity or a passenger the harness
    /// removes out of the compare, printed as declared; a removal matching nothing prints STALE.
    #[test]
    fn entity_alias_passengers_removed() {
        let record = cells(&[([0, 0, 0], "minecraft:stone")]);
        let (lo, hi) = ([0, 0, 0], [3, 2, 1]);
        let none = Declared::default();
        let boat_rec = Ent { kind: "minecraft:oak_boat".into(), pos: [2.5, 1.0, 0.5], passengers: vec!["minecraft:zombie".into()] };
        let boat_bench = Ent { kind: "boat".into(), pos: [2.5, 1.0, 0.5], passengers: vec!["zombie".into()] };
        let m = compare(&record, &[boat_bench], &record, &[boat_rec.clone()], [0, 0, 0], lo, hi, "the test's box", &none, "rec");
        assert_eq!((m.entity_diffs.len(), m.counts_mob()), (0, Counts::Module), "{}", m.line());
        let cart_rec = Ent { kind: "minecraft:minecart".into(), pos: [1.5, 1.0, 0.5], passengers: passengers_of_nbt("{Passengers:[{id:\"minecraft:villager\",VillagerData:{level:1}}],Motion:[0.0d,0.0d,0.0d]}").unwrap() };
        assert_eq!(cart_rec.passengers, vec!["minecraft:villager".to_string()]);
        // (a quoted name's braces don't nest; unbalanced input is refused, not guessed)
        assert_eq!(passengers_of_nbt("{Passengers:[{CustomName:'{\"text\":\"a}b{\"}',id:\"minecraft:zombie\",Uuid:[I;1,2,3,4]}]}").unwrap(), vec!["minecraft:zombie".to_string()]);
        assert!(passengers_of_nbt("{Passengers:[{id:\"minecraft:zombie\"}").is_err() && passengers_of_nbt("{Passengers:[{CustomName:\"open,id:\"minecraft:zombie\"}]}").is_err(), "unbalanced refused");
        let cart_full = Ent { kind: "minecraft:minecart".into(), pos: [1.5, 1.0, 0.5], passengers: vec!["minecraft:villager".into()] };
        let cart_empty = Ent { passengers: Vec::new(), ..cart_full.clone() };
        let m = compare(&record, &[cart_full], &record, &[cart_rec.clone()], [0, 0, 0], lo, hi, "the test's box", &none, "rec");
        assert_eq!(m.counts_mob(), Counts::Module, "{}", m.line());
        let m = compare(&record, &[cart_empty.clone()], &record, &[cart_rec.clone()], [0, 0, 0], lo, hi, "the test's box", &none, "rec");
        assert!(matches!(m.counts_mob(), Counts::No(_)) && m.line().contains("carrying villager"), "{}", m.line());
        let d = declared_from_header("# bench-removed: minecraft:villager 1.5 1.0 0.5 (58f770cc's harness: the lanes' riders off, so they don't claim the stop's bed)\n").unwrap();
        let m = compare(&record, &[cart_empty], &record, &[cart_rec], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert_eq!(m.counts_mob(), Counts::Module, "{}", m.line());
        assert!(m.entity_diffs.iter().any(|e| e.declared.as_deref().is_some_and(|w| w.contains("removed by the harness, a passenger of the minecraft:minecart"))), "{:?}", m.entity_diffs);
        let stale = declared_from_header("# bench-removed: minecraft:villager 9.5 1.0 0.5 (nothing there)\n").unwrap();
        let m = compare(&record, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", &stale, "rec");
        assert!(m.entity_diffs.iter().any(|e| e.declared.as_deref().is_some_and(|w| w.starts_with("STALE"))), "{:?}", m.entity_diffs);
        assert!(matches!(m.counts_mob(), Counts::No(ref w) if w.contains("stale `# bench-removed:`")) && m.counts() == Counts::Module, "a stale removal fails the mob bars: {:?}", m.counts_mob());
        assert!(declared_from_header("# bench-removed: minecraft:villager 1.5 1.0 0.5\n").is_err(), "a removal gives its reason");
    }

    /// `# out-of-scope:` (cc767b78 for 7b90c3f1's `--record` rows, ironspec 21:58's Nether cells): a record
    /// cell redsim can't model (nether_portal) the bench lacks: undeclared it's UNDECLARED, printed
    /// "unmodelled"; declared out of scope it's printed with its reason and counts nothing against the bars;
    /// the same declaration on a cell redsim models is refused (counts NONE).
    #[test]
    fn out_of_scope_only_unmodelled() {
        let record = cells(&[([0, 0, 0], "minecraft:stone"), ([1, 0, 0], "minecraft:nether_portal[axis=x]"), ([2, 0, 0], "minecraft:stone")]);
        let bench = cells(&[([0, 0, 0], "minecraft:stone")]);
        let (lo, hi) = ([0, 0, 0], [2, 1, 0]);
        let m = compare(&bench, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", &Declared::default(), "rec");
        assert!(m.undeclared().len() == 2 && m.line().contains("(1,0,0) bench air vs record nether_portal[axis=x] (unmodelled"), "{}", m.line());
        let d = declared_from_header("# out-of-scope: 1..1,0,0=the Nether pad (ironspec 21:58: Nether cells out of scope)\n").unwrap();
        assert_eq!(d.out_of_scope.len(), 1);
        let m = compare(&bench, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert!(m.out_of_scope().len() == 1 && m.undeclared().len() == 1 && m.line().contains("1 out of scope (record states redsim can't model, declared): (1,0,0) record nether_portal[axis=x] / bench air (the Nether pad"), "{}", m.line());
        let both = declared_from_header("# out-of-scope: 1..2,0,0=the Nether pad (ironspec 21:58)\n").unwrap();
        let m = compare(&bench, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", &both, "rec");
        assert!(matches!(m.counts(), Counts::No(ref w) if w.contains("out-of-scope declared on a modelled cell")) && m.line().contains("REFUSED"), "{}", m.line());
        let full = cells(&[([0, 0, 0], "minecraft:stone"), ([2, 0, 0], "minecraft:stone")]);
        let m = compare(&full, &[], &record, &[], [0, 0, 0], lo, hi, "the test's box", &d, "rec");
        assert_eq!(m.counts(), Counts::Module, "only the declared unmodelled cell differs: {}", m.line());
        assert!(declared_from_header("# out-of-scope: 1,0,0\n").is_err() && declared_from_header("# out-of-scope: 1,0=x\n").is_err());
    }

    /// ironspec 05:13 real (2c), G1 on 153148cb1: one declaration per cell. Out of scope and a stand-in, or out
    /// of scope and a candidate, on one cell is refused for that reason, the cell named (out of scope would
    /// skip the stand-in's guard); the same declarations on different cells read.
    #[test]
    fn one_declaration_per_cell() {
        let why = |r: Result<Declared, String>| r.err().unwrap_or_default();
        let both = "cells declared both out of scope and as a stand-in or candidate (one declaration per cell)";
        let e = why(declared_from_header("# out-of-scope: 1,0,0=the Nether pad (r)\n# stand-in: 1,0,0 (a stand-in)\n"));
        assert!(e.contains(both) && e.contains("(1,0,0)"), "{e}");
        let e = why(declared_from_header("# out-of-scope: 1..2,0,0=the Nether pad (r)\n# candidate: c1: 2,0,0\n"));
        assert!(e.contains(both) && e.contains("(2,0,0)") && !e.contains("(1,0,0)"), "{e}");
        let d = declared_from_header("# out-of-scope: 1,0,0=the Nether pad (r)\n# stand-in: 3,0,0 (a stand-in)\n# candidate: c1: 4,0,0\n").unwrap();
        assert!(d.out_of_scope.len() == 1 && d.standins.len() == 1 && d.candidates.len() == 1);
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
        assert_eq!(d.bench_entities[0], (Ent { kind: "minecraft:minecart".into(), pos: [1.5, 0.0625, 0.5], passengers: Vec::new() }, "the batch cart".to_string()));
        assert_eq!(world_box_from_header(t), Some(([-10, -30, -12], [20, 10, 12])));
        assert!(declared_from_header("# stand-in-guard: 1,2,3: colour=ok\n").is_err());
        // (ironspec Nov 14 08:41a (2), part 65) each declared guard verdict's evidence read back: a note citing an
        // artifact or a recording with its md5 resolves; prose and the bare "declared by its bench" don't
        let ev = guard_evidence(&declared_from_header("# stand-in: 1,2,3 (glass)\n# stand-in-guard: 1,2,3: update reach=ok (a/1458936443005840), power reach=ok (glass carries no power), survival=ok (recording designs/x/run.trace md5 0123456789abcdef), sight=ok\n").unwrap());
        assert_eq!(
            ev,
            vec![
                ("stand-in-guard (1,2,3) update reach=ok: evidence \"a/1458936443005840\" (an artifact)".to_string(), true),
                ("stand-in-guard (1,2,3) power reach=ok: evidence NOT RESOLVABLE \"glass carries no power\" (name an artifact a/<digits>, a test <fn name>, or a recording <path> md5 <hex>)".to_string(), false),
                ("stand-in-guard (1,2,3) survival=ok: evidence \"recording designs/x/run.trace md5 0123456789abcdef\" (a recording with its md5)".to_string(), true),
                ("stand-in-guard (1,2,3) sight=ok: evidence NOT RESOLVABLE \"declared by its bench\" (name an artifact a/<digits>, a test <fn name>, or a recording <path> md5 <hex>)".to_string(), false),
            ]
        );
        assert!(declared_from_header("# bench-entity: minecraft:zombie 1 2\n").is_err());
        assert_eq!(canon("cave_air"), "minecraft:air");
        assert_eq!(canon("minecraft:rail[waterlogged=false,shape=east_west]"), canon("minecraft:rail[shape=east_west,waterlogged=false]"));
    }
}
