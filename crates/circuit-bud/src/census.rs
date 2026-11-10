//! `mctas circuit bud census`: every design of K blocks around a target cell, screened on redsim
//! with the BUD judge (crates/circuit/src/bud.rs, classes only: no footprint probes, no re-arm
//! pairs) at every air cell next to it as the output. The designs are connected (each block beside
//! the target or another block), have a block beside the target and an active component, and are
//! deduplicated by structure (the type key: translation, rotation, mirror). Per detected class set:
//! how many designs, the smallest and the fastest; `--out-dir` writes them for the real judge
//! (`mctas circuit verify` / `archive`), which adds the lamp at the output, the footprint and the
//! re-arm time.

use anyhow::{Result, bail};
use circuit::check::Sizing;
use circuit::stimulus::SETTLE;
use circuit::{Circuit, Pos};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(clap::Args, Debug)]
pub struct CensusArgs {
    /// Blocks per design.
    #[arg(long, default_value_t = 2)]
    pub k: usize,
    /// `all` (pistons, observers, droppers, dispensers in every facing, redstone block, slime,
    /// honey, stone, glass, lamp, dust, torches, repeaters, comparators), `qc` (observer-free piston
    /// parts: pistons, slime, honey, redstone block, stone), `dropper` (droppers, dispensers,
    /// observers, sticky pistons, redstone block, stone, slime) or a `;`-separated list of states.
    #[arg(long, default_value = "all")]
    pub palette: String,
    /// Cells within this distance (per axis) of the target.
    #[arg(long, default_value_t = 2)]
    pub reach: i32,
    /// Worker threads.
    #[arg(long, default_value_t = 6)]
    pub threads: usize,
    /// Only count the designs.
    #[arg(long)]
    pub count: bool,
    /// The frames each event is given.
    #[arg(long, default_value_t = 30)]
    pub gap: u32,
    /// Write the fastest designs of each class set here (`--keep` each).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// How many designs of each class set `--out-dir` gets (fastest first).
    #[arg(long, default_value_t = 3)]
    pub keep: usize,
    /// Only class sets with this detected-classes text (`place remove state update`).
    #[arg(long)]
    pub only: Option<String>,
    /// A base design (its target at 0 0 0) whose blocks every design keeps: the census then adds K
    /// blocks in the air cells of its box grown by 1 (every K-subset, no symmetry reduction).
    #[arg(long)]
    pub base: Option<PathBuf>,
    /// Also find each passing design's densest tiling (`tile:` judged on redsim, 3 copies, pitch
    /// 1..=N along x, y, z); class sets list their densest tileable designs first.
    #[arg(long, default_value_t = 0)]
    pub tile: u32,
}

const FACINGS: [&str; 6] = ["north", "south", "east", "west", "up", "down"];
const HFACINGS: [&str; 4] = ["north", "south", "east", "west"];

fn palette(name: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    match name {
        "dropper" => {
            for f in FACINGS {
                v.push(format!("minecraft:dropper[facing={f},triggered=false]"));
                v.push(format!("minecraft:dispenser[facing={f},triggered=false]"));
                v.push(format!("minecraft:observer[facing={f},powered=false]"));
                v.push(format!("minecraft:sticky_piston[extended=false,facing={f}]"));
            }
            for b in ["redstone_block", "stone", "slime_block"] {
                v.push(format!("minecraft:{b}"));
            }
        }
        "all" | "qc" => {
            for f in FACINGS {
                v.push(format!("minecraft:sticky_piston[extended=false,facing={f}]"));
                v.push(format!("minecraft:piston[extended=false,facing={f}]"));
            }
            for b in ["redstone_block", "slime_block", "honey_block", "stone"] {
                v.push(format!("minecraft:{b}"));
            }
            if name == "all" {
                for f in FACINGS {
                    v.push(format!("minecraft:observer[facing={f},powered=false]"));
                    v.push(format!("minecraft:dropper[facing={f},triggered=false]"));
                    v.push(format!("minecraft:dispenser[facing={f},triggered=false]"));
                }
                v.push("minecraft:glass".into());
                v.push("minecraft:redstone_lamp[lit=false]".into());
                v.push("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into());
                v.push("minecraft:redstone_torch[lit=true]".into());
                for f in HFACINGS {
                    v.push(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
                    v.push(format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]"));
                    v.push(format!("minecraft:comparator[facing={f},mode=compare,powered=false]"));
                }
            }
        }
        list => v.extend(list.split([';', ' ']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
    }
    v
}

/// Blocks that react to updates (a design without one cannot sense anything).
fn active(s: &str) -> bool {
    ["piston", "observer", "dropper", "dispenser", "lamp", "redstone_wire", "torch", "repeater", "comparator"].iter().any(|k| s.contains(k))
}

fn nbrs(p: Pos) -> [Pos; 6] {
    [[p[0] + 1, p[1], p[2]], [p[0] - 1, p[1], p[2]], [p[0], p[1] + 1, p[2]], [p[0], p[1] - 1, p[2]], [p[0], p[1], p[2] + 1], [p[0], p[1], p[2] - 1]]
}

/// The 8 symmetries around the vertical axis through the target (4 turns x mirror).
fn sym(p: Pos, g: usize) -> Pos {
    let [mut x, y, mut z] = p;
    if g >= 4 {
        x = -x;
    }
    for _ in 0..(g % 4) {
        (x, z) = (-z, x);
    }
    [x, y, z]
}

/// Whether a sorted cell set is the smallest of its 8 images (one representative per symmetry
/// class: the palettes hold every facing, so the states cover the other images).
fn canonical(s: &[Pos]) -> bool {
    (1..8).all(|g| {
        let mut img: Vec<Pos> = s.iter().map(|p| sym(*p, g)).collect();
        img.sort();
        s <= img.as_slice()
    })
}

/// Connected cell sets of `k` cells around the target (origin), one of them beside it, one per
/// symmetry class.
fn cell_sets(k: usize, reach: i32) -> Vec<Vec<Pos>> {
    let t: Pos = [0, 0, 0];
    let ok = |p: &Pos| *p != t && p.iter().all(|c| c.abs() <= reach);
    let mut sets: BTreeSet<Vec<Pos>> = nbrs(t).iter().filter(|p| ok(p)).map(|p| vec![*p]).collect();
    for _ in 1..k {
        let mut next = BTreeSet::new();
        for s in &sets {
            let mut cand: BTreeSet<Pos> = nbrs(t).into_iter().collect();
            for p in s {
                cand.extend(nbrs(*p));
            }
            for q in cand {
                if ok(&q) && !s.contains(&q) {
                    let mut n = s.clone();
                    n.push(q);
                    n.sort();
                    next.insert(n);
                }
            }
        }
        sets = next;
    }
    sets.into_iter().filter(|s| canonical(s)).collect()
}

fn design(cells: &[Pos], states: &[&String], out: Option<Pos>) -> mctas::design::Design {
    let mut d = mctas::design::Design { name: "bud-census".into(), meta: BTreeMap::new(), ports: BTreeMap::new(), blocks: BTreeMap::new(), entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() };
    d.ports.insert("target".into(), [0, 0, 0]);
    for (p, s) in cells.iter().zip(states) {
        d.blocks.insert(*p, (*s).clone());
    }
    if let Some(o) = out {
        d.meta.insert("circuit".into(), "bud".into());
        d.ports.insert("output".into(), o);
    }
    d
}

/// One passing (design, output) of the screen.
#[derive(Clone)]
struct Hit {
    classes: String,
    latency: String,
    pulse: String,
    text: String,
    key: String,
    /// `--tile`: the densest tiling found (pitch, step).
    tile: Option<(u32, Pos)>,
}

fn lat_num(s: &str) -> u32 {
    s.split(|c: char| !c.is_ascii_digit()).filter(|w| !w.is_empty()).filter_map(|w| w.parse().ok()).max().unwrap_or(999)
}

/// Screen one design at every candidate output cell.
fn screen(d: mctas::design::Design, gap: u32, tile_max: u32, only: Option<&str>) -> Vec<Hit> {
    let Ok(c) = Circuit::from_design(d.clone()) else { return Vec::new() };
    let s = circuit::bud::stimulus_with(&c, &Sizing { gap }, false);
    let actions = c.actions(&s);
    let (from, to) = circuit::run::watch_box(&c, 2);
    match clock::sim::unsupported(&c.clock, from, to) {
        Ok(v) if v.is_empty() => {}
        _ => return Vec::new(),
    }
    // the candidate outputs: air cells beside any cell a block takes during the run (a moving
    // block can drive a cell nothing touches at rest), from a first run without probes
    let Ok(first) = clock::sim::drive(&c.clock, &actions, s.ticks, &[], from, to) else { return Vec::new() };
    if first.stopped.is_some() {
        return Vec::new();
    }
    let taken: BTreeSet<Pos> = first.cells.iter().flat_map(|f| f.iter().map(|(p, _)| *p)).collect();
    let target: Pos = [0, 0, 0];
    let mut cand: BTreeSet<Pos> = BTreeSet::new();
    for p in &taken {
        for q in nbrs(*p) {
            if q != target && !d.blocks.contains_key(&q) && (0..3).all(|i| q[i] > from[i] && q[i] < to[i]) {
                cand.insert(q);
            }
        }
    }
    let probes: Vec<Pos> = cand.into_iter().collect();
    let Ok(run) = clock::sim::drive(&c.clock, &actions, s.ticks, &probes, from, to) else { return Vec::new() };
    if run.stopped.is_some() {
        return Vec::new();
    }
    let rest = (c.clock.free_from() + SETTLE / 2) as usize;
    let mut hits = Vec::new();
    for (k, p) in probes.iter().enumerate() {
        let out: Vec<bool> = run.signals.iter().map(|v| v.get(k).copied().unwrap_or(0) > 0).collect();
        let o = circuit::bud::judge(&out, &s, rest, 1, &[]);
        if !o.pass {
            continue;
        }
        let m = |name: &str| o.metrics.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()).unwrap_or_default();
        let cells: Vec<Pos> = d.blocks.keys().copied().collect();
        let states: Vec<&String> = d.blocks.values().collect();
        let dd = design(&cells, &states, Some(*p));
        let Ok(cc) = Circuit::from_design(dd.clone()) else { continue };
        // `--only`: tile-test only that class set (the others are many and known)
        let tile = if tile_max > 0 && only.is_none_or(|o| o == m("detects")) { best_tile(&dd, &o, gap, tile_max) } else { None };
        hits.push(Hit { classes: m("detects"), latency: m("latency"), pulse: m("pulse"), text: dd.to_text(), key: circuit::archive::type_key(&cc), tile });
    }
    hits
}

/// The densest tiling of a (design, output) the judge accepts on redsim (`bud::tile_check`, 3
/// copies): the smallest pitch 1..=max along x, y or z, and its step.
fn best_tile(d: &mctas::design::Design, base: &circuit::check::Outcome, gap: u32, max: u32) -> Option<(u32, Pos)> {
    for pitch in 1..=max as i32 {
        for step in [[pitch, 0, 0], [0, pitch, 0], [0, 0, pitch]] {
            let mut dt = d.clone();
            dt.meta.insert("tile".into(), format!("{} {} {}", step[0], step[1], step[2]));
            let Ok(c) = Circuit::from_design(dt) else { continue };
            if let Ok(out) = circuit::bud::tile_check(&c, base, gap, "", |tc, ts| Ok(circuit::run::sim_trace(tc, ts, 2)))
                && out.pass
            {
                return Some((pitch as u32, step));
            }
        }
    }
    None
}

pub fn run(a: CensusArgs) -> Result<()> {
    if a.k == 0 {
        bail!("--k must be at least 1");
    }
    let pal = palette(&a.palette);
    let base: Vec<(Pos, String)> = match &a.base {
        Some(p) => mctas::design::Design::load(p)?.blocks.into_iter().collect(),
        None => Vec::new(),
    };
    let sets = if base.is_empty() {
        cell_sets(a.k, a.reach)
    } else {
        // every K-subset of the air cells in the base's box grown by 1
        let mut lo = [0i32; 3];
        let mut hi = [0i32; 3];
        for (p, _) in &base {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i] - 1);
                hi[i] = hi[i].max(p[i] + 1);
            }
        }
        let taken: BTreeSet<Pos> = base.iter().map(|(p, _)| *p).collect();
        let mut cells: Vec<Pos> = Vec::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let p = [x, y, z];
                    if p != [0, 0, 0] && !taken.contains(&p) {
                        cells.push(p);
                    }
                }
            }
        }
        let mut out: Vec<Vec<Pos>> = Vec::new();
        let mut idx: Vec<usize> = (0..a.k).collect();
        if a.k <= cells.len() {
            loop {
                out.push(idx.iter().map(|i| cells[*i]).collect());
                let mut j = a.k;
                while j > 0 && idx[j - 1] == cells.len() - a.k + j - 1 {
                    j -= 1;
                }
                if j == 0 {
                    break;
                }
                idx[j - 1] += 1;
                for m in j..a.k {
                    idx[m] = idx[m - 1] + 1;
                }
            }
        }
        out
    };
    // per cell set (one per symmetry class: two sets never hold the same structure), every
    // assignment of palette states, deduplicated by structure within the set (a symmetric set
    // repeats some), screened at once: the threads take cell sets in turn
    let next = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);
    let distinct = AtomicUsize::new(0);
    let hits: Mutex<Vec<Hit>> = Mutex::new(Vec::new());
    let n = sets.len();
    std::thread::scope(|sc| {
        for _ in 0..a.threads.max(1) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let cells = &sets[i];
                    let mut seen: HashSet<String> = HashSet::new();
                    let mut idx = vec![0usize; a.k];
                    let mut found: Vec<Hit> = Vec::new();
                    loop {
                        let states: Vec<&String> = idx.iter().map(|i| &pal[*i]).collect();
                        if !base.is_empty() || states.iter().any(|s| active(s)) {
                            total.fetch_add(1, Ordering::Relaxed);
                            let mut d = design(cells, &states, None);
                            for (p, s) in &base {
                                d.blocks.insert(*p, s.clone());
                            }
                            let mut dk = d.clone();
                            dk.blocks.insert([0, 0, 0], "circuit:target".into());
                            if seen.insert(clock::canon::design_type_key(&dk)) {
                                distinct.fetch_add(1, Ordering::Relaxed);
                                if !a.count {
                                    found.extend(screen(d, a.gap, a.tile, a.only.as_deref()));
                                }
                            }
                        }
                        // next assignment
                        let mut j = 0;
                        while j < a.k {
                            idx[j] += 1;
                            if idx[j] < pal.len() {
                                break;
                            }
                            idx[j] = 0;
                            j += 1;
                        }
                        if j == a.k {
                            break;
                        }
                    }
                    if !found.is_empty() {
                        hits.lock().unwrap().extend(found);
                    }
                    eprintln!("  cell set {}/{n} done ({} structures so far)", i + 1, distinct.load(Ordering::Relaxed));
                }
            });
        }
    });
    println!("census k={} palette {} ({} states), reach {}: {} cell sets, {} designs, {} distinct structures", a.k, a.palette, pal.len(), a.reach, sets.len(), total.load(Ordering::Relaxed), distinct.load(Ordering::Relaxed));
    if a.count {
        return Ok(());
    }
    let hits = hits.into_inner().unwrap();
    // per class set: distinct structures (design + output), the fastest
    let mut by: BTreeMap<String, Vec<Hit>> = BTreeMap::new();
    let mut keys: HashSet<String> = HashSet::new();
    for h in hits {
        if keys.insert(h.key.clone()) {
            by.entry(h.classes.clone()).or_default().push(h);
        }
    }
    println!("{} passing (design, output) structures in {} class sets", keys.len(), by.len());
    for (cl, v) in &mut by {
        v.sort_by_key(|h| (lat_num(&h.latency), lat_num(&h.pulse), h.text.len()));
        let best = &v[0];
        println!("  detects {cl}: {} structures; fastest latency {} (pulse {})", v.len(), best.latency, best.pulse);
        if a.tile > 0 {
            // the densest tilings first, then the fastest
            v.sort_by_key(|h| (h.tile.map(|t| t.0).unwrap_or(u32::MAX), lat_num(&h.latency), lat_num(&h.pulse), h.text.len()));
            let n = v.iter().filter(|h| h.tile.is_some()).count();
            match v[0].tile {
                Some((p, st)) => println!("    tileable: {n} structures; densest pitch {p} (step {} {} {}), latency {}", st[0], st[1], st[2], v[0].latency),
                None => println!("    tileable: none up to pitch {}", a.tile),
            }
        }
        if let Some(dir) = &a.out_dir {
            if a.only.as_ref().is_some_and(|o| o != cl) {
                continue;
            }
            std::fs::create_dir_all(dir)?;
            for (j, h) in v.iter().take(a.keep).enumerate() {
                let name = format!("census-k{}-{}-{}", a.k, cl.replace(' ', "-"), j + 1);
                let mut text = h.text.replacen("name: bud-census", &format!("name: {name}"), 1);
                if let Some((_, st)) = h.tile {
                    text = text.replacen("circuit: bud", &format!("circuit: bud\ntile: {} {} {}", st[0], st[1], st[2]), 1);
                }
                std::fs::write(dir.join(format!("{name}.txt")), text)?;
            }
        }
    }
    Ok(())
}

#[derive(clap::Args, Debug)]
pub struct TraceArgs {
    pub design: PathBuf,
    /// First frame shown (default: the first event).
    #[arg(long)]
    pub from: Option<u32>,
    /// Frames shown.
    #[arg(long, default_value_t = 40)]
    pub frames: u32,
    /// The frames each event is given.
    #[arg(long, default_value_t = 30)]
    pub gap: u32,
    /// The judge's whole stimulus (footprint probes and re-arm pairs too).
    #[arg(long)]
    pub full: bool,
    /// Start the window at the first phase whose label starts with this (`rearm update 2`).
    #[arg(long)]
    pub phase: Option<String>,
}

/// Print a BUD design's run on redsim, frame by frame, in a window.
pub fn trace(a: TraceArgs) -> Result<()> {
    let c = Circuit::load(&a.design)?;
    let s = circuit::bud::stimulus_with(&c, &Sizing { gap: a.gap }, a.full);
    let actions = c.actions(&s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let (from, to) = circuit::run::watch_box(&c, 2);
    let run = clock::sim::drive(&c.clock, &actions, s.ticks, &probes, from, to).map_err(|(_, why)| anyhow::anyhow!(why))?;
    if let Some((t, why)) = &run.stopped {
        println!("stopped at frame {t}: {why}");
    }
    let by_phase = a.phase.as_ref().and_then(|p| s.phases.iter().find(|(_, _, l)| l.starts_with(p.as_str())).map(|ph| ph.0));
    let start = a.from.or(by_phase).unwrap_or_else(|| s.phases.first().map(|p| p.0).unwrap_or(0)).saturating_sub(1) as usize;
    let end = (start + a.frames as usize).min(run.cells.len());
    let mut prev: BTreeMap<Pos, String> = run.cells.get(start.saturating_sub(1)).map(|f| f.iter().cloned().collect()).unwrap_or_default();
    for f in start..end {
        let now: BTreeMap<Pos, String> = run.cells[f].iter().cloned().collect();
        let mut diff: Vec<String> = Vec::new();
        for (p, st) in &now {
            if prev.get(p) != Some(st) {
                diff.push(format!("{p:?}={}", clock::short(st)));
            }
        }
        for p in prev.keys() {
            if !now.contains_key(p) {
                diff.push(format!("{p:?}=air"));
            }
        }
        let phase = s.phases.iter().rev().find(|(a0, _, _)| *a0 as usize <= f).map(|(a0, _, l)| format!("{l} +{}", f - *a0 as usize)).unwrap_or_default();
        let lvl = run.signals.get(f).and_then(|v| v.first()).copied().unwrap_or(0);
        println!("{f:5} out {lvl:2} [{phase}] {}", diff.join(" "));
        prev = now;
    }
    Ok(())
}

#[derive(clap::Args, Debug)]
pub struct TileArgs {
    pub designs: Vec<PathBuf>,
    /// The largest pitch tried per axis.
    #[arg(long, default_value_t = 4)]
    pub max: u32,
}

/// Per design: the base judged on redsim (classes only), then the smallest pitch per axis at
/// which 3 copies pass the judge's tile check (`bud::tile_check`).
pub fn tile(a: TileArgs) -> Result<()> {
    for p in &a.designs {
        let mut d = mctas::design::Design::load(p)?;
        d.meta.remove("tile");
        let c = Circuit::from_design(d.clone())?;
        let gap = 30;
        let s = circuit::bud::stimulus_with(&c, &Sizing { gap }, false);
        let base = match circuit::run::sim_trace(&c, &s, 2) {
            Ok(t) => circuit::bud::check(&c, &s, &t),
            Err(why) => {
                println!("{}: {why}", c.name());
                continue;
            }
        };
        if !base.pass {
            println!("{}: FAIL {}", c.name(), base.why);
            continue;
        }
        let mut per: Vec<String> = Vec::new();
        for (axis, unit) in [("x", [1, 0, 0]), ("y", [0, 1, 0]), ("z", [0, 0, 1])] {
            let mut found = None;
            for pitch in 1..=a.max as i32 {
                let step = [unit[0] * pitch, unit[1] * pitch, unit[2] * pitch];
                let mut dt = d.clone();
                dt.meta.insert("tile".into(), format!("{} {} {}", step[0], step[1], step[2]));
                let Ok(ct) = Circuit::from_design(dt) else { continue };
                match circuit::bud::tile_check(&ct, &base, gap, "", |tc, ts| Ok(circuit::run::sim_trace(tc, ts, 2))) {
                    Ok(o) if o.pass => {
                        found = Some(pitch);
                        break;
                    }
                    _ => {}
                }
            }
            per.push(format!("{axis} {}", found.map(|p| p.to_string()).unwrap_or_else(|| format!("> {}", a.max))));
        }
        println!("{}: {} blocks, densest pitch per axis: {}", c.name(), c.blocks(), per.join(", "));
    }
    Ok(())
}
