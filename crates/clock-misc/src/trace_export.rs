//! `mctas clock misc trace-export <archived designs>... [--out-dir designs/clocks/traces]`: record
//! real-game traces of archived clocks for the in-browser viewer (webrender), one JSON Lines file
//! per clock at `<out-dir>/<design stem>.trace` (format agreed with webrender, 2026-10-19):
//!
//! * line 1, the header: `{"trace":1,"name":..,"design":"designs/clocks/archive/<file>","period":P,
//!   "warmup":W,"ticks":N,"box":[[x0,y0,z0],[x1,y1,z1]],"loop":[W,W+P],"output":[x,y,z]}` (output
//!   only when the design has one); ticks = warmup + 2 periods, from the judge's verdict row;
//! * then one line per game tick t = 0..=ticks: `{"t":t,"blocks":[[x,y,z,"state"],..],
//!   "entities":[{"id":0,"type":"minecraft:oak_boat","pos":[x,y,z],"yaw":a,"pitch":b}]}`: at t = 0
//!   every non-air cell of the watched box (the design plus a 3-cell margin), later only the cells
//!   that changed (air included); every entity every tick, with a stable id; positions rounded to
//!   4 decimals, yaw / pitch to 2.
//!
//! Coordinates are the design's own: the build is the judge's first one (rotation 0, position 0),
//! placed and kicked exactly like `clock verify`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use clock::Clock;
use mctas::sim::{SandboxOptions, Worker, WorkerConfig};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug, Clone)]
pub struct ExportArgs {
    /// Archived designs (designs/clocks/archive/<name>.txt).
    pub designs: Vec<PathBuf>,
    /// Where the traces go (default designs/clocks/traces, or designs/clocks/traces/exact with --exact).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Margin of the watched box around the design (the judge's default).
    #[arg(long, default_value_t = 3)]
    pub margin: i32,
    /// Full precision (for redsim's trace-compare gate): positions, motion and rotations as the
    /// game's own doubles / floats (no rounding), plus each entity's "motion" and "net_id" (the
    /// game's network id, which ItemEntity's ticks read).
    #[arg(long)]
    pub exact: bool,
    /// Record this many ticks of any design (not only archived ones: a test scene for redsim);
    /// the header's period and warmup are then 0 (or `--period` / `--warmup`) and "design" is the
    /// path given.
    #[arg(long)]
    pub ticks: Option<u32>,
    /// With `--ticks`: the period the header states (an un-archived design whose loop is known,
    /// e.g. a re-export of a trace that had one).
    #[arg(long, default_value_t = 0, requires = "ticks")]
    pub period: u32,
    /// With `--ticks`: the warmup the header states.
    #[arg(long, default_value_t = 0, requires = "ticks")]
    pub warmup: u32,
    /// Save, unload and load the sandbox's chunks again right after this frame (the clock
    /// judge's chunk-reload check, `clock verify --reload`), to see what a reload changes.
    #[arg(long)]
    pub reload_at: Option<u32>,
    /// Set the sandbox's randomTickSpeed game rule (header `random_tick_speed`): 0 for a scene
    /// with grass, whose random ticks redsim does not model (they draw from the level random at
    /// positions chosen by the level's unseeded randValue).
    #[arg(long)]
    pub random_tick_speed: Option<u32>,
    /// Extra inputs for a test scene (with `--ticks`): `T:use:x,y,z` (a right-click on the block
    /// there), `T:use_entity:x,y,z` (an empty-hand right-click on the item frame in that cell),
    /// `T:set:x,y,z:<state>` (a block placed with updates) or `T:shape_set:x,y,z:<state>` (a block
    /// changed with shape updates only: a crop's growth step, scripted while random ticks are off),
    /// applied before tick T like the design's own kick and listed in the header's `actions`, which
    /// redsim's trace-compare replays.
    #[arg(long = "action", requires = "ticks")]
    pub extra_actions: Vec<String>,
    /// Right-click each design's own `port lever` at these ticks (`T0..T1/STEP` or `T`): a door's
    /// lever flips, like the door judge's open/close cycles, one command for many doors.
    #[arg(long, requires = "ticks")]
    pub lever_flips: Option<String>,
    /// One trace per sandbox seed `A..B` (`<stem>-s<seed>.trace`, header `seed`): another level
    /// random each time (TNT kicks, item drops...), which redsim's trace-compare mirrors.
    #[arg(long)]
    pub seeds: Option<String>,
    /// The design is a circuit: record it as the circuit judge builds and drives it, its consumer
    /// lamps and input drivers placed and `--stimulus` (or the kind's own schedule) applied, for
    /// `--ticks` or the judge's whole schedule; the box is the judge's watched box (a flying
    /// machine's corridor, an extender's reach); the judged build is written next to the trace
    /// (`<stem>-judged.txt`, the header's design) and the input actions go in the header's
    /// `actions` (`set` / `clear` / `use` / `use_entity`), which redsim's trace-compare replays.
    #[arg(long)]
    pub circuit: bool,
    /// With `--circuit`: the stimulus (`circuit sim --stimulus`'s syntax, e.g. `input pulse 4 every 14 x 20`).
    #[arg(long, requires = "circuit")]
    pub stimulus: Option<String>,
    /// With `--circuit`: where the judged build goes (default: the trace's directory).
    #[arg(long, requires = "circuit")]
    pub judged_dir: Option<PathBuf>,
    /// With `--circuit`, a TNT duper: the judge's ground under it (`grass` or `obsidian` layers,
    /// `--reach` blocks around its box, `--drop` air blocks under its lowest layer; default: its
    /// declared stand), part of the judged build.
    #[arg(long, requires = "circuit")]
    pub ground: Option<String>,
    #[arg(long, requires = "ground")]
    pub drop: Option<i32>,
    #[arg(long, default_value_t = 36, requires = "ground")]
    pub reach: i32,
    /// The design is a crop farm (`circuit: harvester CROP`): record it as the farm judge builds
    /// it (`circuit::farm`): the judge's chests on its `port output:` cells and the crop, age 0, on
    /// its `port crop:` cells, written into the judged design (`<stem>-judged.txt`, the header's
    /// design), the box raised for the crop to grow; `--ticks` (default 4000) at
    /// `--random-tick-speed` (default the judge's 300). For redsim's random ticks (trace-compare).
    #[arg(long, conflicts_with = "circuit")]
    pub farm: bool,
    /// With `--farm`: drop the farm judge's intruders (a cobblestone, 64 iron ingots, a shovel:
    /// `circuit::intrusion::DROPS`, the k-th at the k-th tick) on every open hopper intake and
    /// above every crop column, at these ticks (`T1,T2,T3`).
    #[arg(long, requires = "farm")]
    pub drops: Option<String>,
    /// With `--farm`: the build turned this many quarter turns first (`Circuit::turned_pure`, the
    /// farm judge's turned builds; outputs `<stem>-r<T>-judged`).
    #[arg(long, requires = "farm", default_value_t = 0)]
    pub turns: u8,
}

/// `--farm`'s scene: the farm judge's build of a harvester (its chests and crops in the design),
/// the judge's box, its intruder drops.
fn farm_scene(path: &Path, out_dir: &Path, a: &ExportArgs) -> Result<CircuitScene> {
    let c = circuit::Circuit::load(path)?.turned_pure(a.turns)?;
    let Some(circuit::Kind::Harvester(crop)) = &c.kind else { bail!("{}: not a harvester (--farm)", path.display()) };
    let short = crop.strip_prefix("minecraft:").unwrap_or(crop);
    let planted = circuit::farm::CROPS.iter().find(|(n, _)| *n == short).map(|(_, s)| *s).with_context(|| format!("{crop}: not a judged crop"))?;
    let crops: Vec<[i32; 3]> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?;
    let stem = if a.turns % 4 != 0 { format!("{stem}-r{}", a.turns % 4) } else { stem.to_string() };
    let judged = out_dir.join(format!("{stem}-judged.txt"));
    let mut d = c.clock.design.clone();
    d.name = if a.turns % 4 != 0 { format!("{}-r{}-judged", d.name, a.turns % 4) } else { format!("{}-judged", d.name) };
    for o in &c.outputs {
        d.blocks.insert(o.pos, circuit::shulker::CHEST.to_string());
    }
    for p in &crops {
        d.blocks.insert(*p, planted.to_string());
    }
    // the farm judge's box (`farm::game_run`): the watched box, raised for the crop to grow
    let (mut from, mut to) = circuit::run::watch_box(&c, a.margin);
    to[1] = to[1].max(crops.iter().map(|p| p[1] + 3).max().unwrap_or(to[1]));
    from[1] = from[1].min(to[1] - 2);
    std::fs::write(&judged, format!("# {} as the farm judge builds it (its chests on the outputs, the crop planted, age 0): the design of an exact trace (clock misc trace-export --farm)\n{}", c.name(), d.to_text()))?;
    let mut v = Vec::new();
    if let Some(spec) = &a.drops {
        let ticks: Vec<u32> = spec.split(',').map(|t| t.trim().parse::<u32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--drops T1,T2,T3, got {spec:?}"))?;
        // the cells the farm judge drops on: the open hopper intakes, and the cell above each
        // crop column's full height (the drop's cell center: `Action::Drop`)
        let cells: Vec<[i32; 3]> = circuit::intrusion::intakes(&c).into_iter().chain(crops.iter().map(|p| [p[0], p[1] + 3, p[2]])).collect();
        for (k, t) in ticks.iter().enumerate() {
            let item = circuit::intrusion::DROPS[k % circuit::intrusion::DROPS.len()];
            for p in &cells {
                v.push((*t, clock::Action::Drop { pos: *p, item: item.to_string() }, json!([t, "drop", p, item])));
            }
        }
    }
    Ok(CircuitScene { judged, actions: v, ticks: a.ticks.unwrap_or(4000), bbox: (from, to), far: None })
}

/// `--circuit`'s scene: the judged build, its input actions with their header form, the judge's
/// whole run length and its watched box.
struct CircuitScene {
    judged: PathBuf,
    actions: Vec<(u32, clock::Action, Value)>,
    ticks: u32,
    bbox: ([i32; 3], [i32; 3]),
    far: Option<(usize, i32)>,
}

/// `--circuit`: the judged build of a circuit design (consumers, input drivers; the TNT judge's
/// ground with `--ground`) written as a design next to the trace, its input actions up to tick
/// `ticks` (default: the judge's whole schedule) and the judge's watched box.
fn circuit_scene(path: &Path, out_dir: &Path, a: &ExportArgs) -> Result<CircuitScene> {
    let c = circuit::Circuit::load(path)?;
    let cc = c.with_consumers()?.with_drivers();
    let o = circuit::verify::Options { stimulus: a.stimulus.clone(), ..Default::default() };
    let (s, _) = circuit::verify::schedule(&c, &o)?;
    let ticks = a.ticks.unwrap_or(s.ticks);
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?;
    let judged = out_dir.join(format!("{stem}-judged.txt"));
    let mut d = cc.clock.design.clone();
    d.name = format!("{}-judged", d.name);
    let (mut from, mut to) = circuit::run::watch_box(&cc, a.margin);
    let mut what = "consumers and input drivers placed".to_string();
    if let Some(g) = &a.ground {
        let ground = match g.as_str() {
            "grass" => circuit::Ground::Grass,
            "obsidian" => circuit::Ground::Obsidian,
            other => bail!("--ground grass|obsidian, not {other:?}"),
        };
        let drop = a.drop.unwrap_or_else(|| circuit::robust::declared_stand(&c));
        for (p, st) in c.ground_blocks(ground, a.reach, drop) {
            d.blocks.insert(p, st);
        }
        if let Some((lo, hi)) = d.bounds() {
            for i in 0..3 {
                from[i] = from[i].min(lo[i] - a.margin);
                to[i] = to[i].max(hi[i] + a.margin);
            }
        }
        what = format!("{what}, on the judge's {g} ground {drop} blocks under it");
    }
    std::fs::write(&judged, format!("# {} as the circuit judge builds it ({what}): the design of an exact trace (clock misc trace-export --circuit)\n{}", c.name(), d.to_text()))?;
    let mut v = Vec::new();
    for (t, act) in c.actions(&s) {
        if t > ticks {
            continue;
        }
        let j = match &act {
            clock::Action::Place { pos, state, nbt: None } => json!([t, "set", pos, state]),
            clock::Action::Clear { pos } => json!([t, "clear", pos]),
            clock::Action::Use { pos } => json!([t, "use", pos]),
            clock::Action::UseEntity { pos } => json!([t, "use_entity", pos]),
            clock::Action::ShapeSet { pos, state } => json!([t, "shape_set", pos, state]),
            clock::Action::Update { pos } => json!([t, "update", pos]),
            clock::Action::GameEvent { pos, event } => json!([t, "game_event", pos, event]),
            clock::Action::Drop { pos, item } => json!([t, "drop", pos, item]),
            other => bail!("a circuit input action without a trace form: {other:?}"),
        };
        v.push((t, act, j));
    }
    // a flying machine's corridor end along `travel` (the judge's run ends when it gets there)
    let far = match (&c.kind, c.travel) {
        (Some(circuit::Kind::FlyingMachine), Some(tr)) => (0..3).find(|&i| tr[i] != 0).map(|i| (i, if tr[i] > 0 { to[i] } else { from[i] })),
        _ => None,
    };
    Ok(CircuitScene { judged, actions: v, ticks, bbox: (from, to), far })
}

/// A `--action` input: `T:use:x,y,z` or `T:use_entity:x,y,z`; `T0..T1/STEP:...` repeats it every
/// STEP ticks from T0 through T1 (one entry per tick). `T:set:x,y,z:<state>` places a block with
/// updates (`Action::Place`), `T:shape_set:x,y,z:<state>` changes one with shape updates only
/// (`Action::ShapeSet`: a crop growing a step) - the state is the rest of the argument;
/// `T:drop:x,y,z:<stack>` summons an item at the cell's center, at rest (`Action::Drop`, the stack
/// as SNBT: `{id:"minecraft:dirt",count:1}`).
pub fn parse_action(s: &str) -> Result<Vec<(u32, clock::Action, Value)>> {
    let parts: Vec<&str> = s.splitn(4, ':').collect();
    let (t, kind, at, state) = match parts.as_slice() {
        [t, kind, at] => (*t, *kind, *at, None),
        [t, kind, at, state] => (*t, *kind, *at, Some(state.to_string())),
        _ => bail!("--action T:use|use_entity:x,y,z or T:set|shape_set:x,y,z:<state> or T:drop:x,y,z:<stack>, got {s:?}"),
    };
    let ticks: Vec<u32> = match t.split_once("..") {
        Some((a, rest)) => {
            let (b, step) = rest.split_once('/').unwrap_or((rest, "1"));
            let (a, b, step): (u32, u32, u32) = (a.parse()?, b.parse()?, step.parse()?);
            (a..=b).step_by(step.max(1) as usize).collect()
        }
        None => vec![t.parse().with_context(|| format!("--action tick in {s:?}"))?],
    };
    let p: Vec<i32> = at.split(',').map(|v| v.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--action cell in {s:?}"))?;
    let [x, y, z] = p.as_slice() else { bail!("--action cell x,y,z in {s:?}") };
    let pos = [*x, *y, *z];
    let full = |st: &str| if st.contains(':') { st.to_string() } else { format!("minecraft:{st}") };
    let (a, j) = match (kind, &state) {
        ("use", None) => (clock::Action::Use { pos }, None),
        ("use_entity", None) => (clock::Action::UseEntity { pos }, None),
        ("set", Some(st)) => (clock::Action::Place { pos, state: full(st), nbt: None }, Some(full(st))),
        ("shape_set", Some(st)) => (clock::Action::ShapeSet { pos, state: full(st) }, Some(full(st))),
        ("drop", Some(st)) => (clock::Action::Drop { pos, item: st.clone() }, Some(st.clone())),
        (other, _) => bail!("--action kind use, use_entity (no state), set, shape_set or drop (with a state), not {other:?} in {s:?}"),
    };
    Ok(ticks.into_iter().map(|t| (t, a.clone(), match &j { Some(st) => json!([t, kind, pos, st]), None => json!([t, kind, pos]) })).collect())
}

/// (period, warmup) of an archived design, from verdicts-v1.tsv (columns: hash, file, period,
/// warmup, ...).
fn verdict(file: &str) -> Result<(u32, u32)> {
    let tsv = std::fs::read_to_string("designs/clocks/archive/verdicts-v1.tsv").context("read verdicts-v1.tsv (run from the repository root)")?;
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() > 3 && f[1] == file {
            return Ok((f[2].parse()?, f[3].parse()?));
        }
    }
    bail!("{file}: no verdict row (archive it first)")
}

fn round(v: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (v * m).round() / m
}

/// What `--circuit` fixes for one export: the run length, the watched box and a flying machine's
/// corridor end (axis, coordinate of the far face).
#[derive(Default)]
struct Over {
    ticks: Option<u32>,
    bbox: Option<([i32; 3], [i32; 3])>,
    far: Option<(usize, i32)>,
    /// `--farm`: the random tick speed (the farm judge's, unless `--random-tick-speed` says).
    rts: Option<u32>,
}

fn export_one(w: &mut Worker, path: &Path, out_dir: &Path, a: &ExportArgs, extra: &[(u32, clock::Action, Value)], seed_override: Option<i64>, over: &Over) -> Result<(PathBuf, u32)> {
    let (margin, exact, reload_at) = (a.margin, a.exact, a.reload_at);
    let random_tick_speed = over.rts.or(a.random_tick_speed);
    let fixed_ticks = over.ticks.or(a.ticks).map(|n| (n, a.period, a.warmup));
    let file = path.file_name().and_then(|s| s.to_str()).context("file name")?.to_string();
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?.to_string();
    let (period, warmup, ticks, design) = match fixed_ticks {
        Some((n, period, warmup)) => (period, warmup, n, path.display().to_string()),
        None => {
            let (period, warmup) = verdict(&file)?;
            (period, warmup, warmup + 2 * period, format!("designs/clocks/archive/{file}"))
        }
    };
    let c = Clock::load(path).with_context(|| format!("load {}", path.display()))?;
    let (lo, hi) = c.bounds();
    let (from, to) = over.bbox.unwrap_or(([lo[0] - margin, lo[1] - margin, lo[2] - margin], [hi[0] + margin, hi[1] + margin, hi[2] + margin]));
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = seed_override.unwrap_or(0);
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    if let Some(n) = random_tick_speed {
        w.exec(sb, &format!("gamerule random_tick_speed {n}"))?;
    }
    clock::verify::place(w, sb, &c)?;
    let out_path = out_dir.join(match seed_override {
        Some(s) => format!("{stem}-s{s}.trace"),
        None => format!("{stem}.trace"),
    });
    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path)?);
    // the sandbox's forced chunk box and seed: what redsim's level-random mirror needs (the chunk
    // ticks draw randomTickSpeed x nextInt(48) per forced chunk every tick)
    let mut header = json!({
        "trace": 1,
        "name": c.design.name,
        "design": design,
        "period": period,
        "warmup": warmup,
        "ticks": ticks,
        "box": [from, to],
        "chunks": chunks,
        "seed": seed,
        "loop": [warmup, warmup + period],
    });
    if let Some(o) = c.output {
        header["output"] = json!(o);
    }
    if let Some(n) = random_tick_speed {
        header["random_tick_speed"] = json!(n);
    }
    // a chunk reload after that frame (redsim's trace-compare replays it: `redsim::reload`)
    if let Some(r) = reload_at {
        header["reload_at"] = json!(r);
    }
    // the order the level ticks its chunks in (`ServerLevel.tickChunk`: the random ticks' and the
    // precipitation picks' draws, chunk after chunk), which redsim's mirror recomputes
    if exact {
        header["chunk_order"] = json!(w.ticking_chunks(sb)?);
    }
    if !extra.is_empty() {
        header["actions"] = json!(extra.iter().map(|(_, _, v)| v.clone()).collect::<Vec<Value>>());
    }
    // a flying machine's corridor end (the judge's run ends there): written after the run
    let buffer = over.far.is_some();
    if !buffer {
        writeln!(out, "{header}")?;
    }
    let mut lines: Vec<String> = Vec::new();
    let mut last = ticks;
    let mut prev: HashMap<[i32; 3], String> = HashMap::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut actions = c.actions();
    // the --action inputs, after the design's own at the same tick
    actions.extend(extra.iter().map(|(t, act, _)| (*t, act.clone())));
    actions.sort_by_key(|(t, _)| *t);
    for t in 0..=ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, act) = actions.remove(0);
            clock::verify::apply(w, sb, &act)?;
        }
        if t > 0 {
            w.step(sb, 1)?;
        }
        let region = w.get_region(sb, from, to)?;
        // a flying machine reaching the far face of its corridor: the judge's run ends there (its
        // frames stop before this one: run::game_in)
        if let Some((axis, face)) = over.far
            && region.iter().any(|(p, s)| p[axis] == face && !mctas::blockstate::is_air(s))
        {
            last = t.saturating_sub(1);
            break;
        }
        let mut blocks: Vec<Value> = Vec::new();
        for (p, s) in region.iter() {
            let air = mctas::blockstate::is_air(s);
            let changed = match prev.get(&p) {
                Some(x) => x != s,
                None => !air,
            };
            if changed {
                blocks.push(json!([p[0], p[1], p[2], s]));
                prev.insert(p, s.to_string());
            }
        }
        let mut ents: Vec<Value> = Vec::new();
        for e in w.entities(sb)? {
            let n = ids.len();
            let id = *ids.entry(e.uuid.clone()).or_insert(n);
            ents.push(if exact {
                json!({
                    "id": id,
                    "net_id": e.net_id,
                    "type": e.kind,
                    "pos": e.pos,
                    "motion": e.motion,
                    "yaw": e.yaw,
                    "pitch": e.pitch,
                })
            } else {
                json!({
                    "id": id,
                    "type": e.kind,
                    "pos": [round(e.pos[0], 4), round(e.pos[1], 4), round(e.pos[2], 4)],
                    "yaw": round(e.yaw as f64, 2),
                    "pitch": round(e.pitch as f64, 2),
                })
            });
        }
        // exact traces: the level random's state and Level.randValue after this frame (redsim's
        // mirror is checked against both every frame)
        let mut frame = json!({"t": t, "blocks": blocks, "entities": ents});
        if exact {
            let (rng, rv) = w.random_states(sb)?;
            frame["rng"] = json!(rng);
            frame["rv"] = json!(rv);
        }
        if buffer {
            lines.push(frame.to_string());
        } else {
            writeln!(out, "{frame}")?;
        }
        if reload_at == Some(t) {
            // (clockinfra: the judge's chunk-reload check, frame by frame) saved, unloaded and
            // loaded again right after this frame, without a tick
            w.reload(sb)?;
        }
    }
    if buffer {
        header["ticks"] = json!(last);
        writeln!(out, "{header}")?;
        for l in &lines {
            writeln!(out, "{l}")?;
        }
    }
    out.flush()?;
    let _ = w.drop_sandbox(sb);
    Ok((out_path, last))
}

pub fn run(a: ExportArgs) -> Result<()> {
    if a.designs.is_empty() {
        bail!("clock misc trace-export <archived designs>...");
    }
    let out_dir = a.out_dir.clone().unwrap_or_else(|| PathBuf::from(if a.exact { "designs/clocks/traces/exact" } else { "designs/clocks/traces" }));
    std::fs::create_dir_all(&out_dir)?;
    let mut w = Worker::spawn(&WorkerConfig::default())?;
    let extra: Vec<(u32, clock::Action, Value)> = a.extra_actions.iter().map(|s| parse_action(s)).collect::<Result<Vec<_>>>()?.into_iter().flatten().collect();
    for d in &a.designs {
        // --seeds A..B: one trace per sandbox seed (<stem>-s<seed>.trace): a different level random,
        // so different TNT kicks and other random draws, each one checkable by trace-compare
        let seeds: Vec<Option<i64>> = match &a.seeds {
            Some(r) => {
                let (lo, hi) = r.split_once("..").with_context(|| format!("--seeds A..B, got {r:?}"))?;
                let (lo, hi): (i64, i64) = (lo.trim().parse()?, hi.trim().parse()?);
                (lo..=hi).map(Some).collect()
            }
            None => vec![None],
        };
        for s in seeds {
            // --circuit: the judged build, the stimulus' input actions, the judge's run and box;
            // one design that cannot be exported is reported and the rest go on
            let (design, acts, over) = if a.circuit || a.farm {
                let scene = if a.farm { farm_scene(d, a.judged_dir.as_deref().unwrap_or(&out_dir), &a) } else { circuit_scene(d, a.judged_dir.as_deref().unwrap_or(&out_dir), &a) };
                let sc = match scene {
                    Ok(sc) => sc,
                    Err(e) => {
                        println!("SKIP {}: {e:#}", d.display());
                        continue;
                    }
                };
                let mut acts = sc.actions;
                acts.extend(extra.iter().cloned());
                acts.sort_by_key(|(t, _, _)| *t);
                let rts = if a.farm { Some(a.random_tick_speed.unwrap_or(circuit::farm::RTS)) } else { None };
                (sc.judged, acts, Over { ticks: Some(sc.ticks), bbox: Some(sc.bbox), far: sc.far, rts })
            } else {
                let mut acts = extra.clone();
                // --lever-flips: this design's own lever
                if let Some(spec) = &a.lever_flips {
                    let lever = mctas::design::Design::load(d)?.ports.get("lever").copied().with_context(|| format!("{}: no `port lever`", d.display()))?;
                    acts.extend(parse_action(&format!("{spec}:use:{},{},{}", lever[0], lever[1], lever[2]))?);
                    acts.sort_by_key(|(t, _, _)| *t);
                }
                (d.clone(), acts, Over::default())
            };
            let (p, ticks) = match export_one(&mut w, &design, &out_dir, &a, &acts, s, &over) {
                Ok(r) => r,
                Err(e) if a.circuit => {
                    println!("SKIP {}: {e:#}", d.display());
                    continue;
                }
                Err(e) => return Err(e),
            };
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            println!("{}: {ticks} ticks, {} KB", p.display(), size / 1024);
        }
    }
    Ok(())
}
