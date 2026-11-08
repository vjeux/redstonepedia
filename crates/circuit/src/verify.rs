//! `mctas circuit sim` (the redsim screen) and `mctas circuit verify` (the judge, in the real game
//! at N positions x 4 rotations): the kind's stimulus, one recorded run per build, the kind's
//! checker on each, and the builds compared (`robust`, else position- or rotation-dependent).

use crate::check::{self, Outcome, Sizing};
use crate::run;
use crate::stimulus::{SETTLE, Schedule};
use crate::trace::Trace;
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{Pool, Worker, WorkerConfig};

/// Knobs of [`sim`] and [`verify`].
#[derive(Clone, Debug)]
pub struct Options {
    pub positions: usize,
    pub rotations: bool,
    pub workers: usize,
    pub version: String,
    /// Watched cells around the circuit.
    pub margin: i32,
    /// The test spacing; `None`: sized from redsim's recovery time ([`sizing`]).
    pub gap: Option<u32>,
    /// An explicit stimulus instead of the kind's (no checker: the waveform is printed).
    pub stimulus: Option<String>,
    /// Threads a TNT duper's redsim robustness runs use inside one judgement (safe drop, precise
    /// over kicks); 0: the cores, at most 32. A search that judges from many threads at once
    /// passes 1 (clk-rt 2026-10-21: 32 annealing threads x 32 judge threads put a box's load
    /// average at ~1000 and its node agent stopped answering).
    pub sim_threads: usize,
}

impl Options {
    /// The threads one judgement's robustness runs use (`sim_threads`, or the cores up to 32).
    pub fn sim_threads(&self) -> usize {
        if self.sim_threads > 0 { self.sim_threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32) }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options { positions: 4, rotations: true, workers: 8, version: mctas::DEFAULT_VERSION.to_string(), margin: 3, gap: None, stimulus: None, sim_threads: 0 }
    }
}

/// The checkers' test spacing: the time the circuit takes to come back to rest after every input
/// turns on (held until rest, at most 1000 gt) and after they turn off (on redsim), plus 10 gt,
/// longer when an event of the kind's own stimulus (run spaced by 1000 gt) takes longer to settle
/// (then that settling + 10 gt); `None` when redsim does not model it or it never rests.
pub fn sizing(c: &Circuit, margin: i32) -> Option<u32> {
    // a slow circuit (a long item timer: its items move one per 8 gt each way) needs a longer
    // hold to come to rest: 1000 gt, then 4000, then 16000
    [1000u32, 4000, 16000].into_iter().find_map(|hold| sizing_with(c, margin, hold))
}

fn sizing_with(c: &Circuit, margin: i32, hold: u32) -> Option<u32> {
    if c.inputs.is_empty() {
        return None;
    }
    let n = c.inputs.len();
    let on = c.clock.free_from() + SETTLE;
    let last_change = |t: &Trace, from: u32, to: u32| -> u32 {
        // a flying machine's run ends early when it reaches the far end of its corridor: the
        // trace is shorter than the schedule
        let to = (to as usize).min(t.cells.len().saturating_sub(1));
        (from as usize + 1..=to).rev().find(|&f| t.cells[f] != t.cells[f - 1]).map(|f| f as u32 - from).unwrap_or(0)
    };
    // every input on and held: the time to rest (a strength input at 15)
    let strength = |i: usize| matches!(c.inputs[i].how, crate::InputHow::Strength { .. });
    let mut s = Schedule::new(n);
    for i in 0..n {
        s.set(i, on, true);
        if strength(i) {
            // the input's top value (a decimal 7-segment display never sees more than 9)
            let top = match &c.kind {
                Some(Kind::Compute(spec)) if spec.decimal => 9,
                _ => 15,
            };
            s.levels.push((on, i, top));
        }
    }
    s.ticks = on + hold;
    let t = run::sim_trace(c, &s, margin).ok()?;
    let r_on = last_change(&t, on, on + hold);
    if r_on + 10 >= hold {
        return None;
    }
    // then every input off: the time to rest again
    let off = on + r_on + 10;
    for i in 0..n {
        s.set(i, off, false);
        if strength(i) {
            s.levels.push((off, i, 0));
        }
    }
    s.ticks = off + hold;
    let t = run::sim_trace(c, &s, margin).ok()?;
    let r_off = last_change(&t, off, off + hold);
    if r_off + 10 >= hold {
        return None;
    }
    // the kind's own stimulus, spaced by `hold`: the longest settling after any input change (a
    // slow path the all-on / all-off pair never takes, such as a crafter's nine ingots coming back
    // one by one through a hopper after the reverse craft)
    let mut r_stim = 0;
    if let Some(kind) = &c.kind {
        let long = check::stimulus(c, kind, &Sizing { gap: hold });
        let t = run::sim_trace(c, &long, margin).ok()?;
        let frames = t.cells.len();
        let mut changes: Vec<usize> = (1..frames).filter(|&f| t.inputs.iter().any(|i| i[f] != i[f - 1])).collect();
        // a strength input's level changes are in the schedule, not in the on/off record
        changes.extend(long.levels.iter().map(|&(at, _, _)| at as usize).filter(|&f| f > 0 && f < frames));
        changes.sort_unstable();
        changes.dedup();
        for (k, &f) in changes.iter().enumerate() {
            // the window up to the frame before the next input change (or the end)
            let end = changes.get(k + 1).map(|&g| g - 1).unwrap_or(frames - 1);
            let settle = last_change(&t, f as u32, end as u32);
            if end - f >= hold as usize && settle + 10 >= hold {
                return None;
            }
            r_stim = r_stim.max(settle);
        }
    }
    // kept when every event of the kind's own stimulus settles within it; else the longest
    // settling + 10 gt
    let gap = r_on.max(r_off) + 10;
    Some(if r_stim > gap { r_stim + 10 } else { gap })
}

/// What a run is judged against: the explicit stimulus, or the kind's.
pub fn schedule(c: &Circuit, o: &Options) -> Result<(Schedule, Option<Kind>)> {
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    if let Some(st) = o.stimulus.as_ref().or(if c.kind.is_none() { c.stimulus.as_ref() } else { None }) {
        return Ok((crate::stimulus::parse(st, &names, 100)?, None));
    }
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no `circuit: <kind>` line and no stimulus: nothing to judge"))?;
    let gap = o.gap.or_else(|| sizing(c, o.margin)).unwrap_or(Sizing::default().gap);
    if std::env::var("CIRCUIT_SHOW_GAP").is_ok() {
        eprintln!("gap {gap} (sizing {:?})", if o.gap.is_none() { sizing(c, o.margin) } else { None });
    }
    // a triggered TNT duper with explosions: pulses at least a fuse apart (its last TNT has gone
    // off before the next pulse), so the gap is the duper's, not the TNT pile's
    let gap = if c.explodes() && o.gap.is_none() { gap.max(crate::tnt::TRIGGER_GAP) } else { gap };
    let mut s = check::stimulus(c, &kind, &Sizing { gap });
    let _ = &mut s;
    Ok((s, Some(kind)))
}

/// Result of [`sim`].
pub struct SimReport {
    pub schedule: Schedule,
    pub kind: Option<Kind>,
    pub trace: std::result::Result<Trace, String>,
    pub outcome: Option<Outcome>,
}

/// `circuit sim`'s report from a study's whole-judge redsim twin (`*::sim_verify`): the verdict on
/// the base build's card (or the first build's failure), the design as its one frame.
fn twin_sim_report(c: &Circuit, schedule: Schedule, rep: std::result::Result<VerifyReport, String>) -> SimReport {
    let (trace, outcome) = match rep {
        Ok(rep) => {
            let mut cells: Vec<(Pos, String)> = c.clock.design.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
            cells.sort();
            let out = match rep.base() {
                Some(b) if rep.pass => {
                    let mut out = b.outcome.clone();
                    let note = if rep.note.is_empty() { String::new() } else { format!(": {}", rep.note) };
                    out.why = [format!("{}{note}", rep.verdict), out.why].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ");
                    out
                }
                _ => Outcome::fail(rep.verdict.trim_start_matches("FAIL: ").to_string(), rep.builds.first().and_then(|b| b.result.as_ref().ok()).map(|k| k.outcome.table.clone()).unwrap_or_default()),
            };
            (Ok(Trace::new(c, vec![cells], Vec::new())), Some(out))
        }
        Err(why) => (Err(why), None),
    };
    SimReport { schedule, kind: c.kind.clone(), trace, outcome }
}

impl SimReport {
    /// `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD> (redsim)` or why it fails.
    pub fn summary(&self, c: &Circuit) -> String {
        match (&self.trace, &self.outcome, &self.kind) {
            (Err(e), _, Some(k)) if e.contains("no support") || e.starts_with("blown up") || c.input_refused.as_deref() == Some(e.as_str()) => format!("FAIL: {} {}: {e}", c.name(), k.name()),
            (Err(e), _, _) => format!("UNMODELED: {}: {e}", c.name()),
            (Ok(_), None, _) => format!("RUN: {} ({} frames)", c.name(), self.schedule.ticks + 1),
            (Ok(t), Some(out), Some(k)) => line(c, k, out, dims(c, t), "redsim"),
            _ => String::new(),
        }
    }
}

/// The exact gate of a redsim model (clk-item, item frames; any kind): the kind's stimulus on the
/// base build (rotation 0, no offset) in redsim and in the game, EVERY cell of the watched box
/// compared at every frame (full states: block state + comparator strength + container contents).
/// `Ok(Ok(frames))` when identical, `Ok(Err(first divergence))` otherwise.
pub fn trace_compare(c: &Circuit, o: &Options) -> Result<std::result::Result<usize, String>> {
    trace_compare_with(c, o, std::path::Path::new(""), &crate::recorded::Mode::Live)
}

/// [`trace_compare`] live, recording the game's frames, or replaying a recording (no game):
/// `mctas test`'s recorded calibration circuits ([`crate::recorded`]).
pub fn trace_compare_with(c: &Circuit, o: &Options, design: &std::path::Path, mode: &crate::recorded::Mode) -> Result<std::result::Result<usize, String>> {
    let frames_of = |t: &Trace| -> Vec<crate::recorded::Frame> { (0..t.frames()).map(|f| t.cells[f].iter().map(|(p, i)| (*p, t.names[*i as usize].clone())).collect()).collect() };
    let (schedule, _) = schedule(c, o)?;
    let sim = run::sim_trace(c, &schedule, o.margin).map(|t| frames_of(&t));
    let game = || -> Result<std::result::Result<Vec<crate::recorded::Frame>, String>> {
        let pool = spawn_pool(&Options { workers: 1, positions: 1, rotations: false, ..o.clone() })?;
        let game = pool.map(vec![()], |w, _, _| {
            let rc = c.rotated(w, 0)?;
            run::game_trace(w, &rc, &schedule, o.margin)
        });
        match game.into_iter().next() {
            Some(Ok(r)) => Ok(r.map(|t| frames_of(&t))),
            Some(Err(e)) => Err(e),
            None => bail!("no game run"),
        }
    };
    crate::recorded::gate("circuit-trace", design, "stimulus", o.margin, sim, game, mode)
}

/// Run the kind's stimulus (or an explicit one) on redsim and judge it.
pub fn sim(c: &Circuit, o: &Options) -> Result<SimReport> {
    // R7: an input the judge cannot drive like a real build refuses the design
    if let Some(why) = &c.input_refused {
        let schedule = Schedule::new(c.inputs.len());
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err(why.clone()), outcome: c.kind.as_ref().map(|_| Outcome::fail(why.clone(), Vec::new())) });
    }
    if c.kind.as_ref().is_some_and(|k| k.is_shulker()) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("redsim does not judge shulker loaders (study 8: judged in the game, `mctas circuit verify`; `mctas circuit shulker compare` replays their real-game traces in redsim)".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::Sorter(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("item sorters have their own runs (study 13): `mctas circuit sorter-run --sim` (redsim) or `mctas circuit sorter-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::Smelter(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("auto-smelters have their own runs (study 17): `mctas circuit smelter-run --sim` (redsim) or `mctas circuit smelter-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::Brewer(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("auto-brewers have their own runs (study 21): `mctas circuit brewer-run --sim` (redsim) or `mctas circuit brewer-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::KelpFactory(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("a kelp factory has its own run (study 23): `mctas circuit factory-run --sim` (redsim) or `mctas circuit factory-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::Composter(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("auto-composters have their own runs (study 22): `mctas circuit composter-run --sim` (redsim) or `mctas circuit composter-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::Station(_))) && o.stimulus.is_some() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("a minecart station has its own run (study 19), no stimulus: `mctas circuit sim` without --stimulus runs its redsim twin, `mctas circuit station-run` one game run".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::CartUnloader | Kind::CartLoader)) && o.stimulus.is_some() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("cart loaders and unloaders have their own run (study 19), no stimulus: `mctas circuit sim` without --stimulus runs the redsim twin, `mctas circuit cart-run` one game run".into()), outcome: None });
    }
    if matches!(c.kind, Some(Kind::AutoDoor)) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("automatic doors have their own runs in the game, with walkers (study 21): `mctas circuit door-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    // a registered study's kind (crate::registry): its twin's whole judge, or why it has none
    if let Some(Kind::Study(s)) = &c.kind
        && o.stimulus.is_none()
    {
        let d = s.def();
        let mut schedule = Schedule::new(c.inputs.len());
        return Ok(match d.twin {
            crate::registry::Twin::Exact(f) => {
                schedule.ticks = (d.ticks)(c);
                twin_sim_report(c, schedule, f(c, o))
            }
            crate::registry::Twin::GameOnly(why) => {
                schedule.ticks = 0;
                SimReport { schedule, kind: c.kind.clone(), trace: Err(why.to_string()), outcome: None }
            }
        });
    }
    let rail = matches!(c.kind, Some(Kind::Station(_)) | Some(Kind::CartUnloader) | Some(Kind::CartLoader));
    if (matches!(c.kind, Some(Kind::Harvester(_))) || rail) && o.stimulus.is_none() {
        // the whole judge's redsim twin (a harvester: crate::farm::sim_verify, every build of
        // `circuit verify` and the tiled run, the game's random ticks mirrored draw for draw; a
        // minecart station or cart loader / unloader: crate::station / crate::cartio::sim_verify,
        // every build with riders, carts and strays, clk-misc 2026-10-21): its verdict on the base
        // build's card
        let mut schedule = Schedule::new(c.inputs.len());
        let rep = if matches!(c.kind, Some(Kind::Harvester(_))) {
            schedule.ticks = crate::farm::RUN;
            crate::farm::sim_verify(c, o)
        } else if matches!(c.kind, Some(Kind::Station(_))) {
            schedule.ticks = 0;
            crate::station::sim_verify(c, o)
        } else {
            schedule.ticks = 0;
            crate::cartio::sim_verify(c, o)
        };
        return Ok(twin_sim_report(c, schedule, rep));
    }
    if matches!(c.kind, Some(Kind::Crafter(_))) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("auto-crafters have their own runs (study 18): `mctas circuit crafter-run --sim` (redsim) or `mctas circuit crafter-run` (one game run); the judge is `mctas circuit verify`".into()), outcome: None });
    }
    let (schedule, kind) = schedule(c, o)?;
    // a TNT duper with explosions: judged at its safe drop height (the screen: both grounds, a few
    // kicks; `circuit verify` sweeps every kick)
    let mut cd = c.clone();
    let mut drop_fail: Option<String> = None;
    let mut safe = 0;
    if c.explodes() && o.stimulus.is_none() {
        let workers = o.sim_threads();
        match crate::robust::safe_drop_screen(c, workers) {
            Ok(h) => {
                safe = h;
                cd.drop = h.max(crate::robust::declared_stand(c));
            }
            Err(why) => drop_fail = Some(why),
        }
    }
    let c = &cd;
    let trace = run::sim_trace(c, &schedule, o.margin);
    let outcome = match (&trace, &kind) {
        (Ok(_), Some(_)) if drop_fail.is_some() => Some(Outcome::fail(drop_fail.clone().unwrap_or_default(), Vec::new())),
        (Ok(t), Some(k)) => {
            let mut out = check::check(c, k, &schedule, t);
            // ITEM INTRUSION (the real-use audit, crate::intrusion): items dropped on every open
            // hopper top; the run must pass with the same metrics
            if out.pass
                && o.stimulus.is_none()
                && let Some(s2) = crate::intrusion::schedule(c, &schedule, t)
            {
                let why = match run::sim_trace(c, &s2, o.margin) {
                    Ok(t2) => crate::intrusion::judge(&out, &check::check(c, k, &s2, &t2)),
                    Err(e) => Some(e),
                };
                if let Some(why) = why {
                    out = Outcome::fail(format!("item intrusion: {}: {why}", crate::intrusion::describe(c, &schedule)), std::mem::take(&mut out.table));
                }
            }
            if out.pass && c.explodes() && o.stimulus.is_none() {
                out.metrics.extend(crate::robust::drop_metrics(c, safe));
                // this run kicks every TNT the same way: PRECISE must hold over kicks
                // (crate::robust::precise_over_kicks: each TNT its own kick)
                if out.why.contains("precise:") {
                    let workers = o.sim_threads();
                    if let Err(why) = crate::robust::precise_over_kicks(c, &schedule, o.margin, workers) {
                        out.why = crate::robust::strip_precise(&out.why);
                        out.table.push(format!("not precise over kicks: {why}"));
                    }
                }
            }
            // TNT dupers: the turned builds too (2026-10-21: clk-rt's 21-block fast duper passed
            // this run but primes nothing at rotations 1 and 3, in the game and in redsim alike:
            // the turned build's update order differs). A turned build that fails or measures
            // otherwise makes the run `rotation-dependent` (flagged, as the game judge does)
            if out.pass && matches!(k, Kind::TntDuper | Kind::TntDuperTriggered) && o.stimulus.is_none() {
                for turns in 1..4u8 {
                    // (a design with entities keeps its old screen: skipped)
                    if !c.clock.design.entities.is_empty() {
                        break;
                    }
                    let Ok(tc) = c.turned_pure(turns) else { break };
                    let verdict = match run::sim_trace(&tc, &schedule, o.margin) {
                        Ok(tt) => {
                            let to = check::check(&tc, k, &schedule, &tt);
                            if !to.pass {
                                Some(to.why)
                            } else if to.metrics != out.metrics.iter().take(to.metrics.len()).cloned().collect::<Vec<_>>() {
                                Some(format!("measures {}", to.metrics.iter().map(|(a, b)| format!("{a} {b}")).collect::<Vec<_>>().join(", ")))
                            } else {
                                None
                            }
                        }
                        Err(why) => Some(why),
                    };
                    if let Some(why) = verdict {
                        out.why = format!("rotation-dependent: turned {turns} time(s): {}; {}", why.chars().take(120).collect::<String>(), out.why);
                        break;
                    }
                }
            }
            let out = match lamp_part(c, &schedule, o.margin, t) {
                Some(why) if out.pass => Outcome::fail(why, out.table),
                _ => out,
            };
            // R7: the judge's input stones are never moved
            let out = match c.input_entered(t) {
                Some(why) if out.pass => Outcome::fail(why, out.table),
                _ => out,
            };
            // and an output's block stays put
            let out = match c.output_entered(t) {
                Some(why) if out.pass => Outcome::fail(why, out.table),
                _ => out,
            };
            Some(if out.pass && tiles(c, k) { tiled_sim(c, k, &schedule, o.margin, &out) } else { out })
        }
        _ => None,
    };
    // a BUD's `tile:` claim: the copies side by side (crate::bud::tile_check)
    let outcome = match (outcome, c.tile) {
        (Some(out), Some(tl)) if out.pass && kind == Some(Kind::Bud) && o.stimulus.is_none() => {
            Some(crate::bud::tile_check(c, &out, crate::bud::gap_of(&schedule), &crate::bud::tile_label(tl.step), |tc, ts| Ok(run::sim_trace(tc, ts, o.margin)))?)
        }
        (out, _) => out,
    };
    // a sculk design: the device and ambient runs (crate::vibration)
    let outcome = match (outcome, &kind, &trace) {
        (Some(out), Some(k), Ok(qt)) if out.pass && c.has_sculk() && o.stimulus.is_none() => Some(crate::vibration::check(c, k, &out, &schedule, qt, |vc, vs| Ok(run::sim_trace(vc, vs, o.margin)))?),
        (out, _, _) => out,
    };
    // a study 12 bit slice: its stacks of 2..4 copies judged as the N-bit kind
    let outcome = match (outcome, &kind, c.slice) {
        (Some(out), Some(Kind::Compute(spec)), Some(_)) if out.pass && o.stimulus.is_none() => {
            let margin = o.margin;
            let gap0 = crate::compute::gap_of(&schedule);
            Some(crate::compute::slice_check(c, *spec, &out, |sc| sizing(sc, margin).unwrap_or(gap0).max(gap0), |sc, ss| Ok(run::sim_trace(sc, ss, margin)))?)
        }
        (out, _, _) => out,
    };
    // a one-way flyer's start device: its other LEVER_FLIPS runs (check::start_check)
    let outcome = match (outcome, &kind) {
        (Some(out), Some(k)) if out.pass && o.stimulus.is_none() => Some(check::start_check(c, k, out, &schedule, |s| Ok(run::sim_trace(c, s, o.margin)))?),
        (out, _) => out,
    };
    // a randomizer: more seeds (salts), their counts pooled for the uniformity test
    // (crate::random::screen)
    let outcome = match (outcome, &kind) {
        (Some(out), Some(Kind::Randomizer(n))) if out.pass && o.stimulus.is_none() => Some(crate::random::screen(c, *n, &schedule, out, |cs| run::sim_trace(cs, &schedule, o.margin))),
        (out, _) => out,
    };
    // a horizontal flying machine's ground clearance (robust::fly_clearance)
    let outcome = match (outcome, &kind) {
        (Some(mut out), Some(k)) if out.pass && o.stimulus.is_none() && crate::robust::fly_horizontal(c) => {
            let g = crate::robust::fly_clearance(c, k, &schedule, |cg, s| Ok(run::sim_trace(cg, s, o.margin)))?;
            out.metrics.push(crate::robust::clearance_metric(g));
            Some(out)
        }
        (out, _) => out,
    };
    Ok(SimReport { schedule, kind, trace, outcome })
}

/// A design with a `tile:` header whose copies are judged too ([`tiled_check`]): study 11, and
/// the sequential kinds builders tile into registers and counter rows (T flip-flops, latches,
/// D flip-flops, shift cells, counters). The check itself is generic - every copy must pass its
/// kind's own check and measure what the design measures alone.
fn tiles(c: &Circuit, k: &Kind) -> bool {
    // study 11's tile check (crate::pulsemult::check_tiled) judges each copy with the kind's own
    // checker: study 1's pulse circuits and the sequential kinds tile the same way
    c.tile.is_some() && matches!(k.study(), "pulsemult" | "pulse" | "tff" | "memory" | "delay" | "gates")
}

/// The tiled runs of a study-11 design on redsim ([`crate::pulsemult::check_tiled`]): the copies
/// driven 7 gt apart, then in phase ([`crate::pulsemult::TILE_SHIFTS`]); the first failure fails.
fn tiled_sim(c: &Circuit, k: &Kind, s: &Schedule, margin: i32, alone: &Outcome) -> Outcome {
    let tc = match crate::tiled(c) {
        Ok(tc) => tc,
        Err(e) => return Outcome::fail(format!("{e}"), alone.table.clone()),
    };
    let label = c.tile.map(|t| crate::bud::tile_label(t.step)).unwrap_or_default();
    let mut out = alone.clone();
    for shift in crate::pulsemult::TILE_SHIFTS {
        let ts = crate::pulsemult::tiled_schedule(c, &tc, s, shift);
        out = match run::sim_trace(&tc, &ts, margin) {
            Ok(t) => crate::pulsemult::check_tiled(c, k, &tc, s, &t, alone, shift, &label),
            Err(e) => Outcome::fail(format!("tile: {e}"), alone.table.clone()),
        };
        if !out.pass {
            break;
        }
    }
    out
}

/// The tiled runs of a study-11 design in one build of the game (`rc`: the build's circuit).
fn tiled_check(w: &mut Worker, rc: &Circuit, k: &Kind, s: &Schedule, margin: i32, alone: &Outcome, label: &str) -> Result<Outcome> {
    let tc = match crate::tiled(rc) {
        Ok(tc) => tc,
        Err(e) => return Ok(Outcome::fail(format!("{e}"), alone.table.clone())),
    };
    let mut out = alone.clone();
    for shift in crate::pulsemult::TILE_SHIFTS {
        let ts = crate::pulsemult::tiled_schedule(rc, &tc, s, shift);
        out = match run::game_trace(w, &tc, &ts, margin)? {
            Ok(t) => crate::pulsemult::check_tiled(rc, k, &tc, s, &t, alone, shift, label),
            Err(e) => Outcome::fail(format!("tile: {e}"), alone.table.clone()),
        };
        if !out.pass {
            break;
        }
    }
    Ok(out)
}

/// The judge's consumer lamps observe the outputs, never take part in the circuit (clockinfra
/// 2026-10-20): run again on redsim with every consumer cell left as AIR; every output in such a
/// cell must see the same signal in every frame, else the lamp closes a loop or feeds a block (it
/// is a conductor) and the design would not run with another consumer.
pub fn lamp_part(c: &Circuit, s: &Schedule, margin: i32, with: &Trace) -> Option<String> {
    let cells = c.consumer_cells();
    if cells.is_empty() {
        return None;
    }
    let bare = run::sim_trace_with(c, s, margin, false).ok()?;
    for (k, o) in c.outputs.iter().enumerate() {
        if !cells.contains(&o.pos) {
            continue;
        }
        let (a, b) = (with.levels.get(k)?, bare.levels.get(k)?);
        if let Some(f) = (0..a.len().min(b.len())).find(|&f| a[f] != b[f]) {
            return Some(format!(
                "the judge's lamp is part of the circuit: with output {} left as air its signal differs from frame {f} ({} with the lamp, {} without)",
                o.name, a[f], b[f]
            ));
        }
    }
    None
}

/// The box of every non-air cell over the run, the input cells left out: width x height x depth (a
/// flying machine: the design's own box, not its path).
pub fn dims(c: &Circuit, t: &Trace) -> [i32; 3] {
    // a flying machine, an elevator: its own box; a harvester: its design's box (the farm judge's
    // `design_dims`; the redsim twin's trace holds the design only)
    if matches!(c.kind, Some(Kind::FlyingMachine) | Some(Kind::Elevator(_)) | Some(Kind::ElevatorFloors(..)) | Some(Kind::Harvester(_)))
        && let Some((lo, hi)) = c.clock.design.bounds()
    {
        return [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    }
    let mut ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    // the judge's consumers are not the circuit's
    ins.extend(c.consumer_cells());
    // nor are the judge's input devices (R7): a driven input's repeater, support and switch
    for i in &c.inputs {
        if let crate::InputHow::Driven { dir, .. } = i.how {
            let (r, sup, sw) = crate::driver_cells(i.pos, dir);
            ins.extend([r, sup, sw]);
        }
    }
    // nor are the blocks the BUD judge places at the targets and around the design (its probe
    // block there; the design's own blocks moving through those cells count)
    let mut probes: Vec<Pos> = Vec::new();
    let probe_id = c.targets.first().map(|t| t.a.split('[').next().unwrap_or("").to_string()).unwrap_or_default();
    if c.kind == Some(Kind::Bud) {
        ins.extend(c.targets.iter().map(|t| t.pos));
        probes = c.probe_cells();
    }
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for f in &t.cells {
        for (p, _) in f.iter().filter(|(p, s)| !ins.contains(p) && !(probes.contains(p) && t.names[*s as usize].split('[').next() == Some(probe_id.as_str()))) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    if lo[0] == i32::MAX { [0, 0, 0] } else { [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1] }
}

fn line(c: &Circuit, k: &Kind, out: &Outcome, d: [i32; 3], how: &str) -> String {
    if out.pass {
        format!(
            "CIRCUIT: {} {} {}, {} blocks, {}x{}x{}, {how}{}",
            c.name(),
            k.name(),
            out.metrics_text(),
            c.blocks(),
            d[0],
            d[1],
            d[2],
            if out.why.is_empty() { String::new() } else { format!(" ({})", out.why) }
        )
    } else {
        format!("FAIL: {} {}: {}", c.name(), k.name(), out.why)
    }
}

/// One build of [`verify`].
#[derive(Clone, Debug)]
pub struct Build {
    pub turns: u8,
    pub offset: Pos,
    /// The checker's verdict, the box and the family, or why the run broke a rule.
    pub result: std::result::Result<BuildOk, String>,
}

/// A build that ran to the end.
#[derive(Clone, Debug)]
pub struct BuildOk {
    pub outcome: Outcome,
    pub dims: [i32; 3],
    /// The kinds of the blocks whose state changes during the run ([`family`]).
    pub family: String,
}

/// Result of [`verify`].
pub struct VerifyReport {
    pub name: String,
    pub kind: Option<Kind>,
    pub blocks: usize,
    pub schedule: Schedule,
    pub builds: Vec<Build>,
    /// `robust`, `position-dependent`, `rotation-dependent`, or why it fails.
    pub verdict: String,
    pub pass: bool,
    /// On a position- or rotation-dependent pass: which builds fail (empty otherwise).
    pub note: String,
}

impl VerifyReport {
    /// The reference build: as built at the first position, else the first passing one.
    pub fn base(&self) -> Option<&BuildOk> {
        self.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).and_then(|b| b.result.as_ref().ok()).filter(|b| b.outcome.pass).or_else(|| self.builds.iter().find_map(|b| b.result.as_ref().ok().filter(|b| b.outcome.pass)))
    }

    /// `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: ...`.
    pub fn summary(&self, c: &Circuit) -> String {
        match (&self.kind, self.base()) {
            (Some(k), Some(b)) if self.pass => line(c, k, &b.outcome, b.dims, &self.verdict),
            (Some(k), _) => format!("FAIL: {} {}: {}", self.name, k.name(), self.verdict),
            (None, _) => format!("RUN: {}: {}", self.name, self.verdict),
        }
    }
}

/// The family of a run: the kinds of the blocks whose state changes (or that appear or vanish),
/// input cells left out, as `Clock::kinds` names them (a wall torch is a torch, every copper bulb
/// a copper bulb; piston heads and moving blocks are their piston's), `+`-joined.
pub fn family(c: &Circuit, t: &Trace) -> String {
    let mut ins: std::collections::BTreeSet<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    // the judge's consumers (lamps in air output cells) are not the circuit's
    ins.extend(c.consumer_cells());
    // a temporary kick block's cell is the judge's, not the design's
    if let Some(clock::Kick::Place { pos, .. }) = &c.clock.kick {
        ins.insert(*pos);
    }
    let mut first: std::collections::HashMap<Pos, u32> = std::collections::HashMap::new();
    let mut seen: std::collections::HashMap<Pos, usize> = std::collections::HashMap::new();
    let mut varying: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for f in &t.cells {
        for (p, id) in f {
            *seen.entry(*p).or_default() += 1;
            if *first.entry(*p).or_insert(*id) != *id {
                varying.insert(*p);
            }
        }
    }
    varying.extend(seen.iter().filter(|(_, n)| **n < t.frames()).map(|(p, _)| *p));
    let mut kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in &t.cells {
        for (p, id) in f {
            if varying.contains(p) && !ins.contains(p) {
                let s = &t.names[*id as usize];
                let k = s.split(['[', '{']).next().unwrap_or(s);
                let k = k.strip_prefix("minecraft:").unwrap_or(k);
                let k = if k == "redstone_wall_torch" {
                    "redstone_torch"
                } else if k.contains("copper_bulb") {
                    "copper_bulb"
                } else {
                    k
                };
                if !matches!(k, "piston_head" | "moving_piston" | "air") {
                    kinds.insert(k.to_string());
                }
            }
        }
    }
    kinds.into_iter().collect::<Vec<_>>().join("+")
}

/// The judge: see the module doc.
pub fn verify(c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let pool = spawn_pool(o)?;
    verify_in_pool(&pool, c, o)
}

/// The judge's JVM workers (`o.workers`, capped by MCTAS_MAX_WORKERS), to judge many circuits.
pub fn spawn_pool(o: &Options) -> Result<Pool> {
    let builds = o.positions * if o.rotations { 4 } else { 1 };
    Pool::spawn(o.workers.min(builds).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })
}

/// [`verify`] with existing workers.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    // R7: an input the judge cannot drive like a real build refuses the design
    if let Some(why) = &c.input_refused {
        let builds = vec![Build { turns: 0, offset: [0, 0, 0], result: Err(why.clone()) }];
        return Ok(verdict(VerifyReport { name: c.name().to_string(), kind: c.kind.clone(), blocks: c.blocks(), schedule: Schedule::new(c.inputs.len()), builds, verdict: String::new(), pass: false, note: String::new() }));
    }
    if c.kind.as_ref().is_some_and(|k| k.is_shulker()) && o.stimulus.is_none() {
        return crate::shulker::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Sorter(_))) && o.stimulus.is_none() {
        return crate::sorter::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Smelter(_))) && o.stimulus.is_none() {
        return crate::smelter::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Brewer(_))) && o.stimulus.is_none() {
        return crate::brewer::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::KelpFactory(_))) && o.stimulus.is_none() {
        return crate::factory::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Composter(_))) && o.stimulus.is_none() {
        return crate::composter::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Harvester(_))) && o.stimulus.is_none() {
        return crate::farm::verify_in_pool(pool, c, o);
    }
    // a registered study's kind (crate::registry): its own judge
    if let Some(Kind::Study(s)) = &c.kind
        && o.stimulus.is_none()
    {
        return (s.def().verify)(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Station(_))) && o.stimulus.is_none() {
        return crate::station::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::CartUnloader | Kind::CartLoader)) && o.stimulus.is_none() {
        return crate::cartio::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::AutoDoor)) && o.stimulus.is_none() {
        return crate::autodoor::verify_in_pool(pool, c, o);
    }
    if matches!(c.kind, Some(Kind::Crafter(_))) && o.stimulus.is_none() {
        return crate::crafter::verify_in_pool(pool, c, o);
    }
    let (schedule, kind) = schedule(c, o)?;
    // a TNT duper with explosions: its SAFE DROP HEIGHT first (redsim: both grounds, every kick;
    // vjeux 13:31Z), then every build stands that high above the grass ground
    let workers = o.sim_threads();
    let mut c = c.clone();
    let mut safe = 0;
    if c.explodes() && o.stimulus.is_none() {
        match crate::robust::safe_drop(&c, workers) {
            Ok((h, _)) => {
                safe = h;
                c.drop = h.max(crate::robust::declared_stand(&c));
            }
            Err(why) => {
                return Ok(VerifyReport { name: c.name().to_string(), kind: kind.clone(), blocks: c.blocks(), schedule: schedule.clone(), builds: Vec::new(), verdict: why, pass: false, note: String::new() });
            }
        }
    }
    let c = &c;
    // a horizontal flying machine's ground clearance, measured on redsim (robust::fly_clearance);
    // every game build then flies over the ground at that clearance
    let clearance = match &kind {
        Some(k) if o.stimulus.is_none() && crate::robust::fly_horizontal(c) => Some(crate::robust::fly_clearance(c, k, &schedule, |cg, s| Ok(run::sim_trace(cg, s, o.margin)))?),
        _ => None,
    };
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let mut rc = c.rotated(w, t)?.translated(off)?;
        rc.drop = c.drop;
        let tr = match run::game_trace(w, &rc, &schedule, o.margin)? {
            Ok(tr) => tr,
            Err(why) => return Ok(Err(why)),
        };
        let mut out = match &kind {
            Some(k) => check::check(&rc, k, &schedule, &tr),
            None => Outcome { pass: true, why: String::new(), metrics: Vec::new(), table: tr.strip(&rc, 0, tr.frames()) },
        };
        // R7: the judge's input stones are never moved
        if out.pass
            && kind.is_some()
            && let Some(why) = rc.input_entered(&tr)
        {
            out = Outcome::fail(why, out.table);
        }
        // and an output's block stays put
        if out.pass
            && kind.is_some()
            && let Some(why) = rc.output_entered(&tr)
        {
            out = Outcome::fail(why, out.table);
        }
        // ITEM INTRUSION (the real-use audit, crate::intrusion): the first build again with items
        // dropped on every open hopper top; it must pass with the same metrics
        if out.pass
            && o.stimulus.is_none()
            && t == 0
            && off == offsets[0]
            && let Some(k) = &kind
            && let Some(s2) = crate::intrusion::schedule(&rc, &schedule, &tr)
        {
            let what = crate::intrusion::describe(&rc, &schedule);
            let why = match run::game_trace(w, &rc, &s2, o.margin)? {
                Ok(t2) => crate::intrusion::judge(&out, &check::check(&rc, k, &s2, &t2)),
                Err(why) => Some(why),
            };
            if let Some(why) = why {
                out = Outcome::fail(format!("item intrusion: {what}: {why}"), out.table);
            }
        }
        if out.pass && rc.explodes() && o.stimulus.is_none() {
            out.metrics.extend(crate::robust::drop_metrics(&rc, safe));
        }
        // a BUD's `tile:` claim: the copies side by side in this build (crate::bud::tile_check);
        // the metric names the design's own step (the same in every build)
        if out.pass
            && kind == Some(Kind::Bud)
            && o.stimulus.is_none()
            && let Some(tl) = c.tile
        {
            out = crate::bud::tile_check(&rc, &out, crate::bud::gap_of(&schedule), &crate::bud::tile_label(tl.step), |tc, ts| run::game_trace(w, tc, ts, o.margin))?;
        }
        // a study 12 bit slice: its stacks of 2..4 copies in this build (holds sized on redsim)
        if out.pass
            && o.stimulus.is_none()
            && let (Some(Kind::Compute(spec)), Some(_)) = (&kind, rc.slice)
        {
            let gap0 = crate::compute::gap_of(&schedule);
            out = crate::compute::slice_check(&rc, *spec, &out, |sc| sizing(sc, o.margin).unwrap_or(gap0).max(gap0), |sc, ss| run::game_trace(w, sc, ss, o.margin))?;
        }
        // a tiled study-11 design: its copies side by side, each judged on its own (the metric
        // names the design's own step, the same in every build)
        if out.pass
            && let Some(k) = &kind
            && tiles(&rc, k)
            && let Some(tl) = c.tile
        {
            out = tiled_check(w, &rc, k, &schedule, o.margin, &out, &crate::bud::tile_label(tl.step))?;
        }
        // a sculk design: the device and ambient runs in this build (crate::vibration)
        if out.pass
            && rc.has_sculk()
            && o.stimulus.is_none()
            && let Some(k) = &kind
        {
            out = crate::vibration::check(&rc, k, &out, &schedule, &tr, |vc, vs| run::game_trace(w, vc, vs, o.margin))?;
        }
        // a one-way flyer's start device: its other LEVER_FLIPS runs in this build
        if out.pass
            && o.stimulus.is_none()
            && let Some(k) = &kind
        {
            out = check::start_check(&rc, k, out, &schedule, |s| run::game_trace(w, &rc, s, o.margin))?;
        }
        // a horizontal flying machine: this build over the ground at its clearance
        if out.pass
            && let (Some(g), Some(k)) = (clearance, &kind)
        {
            if let Some(gap) = g {
                let mut gc = rc.clone();
                gc.grounded = true;
                gc.drop = gap;
                let why = match run::game_trace(w, &gc, &schedule, o.margin)? {
                    Ok(gt) => {
                        let go = check::check(&gc, k, &schedule, &gt);
                        (!go.pass).then_some(go.why)
                    }
                    Err(why) => Some(why),
                };
                if let Some(why) = why {
                    out = Outcome::fail(format!("over the ground ({}): {why}", crate::robust::clearance_metric(g).1), out.table);
                }
            }
            if out.pass {
                out.metrics.push(crate::robust::clearance_metric(g));
            }
        }
        Ok(Ok(BuildOk { outcome: out, dims: dims(&rc, &tr), family: family(&rc, &tr) }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, result });
    }
    // PRECISE (a game run samples one kick per TNT, about 30): it must hold over kicks
    // (crate::robust::precise_over_kicks, on redsim, tick-identical with the game for TNT; a sliver
    // of kick angles landing elsewhere is missed by a game run most of the time)
    let claims_precise = builds.iter().any(|b| b.result.as_ref().is_ok_and(|r| r.outcome.pass && r.outcome.why.contains("precise:")));
    if claims_precise && c.explodes() && o.stimulus.is_none() {
        let workers = o.sim_threads();
        if let Err(why) = crate::robust::precise_over_kicks(c, &schedule, o.margin, workers) {
            for b in builds.iter_mut() {
                if let Ok(r) = b.result.as_mut() {
                    r.outcome.why = crate::robust::strip_precise(&r.outcome.why);
                    r.outcome.table.push(format!("not precise over kicks (redsim): {why}"));
                }
            }
        }
    }
    let mut rep = verdict(VerifyReport { name: c.name().to_string(), kind: kind.clone(), blocks: c.blocks(), schedule: schedule.clone(), builds, verdict: String::new(), pass: false, note: String::new() });
    // a randomizer: every build's presses pooled, the outputs' counts tested for uniformity
    // (crate::random::pool_report)
    if rep.pass
        && o.stimulus.is_none()
        && let Some(Kind::Randomizer(n)) = &kind
    {
        crate::random::pool_report(&mut rep, *n);
    }
    // the judge's lamp must not take part (lamp_part, on redsim: one more run without it)
    if rep.pass
        && kind.is_some()
        && let Ok(t) = run::sim_trace(c, &schedule, o.margin)
        && let Some(why) = lamp_part(c, &schedule, o.margin, &t)
    {
        rep.pass = false;
        rep.verdict = why;
    }
    // TNT dupers (explosions on): every kick angle on both grounds was swept on redsim at the safe
    // drop height before the builds (crate::robust::safe_drop; vjeux: "pass every possible random
    // number")
    Ok(rep)
}

/// The verdict of a report's builds: no passing build fails it; builds that disagree (other
/// metrics, or failing) make it position- or rotation-dependent (allowed, flagged, as for clocks;
/// the failing builds are listed in `note`), else `robust`.
pub fn verdict(mut rep: VerifyReport) -> VerifyReport {
    let ok_count = rep.builds.iter().filter(|b| b.result.as_ref().is_ok_and(|b| b.outcome.pass)).count();
    if ok_count == 0 {
        let b = &rep.builds[0];
        rep.verdict = match &b.result {
            Ok(b) => b.outcome.why.clone(),
            Err(e) => e.clone(),
        };
        return rep;
    }
    // as for clocks: builds that disagree (other metrics, or failing) make the circuit position-
    // or rotation-dependent (allowed, flagged; the failing builds are listed in `note`). A
    // 7-segment decoder (coordinator 2026-10-21): every build already showed every value's
    // segments; the builds agree when their latency bounds agree, and the glitch count (which
    // update order moves by one or two between builds) is reported as the worst build's, not as
    // a position or rotation dependence; a storage counter's digits too (study 15, clk-obs)
    let seg7 = matches!(&rep.kind, Some(Kind::Compute(spec)) if matches!(spec.op, crate::compute::Op::SevenSegment | crate::compute::Op::StorageCounter(_)));
    // a crop harvester (study 18): its harvest rate and longest wait follow the random growth, so
    // builds agree on everything else (crate::farm reports the builds' mean rate and worst wait)
    let farm = matches!(&rep.kind, Some(Kind::Harvester(_)));
    let sig = |b: &Build| match &b.result {
        Ok(k) if k.outcome.pass && seg7 => {
            let lat = k.outcome.metrics.iter().find(|(n, _)| n == "latency").and_then(|(_, v)| v.split_whitespace().next()).unwrap_or("");
            let rest: Vec<String> = k.outcome.metrics.iter().filter(|(n, _)| n != "latency" && n != "glitches").map(|(n, v)| format!("{n} {v}")).collect();
            format!("latency {lat} gt; {}", rest.join(", "))
        }
        Ok(k) if k.outcome.pass && farm => k.outcome.metrics.iter().filter(|(n, _)| !n.starts_with("items per hour") && !n.starts_with("items smelted") && n != "wait" && n != "harvest height" && n != "game rts").map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", "),
        Ok(k) if k.outcome.pass => k.outcome.metrics_text(),
        _ => "FAIL".to_string(),
    };
    let base_build = rep.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && sig(b) != "FAIL").or_else(|| rep.builds.iter().find(|b| sig(b) != "FAIL")).unwrap();
    let base = sig(base_build);
    // an auto-smelter (study 17): the builds agree when every one passes the whole judge, rests
    // alike and smelts within 20% of the base build's pace (a design's own clock starts at
    // another phase of the load in each build, so its pace and the hoppers it leaves awake move a
    // little: no dependence)
    let smelter = matches!(&rep.kind, Some(Kind::Smelter(_)));
    // an auto-brewer (study 21): the same rule on its potions/min
    let brewer = matches!(&rep.kind, Some(Kind::Brewer(_)));
    // an auto-composter (study 22): the same rule on the items it composts a minute (its bone
    // meal is random: every build draws its own composting chances)
    let composter = matches!(&rep.kind, Some(Kind::Composter(_)));
    // a kelp factory (study 23): its kelp grows by random ticks, which every build draws its own
    // way, so its output differs from build to build: the builds agree when every one passes
    let factory = matches!(&rep.kind, Some(Kind::KelpFactory(_)));
    // a hopper-minecart loader or unloader (study 19; coordinator 2026-10-22): robust when every
    // build passes with every item delivered; its rate is the worst build's (crate::cartio::report),
    // not matched build to build (a 0.02 b/t test cart resting 0.1 block apart moved it by a tick)
    let cart = matches!(&rep.kind, Some(Kind::CartLoader) | Some(Kind::CartUnloader));
    // a registered study's kind (crate::registry): its own rule (an item elevator: every build
    // passes, rests alike and flows within 2% of the base build; its first item may come a few gt
    // apart, a dropper chain's clock starting at another phase)
    let study = match &rep.kind {
        Some(Kind::Study(s)) => Some(s.def()),
        _ => None,
    };
    let num = |b: &Build, key: &str| -> Option<String> { b.result.as_ref().ok().filter(|k| k.outcome.pass).and_then(|k| k.outcome.metrics.iter().find(|(n, _)| n == key).map(|(_, v)| v.clone())) };
    let differs = |b: &Build| -> bool {
        if let Some(d) = study {
            return crate::registry::builds_differ(&d.agree, b, base_build);
        }
        if factory || cart {
            return sig(b) == "FAIL";
        }
        if !smelter && !brewer && !composter {
            return sig(b) != base;
        }
        if sig(b) == "FAIL" {
            return true;
        }
        let (key, tol) = if brewer { ("potions/min per stand", 0.2) } else if composter { ("items composted/min", 0.2) } else { ("items/min per furnace", 0.2) };
        let rate = |b: &Build| num(b, key).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let (r, r0) = (rate(b), rate(base_build));
        num(b, "at rest") != num(base_build, "at rest") || (r - r0).abs() > tol * r.max(r0)
    };
    let pos_dep = rep.builds.iter().filter(|b| b.turns == 0).any(|b| differs(b));
    let rot_dep = rep.builds.iter().filter(|b| b.turns != 0).any(|b| differs(b));
    rep.verdict = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    let fails: Vec<String> = rep
        .builds
        .iter()
        .filter(|b| sig(b) == "FAIL")
        .map(|b| {
            let why = match &b.result {
                Ok(k) => k.outcome.why.clone(),
                Err(e) => e.clone(),
            };
            format!("rotation {} at {:?}: {why}", b.turns, b.offset)
        })
        .collect();
    if !fails.is_empty() {
        rep.note = format!("{} of {} builds fail: {}", fails.len(), rep.builds.len(), fails[0]);
    } else if let Some(b) = rep.builds.iter().find(|b| differs(b)) {
        // no build fails but they disagree: name the first build whose metrics differ
        rep.note = format!("rotation {} at {:?}: {} (base build: {base})", b.turns, b.offset, sig(b));
    }
    rep.pass = true;
    // the worst build's glitch count goes on the base build's metrics (what the card shows)
    if seg7 {
        let worst = rep.builds.iter().filter_map(|b| b.result.as_ref().ok().filter(|k| k.outcome.pass)).filter_map(|k| k.outcome.metrics.iter().find(|(n, _)| n == "glitches").and_then(|(_, v)| v.parse::<u32>().ok())).max();
        if let Some(w) = worst
            && let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
            && let Ok(k) = &mut b.result
        {
            for m in k.outcome.metrics.iter_mut().filter(|(n, _)| n == "glitches") {
                m.1 = w.to_string();
            }
        }
    }
    rep
}
