//! The circuit judge's run of a design, for its page: the circuits' cache domain (build.rs hashes
//! this file with the judge's crates).

use circuit::Circuit as Design;
use circuit::verify::{Options, sim};
use serde::{Deserialize, Serialize};

/// What a circuit page shows of the judge's run: the inputs' devices, the stimulus (`stimulus:`
/// text) and its phases, the redsim run as a timing diagram, the checker's report.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Details {
    pub devices: Vec<String>,
    pub stimulus: String,
    /// `data-levels`: a strength input's values (the hex kinds).
    pub levels: String,
    /// `data-phases` (written by the page from `raw_phases`, in words).
    pub phases: String,
    /// The checker's labelled spans as it names them (`pulse 3`, `clock 40`): the page puts them in
    /// words, here and in `wave`.
    pub raw_phases: Vec<(u32, u32, String)>,
    /// The circuit kind's name, for those words.
    pub kind_name: String,
    pub wave: Option<site_model::Wave>,
    pub table: Vec<String>,
    /// The judge's metrics when its run on the redsim screen passes.
    pub metrics: Option<Vec<(String, String)>>,
    /// The redsim screen's one-line verdict.
    pub summary: String,
    /// Why the screen fails, when it does.
    pub fails: Option<String>,
    /// Every output cell gets the judge's lamp in the 3D view.
    pub output_lamp: bool,
    /// The judge's actions at the target / probe cells (`data-events`).
    pub events: String,
    /// Why redsim cannot run the design (empty: it can), in words (set by the page).
    pub unmodeled: String,
    /// Where the 3D view's demo stops, raw (empty: it plays to its end; set by the page from
    /// [`super::demo_halt`]).
    #[serde(default)]
    pub view_halt: String,
    /// A shulker machine run like its 3D view runs it.
    pub view_run: Option<site_model::ViewRun>,
    /// A counter's measured duty: (pulse windows the output is on, N).
    pub counter_on: Option<(u32, u32)>,
    /// A big crop farm's judge's random tick speed (`circuit::farm::game_rts`), the page's
    /// `data-farm-rts`; None for every other design (a small farm runs at the view's default).
    #[serde(default)]
    pub farm_rts: Option<u32>,
}

/// The kinds whose page uses nothing from the judge's run: a long throughput or yield test (a
/// farm's growth, an item elevator's 193 items, a crafter's or a smelter's supply, a shulker
/// machine's boxes, a minecart station's carts, a mob farm's game days on redsim) with no input to
/// drive it, no timing diagram and no report on its page (their pages read the judge's own numbers
/// from verdicts.tsv; a shulker machine's run is the 3D view's, `view::view_check`). Their
/// simulation was the build's long pole (the coordinator 2026-10-21: a publish at 1458 s, one
/// farm's yield test running for minutes alone on one core while every other core waited; a mob
/// farm's was its whole 8-build judge again, ~45 s each since redsim's fire and random ticks,
/// 2026-11-08, which `mctas site preview` waited for on every new farm text).
const NO_RUN: [&str; 11] = ["harvester", "item-elevator", "crafter", "smelter", "station", "shulker-loader", "shulker-unloader", "cart-unloader", "cart-loader", "auto-door", "mob-farm"];

/// Whether the site runs the judge's test of a kind (`NO_RUN`).
pub fn runs_judge(kind: &str) -> bool {
    !NO_RUN.iter().any(|k| kind == *k || kind.strip_prefix(k).is_some_and(|r| r.starts_with(' ')))
}

/// The circuit judge's run of design `c`, for its page.
pub fn circuit_run(c: &Design) -> Details {
    let devices: Vec<String> = c
        .inputs
        .iter()
        .map(|i| match &i.how {
            circuit::InputHow::Lever => "lever".to_string(),
            circuit::InputHow::Button => "button".to_string(),
            circuit::InputHow::Strength { .. } => "strength (a chest)".to_string(),
            circuit::InputHow::Frame { .. } => "item frame".to_string(),
            circuit::InputHow::Block { on, .. } if on == circuit::INPUT_ON => "redstone block".to_string(),
            circuit::InputHow::Block { on, .. } => on.trim_start_matches("minecraft:").to_string(),
            // R7: a plain input is driven by the judge's repeater outside the build
            circuit::InputHow::Driven { .. } => "repeater".to_string(),
        })
        .collect();
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    // the viewer puts the judge's lamp in every air output cell (webrender bbe3bf7ba): then the
    // lamp shows the output, else the orange marker does
    let consumers = c.consumer_cells();
    let output_lamp = !c.outputs.is_empty() && c.outputs.iter().all(|o| consumers.contains(&o.pos));
    let mut d = Details { devices, output_lamp, ..Default::default() };
    // (a kind whose page shows nothing of the run: not run, `NO_RUN`)
    let kind = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
    if !runs_judge(&kind) {
        d.kind_name = kind;
        return d;
    }
    let r = match sim(c, &Options::default()) {
        Ok(r) => r,
        Err(e) => {
            d.summary = format!("simulation failed: {e:#}");
            return d;
        }
    };
    d.summary = r.summary(c);
    d.kind_name = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
    d.raw_phases = r.schedule.phases.clone();
    if !names.is_empty() && r.schedule.edges.iter().any(|e| !e.is_empty()) {
        d.stimulus = r.schedule.text(&names);
    }
    // a strength input's values (the hex kinds: the judge's chest fills), webrender's data-levels
    if !r.schedule.levels.is_empty() {
        d.levels = r.schedule.levels.iter().filter_map(|(t, i, v)| names.get(*i).map(|n| format!("{t} {n} {v}"))).collect::<Vec<_>>().join("; ");
    }
    // the judge's actions at the BUD's target and probe cells, in the design's coordinates
    let probes = c.probe_cells();
    d.events = r
        .schedule
        .events
        .iter()
        .filter_map(|(frame, at, op)| {
            let p = match at {
                circuit::stimulus::EventAt::Target(i) => c.targets.get(*i)?.pos,
                circuit::stimulus::EventAt::Probe(k) => *probes.get(*k)?,
                // the vibration test's game events change no block: nothing to show
                circuit::stimulus::EventAt::Input(_) | circuit::stimulus::EventAt::Cell(_) => return None,
            };
            let what = match op {
                circuit::stimulus::TargetOp::Place(s) => format!("place {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Clear => format!("clear {} {} {}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Shape(s) => format!("shape {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Update => format!("update {} {} {}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::GameEvent(_) | circuit::stimulus::TargetOp::Drop(_) => return None,
            };
            Some(format!("{frame} {what}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    if let Some(o) = &r.outcome {
        d.table = o.table.clone();
        if o.pass {
            d.metrics = Some(o.metrics.clone());
        } else {
            d.fails = Some(o.why.clone());
        }
    }
    if let Ok(t) = &r.trace {
        let mut rows: Vec<site_model::WaveRow> = Vec::new();
        for (k, i) in c.inputs.iter().enumerate() {
            let lv: Vec<u8> = t.inputs[k].iter().map(|on| if *on { 15 } else { 0 }).collect();
            rows.push(site_model::WaveRow::from_levels(&i.name, true, &lv));
        }
        // a BUD's judge acts at its target cells: one row of marks (the footprint probes left out)
        let marks: Vec<(u32, String)> = r
            .schedule
            .events
            .iter()
            .filter(|(_, at, _)| matches!(at, circuit::stimulus::EventAt::Target(_)))
            .map(|(f, _, op)| {
                let what = match op {
                    circuit::stimulus::TargetOp::Place(_) => "place",
                    circuit::stimulus::TargetOp::Clear => "clear",
                    circuit::stimulus::TargetOp::Shape(_) => "shape",
                    circuit::stimulus::TargetOp::Update => "update",
                    circuit::stimulus::TargetOp::GameEvent(e) => e,
                    circuit::stimulus::TargetOp::Drop(_) => "drop",
                };
                (*f, what.to_string())
            })
            .collect();
        if !marks.is_empty() {
            rows.push(site_model::WaveRow { name: "target".into(), input: true, marks, ..Default::default() });
        }
        for (k, o) in c.outputs.iter().enumerate() {
            rows.push(site_model::WaveRow::from_levels(&o.name, false, &t.levels[k]));
        }
        // the machines' position tracks (vjeux: "Can you put the position of the block?")
        let metric = |k: &str| r.outcome.as_ref().and_then(|o| o.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())).unwrap_or_default();
        match &c.kind {
            Some(circuit::Kind::PistonExtender(n)) => {
                if let Some(row) = payload_track(c, t, *n) {
                    let mut row = row;
                    let (ext, ret) = (metric("extend"), metric("retract"));
                    if !ext.is_empty() {
                        row.note = format!("out to N = {n} in {ext}, back in {ret}");
                    }
                    rows.push(row);
                }
            }
            Some(circuit::Kind::FlyingMachine) => {
                if let Some(mut row) = flight_track(c, t) {
                    let speed = metric("speed");
                    row.note = if speed.is_empty() { metric("two-way") } else { format!("1 block every {}", speed.trim_end_matches("/block")) };
                    rows.push(row);
                }
            }
            Some(circuit::Kind::Counter(n)) if *n > 0 && !t.inputs.is_empty() && !t.levels.is_empty() => {
                // a counter's duty, measured: in how many of every N pulse windows its output is
                // on (sampled half-way between two input pulses, after the first N windows)
                let input = &t.inputs[0];
                let edges: Vec<usize> = (1..input.len()).filter(|&f| input[f] && !input[f - 1]).collect();
                let windows: Vec<bool> = edges.windows(2).skip(*n as usize).map(|w| t.levels[0].get((w[0] + w[1]) / 2).is_some_and(|&l| l > 0)).collect();
                if windows.len() >= *n as usize {
                    let on = windows.iter().filter(|x| **x).count() as f64 / windows.len() as f64;
                    d.counter_on = Some(((on * *n as f64).round() as u32, *n));
                }
            }
            _ => {}
        }
        if !rows.is_empty() && t.frames() > 1 {
            d.wave = Some(site_model::Wave { ticks: t.frames() as u32, rows, phases: d.raw_phases.clone() });
        }
    }
    d
}

/// A piston extender's payload position per frame along its push direction (0 = home .. N =
/// extended); while it moves (carried inside a moving piston) the last position holds.
fn payload_track(c: &Design, t: &circuit::trace::Trace, n: u32) -> Option<site_model::WaveRow> {
    let d = c.travel?;
    let p0 = c.payloads.first()?.pos;
    let rest_at = (c.clock.free_from() + circuit::stimulus::SETTLE).saturating_sub(1) as usize;
    let payload = t.state(rest_at.min(t.frames().saturating_sub(1)), p0)?.to_string();
    let id = t.names.iter().position(|s| *s == payload)? as u32;
    let mut last = 0i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    let mut top = n as i32;
    for f in 0..t.frames() {
        let here: Vec<&circuit::Pos> = t.cells[f].iter().filter(|(_, x)| *x == id).map(|(p, _)| p).collect();
        if here.len() == 1 {
            let p = here[0];
            last = (p[0] - p0[0]) * d[0] + (p[1] - p0[1]) * d[1] + (p[2] - p0[2]) * d[2];
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    let mut row = site_model::WaveRow::from_levels("payload", false, &levels);
    row.scale = top.max(1) as u32;
    row.target = Some(n);
    Some(row)
}

/// A flying machine's displacement along its travel per frame (cells from where it was built);
/// while it moves between intact positions the last one holds.
fn flight_track(c: &Design, t: &circuit::trace::Trace) -> Option<site_model::WaveRow> {
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let kick = match &c.clock.kick {
        Some(clock::Kick::Place { pos, ticks, .. }) => Some((*pos, c.clock.kick_at, c.clock.kick_at + ticks)),
        _ => None,
    };
    let mut rec = circuit::fly::Recorder::new(&c.clock.design.blocks, kick, &c.stations);
    for f in 0..t.frames() {
        rec.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let mut last = 0i32;
    let mut top = 1i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    for s in &rec.shifts {
        if let Some(s) = s {
            last = (s[0] * travel[0] + s[1] * travel[1] + s[2] * travel[2]).abs();
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    if top <= 1 && levels.iter().all(|l| *l == 0) {
        return None;
    }
    let mut row = site_model::WaveRow::from_levels("moved", false, &levels);
    row.scale = top as u32;
    Some(row)
}
