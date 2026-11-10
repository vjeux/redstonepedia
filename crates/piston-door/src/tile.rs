//! Tileable doors: copies of ONE module door side by side (or stacked), all run by ONE lever.
//!
//! `mctas door verify <module> --tile <spec>` builds the tiled arrangement from the module and
//! runs the judge's checks (the same ones as [`crate::verify_in`]) on EVERY opening at once:
//! closed at rest (9 identical plain full blocks per opening, walkways clear, nothing at all in
//! the 3x3 columns in front of and behind any opening at any distance), every block able to
//! stand, nothing random-ticking, the lever opens every opening (door cells + walkways
//! walkable, no entities), the lever closes every opening (the whole watched box back to its
//! rest state, no entities), 3 cycles at every judge position (32 with `--extra-offsets 28`).
//!
//! Tilings (`--tile`):
//! * `wide:N`: N copies, each 3 blocks east of the previous one: the openings touch and form one
//!   3 x 3N opening (a wide door). No side piston can sit between two openings.
//! * `period:P:N`: N copies every P blocks along the wall (x), each with its own 3x3 opening
//!   (P >= 4: a wall of P - 3 blocks between openings). `period:3:N` is `wide:N`.
//! * `stack:N`: N copies, each 3 blocks above the previous one: one 3N x 3 opening (a tall door).
//! * `vperiod:Q:N`: N copies every Q blocks up (one door per floor).
//! * `step:dx,dy,dz:N`: any other translation.
//!
//! One lever: copy 0 keeps the module's lever. In every other copy the lever cell holds the
//! module's `relay:` block (a header line of the module file, e.g. `relay: minecraft:redstone_wire`)
//! or stays empty, so the module must carry the signal to the next copy itself (its "bus").
//! Cells used by two copies must hold the same block (shared, counted once); anything else is a
//! conflict. A module may have several openings (ports `door`, `door2`, `door3`, ...): a tile
//! unit of two different doors (say mirrored, or slime then honey) is one module.
//!
//! `--tile-levers each` is a diagnostic, not a tiling verdict: every copy keeps its own lever
//! and all levers flip in the same tick (do copies interfere when they all run?).

use crate::{CycleReport, DoorSpec, Report, VerifyOptions, closed_problems, open_problems, settled_from};
use anyhow::{Context, Result, anyhow, bail, ensure};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// How the copies are switched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeverMode {
    /// Copy 0 keeps the lever; the others get the module's `relay:` block (the real tiling).
    One,
    /// Every copy keeps its lever, all flipped in the same tick (diagnostic).
    Each,
}

/// A tiling of one module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    /// The spec as given (`wide:3`, `period:7:3`, ...).
    pub label: String,
    /// Translation from one copy to the next.
    pub step: Pos,
    pub copies: usize,
    pub levers: LeverMode,
    /// A shared end piece placed once in copy 0's frame (`--tile-extra`): the lever and the
    /// connection a definition needs at the start of the row (e.g. a riser from the lever to a
    /// bus under a wide door). With a `port lever`, that lever replaces copy 0's (every copy then
    /// holds the relay).
    pub extra: Option<Design>,
}

impl Tile {
    /// `wide:N`, `period:P:N`, `stack:N`, `vperiod:Q:N` or `step:dx,dy,dz:N`.
    pub fn parse(s: &str) -> Result<Tile> {
        let parts: Vec<&str> = s.trim().split(':').collect();
        let num = |t: &str| -> Result<i32> { t.trim().parse::<i32>().map_err(|_| anyhow!("--tile {s:?}: {t:?} is not a number")) };
        let (step, n) = match parts.as_slice() {
            ["wide", n] => ([3, 0, 0], num(n)?),
            ["period", p, n] => ([num(p)?, 0, 0], num(n)?),
            ["stack", n] => ([0, 3, 0], num(n)?),
            ["vperiod", q, n] => ([0, num(q)?, 0], num(n)?),
            ["step", v, n] => {
                let c: Vec<i32> = v.split(',').map(num).collect::<Result<_>>()?;
                ensure!(c.len() == 3, "--tile {s:?}: step needs dx,dy,dz");
                ([c[0], c[1], c[2]], num(n)?)
            }
            _ => bail!("--tile {s:?}: expected wide:N, period:P:N, stack:N, vperiod:Q:N or step:dx,dy,dz:N"),
        };
        ensure!(n >= 1, "--tile {s:?}: at least one copy");
        ensure!(step != [0, 0, 0], "--tile {s:?}: copies would all stand in one place");
        Ok(Tile { label: s.trim().to_string(), step, copies: n as usize, levers: LeverMode::One, extra: None })
    }

    /// The same tiling with another number of copies.
    pub fn with_copies(&self, n: usize) -> Tile {
        let mut t = self.clone();
        t.copies = n;
        t
    }
}

/// A module's openings: port `door` first, then `door2`, `door3`, ... (by name).
pub fn module_openings(module: &Design) -> Result<Vec<DoorSpec>> {
    let lever = *module.ports.get("lever").context("module has no 'port lever: x y z'")?;
    let mut doors: Vec<(&String, Pos)> = module
        .ports
        .iter()
        .filter(|(k, _)| k.as_str() == "door" || (k.starts_with("door") && k[4..].parse::<u32>().is_ok()))
        .map(|(k, p)| (k, *p))
        .collect();
    ensure!(doors.iter().any(|(k, _)| k.as_str() == "door"), "module has no 'port door: x y z'");
    doors.sort_by_key(|(k, _)| if k.as_str() == "door" { 0 } else { k[4..].parse::<u32>().unwrap() });
    Ok(doors.into_iter().map(|(_, door)| DoorSpec { door, lever, along_z: false, size: 3 }).collect())
}

/// The module's relay block for the copies without the lever (`relay:` header line), if any.
pub fn relay_state(module: &Design) -> Result<Option<String>> {
    match module.meta.get("relay").map(|s| s.trim()) {
        None | Some("") | Some("air") | Some("minecraft:air") => Ok(None),
        Some(s) => Ok(Some(mctas::blockstate::BlockState::parse(s).with_context(|| format!("relay: {s:?}"))?.to_string())),
    }
}

/// The tiled arrangement.
#[derive(Clone, Debug)]
pub struct Tiled {
    /// Every block of every copy (ports: `lever` and `door` of copy 0).
    pub design: Design,
    /// Every opening, copy by copy.
    pub specs: Vec<DoorSpec>,
    /// The levers flipped together (one, unless [`LeverMode::Each`]).
    pub levers: Vec<Pos>,
    pub copies: usize,
    /// Cells shared by two or more copies (counted once).
    pub shared: usize,
}

/// Build the arrangement: `tile.copies` copies of `module`, copy k moved by k * step (the whole
/// row centered on the module's own position so it fits the judge's sandbox).
pub fn expand(module: &Design, tile: &Tile) -> Result<Tiled> {
    if !module.entities.is_empty() {
        bail!("tile {}: modules with entities cannot be tiled yet (only blocks are copied)", tile.label);
    }
    let openings = module_openings(module)?;
    let lever = openings[0].lever;
    let relay = relay_state(module)?;
    let n = tile.copies as i32;
    let center = (n - 1) / 2;
    let mut blocks: BTreeMap<Pos, (String, usize)> = BTreeMap::new();
    let mut shared = 0;
    let mut specs = Vec::new();
    let mut levers = Vec::new();
    let extra_lever = tile.extra.as_ref().and_then(|e| e.ports.get("lever").copied());
    let o0 = [-center * tile.step[0], -center * tile.step[1], -center * tile.step[2]];
    for k in 0..tile.copies {
        let o = [(k as i32 - center) * tile.step[0], (k as i32 - center) * tile.step[1], (k as i32 - center) * tile.step[2]];
        let mv = |p: Pos| [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
        let keeps_lever = (k == 0 && extra_lever.is_none()) || tile.levers == LeverMode::Each;
        for (p, s) in &module.blocks {
            let state = if *p == lever && !keeps_lever {
                match &relay {
                    Some(r) => r.clone(),
                    None => continue,
                }
            } else {
                s.clone()
            };
            let q = mv(*p);
            match blocks.get(&q) {
                Some((other, j)) if *other != state => {
                    bail!("tile {}: copies {j} and {k} both need {q:?}: {other} vs {state}", tile.label)
                }
                Some(_) => shared += 1,
                None => {
                    blocks.insert(q, (state, k));
                }
            }
        }
        if keeps_lever {
            levers.push(mv(lever));
        }
        for s in &openings {
            specs.push(DoorSpec { door: mv(s.door), lever: mv(lever), along_z: false, size: s.size });
        }
    }
    // The end piece: placed once, in copy 0's frame; its lever (if any) is THE lever.
    if let Some(e) = &tile.extra {
        let mv0 = |p: Pos| [p[0] + o0[0], p[1] + o0[1], p[2] + o0[2]];
        for (p, s) in &e.blocks {
            let q = mv0(*p);
            match blocks.get(&q) {
                Some((other, j)) if other != s => bail!("tile {}: the end piece and copy {j} both need {q:?}: {s} vs {other}", tile.label),
                Some(_) => shared += 1,
                None => {
                    blocks.insert(q, (s.clone(), usize::MAX));
                }
            }
        }
        if let Some(l) = extra_lever {
            if tile.levers == LeverMode::One {
                levers.insert(0, mv0(l));
            }
        }
    }
    ensure!(!levers.is_empty(), "tile {}: no lever", tile.label);
    // Copy 0's lever is the lever of every opening.
    let first_lever = levers[0];
    for s in specs.iter_mut() {
        s.lever = first_lever;
    }
    // Openings may not overlap (stack:N and wide:N touch, they do not overlap).
    for (i, a) in specs.iter().enumerate() {
        for b in specs.iter().skip(i + 1) {
            let ca = a.door_cells();
            if b.door_cells().iter().any(|p| ca.contains(p)) {
                bail!("tile {}: two openings overlap ({:?} and {:?})", tile.label, a.door, b.door);
            }
        }
    }
    let mut design = Design {
        name: format!("{} x{}", module.name, tile.copies),
        meta: module.meta.clone(),
        ports: BTreeMap::new(),
        blocks: blocks.into_iter().map(|(p, (s, _))| (p, s)).collect(),
        // Tiling copies blocks only: a module with entities is not tiled (yet).
        entities: Vec::new(),
        block_entities: BTreeMap::new(),
        ..Default::default()
    };
    design.meta.insert("tile".into(), tile.label.clone());
    design.ports.insert("lever".into(), first_lever);
    design.ports.insert("door".into(), specs[0].door);
    for (i, s) in specs.iter().enumerate().skip(1) {
        design.ports.insert(format!("door{}", i + 1), s.door);
    }
    Ok(Tiled { design, specs, levers, copies: tile.copies, shared })
}

/// Blocks of the arrangement (piston heads excluded, as the judge counts).
pub fn tiled_blocks(t: &Tiled) -> usize {
    crate::block_count(&t.design)
}

/// The judge's watch box for several openings: the build, every opening and walkway, plus 2.
fn watch_box_multi(d: &Design, specs: &[DoorSpec]) -> (Pos, Pos) {
    let (mut lo, mut hi) = d.bounds().unwrap();
    for spec in specs {
        for p in spec.door_cells().iter().chain(spec.corridor_cells().iter()) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    ([lo[0] - 2, lo[1] - 2, lo[2] - 2], [hi[0] + 2, hi[1] + 2, hi[2] + 2])
}

/// Sandbox chunks covering the arrangement at every judge position (plus a margin).
pub fn sandbox_for(d: &Design, specs: &[DoorSpec], offsets: &[Pos]) -> SandboxOptions {
    let (lo, hi) = watch_box_multi(d, specs);
    let (mut x0, mut z0, mut x1, mut z1) = (lo[0], lo[2], hi[0], hi[2]);
    for o in offsets {
        x0 = x0.min(lo[0] + o[0]);
        z0 = z0.min(lo[2] + o[2]);
        x1 = x1.max(hi[0] + o[0]);
        z1 = z1.max(hi[2] + o[2]);
    }
    let c = |v: i32| v.div_euclid(16);
    SandboxOptions { chunks: [c(x0 - 16).min(-2), c(z0 - 16).min(-2), c(x1 + 16).max(1), c(z1 + 16).max(1)], seed: 0 }
}

/// Does the arrangement fit the default sandbox (chunks -2..1, x and z in -32..31) at every
/// position? The search's reused sandboxes have that size.
fn fits_default_sandbox(d: &Design, specs: &[DoorSpec], offsets: &[Pos]) -> bool {
    let (lo, hi) = watch_box_multi(d, specs);
    offsets.iter().all(|o| lo[0] + o[0] >= -32 && hi[0] + o[0] <= 31 && lo[2] + o[2] >= -32 && hi[2] + o[2] <= 31)
}

/// Per-opening timing of the first position: (open, close) ticks, worst cycle.
pub type OpeningTicks = Vec<(u32, u32)>;

/// The judge on a tiled module, in an existing sandbox of the default size (the search path).
/// An arrangement too wide for it runs in a larger sandbox kept per worker.
pub fn verify_tiled_in(w: &mut Worker, sb: u32, module: &Design, tile: &Tile, opts: &VerifyOptions) -> Result<(Report, OpeningTicks)> {
    let t = expand(module, tile)?;
    if fits_default_sandbox(&t.design, &t.specs, &opts.offsets) {
        return verify_multi_in(w, sb, &t.design, &t.specs, &t.levers, opts);
    }
    let want = sandbox_for(&t.design, &t.specs, &opts.offsets);
    let big = SandboxOptions { chunks: [-5, -5, 4, 4], seed: 0 };
    let fits_big = want.chunks[0] >= big.chunks[0] && want.chunks[1] >= big.chunks[1] && want.chunks[2] <= big.chunks[2] && want.chunks[3] <= big.chunks[3];
    if !fits_big {
        let own = w.create(&want)?;
        let r = verify_multi_in(w, own, &t.design, &t.specs, &t.levers, opts);
        let _ = w.drop_sandbox(own);
        return r;
    }
    let pid = w.pid();
    let cached = BIG_SANDBOXES.lock().unwrap().get(&pid).copied();
    let bsb = match cached {
        Some(b) => b,
        None => {
            let b = w.create(&big)?;
            BIG_SANDBOXES.lock().unwrap().insert(pid, b);
            b
        }
    };
    verify_multi_in(w, bsb, &t.design, &t.specs, &t.levers, opts)
}

/// Large sandboxes for wide arrangements, one per worker JVM (by process id).
static BIG_SANDBOXES: std::sync::Mutex<BTreeMap<u32, u32>> = std::sync::Mutex::new(BTreeMap::new());

/// The judge on a tiled module in a fresh sandbox sized for the arrangement.
pub fn verify_tiled(w: &mut Worker, module: &Design, tile: &Tile, opts: &VerifyOptions) -> Result<(Report, OpeningTicks, Tiled)> {
    let t = expand(module, tile)?;
    let sb = w.create(&sandbox_for(&t.design, &t.specs, &opts.offsets))?;
    let r = verify_multi_in(w, sb, &t.design, &t.specs, &t.levers, opts);
    let _ = w.drop_sandbox(sb);
    let (rep, ticks) = r?;
    Ok((rep, ticks, t))
}

/// The tiled arrangement turned `turns` quarter turns (game states turned by the game).
pub fn rotate_tiled(w: &mut Worker, t: &Tiled, turns: u8) -> Result<Tiled> {
    let turns = turns % 4;
    let states: Vec<String> = t.design.blocks.values().cloned().collect();
    let turned = w.rotate_states(&states, turns)?;
    let blocks = t.design.blocks.keys().zip(turned).map(|(p, s)| (crate::rotate_pos(*p, turns), s)).collect();
    let rot_spec = |s: &DoorSpec| {
        let cells: Vec<Pos> = s.door_cells().iter().map(|p| crate::rotate_pos(*p, turns)).collect();
        let min = |i: usize| cells.iter().map(|p| p[i]).min().unwrap();
        DoorSpec { door: [min(0), min(1), min(2)], lever: crate::rotate_pos(s.lever, turns), along_z: (turns % 2 == 1) != s.along_z, size: s.size }
    };
    let specs: Vec<DoorSpec> = t.specs.iter().map(rot_spec).collect();
    let levers: Vec<Pos> = t.levers.iter().map(|p| crate::rotate_pos(*p, turns)).collect();
    let mut design = Design { name: t.design.name.clone(), meta: t.design.meta.clone(), ports: BTreeMap::new(), blocks, entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() };
    design.ports.insert("lever".into(), levers[0]);
    design.ports.insert("door".into(), specs[0].door);
    Ok(Tiled { design, specs, levers, copies: t.copies, shared: t.shared })
}

/// Rotation check of a tiled arrangement (not part of the judge): `None` = works turned 90, 180
/// and 270 degrees; `Some((turns, why))` = the first turn that breaks it.
pub fn tiled_rotation_check(w: &mut Worker, t: &Tiled, opts: &VerifyOptions) -> Result<Option<(u8, String)>> {
    for turns in 1..4u8 {
        let r = rotate_tiled(w, t, turns)?;
        let sb = w.create(&sandbox_for(&r.design, &r.specs, &opts.offsets))?;
        let v = verify_multi_in(w, sb, &r.design, &r.specs, &r.levers, opts);
        let _ = w.drop_sandbox(sb);
        if let Err(e) = v {
            return Ok(Some((turns, format!("{e:#}"))));
        }
    }
    Ok(None)
}

/// The judge's checks ([`crate::verify_in_spec`]) generalized to several openings and the levers
/// flipped together: every check runs on every opening.
pub fn verify_multi_in(w: &mut Worker, sb: u32, design: &Design, specs: &[DoorSpec], levers: &[Pos], opts: &VerifyOptions) -> Result<(Report, OpeningTicks)> {
    let illegal = crate::legality_problems(design);
    if !illegal.is_empty() {
        bail!("illegal design: {}", illegal.join("; "));
    }
    ensure!(!specs.is_empty() && !levers.is_empty(), "no opening or no lever");
    let (lo, hi) = design.bounds().context("empty design")?;
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut report =
        Report { blocks: crate::block_count(design), size, cycles: Vec::new(), open_frames: Vec::new(), close_frames: Vec::new(), warnings: Vec::new() };
    let mut per_opening: OpeningTicks = vec![(0, 0); specs.len()];
    for (i, off) in opts.offsets.iter().enumerate() {
        let d = design.translated(*off);
        let ss: Vec<DoorSpec> = specs.iter().map(|s| s.translated(*off)).collect();
        let ls: Vec<Pos> = levers.iter().map(|p| [p[0] + off[0], p[1] + off[1], p[2] + off[2]]).collect();
        multi_at(w, sb, &d, &ss, &ls, opts, &mut report, &mut per_opening, i == 0).with_context(|| format!("at offset {off:?}"))?;
    }
    Ok((report, per_opening))
}

#[allow(clippy::too_many_arguments)]
fn multi_at(
    w: &mut Worker,
    sb: u32,
    d: &Design,
    specs: &[DoorSpec],
    levers: &[Pos],
    opts: &VerifyOptions,
    report: &mut Report,
    per_opening: &mut OpeningTicks,
    first_position: bool,
) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, opts.max_ticks)?;
    if settle.idle != Some(true) {
        bail!("the build never settles after placement ({} ticks)", settle.ticked);
    }
    let (wlo, whi) = watch_box_multi(d, specs);
    let rest = w.get_region(sb, wlo, whi)?;
    let (floating, ticking) = w.static_problems(sb, wlo, whi)?;
    if !floating.is_empty() {
        let list: Vec<String> = floating.iter().map(|p| format!("{p:?} {}", rest.get(*p).unwrap_or("?"))).collect();
        bail!("unsupported blocks (they could not stand there in survival): {}", list.join("; "));
    }
    if !ticking.is_empty() {
        let list: Vec<String> = ticking.iter().map(|p| format!("{p:?} {}", rest.get(*p).unwrap_or("?"))).collect();
        bail!("blocks that change by themselves over time (random ticks, e.g. unwaxed copper oxidizes; use waxed): {}", list.join("; "));
    }
    for (p, s) in &blocks {
        if rest.get(*p) != Some(s.as_str()) && first_position {
            report.warnings.push(format!("not at rest as written: {p:?} {s} became {}", rest.get(*p).unwrap_or("?")));
        }
    }
    for (k, spec) in specs.iter().enumerate() {
        let problems = closed_problems(&rest, spec);
        if !problems.is_empty() {
            bail!("opening {k}: not closed at rest: {}", problems.join("; "));
        }
    }
    if opts.empty_walkway {
        let reach = (whi[0] - wlo[0]).max(whi[2] - wlo[2]).max(whi[1] - wlo[1]) + 1;
        for (k, spec) in specs.iter().enumerate() {
            let clutter: Vec<String> = spec
                .in_front_and_behind(reach)
                .iter()
                .filter_map(|p| rest.get(*p).filter(|s| !mctas::blockstate::is_air(s)).map(|s| format!("{p:?} {s}")))
                .collect();
            if !clutter.is_empty() {
                bail!(
                    "opening {k}: blocks in front of or behind the door (nothing may stand in the 3x3 columns in front of and behind the door, at any distance, at rest: not even dust, torches, rails or levers): {}",
                    clutter.join("; ")
                );
            }
        }
    }
    for l in levers {
        let lever = rest.get(*l).unwrap_or("?");
        if !lever.starts_with("minecraft:lever") {
            bail!("lever {l:?} is {lever}, expected a lever");
        }
    }
    let closed: Vec<Vec<String>> = specs.iter().map(|s| s.door_cells().iter().map(|p| rest.get(*p).unwrap().to_string()).collect()).collect();
    let all_open = |f: &Region| specs.iter().all(|s| open_problems(f, s).is_empty());
    let all_closed = |f: &Region| {
        specs.iter().zip(closed.iter()).all(|(s, c)| s.door_cells().iter().zip(c.iter()).all(|(p, st)| f.get(*p) == Some(st.as_str())))
    };
    // Periodic rest, like the judge (96a62d6, vjeux: internal state doesn't matter as long as the
    // door works repeatedly): every close restores every opening's door blocks, and some closed
    // state repeats an earlier one within cycles + MAX_PERIOD_EXTRA.
    let mut seen_closed: Vec<Region> = vec![rest.clone()];
    let mut periodic = false;
    for cycle in 0..opts.cycles + crate::MAX_PERIOD_EXTRA {
        if cycle >= opts.cycles && periodic {
            break;
        }
        for l in levers {
            w.use_block(sb, *l)?;
        }
        let open = w.step_record(sb, opts.max_ticks, true, wlo, whi)?;
        if open.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after opening", open.ticked);
        }
        let last = open.frames.last().context("no frames")?;
        for (k, spec) in specs.iter().enumerate() {
            let problems = open_problems(last, spec);
            if !problems.is_empty() {
                bail!("cycle {cycle}: opening {k} not open after the lever was switched on: {}", problems.join("; "));
            }
        }
        let open_ticks = settled_from(&open.frames, all_open).unwrap();
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while opening (something broke): {:?}", w.entities(sb)?);
        }
        for l in levers {
            w.use_block(sb, *l)?;
        }
        let close = w.step_record(sb, opts.max_ticks, true, wlo, whi)?;
        if close.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after closing", close.ticked);
        }
        let last = close.frames.last().context("no frames")?;
        for (k, (spec, c)) in specs.iter().zip(closed.iter()).enumerate() {
            let now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap().to_string()).collect();
            if now != *c {
                bail!(
                    "cycle {cycle}: opening {k}: door not restored: {}",
                    spec.door_cells()
                        .iter()
                        .zip(now.iter().zip(c.iter()))
                        .filter(|(_, (a, b))| a != b)
                        .map(|(p, (a, b))| format!("{p:?} is {a}, was {b}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
        }
        if seen_closed.iter().any(|s| crate::same_region(s, last)) {
            periodic = true;
        } else {
            seen_closed.push(last.clone());
        }
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while closing (something broke)");
        }
        let close_ticks = settled_from(&close.frames, all_closed).unwrap();
        if first_position {
            report.cycles.push(CycleReport { open_ticks, open_settle: open.ticked, close_ticks, close_settle: close.ticked });
            for (k, (spec, c)) in specs.iter().zip(closed.iter()).enumerate() {
                let o = settled_from(&open.frames, |f| open_problems(f, spec).is_empty()).unwrap_or(0);
                let cl = settled_from(&close.frames, |f| spec.door_cells().iter().zip(c.iter()).all(|(p, st)| f.get(*p) == Some(st.as_str()))).unwrap_or(0);
                per_opening[k].0 = per_opening[k].0.max(o);
                per_opening[k].1 = per_opening[k].1.max(cl);
            }
            if opts.keep_frames && cycle == 0 {
                report.open_frames = open.frames;
                report.close_frames = close.frames;
            }
        }
    }
    if !periodic {
        bail!("the build never comes back to a closed state it had before within {} cycles (its internal state drifts, so it may stop working)", opts.cycles + crate::MAX_PERIOD_EXTRA);
    }
    Ok(())
}

/// Blocks per copy once the row is long: blocks(N) - blocks(N - 1) (shared cells counted once).
pub fn marginal_blocks(module: &Design, tile: &Tile) -> Result<usize> {
    let a = tiled_blocks(&expand(module, tile)?);
    if tile.copies < 2 {
        return Ok(a);
    }
    let b = tiled_blocks(&expand(module, &tile.with_copies(tile.copies - 1))?);
    Ok(a.saturating_sub(b))
}

/// Tile specs from a comma list where `period` and `vperiod` may take a range of periods:
/// `wide:3,stack:3,period:4..12:3`.
pub fn parse_tile_list(s: &str) -> Result<Vec<Tile>> {
    let mut out = Vec::new();
    for item in s.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        let parts: Vec<&str> = item.split(':').collect();
        match parts.as_slice() {
            [kind @ ("period" | "vperiod"), range, n] if range.contains("..") => {
                let (a, b) = range.split_once("..").unwrap();
                let (a, b): (i32, i32) = (a.parse()?, b.parse()?);
                for p in a..=b {
                    out.push(Tile::parse(&format!("{kind}:{p}:{n}"))?);
                }
            }
            _ => out.push(Tile::parse(item)?),
        }
    }
    Ok(out)
}

/// `door tile-scan`: every design under every tiling (in parallel); per design and tiling, the
/// verdict. Returns (design index, tile, Ok((total blocks, marginal, open, close)) or the reason).
pub fn scan(
    pool: &mctas::sim::Pool,
    designs: &[(String, Design)],
    tiles: &[Tile],
    opts: &VerifyOptions,
) -> Vec<(usize, Tile, std::result::Result<(usize, usize, u32, u32), String>)> {
    let mut jobs = Vec::new();
    for (i, _) in designs.iter().enumerate() {
        for t in tiles {
            jobs.push((i, t.clone()));
        }
    }
    let results = pool.map(jobs.clone(), |w, _, (i, t)| {
        let d = &designs[i].1;
        let marginal = marginal_blocks(d, &t)?;
        let (r, _, tiled) = verify_tiled(w, d, &t, opts)?;
        let o = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
        let c = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
        Ok((tiled_blocks(&tiled), marginal, o, c))
    });
    jobs.into_iter().zip(results).map(|((i, t), r)| (i, t, r.map_err(|e| format!("{e:#}")))).collect()
}

/// One row of the tile board.
pub struct BoardRow {
    pub source: String,
    pub name: String,
    pub tiling: String,
    pub kind: String,
    /// Translation length (P or Q; 3 for wide/stack).
    pub period: i32,
    pub verdict: std::result::Result<(usize, usize, usize, u32, u32, bool), String>,
}

/// `door tile-board`: every candidate with a `tiling:` header (`tiling: period:8:3`) judged as a
/// tile module (one lever, every opening, `opts` positions, rotation check), ranked per kind by
/// blocks per opening in a long row (blocks(N) - blocks(N-1)), then by period. Markdown.
pub fn board(pool: &mctas::sim::Pool, cands: Vec<(String, Design)>, opts: &VerifyOptions) -> (Vec<BoardRow>, String) {
    let tiled: Vec<(String, Design, Tile)> = cands
        .into_iter()
        .filter_map(|(src, d)| {
            let spec = d.meta.get("tiling")?.clone();
            Tile::parse(&spec).ok().map(|t| (src, d, t))
        })
        .collect();
    let verdicts = pool.map(tiled.clone(), |w, _, (_, d, t)| {
        let marginal = marginal_blocks(&d, &t)?;
        let (r, _, x) = verify_tiled(w, &d, &t, opts)?;
        let o = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
        let c = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
        let safe = tiled_rotation_check(w, &x, opts)?.is_none();
        Ok((marginal, tiled_blocks(&x), x.specs.len(), o, c, safe))
    });
    let mut rows: Vec<BoardRow> = tiled
        .into_iter()
        .zip(verdicts)
        .map(|((source, d, t), v)| BoardRow {
            source,
            name: d.name.clone(),
            kind: t.label.split(':').next().unwrap_or("").to_string(),
            period: t.step.iter().map(|v| v.abs()).max().unwrap_or(0),
            tiling: t.label.clone(),
            verdict: v.map_err(|e| format!("{e:#}")),
        })
        .collect();
    rows.sort_by_key(|r| (r.kind.clone(), r.verdict.as_ref().map(|v| v.0).unwrap_or(usize::MAX), r.period, r.source.clone()));
    let mut s = String::from(
        "# Tileable 3x3 piston doors\n\nEvery design with a `tiling:` header on main and the `explore/*` branches, judged by `door verify --tile` \
         (copies of one module, ONE lever, every judge check on every opening, 32 positions). Ranked by blocks per opening in a long row \
         (blocks(N) - blocks(N-1)), then by period. Definitions: designs/piston-door-3x3/tracks/tile/NOTES.md.\n\n",
    );
    let kinds = [("period", "PERIODIC: an opening every P blocks along the wall"), ("vperiod", "VPERIOD: a door every Q blocks up"), ("wide", "WIDE: openings side by side, one 3 x 3N opening"), ("stack", "STACK: openings stacked, one 3N x 3 opening"), ("step", "Other translations")];
    for (k, title) in kinds {
        let ok: Vec<&BoardRow> = rows.iter().filter(|r| r.kind == k && r.verdict.is_ok()).collect();
        s.push_str(&format!("## {title}\n\n"));
        if ok.is_empty() {
            s.push_str("No working design yet.\n\n");
            continue;
        }
        s.push_str("| # | blocks per opening | period | tested | open / close | rotations | source |\n|---|---|---|---|---|---|---|\n");
        for (i, r) in ok.iter().enumerate() {
            let (m, total, n, o, c, safe) = r.verdict.as_ref().unwrap();
            s.push_str(&format!(
                "| {} | {m} | {} | {n} openings, {total} blocks | {o} / {c} | {} | `{}` |\n",
                i + 1,
                r.period,
                if *safe { "safe" } else { "NOT safe" },
                r.source
            ));
        }
        s.push('\n');
    }
    let broken: Vec<&BoardRow> = rows.iter().filter(|r| r.verdict.is_err()).collect();
    if !broken.is_empty() {
        s.push_str("## Broken\n\n| tiling | source | why |\n|---|---|---|\n");
        for r in broken {
            let why: String = r.verdict.as_ref().err().unwrap().chars().take(160).collect();
            s.push_str(&format!("| {} | `{}` | {} |\n", r.tiling, r.source, why.replace('|', "/")));
        }
    }
    (rows, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> Design {
        Design::parse(
            "name: m\nport door: 0 0 0\nport lever: 1 4 0\nrelay: minecraft:redstone_wire\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\nlayer 0:\n  SSS\nlayer 1:\n  SSS\nlayer 2:\n  SSS\nlayer 3:\n  SSSS\nlayer 4:\n  .L\n",
        )
        .unwrap()
    }

    #[test]
    fn parse_specs() {
        assert_eq!(Tile::parse("wide:4").unwrap().step, [3, 0, 0]);
        assert_eq!(Tile::parse("period:7:3").unwrap().step, [7, 0, 0]);
        assert_eq!(Tile::parse("stack:2").unwrap().step, [0, 3, 0]);
        assert_eq!(Tile::parse("vperiod:5:2").unwrap().step, [0, 5, 0]);
        assert_eq!(Tile::parse("step:0,4,0:2").unwrap().step, [0, 4, 0]);
        assert!(Tile::parse("wide").is_err());
        assert!(Tile::parse("step:0,0,0:2").is_err());
    }

    #[test]
    fn expand_wide() {
        let m = module();
        let t = expand(&m, &Tile::parse("wide:3").unwrap()).unwrap();
        assert_eq!(t.specs.len(), 3);
        assert_eq!(t.levers.len(), 1);
        // 13 stone per copy, the 4th stone of layer 3 shared with the next copy's first.
        assert_eq!(t.shared, 2);
        let wires = t.design.blocks.values().filter(|s| s.contains("redstone_wire")).count();
        assert_eq!(wires, 2);
        assert_eq!(t.design.blocks.values().filter(|s| s.contains("lever")).count(), 1);
        assert_eq!(marginal_blocks(&m, &Tile::parse("wide:3").unwrap()).unwrap(), 13);
    }

    #[test]
    fn conflicts() {
        let m = module();
        // Period 2: the openings overlap.
        assert!(expand(&m, &Tile::parse("period:2:2").unwrap()).is_err());
    }

    #[test]
    fn end_piece_lever() {
        let m = module();
        let extra = Design::parse(
            "name: e\nport lever: -2 4 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\nlayer 3:\n  S\nlayer 4:\n  L\n",
        )
        .unwrap()
        .translated([-2, 0, 0]);
        let mut t = Tile::parse("wide:2").unwrap();
        t.extra = Some(Design { ports: [("lever".to_string(), [-2, 4, 0])].into_iter().collect(), ..extra });
        let x = expand(&m, &t).unwrap();
        // The end piece's lever is the only lever; both copies hold the relay.
        assert_eq!(x.levers, vec![[-2, 4, 0]]);
        assert_eq!(x.design.blocks.values().filter(|s| s.contains("lever")).count(), 1);
        assert_eq!(x.design.blocks.values().filter(|s| s.contains("redstone_wire")).count(), 2);
    }
}
