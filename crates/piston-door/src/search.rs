//! Local search for fewer blocks: `mctas door minimize --moves ...`.
//!
//! The only oracle is the judge, [`verify_in`]; moves are plain design edits (remove blocks,
//! set blocks), so this works for any contraption the verifier accepts. Move families
//! (`--moves`, a comma list or `all`):
//! * `singles`, `pairs`, `triples`: remove 1, 2 or 3 blocks (triples are local: at least two of
//!   the three distances are within `radius`);
//! * `delays`: another delay on a repeater (neutral);
//! * `moves`: a block moves to a free face-neighbor cell (neutral);
//! * `subs`: a block becomes another component: dust <-> repeater, torch <-> observer, other
//!   facings, glass floors (neutral);
//! * `reroute`: a dust segment between two anchors (branch points, ends, cells touching
//!   components) is ripped up with its floors and re-laid by the router, from anywhere on one
//!   side of the net to the other end (improving or neutral).
//!
//! Neutral edits are also tried together with one removal within `radius` (an improving
//! compound). When nothing improves (`plateau`), every neutral edit that passes the judge is
//! followed by every single (and pair) removal; then, with `walk`, the search moves to a random
//! verified neutral design and starts over (at most `walk` such steps without improving).
//!
//! Every candidate is first screened at a few judge positions with fewer cycles (`screen`);
//! survivors get the full judge, the best one is applied, and the other survivors are re-tried
//! on top of it.

use crate::minimize::Sandboxes;
use crate::route::Router;
use crate::{DoorSpec, VerifyOptions, block_count, verify_in};
use anyhow::{Context, Result, anyhow};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{Pool, Pos};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::time::Instant;

/// The judge's positions: the default four plus `extra` pseudo-random ones (the same sequence
/// as `door verify --extra-offsets`).
pub fn judge_offsets(extra: u32) -> Vec<Pos> {
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra));
    offsets
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Singles,
    Pairs,
    Triples,
    Delays,
    Moves,
    Subs,
    Reroute,
    Renet,
    Swap,
}

const FAMILIES: [(Family, &str); 9] = [
    (Family::Singles, "singles"),
    (Family::Pairs, "pairs"),
    (Family::Triples, "triples"),
    (Family::Delays, "delays"),
    (Family::Moves, "moves"),
    (Family::Subs, "subs"),
    (Family::Reroute, "reroute"),
    (Family::Renet, "renet"),
    (Family::Swap, "swap"),
];

/// `all` or a comma list of family names.
pub fn parse_families(s: &str) -> Result<Vec<Family>> {
    if s.trim() == "all" {
        return Ok(FAMILIES.iter().map(|(f, _)| *f).collect());
    }
    s.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| {
            FAMILIES.iter().find(|(_, n)| *n == x).map(|(f, _)| *f).ok_or_else(|| {
                anyhow!("unknown move family {x:?}; have {} or all", FAMILIES.iter().map(|(_, n)| *n).collect::<Vec<_>>().join(","))
            })
        })
        .collect()
}

#[derive(Clone)]
pub struct SearchOptions {
    pub families: Vec<Family>,
    /// Cheap first pass (a few positions, fewer cycles).
    pub screen: VerifyOptions,
    /// Acceptance: the judge.
    pub judge: VerifyOptions,
    /// Neighborhood radius (Chebyshev) of compounds and triples.
    pub radius: i32,
    /// When nothing improves, explore verified neutral edits one level deep.
    pub plateau: bool,
    /// ... followed by pair removals too (singles only otherwise).
    pub plateau_pairs: bool,
    /// Plateau depth: 2 also tries a second neutral edit after each verified first one.
    pub depth: usize,
    /// Random-walk steps over verified neutral designs allowed without improving.
    pub walk: usize,
    pub deadline: Option<Instant>,
    pub seed: u64,
    /// What to minimize (blocks by default).
    pub objective: Objective,
    /// Speed objectives: allow up to this many blocks more than the start design.
    pub slack: usize,
    /// Screen with the judge's own code (per-tick frames) instead of the frame-less copy.
    pub screen_frames: bool,
    /// Speed constraints (speed boards): reject candidates that open (close) later than this
    /// many ticks in any recorded cycle, as the leaderboard measures them.
    pub max_open: Option<u32>,
    pub max_close: Option<u32>,
    /// Also accept candidates that turn a solid design into a sculk one (they add a sculk
    /// sensor): searches for the sculk board starting from solid designs.
    pub to_sculk: bool,
    /// Shape boards: reject candidates outside these shape limits (`--shape wall`, `ground`,
    /// `side<=1`...; see [`crate::ShapeLimits`]).
    pub shape: Option<crate::ShapeLimits>,
    /// Accept only rotation-safe designs: an accepted candidate must also pass the judge turned
    /// 90, 180 and 270 degrees ([`crate::rotation_check`], at the judge's positions). The boards
    /// rank rotation-safe designs, so a search from one should not trade that away for a block.
    pub rotation_safe: bool,
    /// Accept only designs at least this seamless (`--seamless semi|full|super`): one recorded
    /// cycle at the design's own position, judged by `door properties`' definition
    /// ([`crate::properties::seamless`]); checked before the judge (it is one cycle).
    pub seamless: Option<crate::properties::SeamReq>,
    /// Also accept candidates that turn a solid design into an entity one (they add entities;
    /// the door stays otherwise solid): searches for the "Entities allowed" board.
    pub to_entity: bool,
    /// Accept only rotation-safe candidates ([`crate::rotation_check`] at the judge's positions)
    /// when the start design is rotation-safe: the boards rank rotation-safe doors, so an
    /// improvement that breaks a turn lands on the other board. Checked after the full judge.
    pub keep_rotations: bool,
    /// Record eligibility (Squid regulation 3.1.3, MC-172213): accept only designs that also work
    /// with REPEATER input, the lever left off and its support block powered by a delay-1 repeater
    /// (props-dyn's check, `properties_dyn::repeater_check`: both edges) at every judge position.
    /// Screened at the first position, checked everywhere after the full judge.
    pub repeater_input: bool,
    /// `--eligible`: `repeater_input`, plus props-dyn's hallway check, footsteps for sculk designs
    /// and props' QUART SEAMLESS or better (`properties::seamless_tier`).
    pub eligible: bool,
}

/// The `--repeater-input` / `--eligible` filter at `offsets` (see [`SearchOptions::repeater_input`]).
pub fn eligibility_check(w: &mut mctas::sim::Worker, sb: u32, d: &Design, opts: &SearchOptions, offsets: &[Pos]) -> Result<()> {
    eligibility(w, sb, d, opts.repeater_input, opts.eligible, offsets)
}

/// The eligibility filter with explicit switches (resynth's `Target::Door` uses it too).
pub fn eligibility(w: &mut mctas::sim::Worker, sb: u32, d: &Design, repeater_input: bool, eligible: bool, offsets: &[Pos]) -> Result<()> {
    if !repeater_input && !eligible {
        return Ok(());
    }
    crate::properties_dyn::eligibility_in(w, sb, d, offsets, 1, eligible, eligible && crate::category(d) == "sculk")?;
    if eligible {
        let p = crate::properties::input_props(w, sb, d)?;
        if p.get("seamless").is_none_or(|t| t == "-") {
            anyhow::bail!("not seamless (no SEAMLESS tier)");
        }
        if p.get("lever_surface").is_none_or(|v| v != "yes") {
            anyhow::bail!("the lever is not on the surface of the build's volume (3.1.2)");
        }
        if p.get("lever_support").is_none_or(|v| v != "yes") {
            anyhow::bail!("the lever's support block moves or changes (3.1.5)");
        }
    }
    Ok(())
}

/// The first judge position only: the cheap screen of the eligibility filter.
fn eligibility_screen(w: &mut mctas::sim::Worker, sb: u32, d: &Design, opts: &SearchOptions) -> Result<()> {
    let first: Vec<Pos> = opts.judge.offsets.iter().take(1).copied().collect();
    eligibility_check(w, sb, d, opts, &first)
}

/// [`same_league`], or (with `to_sculk`) a solid base whose candidate became sculk, or (with
/// `to_entity`) a solid base whose candidate added entities; and within the `--shape` limits
/// when set. A shape limit replaces the "a flat base stays flat" rule (the shape decides which
/// layers the machine may use: `--shape wall` lets a flat design grow into the layers next to the
/// door plane).
fn league_ok(base: &Design, d: &Design, opts: &SearchOptions) -> bool {
    if let Some(l) = &opts.shape {
        let slimy = |x: &Design| x.blocks.values().any(|s| matches!(kind(s), "slime_block" | "honey_block"));
        return l.allows_design(d) && crate::category(base) == crate::category(d) && (slimy(base) || !slimy(d));
    }
    if same_league(base, d) {
        return true;
    }
    let slimy = |x: &Design| x.blocks.values().any(|s| matches!(kind(s), "slime_block" | "honey_block"));
    let keeps = (slimy(base) || !slimy(d)) && (!crate::single_layer(base) || crate::single_layer(d));
    let solid_base = crate::category(base) == "solid";
    (opts.to_sculk && solid_base && crate::category(d) == "sculk" && keeps)
        || (opts.to_entity && solid_base && crate::category(d) == "entity" && crate::entities_allowed_board(d) && keeps)
}

/// [`verify_in`] plus the search's speed constraints, and (full judge only, with
/// `keep_rotations`) the rotation check.
fn judge_in(w: &mut mctas::sim::Worker, sb: u32, d: &Design, v: &VerifyOptions, opts: &SearchOptions) -> Result<crate::Report> {
    if let Some(t) = opts.seamless {
        t.check(w, sb, d)?;
    }
    let r = speed_check(verify_in(w, sb, d, v)?, opts)?;
    // Shape boards measure the space a door needs while it moves too: check the swept shape.
    if let Some(l) = &opts.shape {
        let s = crate::swept_shape_in(w, sb, d)?;
        if !l.allows(&s) {
            anyhow::bail!("outside the --shape limits while it moves: {s:?}");
        }
    }
    // The full judge (not a screen): with `keep_rotations`, the rotation check too.
    if opts.keep_rotations && !opts.rotation_safe && std::ptr::eq(v, &opts.judge) {
        if let Some((turns, why)) = crate::rotation_check(w, d, v)? {
            anyhow::bail!("not rotation-safe: turned {} degrees: {why}", turns as u32 * 90);
        }
    }
    Ok(r)
}

/// The acceptance test: [`judge_in`] with the judge's options, and with `rotation_safe` the
/// door turned 90, 180 and 270 degrees too.
fn accept_in(w: &mut mctas::sim::Worker, sb: u32, d: &Design, opts: &SearchOptions) -> Result<crate::Report> {
    // Fail fast on the eligibility filter (one position) before the full judge.
    eligibility_screen(w, sb, d, opts)?;
    let r = judge_in(w, sb, d, &opts.judge, opts)?;
    if opts.rotation_safe {
        let v = VerifyOptions { keep_frames: false, ..opts.judge.clone() };
        if let Some((turns, why)) = crate::rotation_check(w, d, &v)? {
            anyhow::bail!("not rotation-safe: turned {} degrees: {why}", turns as u32 * 90);
        }
    }
    eligibility_check(w, sb, d, opts, &opts.judge.offsets)?;
    Ok(r)
}

/// The speed constraints (`max_open`, `max_close`) on a judge report.
fn speed_check(r: crate::Report, opts: &SearchOptions) -> Result<crate::Report> {
    let open = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
    let close = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
    if let Some(m) = opts.max_open.filter(|m| open > *m) {
        anyhow::bail!("too slow: opens after {open} ticks (max {m})");
    }
    if let Some(m) = opts.max_close.filter(|m| close > *m) {
        anyhow::bail!("too slow: closes after {close} ticks (max {m})");
    }
    Ok(r)
}

impl SearchOptions {
    fn has(&self, f: Family) -> bool {
        self.families.contains(&f)
    }
    fn timed_out(&self) -> bool {
        self.deadline.is_some_and(|d| Instant::now() > d)
    }
}

/// A design edit: blocks removed (an extended piston loses its head too), then blocks set, then
/// ports moved, then entities added.
#[derive(Clone, Debug, Default)]
pub struct Edit {
    pub remove: Vec<Pos>,
    pub set: Vec<(Pos, String)>,
    pub ports: Vec<(String, Pos)>,
    pub entities: Vec<mctas::design::EntitySpec>,
    pub desc: String,
}

impl Edit {
    fn removal(ps: &[Pos], d: &Design) -> Edit {
        let desc = ps.iter().map(|p| format!("-{} {}", fmt_pos(*p), short(&d.blocks[p]))).collect::<Vec<_>>().join(" ");
        Edit { remove: ps.to_vec(), desc, ..Default::default() }
    }

    pub fn apply(&self, d: &Design) -> Design {
        let mut out = d.clone();
        for p in &self.remove {
            remove_block(&mut out, *p);
        }
        for (p, s) in &self.set {
            out.blocks.insert(*p, s.clone());
        }
        for (name, p) in &self.ports {
            out.ports.insert(name.clone(), *p);
        }
        out.entities.extend(self.entities.iter().cloned());
        out
    }

    fn cells(&self) -> Vec<Pos> {
        self.remove.iter().copied().chain(self.set.iter().map(|(p, _)| *p)).collect()
    }
}

const WIRE: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const TRAPDOOR: &str = "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]";
const NOTE: &str = "minecraft:note_block[instrument=harp,note=0,powered=false]";
const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const HORIZONTAL: [&str; 4] = ["north", "east", "south", "west"];
const ALL_DIRS: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cheb(a: Pos, b: Pos) -> i32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs())
}
fn fmt_pos(p: Pos) -> String {
    format!("({},{},{})", p[0], p[1], p[2])
}
fn dir(f: &str) -> Option<Pos> {
    Some(match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}
fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}
/// Block id without namespace or properties.
fn kind(s: &str) -> &str {
    short(mctas::blockstate::block_name(s))
}
/// Can hold dust or a torch (roughly: a full block).
fn is_support(s: &str) -> bool {
    let k = kind(s);
    !matches!(
        k,
        "redstone_wire" | "repeater" | "comparator" | "redstone_torch" | "redstone_wall_torch" | "lever" | "piston_head" | "moving_piston" | "scaffolding"
    ) && !(k.ends_with("trapdoor") || k.ends_with("fence_gate") || k.ends_with("_rail") || k == "rail" || k.ends_with("_skull") || k.ends_with("_head") || k.ends_with("_door") || k.ends_with("_shelf"))
        && !((k.contains("slab") && !s.contains("type=top") && !s.contains("type=double")) || (k.contains("stairs") && !s.contains("half=top")))
}
/// Plain building blocks: full blocks with no behavior of their own besides holding things up
/// and conducting (a name-based denylist of mechanism, partial and gravity blocks).
fn is_plain(s: &str) -> bool {
    const MECH: [&str; 22] = [
        "air", "redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "lever", "piston", "sticky_piston",
        "piston_head", "moving_piston", "slime_block", "honey_block", "observer", "redstone_block", "redstone_lamp", "target",
        "dispenser", "dropper", "hopper", "note_block", "tnt",
    ];
    const PARTIAL: [&str; 26] = [
        "slab", "stairs", "wall", "fence", "door", "button", "plate", "rail", "torch", "sign", "carpet", "pane", "bars", "chain",
        "lantern", "ladder", "leaves", "sand", "gravel", "powder", "bulb", "sculk", "daylight", "lectern", "chest", "rod",
    ];
    let k = kind(s);
    !MECH.contains(&k) && !PARTIAL.iter().any(|x| k.contains(x))
}
/// Approximation of `isRedstoneConductor` (cuts dust steps).
fn is_conductor(s: &str) -> bool {
    let k = kind(s);
    is_support(s)
        && !matches!(k, "glass" | "tinted_glass" | "observer" | "piston" | "sticky_piston" | "slime_block" | "honey_block" | "redstone_block")
        && !k.ends_with("copper_grate")
        && !k.contains("slab")
        && !k.contains("stairs")
}
fn movable(k: &str) -> bool {
    !matches!(k, "piston_head" | "moving_piston" | "lever")
}

/// Remove the block at `p` in place; an extended piston loses its head.
fn remove_block(d: &mut Design, p: Pos) {
    let Some(s) = d.blocks.remove(&p) else { return };
    let Ok(b) = BlockState::parse(&s) else { return };
    if b.get("extended") == Some("true") {
        if let Some(v) = b.get("facing").and_then(dir) {
            let h = add(p, v);
            if d.blocks.get(&h).is_some_and(|s| s.starts_with("minecraft:piston_head")) {
                d.blocks.remove(&h);
            }
        }
    }
}

pub fn design_hash(d: &Design) -> u64 {
    let mut h = DefaultHasher::new();
    d.blocks.hash(&mut h);
    // Entities (category entity) as their text; designs without entities hash as before.
    for e in &d.entities {
        e.to_text().hash(&mut h);
    }
    h.finish()
}

/// Where a wall-mounted block hangs from, if it is one.
fn attachment(p: Pos, s: &str) -> Option<Pos> {
    let b = BlockState::parse(s).ok()?;
    match b.short_name() {
        "redstone_wall_torch" => Some(sub(p, dir(b.get("facing")?)?)),
        "redstone_torch" | "redstone_wire" | "repeater" | "comparator" | "powered_rail" | "activator_rail" | "rail" | "detector_rail" => Some(add(p, [0, -1, 0])),
        n if n.ends_with("_door") && b.get("half") == Some("lower") => Some(add(p, [0, -1, 0])),
        "lever" => match b.get("face") {
            Some("floor") => Some(add(p, [0, -1, 0])),
            Some("ceiling") => Some(add(p, [0, 1, 0])),
            _ => Some(sub(p, dir(b.get("facing")?)?)),
        },
        _ => None,
    }
}

/// Something hangs from or stands on `p`.
fn supports_something(d: &Design, p: Pos) -> bool {
    FACES.iter().any(|f| {
        let n = add(p, *f);
        d.blocks.get(&n).is_some_and(|s| attachment(n, s) == Some(p))
    })
}

/// `s` at `q` would stand (its attachment block exists in `d`).
fn support_ok(d: &Design, q: Pos, s: &str) -> bool {
    match attachment(q, s) {
        Some(a) => d.blocks.get(&a).is_some_and(|x| is_support(x)),
        None => true,
    }
}

/// What may not be touched: the door cells, the lever, piston heads; and where nothing may be
/// placed: the door cells and the walkway.
struct Ctx {
    keep: HashSet<Pos>,
    forbidden: HashSet<Pos>,
    /// The door (None for a non-door design searched with [`generic_edits`]).
    spec: Option<DoorSpec>,
}

impl Ctx {
    fn new(d: &Design) -> Result<Ctx> {
        let spec = DoorSpec::from_design(d)?;
        let mut keep: HashSet<Pos> = spec.door_cells().into_iter().collect();
        keep.insert(spec.lever);
        for (p, s) in &d.blocks {
            if s.starts_with("minecraft:piston_head") {
                keep.insert(*p);
            }
        }
        let forbidden = crate::route::keep_clear(&spec).into_iter().collect();
        Ok(Ctx { keep, forbidden, spec: Some(spec) })
    }

    fn removable(&self, d: &Design) -> Vec<Pos> {
        d.blocks.keys().filter(|p| !self.keep.contains(*p)).copied().collect()
    }
}

/// Removal candidates: singles, pairs, local triples.
fn removals(ctx: &Ctx, d: &Design, singles: bool, pairs: bool, triples: bool, radius: i32) -> Vec<Edit> {
    let r = ctx.removable(d);
    let mut out = Vec::new();
    if singles {
        for &p in &r {
            out.push(Edit::removal(&[p], d));
        }
    }
    if pairs {
        for i in 0..r.len() {
            for j in i + 1..r.len() {
                out.push(Edit::removal(&[r[i], r[j]], d));
            }
        }
    }
    if triples {
        for i in 0..r.len() {
            for j in i + 1..r.len() {
                let ab = cheb(r[i], r[j]) <= radius;
                for k in j + 1..r.len() {
                    let near = ab as u8 + (cheb(r[i], r[k]) <= radius) as u8 + (cheb(r[j], r[k]) <= radius) as u8;
                    if near >= 2 {
                        out.push(Edit::removal(&[r[i], r[j], r[k]], d));
                    }
                }
            }
        }
    }
    out
}

/// Other components that could stand at `p` instead of `b`.
fn substitutes(d: &Design, p: Pos, b: &BlockState) -> Vec<String> {
    let mut out = Vec::new();
    let has_floor = d.blocks.get(&add(p, [0, -1, 0])).is_some_and(|s| is_support(s));
    let wall_ok = |f: &str| d.blocks.get(&sub(p, dir(f).unwrap())).is_some_and(|s| is_support(s));
    let torches = |out: &mut Vec<String>| {
        if has_floor {
            out.push("minecraft:redstone_torch[lit=true]".to_string());
        }
        for f in HORIZONTAL {
            if wall_ok(f) {
                out.push(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
            }
        }
    };
    let observers = |out: &mut Vec<String>| {
        for f in ALL_DIRS {
            out.push(format!("minecraft:observer[facing={f},powered=false]"));
        }
    };
    // Watchable relays (exotic track): an iron trapdoor shows its power in its own state at
    // once (an observer reads it 2 gt after the edge), conducts nothing, needs no support; a
    // note block does the same but conducts. An observer whose only job is to make a power edge
    // visible to the next observer can be a trapdoor (0 gt instead of 2), and a trapdoor or note
    // block can stand where a plain block only relays power to an observer's neighbor.
    let relays = |out: &mut Vec<String>| {
        out.push(TRAPDOOR.to_string());
        out.push(NOTE.to_string());
    };
    match b.short_name() {
        "redstone_wire" => {
            for f in HORIZONTAL {
                out.push(format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]"));
            }
        }
        "repeater" => {
            out.push(WIRE.to_string());
            for f in HORIZONTAL {
                out.push(b.with("facing", f).to_string());
            }
        }
        "redstone_wall_torch" | "redstone_torch" => {
            observers(&mut out);
            torches(&mut out);
        }
        "observer" => {
            observers(&mut out);
            torches(&mut out);
            relays(&mut out);
        }
        "smooth_stone" | "stone" => {
            out.push("minecraft:glass".to_string());
            out.push(crate::route::NONSTICK_FLOOR.to_string());
            out.push(NOTE.to_string());
        }
        "note_block" => {
            out.push(TRAPDOOR.to_string());
            out.push("minecraft:smooth_stone".to_string());
        }
        k if k.ends_with("trapdoor") => {
            out.push(NOTE.to_string());
            observers(&mut out);
        }
        "glass" => out.push("minecraft:smooth_stone".to_string()),
        k if k.ends_with("glazed_terracotta") => out.push("minecraft:smooth_stone".to_string()),
        "slime_block" => out.push("minecraft:honey_block".to_string()),
        "honey_block" => out.push("minecraft:slime_block".to_string()),
        _ => {}
    }
    let me = b.to_string();
    out.retain(|s| BlockState::parse(s).map(|x| x.to_string()).ok().as_deref() != Some(me.as_str()));
    out
}

/// Count-neutral edits: repeater delays, substitutions, moves to a neighbor cell (an extended
/// piston moves with its head), and the lever moved or re-attached nearby (its port follows).
fn neutral_edits(ctx: &Ctx, d: &Design, opts: &SearchOptions) -> Vec<Edit> {
    neutral_edits_by(ctx, d, &|f| opts.has(f))
}

/// The local-edit neighbourhood `door minimize` searches, for ANY design (the bedrock study's
/// machine minimizer, `mctas bedrock minimize`): removals of 1-3 blocks (`Singles`, `Pairs`,
/// `Triples`, triples within `radius`), repeater delays, substitutions, moves, and with `Swap`
/// the swaps (two blocks out, one in), relocations and compounds. Cells in `keep` are never
/// touched and nothing is put on `forbidden`. The door-only families (`Reroute`, `Renet`: they
/// need the door's swept space) are ignored. Returns (improving: fewer blocks, neutral: the same
/// count), as `search` splits them.
pub fn generic_edits(d: &Design, keep: &HashSet<Pos>, forbidden: &HashSet<Pos>, families: &[Family], radius: i32) -> (Vec<Edit>, Vec<Edit>) {
    let ctx = Ctx { keep: keep.clone(), forbidden: forbidden.clone(), spec: None };
    let has = |f: Family| families.contains(&f);
    let neutral = neutral_edits_by(&ctx, d, &has);
    let mut improving = removals(&ctx, d, has(Family::Singles), has(Family::Pairs), has(Family::Triples), radius);
    improving.extend(compounds(&ctx, d, &neutral, radius));
    let mut neutral_all = neutral;
    if has(Family::Swap) {
        improving.extend(swaps(&ctx, d, radius));
        neutral_all.extend(relocations(&ctx, d, radius));
    }
    (improving, neutral_all)
}

fn neutral_edits_by(ctx: &Ctx, d: &Design, has: &dyn Fn(Family) -> bool) -> Vec<Edit> {
    let mut out = Vec::new();
    let set1 = |p: Pos, s: String, desc: String| Edit { set: vec![(p, s)], desc, ..Default::default() };
    for (&p, s) in &d.blocks {
        if ctx.keep.contains(&p) {
            continue;
        }
        let Ok(b) = BlockState::parse(s) else { continue };
        let k = b.short_name().to_string();
        if has(Family::Delays) && k == "repeater" {
            let cur = b.get("delay").unwrap_or("1").to_string();
            for dl in ["1", "2", "3", "4"] {
                if dl != cur {
                    out.push(set1(p, b.with("delay", dl).to_string(), format!("{} delay {cur}->{dl}", fmt_pos(p))));
                }
            }
        }
        if has(Family::Subs) {
            for alt in substitutes(d, p, &b) {
                out.push(set1(p, alt.clone(), format!("{} {} => {}", fmt_pos(p), short(s), short(&alt))));
            }
            // A piston of the other kind (the head follows).
            if k == "piston" || k == "sticky_piston" {
                let other = if k == "piston" { "sticky_piston" } else { "piston" };
                let mut e = set1(p, s.replacen(&format!("minecraft:{k}["), &format!("minecraft:{other}["), 1), format!("{} {k} => {other}", fmt_pos(p)));
                if b.get("extended") == Some("true") {
                    if let Some(v) = b.get("facing").and_then(dir) {
                        let h = add(p, v);
                        if let Some(hs) = d.blocks.get(&h) {
                            let ty = if other == "piston" { "type=normal" } else { "type=sticky" };
                            e.set.push((h, hs.replace("type=sticky", ty).replace("type=normal", ty)));
                        }
                    }
                }
                out.push(e);
            }
        }
        if has(Family::Moves) && movable(&k) && !supports_something(d, p) {
            // An extended piston moves with its head.
            let head = (b.get("extended") == Some("true"))
                .then(|| b.get("facing").and_then(dir).map(|v| add(p, v)))
                .flatten()
                .filter(|h| d.blocks.get(h).is_some_and(|x| x.starts_with("minecraft:piston_head")));
            for f in FACES {
                let q = add(p, f);
                let free = |c: Pos| (!d.blocks.contains_key(&c) || Some(c) == head || c == p) && !ctx.forbidden.contains(&c);
                if !free(q) {
                    continue;
                }
                let state = if k == "redstone_wire" { WIRE.to_string() } else { s.clone() };
                let mut e = Edit { remove: vec![p], set: vec![(q, state.clone())], desc: format!("move {} {} -> {}", fmt_pos(p), short(s), fmt_pos(q)), ..Default::default() };
                if let Some(h) = head {
                    let qh = add(h, f);
                    if !free(qh) {
                        continue;
                    }
                    e.set.push((qh, d.blocks[&h].clone()));
                }
                let after = e.apply(d);
                if !support_ok(&after, q, &state) {
                    continue;
                }
                out.push(e);
            }
        }
    }
    if has(Family::Moves) {
        out.extend(lever_moves(ctx, d));
    }
    out
}

/// The lever moved to a free neighbor cell (or kept) and attached to any block next to it.
fn lever_moves(ctx: &Ctx, d: &Design) -> Vec<Edit> {
    let mut out = Vec::new();
    let Some(&lp) = d.ports.get("lever") else { return out };
    let Some(ls) = d.blocks.get(&lp) else { return out };
    for q in std::iter::once(lp).chain(FACES.iter().map(|f| add(lp, *f))) {
        if q != lp && (d.blocks.contains_key(&q) || ctx.forbidden.contains(&q)) {
            continue;
        }
        let mut states = Vec::new();
        // keep the design's rest polarity (either counts, vjeux 2026-10-19): a lever-ON door's
        // moved lever stays on
        let pw = if ls.contains("powered=true") { "true" } else { "false" };
        let solid = |c: Pos| c != lp && d.blocks.get(&c).is_some_and(|s| is_support(s));
        if solid(add(q, [0, -1, 0])) {
            states.push(format!("minecraft:lever[face=floor,facing=west,powered={pw}]"));
        }
        if solid(add(q, [0, 1, 0])) {
            states.push(format!("minecraft:lever[face=ceiling,facing=west,powered={pw}]"));
        }
        for f in HORIZONTAL {
            if solid(sub(q, dir(f).unwrap())) {
                states.push(format!("minecraft:lever[face=wall,facing={f},powered={pw}]"));
            }
        }
        for st in states {
            if q == lp && attachment(lp, &st) == attachment(lp, ls) {
                continue;
            }
            out.push(Edit {
                remove: vec![lp],
                set: vec![(q, st.clone())],
                ports: vec![("lever".to_string(), q)],
                desc: format!("lever {} -> {} {}", fmt_pos(lp), fmt_pos(q), short(&st)),
                ..Default::default()
            });
        }
    }
    out
}

/// Block states a swap may put in: floors, dust, repeaters, observers, torches, power blocks.
fn swap_vocabulary() -> Vec<String> {
    let mut v = vec![
        "minecraft:smooth_stone".to_string(),
        crate::route::NONSTICK_FLOOR.to_string(),
        "minecraft:obsidian".to_string(),
        "minecraft:redstone_block".to_string(),
        WIRE.to_string(),
        "minecraft:redstone_torch[lit=true]".to_string(),
    ];
    for f in HORIZONTAL {
        for dl in ["1", "4"] {
            v.push(format!("minecraft:repeater[delay={dl},facing={f},locked=false,powered=false]"));
        }
        v.push(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
    }
    for f in ALL_DIRS {
        v.push(format!("minecraft:observer[facing={f},powered=false]"));
    }
    v
}

/// Swaps: two removable blocks within `radius` of each other out, one block of the vocabulary in
/// a free cell within `radius` of either (it must stand: dust, repeaters and torches need their
/// support). A move to any nearby cell plus a removal, a 2-for-1 replacement, and so on.
fn swaps(ctx: &Ctx, d: &Design, radius: i32) -> Vec<Edit> {
    let r = ctx.removable(d);
    let rset: HashSet<Pos> = r.iter().copied().collect();
    let vocab = swap_vocabulary();
    // Groups going out: local pairs, and face-connected triples (a dust with its floor and a
    // neighbor, short chains...).
    let mut groups: Vec<Vec<Pos>> = Vec::new();
    for i in 0..r.len() {
        for j in i + 1..r.len() {
            if cheb(r[i], r[j]) <= radius {
                groups.push(vec![r[i], r[j]]);
            }
        }
    }
    let mut seen3: HashSet<Vec<Pos>> = HashSet::new();
    for &a in &r {
        for f in FACES {
            let b = add(a, f);
            if !rset.contains(&b) {
                continue;
            }
            for c0 in [a, b] {
                for g in FACES {
                    let c = add(c0, g);
                    if c == a || c == b || !rset.contains(&c) {
                        continue;
                    }
                    let mut t = vec![a, b, c];
                    t.sort();
                    if seen3.insert(t.clone()) {
                        groups.push(t);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for group in groups {
        let base = Edit::removal(&group, d);
        let after = base.apply(d);
        let mut cells: Vec<Pos> = Vec::new();
        for &c in &group {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    for dz in -radius..=radius {
                        let q = add(c, [dx, dy, dz]);
                        if !after.blocks.contains_key(&q) && !ctx.forbidden.contains(&q) && !cells.contains(&q) {
                            cells.push(q);
                        }
                    }
                }
            }
        }
        for q in cells {
            for s in &vocab {
                if group.contains(&q) && d.blocks[&q] == *s {
                    continue;
                }
                if !support_ok(&after, q, s) {
                    continue;
                }
                let mut e = base.clone();
                e.set.push((q, s.clone()));
                e.desc = format!("{} +{} {}", base.desc, fmt_pos(q), short(s));
                out.push(e);
            }
        }
    }
    out
}

/// Relocations (neutral, for the plateau): one removable block out, one vocabulary block in a
/// free cell within `radius` (or in its own cell as another state).
fn relocations(ctx: &Ctx, d: &Design, radius: i32) -> Vec<Edit> {
    let vocab = swap_vocabulary();
    let mut out = Vec::new();
    for p in ctx.removable(d) {
        let base = Edit::removal(&[p], d);
        let after = base.apply(d);
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    let q = add(p, [dx, dy, dz]);
                    if after.blocks.contains_key(&q) || ctx.forbidden.contains(&q) {
                        continue;
                    }
                    for s in &vocab {
                        if (q == p && d.blocks[&p] == *s) || !support_ok(&after, q, s) {
                            continue;
                        }
                        let mut e = base.clone();
                        e.set.push((q, s.clone()));
                        e.desc = format!("{} +{} {}", base.desc, fmt_pos(q), short(s));
                        out.push(e);
                    }
                }
            }
        }
    }
    out
}

/// Each neutral edit plus one removal within `radius` of the cells it touches.
fn compounds(ctx: &Ctx, d: &Design, neutral: &[Edit], radius: i32) -> Vec<Edit> {
    let r = ctx.removable(d);
    let mut out = Vec::new();
    for e in neutral {
        let cells = e.cells();
        for &q in &r {
            if cells.contains(&q) || cells.iter().all(|c| cheb(*c, q) > radius) {
                continue;
            }
            let mut e2 = e.clone();
            e2.remove.push(q);
            e2.desc = format!("{} -{} {}", e.desc, fmt_pos(q), short(&d.blocks[&q]));
            out.push(e2);
        }
    }
    out
}

/// Dust cells and their connections (same level, or a step up/down with clearance).
fn dust_graph(d: &Design) -> HashMap<Pos, Vec<Pos>> {
    let dust: HashSet<Pos> = d.blocks.iter().filter(|(_, s)| kind(s) == "redstone_wire").map(|(p, _)| *p).collect();
    let conductor = |p: Pos| d.blocks.get(&p).is_some_and(|s| is_conductor(s));
    let mut g = HashMap::new();
    for &c in &dust {
        let mut ns = Vec::new();
        for h in HORIZ {
            let n = add(c, h);
            if dust.contains(&n) {
                ns.push(n);
            }
            let up = add(n, [0, 1, 0]);
            if dust.contains(&up) && !conductor(add(c, [0, 1, 0])) {
                ns.push(up);
            }
            let down = add(n, [0, -1, 0]);
            if dust.contains(&down) && !conductor(n) {
                ns.push(down);
            }
        }
        g.insert(c, ns);
    }
    g
}

/// A block that does something (not plain, not dust).
fn is_component(d: &Design, p: Pos) -> bool {
    d.blocks.get(&p).is_some_and(|s| !is_plain(s) && kind(s) != "redstone_wire")
}

/// Dust that interacts with something besides dust: its floor is not plain, it touches a
/// component, or a plain block it touches (its floor included) touches a component (dust powers
/// the block below it and the blocks it points into).
fn is_terminal(d: &Design, c: Pos) -> bool {
    let floor = add(c, [0, -1, 0]);
    if !d.blocks.get(&floor).is_some_and(|s| is_plain(s)) {
        return true;
    }
    FACES.iter().any(|f| {
        let n = add(c, *f);
        if is_component(d, n) {
            return true;
        }
        d.blocks.get(&n).is_some_and(|s| is_plain(s)) && FACES.iter().any(|g| add(n, *g) != c && is_component(d, add(n, *g)))
    })
}

/// Dust segments: (anchor, inner cells in order, anchor). Inner cells have two connections and
/// are plain wiring by one of two rules: loose (no component next to the dust, a plain floor
/// touching no component) or strict (not a terminal).
fn dust_segments(d: &Design, g: &HashMap<Pos, Vec<Pos>>) -> Vec<(Pos, Vec<Pos>, Pos)> {
    let loose = |c: Pos| -> bool {
        if g[&c].len() != 2 {
            return false;
        }
        let floor = add(c, [0, -1, 0]);
        if !d.blocks.get(&floor).is_some_and(|s| is_plain(s)) {
            return false;
        }
        if FACES.iter().any(|f| *f != [0, -1, 0] && is_component(d, add(c, *f))) {
            return false;
        }
        !FACES.iter().any(|f| add(floor, *f) != c && is_component(d, add(floor, *f)))
    };
    let strict = |c: Pos| -> bool { g[&c].len() == 2 && !is_terminal(d, c) };
    let mut seen: HashSet<Vec<Pos>> = HashSet::new();
    let mut out = Vec::new();
    let rules: [&dyn Fn(Pos) -> bool; 2] = [&loose, &strict];
    let mut anchors: Vec<Pos> = g.keys().copied().collect();
    anchors.sort();
    for inner in rules {
        for &a in &anchors {
            if inner(a) {
                continue;
            }
            for &n in &g[&a] {
                if !inner(n) {
                    continue;
                }
                let mut path = vec![n];
                let (mut prev, mut cur) = (a, n);
                let end = loop {
                    let next = g[&cur].iter().copied().find(|x| *x != prev);
                    let Some(next) = next else { break None };
                    if !inner(next) {
                        break Some(next);
                    }
                    if path.contains(&next) {
                        break None;
                    }
                    path.push(next);
                    prev = cur;
                    cur = next;
                };
                let Some(b) = end else { continue };
                let mut key = path.clone();
                key.sort();
                if seen.insert(key) {
                    out.push((a, path, b));
                }
            }
        }
    }
    out
}

fn router_box(d: &Design) -> (Pos, Pos) {
    let (lo, hi) = d.bounds().unwrap();
    ([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3])
}

/// The router stands dust only on smooth stone: show it every plain block outside the door as
/// smooth stone (edits keep the real blocks).
fn router_view(d: &Design, blocks: &BTreeMap<Pos, String>) -> BTreeMap<Pos, String> {
    let door: HashSet<Pos> = DoorSpec::from_design(d).map(|s| s.door_cells().into_iter().collect()).unwrap_or_default();
    blocks
        .iter()
        .map(|(p, s)| {
            let plain = is_plain(s) && !door.contains(p) && kind(s) != "smooth_stone";
            (*p, if plain { crate::route::FLOOR.to_string() } else { s.clone() })
        })
        .collect()
}

/// Where a design's machine goes during one open/close cycle: cells non-air at some tick
/// (`occupied`), and cells next to a slime/honey or moving block at some tick but never occupied
/// (`drag`: a block there would be dragged, unless it is non-sticky like glazed terracotta).
#[derive(Clone, Default)]
pub struct Space {
    pub occupied: HashSet<Pos>,
    pub drag: HashSet<Pos>,
}

pub fn sweep(w: &mut mctas::sim::Worker, d: &Design, spec: &DoorSpec) -> Result<Space> {
    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Space> {
        crate::video::place(w, sb, d)?;
        let (lo, hi) = d.bounds().context("empty design")?;
        let (lo, hi) = ([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]);
        let (mut occupied, mut near) = (HashSet::new(), HashSet::new());
        for _ in 0..2 {
            w.use_block(sb, spec.lever)?;
            for _ in 0..70 {
                w.step(sb, 1)?;
                let region = w.get_region(sb, lo, hi)?;
                for (p, s) in region.iter() {
                    if mctas::blockstate::is_air(s) {
                        continue;
                    }
                    occupied.insert(p);
                    if s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block") || s.contains("moving_piston") {
                        for f in FACES {
                            near.insert(add(p, f));
                        }
                    }
                }
            }
        }
        let drag = near.difference(&occupied).copied().collect();
        Ok(Space { occupied, drag })
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// Cells new wiring may not use: what the machine occupies while it runs, the door and walkway.
fn reserved_cells(ctx: &Ctx, d: &Design, space: &Space) -> HashSet<Pos> {
    let mut reserved: HashSet<Pos> = space.occupied.iter().filter(|p| !d.blocks.contains_key(*p)).copied().collect();
    reserved.extend(ctx.forbidden.iter().copied());
    reserved
}

/// Free cells where only a non-sticky floor may go (next to moving slime).
fn floor_only_cells(ctx: &Ctx, d: &Design, space: &Space) -> HashSet<Pos> {
    space.drag.iter().filter(|p| !d.blocks.contains_key(*p) && !ctx.forbidden.contains(*p)).copied().collect()
}

/// Sort an edit into improving / neutral by its block-count change; drops no-ops and repeats.
fn classify(d: &Design, mut e: Edit, seen: &mut HashSet<u64>, better: &mut Vec<Edit>, neutral: &mut Vec<Edit>) {
    let new = e.apply(d);
    if new.blocks == d.blocks || !seen.insert(design_hash(&new)) {
        return;
    }
    let delta = block_count(&new) as i64 - block_count(d) as i64;
    e.desc = format!("{} [{delta:+}]", e.desc);
    match delta {
        x if x < 0 => better.push(e),
        0 => neutral.push(e),
        _ => {}
    }
}

/// Dust nets: the connected components of the dust graph.
fn nets(g: &HashMap<Pos, Vec<Pos>>) -> Vec<Vec<Pos>> {
    let mut done: HashSet<Pos> = HashSet::new();
    let mut cells: Vec<Pos> = g.keys().copied().collect();
    cells.sort();
    let mut out = Vec::new();
    for &c0 in &cells {
        if !done.insert(c0) {
            continue;
        }
        let mut net = vec![c0];
        let mut q = VecDeque::from([c0]);
        while let Some(c) = q.pop_front() {
            for n in &g[&c] {
                if done.insert(*n) {
                    net.push(*n);
                    q.push_back(*n);
                }
            }
        }
        out.push(net);
    }
    out
}

/// Edits learnt from pairs of designs: for every (a, b) with the same door port where b has fewer
/// blocks than a and differs from it in at most `max_cells` cells, the edit a -> b (with a, to
/// check where it applies).
pub fn library_patches(designs: &[Design], max_cells: usize) -> Vec<(Edit, Design)> {
    let mut out: Vec<(Edit, Design)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for a in designs {
        for b in designs {
            if block_count(b) >= block_count(a) || a.ports.get("door") != b.ports.get("door") {
                continue;
            }
            let e = diff_edit(a, b, format!("patch {} -> {}", a.name, b.name));
            let size = e.remove.len() + e.set.len() + e.ports.len();
            if size == 0 || size > max_cells {
                continue;
            }
            let key = format!("{:?}|{:?}|{:?}", e.remove, e.set, e.ports);
            if seen.insert(key) {
                out.push((e, a.clone()));
            }
        }
    }
    out
}

/// The patches that apply to `target`: every cell a patch removes or changes holds in `target`
/// what it held in the patch's source design.
pub fn transplants(target: &Design, patches: &[(Edit, Design)]) -> Vec<Edit> {
    let mut out = Vec::new();
    for (e, a) in patches {
        let same = |p: &Pos| target.blocks.get(p) == a.blocks.get(p);
        if e.remove.iter().all(same) && e.set.iter().all(|(p, _)| same(p)) && e.ports.iter().all(|(k, _)| target.ports.get(k) == a.ports.get(k)) {
            let new = e.apply(target);
            if new.blocks != target.blocks {
                out.push(e.clone());
            }
        }
    }
    out
}

/// Screen and judge `edits` of `base` (only those with fewer blocks); the verified designs, fewest
/// blocks first.
pub fn try_edits(pool: &Pool, base: &Design, edits: Vec<Edit>, opts: &SearchOptions, log: &dyn Fn(&str)) -> Vec<(Design, String)> {
    let sandboxes = Sandboxes::new(pool);
    let failed: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let cands: Vec<Cand> =
        edits.into_iter().filter(|e| block_count(&e.apply(base)) < block_count(base)).map(|edit| Cand { base: 0, edit }).collect();
    evaluate(pool, &sandboxes, std::slice::from_ref(base), cands, opts, &failed, 0, log).into_iter().map(|(_, d, s)| (d, s)).collect()
}

/// The edit that turns `d` into `new`.
fn diff_edit(d: &Design, new: &Design, desc: String) -> Edit {
    let remove = d.blocks.keys().filter(|p| !new.blocks.contains_key(*p)).copied().collect();
    let set = new.blocks.iter().filter(|(p, s)| d.blocks.get(*p) != Some(*s)).map(|(p, s)| (*p, s.clone())).collect();
    let ports = new.ports.iter().filter(|(k, p)| d.ports.get(*k) != Some(*p)).map(|(k, p)| (k.clone(), *p)).collect();
    Edit { remove, set, ports, desc, ..Default::default() }
}

/// Net re-synthesis candidates: (improving, neutral). For each dust net with 2+ terminals: the
/// terminals reconnected from scratch ([`resynth`]), the same with one terminal dropped, and for
/// each repeater, the repeater turned into dust and its merged net re-synthesized.
fn renets(ctx: &Ctx, d: &Design, space: &Space) -> (Vec<Edit>, Vec<Edit>) {
    let reserved = reserved_cells(ctx, d, space);
    let fo = floor_only_cells(ctx, d, space);
    let (mut better, mut neutral) = (Vec::new(), Vec::new());
    let mut seen: HashSet<u64> = HashSet::new();
    let g = dust_graph(d);
    for net in nets(&g) {
        let terminals: Vec<Pos> = net.iter().copied().filter(|c| is_terminal(d, *c)).collect();
        if terminals.len() < 2 {
            continue;
        }
        let mut sets = vec![(terminals.clone(), String::new())];
        for t in &terminals {
            sets.push((terminals.iter().copied().filter(|x| x != t).collect(), format!(" without {}", fmt_pos(*t))));
        }
        for (ts, note) in sets {
            for (new, desc) in resynth(d, &g, &net, &ts, &reserved, &fo) {
                classify(d, diff_edit(d, &new, format!("{desc}{note}")), &mut seen, &mut better, &mut neutral);
            }
        }
    }
    for (&p, s) in &d.blocks {
        if kind(s) != "repeater" || ctx.keep.contains(&p) {
            continue;
        }
        let mut d2 = d.clone();
        d2.blocks.insert(p, WIRE.to_string());
        let g2 = dust_graph(&d2);
        let Some(net) = nets(&g2).into_iter().find(|n| n.contains(&p)) else { continue };
        let terminals: Vec<Pos> = net.iter().copied().filter(|c| is_terminal(&d2, *c)).collect();
        if terminals.len() < 2 {
            continue;
        }
        for (new, desc) in resynth(&d2, &g2, &net, &terminals, &reserved, &fo) {
            classify(d, diff_edit(d, &new, format!("repeater {} to dust, {desc}", fmt_pos(p))), &mut seen, &mut better, &mut neutral);
        }
    }
    (better, neutral)
}

/// Rip every cell of `net` but `terminals` (with the floors of plain wiring) and reconnect the
/// terminals with the router as a greedy Steiner tree, once from each terminal as the root
/// (each step routes the tree to the cheapest remaining terminal).
fn resynth(
    d: &Design,
    g: &HashMap<Pos, Vec<Pos>>,
    net: &[Pos],
    terminals: &[Pos],
    reserved: &HashSet<Pos>,
    floor_only: &HashSet<Pos>,
) -> Vec<(Design, String)> {
    let (lo, hi) = router_box(d);
    let tset: HashSet<Pos> = terminals.iter().copied().collect();
    let mut rip = Vec::new();
    for &c in net {
        if tset.contains(&c) {
            continue;
        }
        rip.push(c);
        if !is_terminal(d, c) {
            rip.push(add(c, [0, -1, 0]));
        }
    }
    let mut out = Vec::new();
    if rip.is_empty() {
        return out;
    }
    let wiring = net.len() - terminals.len();
    // Strict: new wiring may touch only what the old net touched; relaxed: anything (most such
    // candidates fail, but screening is cheap).
    for relaxed in [false, true] {
        let mut after = d.clone();
        for p in &rip {
            after.blocks.remove(p);
        }
        let mut base_allow: HashSet<Pos> = HashSet::new();
        if relaxed {
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        base_allow.insert([x, y, z]);
                    }
                }
            }
        }
        for c in net {
            for f in FACES {
                base_allow.insert(add(*c, f));
                base_allow.insert(add(add(*c, [0, -1, 0]), f));
            }
        }
        for p in &rip {
            base_allow.remove(p);
        }
        // Terminals still connected to `t` through terminals.
        let group = |t: Pos| -> Vec<Pos> {
            let mut out = vec![t];
            let mut q = VecDeque::from([t]);
            let mut seen: HashSet<Pos> = [t].into_iter().collect();
            while let Some(c) = q.pop_front() {
                for n in &g[&c] {
                    if tset.contains(n) && seen.insert(*n) {
                        out.push(*n);
                        q.push_back(*n);
                    }
                }
            }
            out
        };
        for &root in terminals {
            let mut blocks = router_view(d, &after.blocks);
            let mut tree = group(root);
            let mut remaining: Vec<Pos> = terminals.iter().copied().filter(|t| !tree.contains(t)).collect();
            let mut added: Vec<(Pos, String)> = Vec::new();
            let mut ok = true;
            while !remaining.is_empty() {
                let mut best: Option<(usize, Vec<(Pos, String)>, usize)> = None;
                for (i, &t) in remaining.iter().enumerate() {
                    let mut bl = blocks.clone();
                    bl.remove(&t);
                    let mut allow = base_allow.clone();
                    allow.extend(tree.iter().copied());
                    let r = Router { blocks: &bl, reserved: reserved.clone(), allow, lo, hi, floor_only: floor_only.clone() };
                    if let Some(path) = r.route(&tree, t) {
                        let cost = path.iter().filter(|(p, _)| *p != t).count();
                        if best.as_ref().is_none_or(|b| cost < b.2) {
                            best = Some((i, path, cost));
                        }
                    }
                }
                let Some((i, path, _)) = best else {
                    ok = false;
                    break;
                };
                let t = remaining.swap_remove(i);
                for (p, s) in path {
                    if p == t {
                        continue;
                    }
                    let s = if kind(&s) == "redstone_wire" {
                        tree.push(p);
                        WIRE.to_string()
                    } else {
                        s
                    };
                    blocks.insert(p, s.clone());
                    added.push((p, s));
                }
                for x in group(t) {
                    if !tree.contains(&x) {
                        tree.push(x);
                    }
                }
                remaining.retain(|x| !tree.contains(x));
            }
            if !ok {
                continue;
            }
            let mut new = after.clone();
            for (p, s) in added {
                new.blocks.insert(p, s);
            }
            out.push((new, format!("renet{} from {} ({} terminals, {wiring} ripped)", if relaxed { " (relaxed)" } else { "" }, fmt_pos(root), terminals.len())));
        }
    }
    out
}

/// Rip-up-and-reroute candidates: (improving, neutral).
fn reroutes(ctx: &Ctx, d: &Design, space: &Space) -> (Vec<Edit>, Vec<Edit>) {
    let g = dust_graph(d);
    let segs = dust_segments(d, &g);
    let (lo, hi) = router_box(d);
    let reserved = reserved_cells(ctx, d, space);
    let fo = floor_only_cells(ctx, d, space);
    let (mut better, mut neutral) = (Vec::new(), Vec::new());
    let mut seen: HashSet<u64> = HashSet::new();
    for (a, inner, b) in &segs {
        for (keep, goal) in [(*a, *b), (*b, *a)] {
            if ctx.keep.contains(&goal) {
                continue;
            }
            let mut rip: Vec<Pos> = Vec::new();
            for c in inner {
                rip.push(*c);
                rip.push(add(*c, [0, -1, 0]));
            }
            rip.push(goal);
            let mut after = d.clone();
            for p in &rip {
                after.blocks.remove(p);
            }
            // Start: every dust cell still connected to `keep` once the segment is gone.
            let mut start = vec![keep];
            let mut q = VecDeque::from([keep]);
            let mut seen_cells: HashSet<Pos> = [keep].into_iter().collect();
            while let Some(c) = q.pop_front() {
                for n in &g[&c] {
                    if after.blocks.contains_key(n) && seen_cells.insert(*n) {
                        start.push(*n);
                        q.push_back(*n);
                    }
                }
            }
            // The new path may touch what the old segment and the goal touched.
            let mut allow: HashSet<Pos> = HashSet::new();
            for c in inner.iter().chain([&goal]) {
                for f in FACES {
                    allow.insert(add(*c, f));
                    allow.insert(add(add(*c, [0, -1, 0]), f));
                }
            }
            for p in &rip {
                allow.remove(p);
            }
            let view = router_view(d, &after.blocks);
            let router = Router { blocks: &view, reserved: reserved.clone(), allow, lo, hi, floor_only: fo.clone() };
            let Some(path) = router.route(&start, goal) else { continue };
            let mut e = Edit { remove: rip.clone(), ..Default::default() };
            for (p, s) in &path {
                let s = if kind(s) == "redstone_wire" { WIRE.to_string() } else { s.clone() };
                e.set.push((*p, s));
            }
            e.desc = format!("reroute {} -> {} ({} inner dust, {} new blocks)", fmt_pos(keep), fmt_pos(goal), inner.len(), path.len());
            classify(d, e, &mut seen, &mut better, &mut neutral);
        }
    }
    (better, neutral)
}

/// The judge's verdict without per-tick frames: same placement, same checks on the final state of
/// every phase (verify_at only uses frames for timings). Screening only; the judge is verify_in.
pub(crate) fn screen_in(w: &mut mctas::sim::Worker, sb: u32, design: &Design, opts: &VerifyOptions) -> Result<()> {
    use anyhow::bail;
    use mctas::sim::{PlaceMode, Region};
    if let Some(t) = &opts.tile {
        return crate::tile::verify_tiled_in(w, sb, design, t, opts).map(|_| ());
    }
    // A design with entities is screened by the judge's own code (this frame-less copy knows
    // nothing of entities).
    if !design.entities.is_empty() {
        return verify_in(w, sb, design, opts).map(|_| ());
    }
    let illegal = crate::legality_problems(design);
    if !illegal.is_empty() {
        bail!("illegal design: {}", illegal.join("; "));
    }
    let moving = |s: &str| s.starts_with("minecraft:moving_piston");
    for off in &opts.offsets {
        let d = design.translated(*off);
        let spec = DoorSpec::from_design(&d)?;
        w.clear(sb)?;
        let blocks = d.block_list();
        w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
        let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
        w.update(sb, &wires, "shape")?;
        let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
        w.update(sb, &positions, "self")?;
        if w.step_until_idle(sb, opts.max_ticks)?.idle != Some(true) {
            bail!("never settles");
        }
        let (mut lo, mut hi) = d.bounds().context("empty design")?;
        for p in spec.door_cells().iter().chain(spec.corridor_cells().iter()) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let (wlo, whi) = ([lo[0] - 2, lo[1] - 2, lo[2] - 2], [hi[0] + 2, hi[1] + 2, hi[2] + 2]);
        let rest = w.get_region(sb, wlo, whi)?;
        // Closed: 9 identical full non-mechanism door blocks, the walkway clear.
        let first = rest.get(spec.door_cells()[0]).unwrap_or("?").to_string();
        for p in spec.door_cells() {
            let s = rest.get(p).unwrap_or("?");
            if s != first || is_mechanism_door_block(s) || !rest.flags_at(p).is_some_and(|f| f.full) || moving(s) {
                bail!("not closed at rest");
            }
        }
        if spec.corridor_cells().iter().any(|p| rest.flags_at(*p).is_some_and(|f| f.collides)) {
            bail!("walkway blocked at rest");
        }
        let lever = rest.get(spec.lever).unwrap_or("?");
        if !lever.starts_with("minecraft:lever") {
            bail!("lever");
        }
        let open_ok = |r: &Region| {
            spec.door_cells().into_iter().chain(spec.corridor_cells()).all(|p| {
                let s = r.get(p).unwrap_or("?");
                !(r.flags_at(p).is_some_and(|f| f.collides) || moving(s))
            })
        };
        // Periodic rest, like the judge (96a62d6, vjeux: internal state doesn't matter as long as
        // the door works repeatedly): each close leaves the door closed (its rest door blocks, the
        // walkway clear) and some closed state repeats an earlier one within cycles +
        // MAX_PERIOD_EXTRA.
        let mut seen_closed: Vec<Region> = vec![rest.clone()];
        let mut periodic = false;
        for cycle in 0..opts.cycles + crate::MAX_PERIOD_EXTRA {
            if cycle >= opts.cycles && periodic {
                break;
            }
            w.use_block(sb, spec.lever)?;
            if w.step_until_idle(sb, opts.max_ticks)?.idle != Some(true) {
                bail!("still moving after opening");
            }
            if !open_ok(&w.get_region(sb, wlo, whi)?) || !w.entities(sb)?.is_empty() {
                bail!("not open");
            }
            w.use_block(sb, spec.lever)?;
            if w.step_until_idle(sb, opts.max_ticks)?.idle != Some(true) {
                bail!("still moving after closing");
            }
            let last = w.get_region(sb, wlo, whi)?;
            let closed = spec.door_cells().iter().all(|p| last.get(*p) == rest.get(*p)) && !spec.corridor_cells().iter().any(|p| last.flags_at(*p).is_some_and(|f| f.collides));
            if !closed || !w.entities(sb)?.is_empty() {
                bail!("not closed after closing");
            }
            if seen_closed.iter().any(|s| crate::same_region(s, &last)) {
                periodic = true;
            } else {
                seen_closed.push(last);
            }
        }
        if !periodic {
            bail!("not back at rest (nor at any earlier closed state)");
        }
    }
    Ok(())
}

/// The judge's list of blocks a door may not be made of.
fn is_mechanism_door_block(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    [
        "slime_block", "honey_block", "piston", "sticky_piston", "piston_head", "moving_piston", "observer",
        "redstone_block", "redstone_lamp", "target", "repeater", "comparator", "redstone_wire", "redstone_torch",
        "redstone_wall_torch", "lever", "dispenser", "dropper", "hopper", "note_block", "tnt",
    ]
    .iter()
    .any(|n| name.strip_prefix("minecraft:") == Some(*n))
}

/// A candidate stays in its base's leaderboard: same category, no slime/honey added to a
/// slime-free design, and no block outside the door plane added to a flat design.
pub(crate) fn same_league(base: &Design, d: &Design) -> bool {
    let slimy = |x: &Design| x.blocks.values().any(|s| matches!(kind(s), "slime_block" | "honey_block"));
    crate::category(base) == crate::category(d) && (slimy(base) || !slimy(d)) && (!crate::single_layer(base) || crate::single_layer(d))
}

/// A candidate: an edit of `bases[base]`.
struct Cand {
    base: usize,
    edit: Edit,
}

/// Router-based candidates (reroute, renet) of each design, (improving, neutral), on threads.
fn routed(opts: &SearchOptions, designs: &[Design], swept: &[Space]) -> Vec<(Vec<Edit>, Vec<Edit>)> {
    let n = designs.len();
    let results: Mutex<Vec<Option<(Vec<Edit>, Vec<Edit>)>>> = Mutex::new((0..n).map(|_| None).collect());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..n.min(16) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let d = &designs[i];
                    let (mut b, mut nn) = (Vec::new(), Vec::new());
                    if let Ok(ctx) = Ctx::new(d) {
                        if opts.has(Family::Reroute) {
                            let (x, y) = reroutes(&ctx, d, &swept[i]);
                            b.extend(x);
                            nn.extend(y);
                        }
                        if opts.has(Family::Renet) {
                            let (x, y) = renets(&ctx, d, &swept[i]);
                            b.extend(x);
                            nn.extend(y);
                        }
                    }
                    results.lock().unwrap()[i] = Some((b, nn));
                }
            });
        }
    });
    results.into_inner().unwrap().into_iter().map(|r| r.unwrap_or_default()).collect()
}

enum Screen {
    Pass((u32, u32)),
    Fail,
    Skip,
    Error(String),
}

/// What the search minimizes, lexicographically: blocks, or worst-cycle ticks (open, close,
/// open + close) then blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    Blocks,
    Open,
    Close,
    Total,
    /// The community volume (`door properties` volume), then blocks.
    Volume,
}

pub fn parse_objective(s: &str) -> Result<Objective> {
    Ok(match s {
        "blocks" => Objective::Blocks,
        "open" => Objective::Open,
        "close" => Objective::Close,
        "total" | "open+close" => Objective::Total,
        "volume" => Objective::Volume,
        _ => anyhow::bail!("unknown objective {s:?}; have blocks, open, close, total, volume"),
    })
}

/// The community VOLUME of a design (Squid rules 4.2, `door properties` `volume`: the bounding
/// box of the circuitry over every state and tick of one cycle, the lever, the door blocks and
/// quartz-like hallway-ring blocks excluded), from one recorded cycle at its own position.
pub fn community_volume(w: &mut mctas::sim::Worker, sb: u32, d: &Design) -> Result<u32> {
    let p = crate::properties::input_props(w, sb, d)?;
    Ok(p.get("volume").and_then(|v| v.parse::<u32>().ok()).unwrap_or(u32::MAX))
}

/// [`objective_key`], with the volume objective computed in the sandbox (one recorded cycle).
fn objective_key_in(w: &mut mctas::sim::Worker, sb: u32, obj: Objective, d: &Design, r: &crate::Report) -> Result<(u32, u32)> {
    match obj {
        Objective::Volume => Ok((community_volume(w, sb, d)?, block_count(d) as u32)),
        _ => Ok(objective_key(obj, d, r)),
    }
}

/// Worst-cycle ticks (open, close) of a judge report, as the speed leaderboards count them.
pub fn worst_ticks(r: &crate::Report) -> (u32, u32) {
    let open = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(9999);
    let close = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(9999);
    (open, close)
}

fn objective_key(obj: Objective, d: &Design, r: &crate::Report) -> (u32, u32) {
    let b = block_count(d) as u32;
    let (open, close) = worst_ticks(r);
    match obj {
        Objective::Blocks => (b, 0),
        Objective::Open => (open, b),
        Objective::Close => (close, b),
        Objective::Total => (open + close, b),
        // Needs a sandbox: see objective_key_in.
        Objective::Volume => (u32::MAX, b),
    }
}

/// Screen then judge `cands`; returns the judge-passing designs, best (fewest blocks) first,
/// with their descriptions and base index. Stops judging after `want` passes (0 = judge all).
fn evaluate(
    pool: &Pool,
    sandboxes: &Sandboxes,
    bases: &[Design],
    cands: Vec<Cand>,
    opts: &SearchOptions,
    failed: &Mutex<HashSet<u64>>,
    want: usize,
    log: &dyn Fn(&str),
) -> Vec<(usize, Design, String)> {
    evaluate_with(pool, sandboxes, bases, cands, opts, failed, want, None, log)
}

/// [`evaluate`], or with `bar` (speed objectives): keep only candidates whose objective key
/// beats `bar`, at the screen (estimated from its first cycle) and at the judge; results by key.
#[allow(clippy::too_many_arguments)]
fn evaluate_with(
    pool: &Pool,
    sandboxes: &Sandboxes,
    bases: &[Design],
    cands: Vec<Cand>,
    opts: &SearchOptions,
    failed: &Mutex<HashSet<u64>>,
    want: usize,
    bar: Option<(u32, u32)>,
    log: &dyn Fn(&str),
) -> Vec<(usize, Design, String)> {
    let t = Instant::now();
    let n = cands.len();
    let batch_seen: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    // The candidate's key: its objective with a bar (speed / volume objectives), blocks otherwise.
    let rank = |w: &mut mctas::sim::Worker, sb: u32, d: &Design, r: &crate::Report| -> Result<(u32, u32)> {
        match bar {
            Some(_) => objective_key_in(w, sb, opts.objective, d, r),
            None => Ok((block_count(d) as u32, 0)),
        }
    };
    let results = pool.map((0..n).collect(), |w, wi, i| {
        let c = &cands[i];
        let d = c.edit.apply(&bases[c.base]);
        let h = design_hash(&d);
        if failed.lock().unwrap().contains(&h) || !batch_seen.lock().unwrap().insert(h) || !league_ok(&bases[c.base], &d, opts) {
            return Ok(Screen::Skip);
        }
        let sb = sandboxes.get(w, wi)?;
        // Blocks objective without speed constraints: the frame-less screen (same verdict, no
        // per-tick regions). Timings (objective or --max-open/--max-close) need the frames.
        let frames = opts.screen_frames || bar.is_some() || opts.max_open.is_some() || opts.max_close.is_some();
        let verdict = if frames { judge_in(w, sb, &d, &opts.screen, opts).map(Some) } else { screen_in(w, sb, &d, &opts.screen).map(|_| None) };
        // The eligibility filter's first position, on screen survivors only.
        let verdict = verdict.and_then(|r| eligibility_screen(w, sb, &d, opts).map(|_| r));
        match verdict {
            Ok(r) => {
                let k = match &r {
                    Some(r) => match rank(w, sb, &d, r) {
                        Ok(k) => k,
                        Err(_) => return Ok(Screen::Fail),
                    },
                    None => (block_count(&d) as u32, 0),
                };
                if bar.is_some_and(|b| k >= b) {
                    return Ok(Screen::Fail);
                }
                Ok(Screen::Pass(k))
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if msg.contains("bridge exited") || msg.contains("Broken pipe") {
                    return Ok(Screen::Error(msg));
                }
                failed.lock().unwrap().insert(h);
                Ok(Screen::Fail)
            }
        }
    });
    let mut pass: Vec<(usize, (u32, u32))> = Vec::new();
    let (mut skipped, mut errors) = (0, 0);
    for (i, r) in results.into_iter().enumerate() {
        let err = match r {
            Ok(Screen::Pass(k)) => {
                pass.push((i, k));
                None
            }
            Ok(Screen::Skip) => {
                skipped += 1;
                None
            }
            Ok(Screen::Fail) => None,
            Ok(Screen::Error(e)) => Some(e),
            Err(e) => Some(format!("{e:#}")),
        };
        if let Some(e) = err {
            if errors == 0 {
                log(&format!("  worker error: {e}"));
            }
            errors += 1;
        }
    }
    let screen_time = t.elapsed().as_secs_f64();
    // Judge the survivors, best key first, a pool-sized chunk at a time.
    pass.sort_by_key(|&(i, k)| (k, i));
    let pass: Vec<usize> = pass.into_iter().map(|(i, _)| i).collect();
    let mut verified = Vec::new();
    let chunk = pool.len().max(1);
    let mut judged = 0;
    for part in pass.chunks(chunk) {
        let res = pool.map(part.to_vec(), |w, wi, i| {
            let c = &cands[i];
            let d = c.edit.apply(&bases[c.base]);
            let sb = sandboxes.get(w, wi)?;
            Ok(accept_in(w, sb, &d, opts).and_then(|r| Ok((rank(w, sb, &d, &r)?, d))).map_err(|e| format!("{e:#}")))
        });
        judged += part.len();
        for (&i, r) in part.iter().zip(res) {
            match r {
                Ok(Ok((k, d))) if bar.is_none_or(|b| k < b) => verified.push((k, cands[i].base, d, cands[i].edit.desc.clone())),
                Ok(Ok(_)) => {}
                Ok(Err(_)) => {
                    let d = cands[i].edit.apply(&bases[cands[i].base]);
                    failed.lock().unwrap().insert(design_hash(&d));
                }
                Err(e) => log(&format!("  judge worker error: {e:#}")),
            }
        }
        if want > 0 && verified.len() >= want {
            break;
        }
        if opts.timed_out() {
            break;
        }
    }
    verified.sort_by_key(|(k, _, _, _)| *k);
    log(&format!(
        "  {n} candidates: {skipped} seen, {} screened in {screen_time:.1}s, {} passed the screen, {judged} judged, {} passed the judge ({:.1}s){}",
        n - skipped - errors,
        pass.len(),
        verified.len(),
        t.elapsed().as_secs_f64(),
        if errors > 0 { format!(", {errors} worker errors") } else { String::new() }
    ));
    verified.into_iter().map(|(_, b, d, s)| (b, d, s)).collect()
}

fn rand_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Canonicalize a verified design (true rest states) and keep it if it still passes the judge.
fn finalize(pool: &Pool, sandboxes: &Sandboxes, d: Design, opts: &SearchOptions, log: &dyn Fn(&str)) -> Result<Design> {
    Ok(finalize_report(pool, sandboxes, d, opts, log)?.0)
}

/// [`finalize`], with the judge report of the kept design.
fn finalize_report(pool: &Pool, _sandboxes: &Sandboxes, d: Design, opts: &SearchOptions, log: &dyn Fn(&str)) -> Result<(Design, crate::Report)> {
    // The exact `door verify` path (a fresh sandbox), so nothing a reused sandbox leaks can let
    // a design through.
    let mut w = pool.worker(0);
    // The eligibility filter (--repeater-input / --eligible) in a fresh sandbox too.
    let elig = |w: &mut mctas::sim::Worker, d: &Design| -> Result<()> {
        if !opts.repeater_input && !opts.eligible {
            return Ok(());
        }
        let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        let r = eligibility_check(w, sb, d, opts, &opts.judge.offsets);
        let _ = w.drop_sandbox(sb);
        r
    };
    let canon = crate::canonicalize(&mut w, &d)?;
    // A block that does not stay where it is written (it falls, pops or moves at rest) would
    // turn into an air entry: keep the design as edited then.
    if canon.blocks.values().any(|s| mctas::blockstate::is_air(s)) {
        log("  the canonical form has air entries (something moves at rest); keeping the design as edited");
        let r = crate::verify(&mut w, &d, &opts.judge)
            .and_then(|r| speed_check(r, opts))
            .and_then(|r| elig(&mut w, &d).map(|_| r))
            .context("the accepted design fails the judge in a fresh sandbox")?;
        return Ok((d, r));
    }
    match crate::verify(&mut w, &canon, &opts.judge).and_then(|r| speed_check(r, opts)).and_then(|r| elig(&mut w, &canon).map(|_| r)) {
        Ok(r) => Ok((canon, r)),
        Err(e) => {
            log(&format!("  canonical form fails the judge ({e:#}); keeping the design as edited"));
            let r = crate::verify(&mut w, &d, &opts.judge)
                .and_then(|r| speed_check(r, opts))
                .and_then(|r| elig(&mut w, &d).map(|_| r))
                .context("the accepted design fails the judge in a fresh sandbox")?;
            Ok((d, r))
        }
    }
}

/// The local search. `on_improve` gets every accepted (judge-verified) improvement.
pub fn search(
    pool: &Pool,
    start: &Design,
    opts: &SearchOptions,
    log: &dyn Fn(&str),
    on_improve: &mut dyn FnMut(&Design, &str) -> Result<()>,
) -> Result<Design> {
    let sandboxes = Sandboxes::new(pool);
    let mut rng = opts.seed.max(1);
    let mut cur = start.clone();
    // Air entries place nothing but would count as blocks (and removing them would look like an
    // improvement): drop them.
    let airs = cur.blocks.len();
    cur.blocks.retain(|_, s| !mctas::blockstate::is_air(s));
    if cur.blocks.len() != airs {
        log(&format!("dropped {} air entries from the start design", airs - cur.blocks.len()));
    }
    // Keep rotation safety only if the start design has it.
    let relaxed;
    let opts = if opts.keep_rotations {
        let mut w = pool.worker(0);
        let start_rot = crate::rotation_check(&mut w, &cur, &opts.judge)?;
        match start_rot {
            None => {
                log("the start design is rotation-safe: candidates must stay rotation-safe (--any-rotation to allow others)");
                opts
            }
            Some((turns, _)) => {
                log(&format!("the start design is NOT rotation-safe (turned {} degrees): rotations not checked", turns as u32 * 90));
                relaxed = SearchOptions { keep_rotations: false, ..opts.clone() };
                &relaxed
            }
        }
    } else {
        opts
    };
    let start_report = {
        let mut w = pool.worker(0);
        let sb = sandboxes.get(&mut w, 0)?;
        accept_in(&mut w, sb, &cur, opts).context("the start design fails the judge")?
    };
    let mut cur_key = {
        let mut w = pool.worker(0);
        let sb = sandboxes.get(&mut w, 0)?;
        objective_key_in(&mut w, sb, opts.objective, &cur, &start_report)?
    };
    if opts.objective == Objective::Volume {
        log(&format!("start: volume {}", cur_key.0));
    }
    let (o, c) = worst_ticks(&start_report);
    log(&format!(
        "start: {} blocks, open {o} close {c} ticks, passes the judge ({} positions x {} cycles)",
        block_count(&cur),
        opts.judge.offsets.len(),
        opts.judge.cycles
    ));
    let failed: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let mut visited: HashSet<u64> = [design_hash(&cur)].into_iter().collect();
    let mut walk_left = opts.walk;
    while !opts.timed_out() {
        let ctx = Ctx::new(&cur)?;
        let neutral = neutral_edits(&ctx, &cur, opts);
        let mut improving =
            removals(&ctx, &cur, opts.has(Family::Singles), opts.has(Family::Pairs), opts.has(Family::Triples), opts.radius);
        let n_removals = improving.len();
        let mut neutral_all = neutral.clone();
        let mut n_reroute = (0, 0);
        if opts.has(Family::Reroute) || opts.has(Family::Renet) {
            let swept = {
                let mut w = pool.worker(0);
                sweep(&mut w, &cur, ctx.spec.as_ref().expect("the door search needs a door design"))?
            };
            let tr = Instant::now();
            let (b, n) = routed(opts, std::slice::from_ref(&cur), std::slice::from_ref(&swept)).remove(0);
            n_reroute = (b.len(), n.len());
            improving.extend(b);
            neutral_all.extend(n);
            if tr.elapsed().as_secs_f64() > 5.0 {
                log(&format!("  routing took {:.1}s", tr.elapsed().as_secs_f64()));
            }
        }
        let comp = compounds(&ctx, &cur, &neutral, opts.radius);
        let n_comp = comp.len();
        improving.extend(comp);
        if opts.has(Family::Swap) {
            let s = swaps(&ctx, &cur, opts.radius);
            log(&format!("  {} swaps (two blocks out, one block in nearby)", s.len()));
            improving.extend(s);
        }
        log(&format!(
            "{} blocks: {} removals, {} reroutes (+{} neutral), {} compounds, {} neutral edits",
            block_count(&cur),
            n_removals,
            n_reroute.0,
            n_reroute.1,
            n_comp,
            neutral.len()
        ));
        if opts.objective != Objective::Blocks {
            // Speed: every edit within the block budget; keep the best key that beats the current.
            let limit = block_count(start) + opts.slack;
            let mut all = improving;
            all.extend(neutral_all);
            let cands: Vec<Cand> = all.into_iter().filter(|e| block_count(&e.apply(&cur)) <= limit).map(|edit| Cand { base: 0, edit }).collect();
            let bases = vec![cur.clone()];
            let verified = evaluate_with(pool, &sandboxes, &bases, cands, opts, &failed, 1, Some(cur_key), log);
            let Some((_, best, desc)) = verified.into_iter().next() else { break };
            let (d, r) = finalize_report(pool, &sandboxes, best, opts, log)?;
            let k = {
                let mut w = pool.worker(0);
                let sb = sandboxes.get(&mut w, 0)?;
                objective_key_in(&mut w, sb, opts.objective, &d, &r)?
            };
            if k >= cur_key {
                log("  the canonical form is not better; stopping");
                break;
            }
            let (o, c) = worst_ticks(&r);
            let vol = if opts.objective == Objective::Volume { format!(", volume {}", k.0) } else { String::new() };
            log(&format!("  accepted: {desc} -> open {o}, close {c}, {} blocks{vol}", block_count(&d)));
            cur = d;
            cur_key = k;
            on_improve(&cur, &format!("{desc}; open {o} close {c} ticks{vol}"))?;
            continue;
        }
        let bases = vec![cur.clone()];
        let cands: Vec<Cand> = improving.into_iter().map(|edit| Cand { base: 0, edit }).collect();
        let edits: Vec<Edit> = cands.iter().map(|c| c.edit.clone()).collect();
        let verified = evaluate(pool, &sandboxes, &bases, cands, opts, &failed, 1, log);
        if let Some((_, best, desc)) = verified.first().cloned() {
            log(&format!("  accepted: {desc} -> {} blocks", block_count(&best)));
            let mut next = best;
            // Re-try the other judge-passing edits on top.
            for (_, _, d2) in verified.iter().skip(1) {
                let Some(e) = edits.iter().find(|e| &e.desc == d2) else { continue };
                if e.cells().iter().any(|p| next.blocks.get(p) != cur.blocks.get(p)) {
                    continue;
                }
                let cand = e.apply(&next);
                if block_count(&cand) >= block_count(&next) {
                    continue;
                }
                let mut w = pool.worker(0);
                let sb = sandboxes.get(&mut w, 0)?;
                if accept_in(&mut w, sb, &cand, opts).is_ok() {
                    log(&format!("  also: {d2} -> {} blocks", block_count(&cand)));
                    next = cand;
                }
            }
            cur = finalize(pool, &sandboxes, next, opts, log)?;
            visited.insert(design_hash(&cur));
            on_improve(&cur, &desc)?;
            walk_left = opts.walk;
            continue;
        }
        if !opts.plateau || opts.timed_out() {
            break;
        }
        // Plateau: neutral edits that pass the judge, each followed by single (and pair) removals.
        if opts.has(Family::Swap) {
            neutral_all.extend(relocations(&ctx, &cur, opts.radius));
        }
        log(&format!("  no improving move; plateau: judging {} neutral edits", neutral_all.len()));
        let cands: Vec<Cand> = neutral_all.into_iter().map(|edit| Cand { base: 0, edit }).collect();
        let mut plateau: Vec<(usize, Design, String)> = evaluate(pool, &sandboxes, &bases, cands, opts, &failed, 0, log)
            .into_iter()
            .filter(|(_, d, _)| block_count(d) == block_count(&cur) && !visited.contains(&design_hash(d)))
            .collect();
        if plateau.is_empty() {
            log("  plateau: no verified neutral design left");
            break;
        }
        let pbases: Vec<Design> = plateau.iter().map(|(_, d, _)| d.clone()).collect();
        let mut cands = Vec::new();
        for (i, b) in pbases.iter().enumerate() {
            let c = Ctx::new(b)?;
            for edit in removals(&c, b, true, opts.plateau_pairs, false, opts.radius) {
                cands.push(Cand { base: i, edit });
            }
        }
        let n_removals = cands.len();
        if opts.has(Family::Reroute) || opts.has(Family::Renet) {
            let tr = Instant::now();
            let spec = ctx.spec.expect("the door search needs a door design");
            let swept: Vec<Space> = pool
                .map((0..pbases.len()).collect(), |w, _, i| sweep(w, &pbases[i], &spec))
                .into_iter()
                .map(|r| r.unwrap_or_default())
                .collect();
            for (i, (b, _)) in routed(opts, &pbases, &swept).into_iter().enumerate() {
                for edit in b {
                    cands.push(Cand { base: i, edit });
                }
            }
            log(&format!("  plateau: routing took {:.1}s", tr.elapsed().as_secs_f64()));
        }
        log(&format!(
            "  plateau: {} verified neutral designs, {n_removals} removals and {} reroutes after them",
            pbases.len(),
            cands.len() - n_removals
        ));
        let found = evaluate(pool, &sandboxes, &pbases, cands, opts, &failed, 1, log);
        if let Some((bi, best, desc)) = found.into_iter().next() {
            let desc = format!("{} then {desc}", plateau[bi].2);
            log(&format!("  accepted: {desc} -> {} blocks", block_count(&best)));
            cur = finalize(pool, &sandboxes, best, opts, log)?;
            visited.insert(design_hash(&cur));
            on_improve(&cur, &desc)?;
            walk_left = opts.walk;
            continue;
        }
        if opts.depth >= 2 && !opts.timed_out() {
            // Level 2: neutral edits of every level-1 design, then single removals after those.
            let mut cands = Vec::new();
            for (i, b) in pbases.iter().enumerate() {
                let c = Ctx::new(b)?;
                for edit in neutral_edits(&c, b, opts) {
                    cands.push(Cand { base: i, edit });
                }
            }
            log(&format!("  plateau level 2: judging {} neutral edits of the level-1 designs", cands.len()));
            let level2: Vec<(usize, Design, String)> = evaluate(pool, &sandboxes, &pbases, cands, opts, &failed, 0, log)
                .into_iter()
                .filter(|(_, d, _)| block_count(d) == block_count(&cur) && visited.insert(design_hash(d)))
                .collect();
            let l2bases: Vec<Design> = level2.iter().map(|(_, d, _)| d.clone()).collect();
            let mut cands = Vec::new();
            for (i, b) in l2bases.iter().enumerate() {
                let c = Ctx::new(b)?;
                for edit in removals(&c, b, true, false, false, opts.radius) {
                    cands.push(Cand { base: i, edit });
                }
            }
            log(&format!("  plateau level 2: {} verified designs, {} removals after them", l2bases.len(), cands.len()));
            let found = evaluate(pool, &sandboxes, &l2bases, cands, opts, &failed, 1, log);
            if let Some((bi, best, desc)) = found.into_iter().next() {
                let (b1, _, d2) = &level2[bi];
                let desc = format!("{} then {d2} then {desc}", plateau[*b1].2);
                log(&format!("  accepted: {desc} -> {} blocks", block_count(&best)));
                cur = finalize(pool, &sandboxes, best, opts, log)?;
                visited.insert(design_hash(&cur));
                on_improve(&cur, &desc)?;
                walk_left = opts.walk;
                continue;
            }
        }
        if walk_left == 0 {
            break;
        }
        walk_left -= 1;
        let pick = (rand_next(&mut rng) % plateau.len() as u64) as usize;
        let (_, d, desc) = plateau.swap_remove(pick);
        log(&format!("  walk ({} steps left): {desc}", walk_left));
        cur = d;
        visited.insert(design_hash(&cur));
    }
    Ok(cur)
}

/// `door regrow`: every face-connected group of `size` non-piston control blocks is cut and
/// re-grown with `size - 1` blocks from `states` inside the group's bounding box grown by
/// `margin` (a "cut n, add n-1" move the removal-only minimizer cannot make). With `any`, every
/// group of `size` control blocks is cut, connected or not, and the new blocks go in the cells
/// within `margin` of any cut block (two far-apart parts replaced by one block somewhere near
/// either). `only` restricts the cut blocks to a box. Returns the verified designs, fewest blocks
/// first.
#[allow(clippy::too_many_arguments)]
pub fn regrow(pool: &Pool, base: &Design, size: usize, margin: i32, states: &[String], only: Option<(Pos, Pos)>, opts: &SearchOptions, want: usize, any: bool, require_any: &[String], log: &dyn Fn(&str)) -> Result<Vec<(Design, String)>> {
    // `require_any`: a candidate must add one block whose state names one of these.
    let wanted = |s: &str| require_any.is_empty() || require_any.iter().any(|r| s.contains(r.as_str()));
    let sandboxes = Sandboxes::new(pool);
    let ctx = Ctx::new(base)?;
    let inside = |p: &Pos| only.is_none_or(|(lo, hi)| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]));
    let cuttable: Vec<Pos> = ctx
        .removable(base)
        .into_iter()
        .filter(|p| {
            let k = kind(&base.blocks[p]);
            !k.contains("piston") && inside(p)
        })
        .collect();
    let set: HashSet<Pos> = cuttable.iter().copied().collect();
    // Connected groups of `size` (as sorted vectors, deduplicated).
    let mut groups: HashSet<Vec<Pos>> = HashSet::new();
    let mut frontier: Vec<Vec<Pos>> = cuttable.iter().map(|p| vec![*p]).collect();
    for _ in 1..size {
        let mut next = Vec::new();
        for g in &frontier {
            let around: Vec<Pos> = if any {
                cuttable.clone()
            } else {
                g.iter().flat_map(|p| FACES.iter().map(move |f| add(*p, *f))).collect()
            };
            for n in around {
                if set.contains(&n) && !g.contains(&n) {
                    let mut h = g.clone();
                    h.push(n);
                    h.sort();
                    if groups.insert(h.clone()) {
                        next.push(h);
                    }
                }
            }
        }
        frontier = next;
    }
    let mut groups: Vec<Vec<Pos>> = groups.into_iter().filter(|g| g.len() == size).collect();
    groups.sort();
    log(&format!("{} groups of {size}", groups.len()));
    // Candidates are built and judged in batches of at most BATCH (size-3 `--any` cuts make
    // tens of millions of candidates: materializing them all at once took 55 GB).
    const BATCH: usize = 400_000;
    let failed: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let mut verified: Vec<(usize, Design, String)> = Vec::new();
    let mut cands: Vec<Cand> = Vec::new();
    let mut total = 0usize;
    let mut flush = |cands: &mut Vec<Cand>, verified: &mut Vec<(usize, Design, String)>| {
        if cands.is_empty() {
            return;
        }
        total += cands.len();
        log(&format!("batch of {} candidates ({total} so far)", cands.len()));
        let left = if want == 0 { 0 } else { want.saturating_sub(verified.len()).max(1) };
        verified.extend(evaluate(pool, &sandboxes, &[base.clone()], std::mem::take(cands), opts, &failed, left, log));
    };
    for g in &groups {
        if want > 0 && verified.len() >= want {
            break;
        }
        if cands.len() >= BATCH {
            flush(&mut cands, &mut verified);
            if want > 0 && verified.len() >= want {
                break;
            }
        }
        let cut = Edit::removal(g, base);
        let d = cut.apply(base);
        let mut cells: Vec<Pos> = Vec::new();
        if any {
            let mut seen: HashSet<Pos> = HashSet::new();
            for p in g {
                for x in p[0] - margin..=p[0] + margin {
                    for y in p[1] - margin..=p[1] + margin {
                        for z in p[2] - margin..=p[2] + margin {
                            if seen.insert([x, y, z]) {
                                cells.push([x, y, z]);
                            }
                        }
                    }
                }
            }
        } else {
            let (mut lo, mut hi) = (g[0], g[0]);
            for p in g {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i] - margin);
                    hi[i] = hi[i].max(p[i] + margin);
                }
            }
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        cells.push([x, y, z]);
                    }
                }
            }
        }
        let mut singles: Vec<(Pos, String)> = Vec::new();
        for p in cells {
            if d.blocks.contains_key(&p) || ctx.forbidden.contains(&p) {
                continue;
            }
            singles.extend(states.iter().map(|s| (p, s.clone())));
        }
        let stands = |a: Pos, sa: &String, other: Option<(Pos, &String)>| match attachment(a, sa) {
            Some(at) => match other {
                Some((b, sb)) if b == at => is_support(sb),
                _ => d.blocks.get(&at).is_some_and(|x| is_support(x)),
            },
            None => true,
        };
        let one = |p: Pos, s: &String| format!("+{} {}", fmt_pos(p), short(s));
        for (i, (p, s)) in singles.iter().enumerate() {
            if size == 2 {
                if stands(*p, s, None) && wanted(s) {
                    let mut e = cut.clone();
                    e.set.push((*p, s.clone()));
                    e.desc = format!("{} {}", cut.desc, one(*p, s));
                    cands.push(Cand { base: 0, edit: e });
                }
                continue;
            }
            for (q, t) in singles.iter().skip(i + 1) {
                if q == p || !stands(*p, s, Some((*q, t))) || !stands(*q, t, Some((*p, s))) || !(wanted(s) || wanted(t)) {
                    continue;
                }
                let mut e = cut.clone();
                e.set.push((*p, s.clone()));
                e.set.push((*q, t.clone()));
                e.desc = format!("{} {} {}", cut.desc, one(*p, s), one(*q, t));
                cands.push(Cand { base: 0, edit: e });
            }
        }
    }
    if want == 0 || verified.len() < want {
        flush(&mut cands, &mut verified);
    }
    log(&format!("{} groups of {size}; {total} candidates judged", groups.len()));
    Ok(verified.into_iter().map(|(_, d, desc)| (d, desc)).collect())
}

/// `door grow`: which blocks to try, in which cells.
pub struct GrowOptions {
    /// Inclusive box of cells to fill (cells already used, door cells and the walkway skipped).
    pub lo: Pos,
    pub hi: Pos,
    /// Block states to try in each cell.
    pub states: Vec<String>,
    /// Blocks added together: 1 or 2 (2 also tries every single).
    pub k: usize,
    /// Stop judging after this many verified designs (0 = judge every survivor).
    pub want: usize,
    /// Riders only: every added block must touch a slime/honey block (an existing one or, with
    /// k=2, an added slime/honey that itself touches an existing one), so it moves with a structure.
    pub glue: bool,
}

/// Expand `door grow --states` shorthands: `obs` (observer, 6 facings), `block`
/// (smooth_stone), `dust` (cross wire), `rep`/`rep2`/`rep3`/`rep4` (repeater, 4 facings, that
/// delay), `torch` (standing + 4 wall torches), `glass`, `obsidian`, `rblock` (redstone block),
/// `lamp`, `note` (note block: a conductor whose `powered` state an observer can watch), `bulb`
/// (waxed copper bulb), `target`, or a full `minecraft:...` state.
pub fn grow_states(spec: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tok {
            "obs" => out.extend(ALL_DIRS.iter().map(|f| format!("minecraft:observer[facing={f},powered=false]"))),
            "block" => out.push("minecraft:smooth_stone".into()),
            "dust" => out.push("minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into()),
            "rep" | "rep1" | "rep2" | "rep3" | "rep4" => {
                let delay = tok.strip_prefix("rep").and_then(|d| d.parse::<u32>().ok()).unwrap_or(1);
                out.extend(HORIZONTAL.iter().map(|f| format!("minecraft:repeater[delay={delay},facing={f},locked=false,powered=false]")));
            }
            "torch" => {
                out.push("minecraft:redstone_torch[lit=true]".into());
                out.extend(HORIZONTAL.iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f},lit=true]")));
            }
            "glass" => out.push("minecraft:glass".into()),
            "obsidian" => out.push("minecraft:obsidian".into()),
            "rblock" => out.push("minecraft:redstone_block".into()),
            "lamp" => out.push("minecraft:redstone_lamp[lit=false]".into()),
            "note" => out.push("minecraft:note_block[instrument=harp,note=0,powered=false]".into()),
            "bulb" => out.push("minecraft:waxed_copper_bulb[lit=false,powered=false]".into()),
            "target" => out.push("minecraft:target[power=0]".into()),
            // Persistent leaves (never random-tick): re-read their distance 1 gt after a neighbor
            // changes, so next to a moving log they are an asymmetric 1 gt delay (pareto-mid).
            "leaves" => out.push("minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]".into()),
            "log" => out.extend(["x", "y", "z"].iter().map(|a| format!("minecraft:oak_log[axis={a}]"))),
            // Entities (category entity, with --to-entity): a minecart, a chest minecart full of
            // stone (a comparator reads 15 through a detector rail), an armor stand; and the
            // blocks that sense them. (`rail` is the exotic palette's powered rail: the plain
            // rail a cart rides is `plainrail`.)
            "cart" => out.push("entity:minecraft:minecart".into()),
            "chestcart" => out.push("entity:minecraft:chest_minecart {Items:[{Slot:0b,id:\"minecraft:stone\",count:64}]}".into()),
            "stand" => out.push("entity:minecraft:armor_stand".into()),
            "detector" => out.extend(["east_west", "north_south"].iter().map(|s| format!("minecraft:detector_rail[powered=false,shape={s}]"))),
            "plainrail" => out.extend(["east_west", "north_south"].iter().map(|s| format!("minecraft:rail[shape={s}]"))),
            "plate" => out.push("minecraft:stone_pressure_plate[powered=false]".into()),
            "wplate" => out.push("minecraft:oak_pressure_plate[powered=false]".into()),
            s => match crate::synth::exotic_states(s) {
                Some(v) => out.extend(v),
                None if s.starts_with("minecraft:") => out.push(s.to_string()),
                None => return Err(anyhow!("unknown grow state {s:?}")),
            },
        }
    }
    Ok(out)
}

/// Add up to `g.k` blocks to `base` (which may be broken, e.g. a piston whose wiring was
/// removed) from `g.states` in the free cells of the box; every combination is screened and the
/// survivors judged. Returns the verified designs, fewest blocks first.
pub fn grow(pool: &Pool, base: &Design, g: &GrowOptions, opts: &SearchOptions, log: &dyn Fn(&str)) -> Result<Vec<(Design, String)>> {
    let sandboxes = Sandboxes::new(pool);
    let ctx = Ctx::new(base)?;
    // Entity tokens ("entity:<type> [snbt]") go in a free cell or one holding a rail or a plate
    // (they share it), feet on the cell floor, or 1/16 up on a rail.
    let is_entity = |s: &str| s.starts_with("entity:");
    let shares = |s: &str| {
        let k = kind(s);
        k.ends_with("rail") || k.ends_with("pressure_plate")
    };
    let mut singles: Vec<(Pos, String)> = Vec::new();
    for x in g.lo[0]..=g.hi[0] {
        for y in g.lo[1]..=g.hi[1] {
            for z in g.lo[2]..=g.hi[2] {
                let p = [x, y, z];
                if ctx.forbidden.contains(&p) {
                    continue;
                }
                match base.blocks.get(&p) {
                    None => singles.extend(g.states.iter().map(|s| (p, s.clone()))),
                    Some(b) if shares(b) => singles.extend(g.states.iter().filter(|s| is_entity(s)).map(|s| (p, s.clone()))),
                    Some(_) => {}
                }
            }
        }
    }
    let entity_at = |p: Pos, s: &str, rail: bool| -> mctas::design::EntitySpec {
        let body = s.trim_start_matches("entity:");
        let (kind_s, nbt) = match body.split_once(' ') {
            Some((k, n)) => (k.to_string(), Some(n.trim().to_string())),
            None => (body.to_string(), None),
        };
        let h = if rail { 0.0625 } else { 0.0 };
        mctas::design::EntitySpec { kind: kind_s, pos: [p[0] as f64 + 0.5, p[1] as f64 + h, p[2] as f64 + 0.5], nbt }
    };
    let rail_in_base = |p: Pos| base.blocks.get(&p).is_some_and(|b| kind(b).ends_with("rail"));
    // One added thing: a block, or an entity (on whatever the base has in its cell).
    let edit1 = |p: Pos, s: &String, desc: String| -> Edit {
        if is_entity(s) {
            Edit { entities: vec![entity_at(p, s, rail_in_base(p))], desc, ..Default::default() }
        } else {
            Edit { set: vec![(p, s.clone())], desc, ..Default::default() }
        }
    };
    let one = |p: Pos, s: &String| format!("+{} {}", fmt_pos(p), short(s));
    let sticky = |s: &str| matches!(kind(s), "slime_block" | "honey_block");
    let adj = |a: Pos, b: Pos| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<i32>() == 1;
    let glued_to_base = |p: Pos| {
        [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
            .iter()
            .any(|d| base.blocks.get(&add(p, *d)).is_some_and(|s| sticky(s)))
    };
    let mut cands: Vec<Cand> = Vec::new();
    for (i, (p, s)) in singles.iter().enumerate() {
        if (is_entity(s) || support_ok(base, *p, s)) && (!g.glue || glued_to_base(*p)) {
            cands.push(Cand { base: 0, edit: edit1(*p, s, one(*p, s)) });
        }
        if g.k < 2 {
            continue;
        }
        for (q, t) in singles.iter().skip(i + 1) {
            if q == p {
                // Only an entity and the rail or plate it stands on share a cell.
                let (e, b) = if is_entity(s) { (s, t) } else { (t, s) };
                if is_entity(e) && !is_entity(b) && shares(b) {
                    let desc = format!("{} {}", one(*p, b), one(*p, e));
                    let edit = Edit { set: vec![(*p, b.clone())], entities: vec![entity_at(*p, e, kind(b).ends_with("rail"))], desc, ..Default::default() };
                    cands.push(Cand { base: 0, edit });
                }
                continue;
            }
            if g.glue {
                // One added slime/honey stuck to a structure, the other block stuck to it (or both
                // stuck to the structure directly).
                let rides = |a: Pos, sa: &String, b: Pos| glued_to_base(a) || (sticky(sa) && adj(a, b));
                let ok = (sticky(s) && glued_to_base(*p) && rides(*q, t, *p))
                    || (sticky(t) && glued_to_base(*q) && rides(*p, s, *q))
                    || (glued_to_base(*p) && glued_to_base(*q));
                if !ok {
                    continue;
                }
            }
            // Each block stands on the base or on the other one.
            let stands = |a: Pos, sa: &String, b: Pos, sb: &String| match attachment(a, sa) {
                Some(at) if at == b => is_support(sb),
                Some(at) => base.blocks.get(&at).is_some_and(|x| is_support(x)),
                None => true,
            };
            if !stands(*p, s, *q, t) || !stands(*q, t, *p, s) {
                continue;
            }
            let desc = format!("{} {}", one(*p, s), one(*q, t));
            let mut edit = edit1(*p, s, desc);
            let other = edit1(*q, t, String::new());
            edit.set.extend(other.set);
            edit.entities.extend(other.entities);
            cands.push(Cand { base: 0, edit });
        }
    }
    log(&format!("{} cells x states = {} singles; {} candidates", singles.len() / g.states.len().max(1), singles.len(), cands.len()));
    let failed: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let verified = evaluate(pool, &sandboxes, &[base.clone()], cands, opts, &failed, g.want, log);
    Ok(verified.into_iter().map(|(_, d, desc)| (d, desc)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2_min() -> Design {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/piston-door-3x3/v2-min.txt");
        Design::load(&path).unwrap()
    }

    #[test]
    fn swaps_take_out_pairs_and_connected_triples() {
        let d = v2_min();
        let ctx = Ctx::new(&d).unwrap();
        let s = swaps(&ctx, &d, 2);
        assert!(s.iter().any(|e| e.remove.len() == 2));
        let triples: Vec<&Edit> = s.iter().filter(|e| e.remove.len() == 3).collect();
        assert!(!triples.is_empty());
        for e in triples {
            // Face-connected: some block touches both others, or forms a chain.
            let adj = |a: Pos, b: Pos| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<i32>() == 1;
            let [a, b, c] = [e.remove[0], e.remove[1], e.remove[2]];
            let links = adj(a, b) as u8 + adj(a, c) as u8 + adj(b, c) as u8;
            assert!(links >= 2, "{:?}", e.remove);
            assert_eq!(e.set.len(), 1);
            assert!(!ctx.keep.contains(&a) && !ctx.keep.contains(&b) && !ctx.keep.contains(&c));
        }
        // Every swap removes more blocks than it adds.
        for e in s.iter().take(2000) {
            assert!(block_count(&e.apply(&d)) < block_count(&d));
        }
    }

    #[test]
    fn edits_keep_the_league() {
        let d = v2_min();
        let mut no_slime = d.clone();
        no_slime.blocks.retain(|_, s| kind(s) != "slime_block");
        let mut with_slime = no_slime.clone();
        with_slime.blocks.insert([20, 20, 20], "minecraft:slime_block".to_string());
        assert!(!same_league(&no_slime, &with_slime));
        assert!(same_league(&d, &d));
        // A flat design (every block in the door plane) stays flat.
        let mut flat = Design::default();
        flat.ports.insert("door".to_string(), [0, 0, 0]);
        flat.blocks.insert([0, 0, 0], "minecraft:stone".to_string());
        let mut thick = flat.clone();
        thick.blocks.insert([0, 0, 1], "minecraft:stone".to_string());
        assert!(!same_league(&flat, &thick));
        assert!(same_league(&thick, &flat));
    }
}
