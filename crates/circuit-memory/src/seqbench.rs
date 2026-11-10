//! The carousel counter's bench (designs/briefs/farm-iron-carousel.md, part 1: the counter; day-1
//! lane 2026-11-10): `mctas circuit memory counter-bench <design>`.
//!
//! The counter takes one pulse per golem spawn on its input and drives 7 level lines (`port
//! output.<name>`, in name order), exactly one high once settled, advanced one state per pulse;
//! when a slot brings no golem its own fallback advances it at (the last advance's edge) + 100 + m,
//! m in [6, 10], re-armed by every advance (the brief's contract, ironspec / ironfarm 2026-11-10).
//!
//! The bench drives the design on redsim (tick-exact, the judge's input device: a redstone block
//! behind the judge's repeater) with a reactive farm: after every advance the next batch is ready
//! `--ready` gt later and summons on the first slot tick (gameTime % 100 = 0) from then, the golem
//! pulse rising 0..=`--jitter` gt after it, 2..=15 gt long; a slot is missed with probability
//! `--miss` (and a miss repeats with `--burst`), so only the counter's fallback can move it on. Every
//! tick it reads the 7 lines and checks: every golem pulse gives exactly one advance within
//! `--tc` gt of its rising edge; every advance follows the cycle order the first lap showed (no
//! skip, no double, no reorder); an advance with no golem pulse is a fallback, and comes 100 + m
//! after the previous advance (line change to line change, both through the same lines) with m in
//! `--m`; a stall (no advance for 100 + m + `--tc` gt) fails. It reports T_c, the transition window
//! (gt with two lines high, gt with none, per change) and the fallback's m.
//!
//! Restarts: the same run again with the build's chunks saved, unloaded and loaded at `--restarts`
//! random ticks (`redsim::reload`, the game's reload model: scheduled ticks renumbered, block
//! entities registered again); its advances must equal the uninterrupted run's tick for tick.
//!
//! `--dump FILE`: the trace in mobviewer's `timeline --from-dump` format (`t= gt=` then `signal
//! NAME=on|off`, `mark pulse golem|fallback`, `mark slot missed`, `VIOLATION <rule> <lane>: ...`).

use anyhow::{Context as _, Result, anyhow, bail};
use circuit::{Circuit, InputHow};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The bench's options (`mctas circuit memory counter-bench --help`).
#[derive(Clone, Debug)]
pub struct Opts {
    /// Golem pulses to give.
    pub pulses: usize,
    pub seed: u64,
    /// Restart test: chunk reloads at this many random ticks (0: no restart run).
    pub restarts: usize,
    /// Golem pulse lengths, gt.
    pub width: (u32, u32),
    /// The golem pulse rises 0..=jitter gt after its slot tick.
    pub jitter: u32,
    /// Probability that a slot brings no golem.
    pub miss: f64,
    /// Probability that a missed slot is followed by another miss.
    pub burst: f64,
    /// gt from an advance (its line change) until the next batch can summon.
    pub ready: u32,
    /// The most gt from a golem pulse's rising edge to its advance.
    pub tc: u32,
    /// The fewest gt from a golem pulse's rise to an advance that can be its own (sooner, it is
    /// a fallback the pulse came on top of).
    pub tc_min: u32,
    /// The fallback's m range (gt past 100, line change to line change).
    pub m: (i64, i64),
    /// gt before the first slot (the design settles).
    pub warmup: u32,
    pub dump: Option<PathBuf>,
    /// Ticks of the trace written to the dump (from 0); violations are written wherever they are.
    pub dump_ticks: u32,
    /// Write a copy of the design carrying the base run's input as its `stimulus:` line
    /// (`in R+ F- ...`, the circuit judge's format): the real game's check of this very run is
    /// `mctas circuit trace-compare <copy>` (every cell, every gt, redsim against the game).
    pub stimulus_out: Option<PathBuf>,
    /// Quarter turns the same run is repeated on (the design turned clockwise seen from above):
    /// each must give the same advances tick for tick.
    pub turns: Vec<u8>,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { pulses: 100_000, seed: 1, restarts: 20, width: (2, 15), jitter: 5, miss: 0.05, burst: 0.3, ready: 40, tc: 40, tc_min: 4, m: (6, 10), warmup: 200, dump: None, dump_ticks: 20_000, stimulus_out: None, turns: vec![1, 2, 3] }
    }
}

/// splitmix64: the bench's own deterministic random.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next() % (hi - lo + 1) as u64) as u32
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Why an advance happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum By {
    Golem,
    Fallback,
}

/// One settled advance: the tick its new line stood alone, the lines, why.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Advance {
    t: u32,
    from: usize,
    to: usize,
    by: By,
}

/// One run's findings.
#[derive(Default)]
struct Run {
    advances: Vec<Advance>,
    /// Every golem pulse the farm sent: (rise, fall) of the input.
    pulses: Vec<(u32, u32)>,
    golems: usize,
    /// Golem pulses within 95 gt after a fallback advance the counter absorbed (contract (c)).
    absorbed: usize,
    misses: usize,
    fallbacks: usize,
    tc: Vec<u32>,
    m: Vec<i64>,
    overlap_max: u32,
    gap_max: u32,
    violations: Vec<(u32, String, String, String)>,
    ticks: u32,
    dump: String,
    stopped: Option<String>,
}

/// A pending golem pulse: its input rise, its fall, whether an advance answered it.
#[derive(Clone, Copy)]
struct Pulse {
    rise: u32,
    fall: u32,
    answered: bool,
}

fn run_once(c: &Circuit, o: &Opts, reloads: &[u32]) -> Result<Run> {
    if c.inputs.len() != 1 {
        bail!("the counter has one input (the golem pulse); {} has {}", c.clock.design.name, c.inputs.len());
    }
    let InputHow::Driven { dir, .. } = c.inputs[0].how else {
        bail!("the counter's input must be a driven cell (`port input: x y z` on the build's edge)");
    };
    let (_, _, sw) = circuit::driver_cells(c.inputs[0].pos, dir);
    let mut outs = c.outputs.clone();
    outs.sort_by(|a, b| a.name.cmp(&b.name));
    if outs.len() != 7 {
        bail!("the counter drives 7 lines (`port output.<name>`); found {}", outs.len());
    }
    let names: Vec<String> = outs.iter().map(|p| p.name.clone()).collect();
    let probes: Vec<redsim::pos::BlockPos> = outs.iter().map(|p| redsim::pos::BlockPos::new(p.pos[0], p.pos[1], p.pos[2])).collect();
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?.with_drivers();
    let (from, to) = circuit::run::watch_box(c, 3);
    let chunks = redsim::reload::box_chunks([from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)]);
    let swp = redsim::pos::BlockPos::new(sw[0], sw[1], sw[2]);
    let on_block = redsim::Block::parse(circuit::INPUT_ON).map_err(|e| anyhow!(e))?;

    let mut rng = Rng(o.seed);
    let mut r = Run::default();
    let mut lines = [false; 7];
    let mut cur: Option<usize> = None;
    let mut succ: [Option<usize>; 7] = [None; 7];
    // the farm: the next slot tick that may summon (None: waiting for an advance), whether the
    // previous slot was missed, the pending golem pulse
    let mut next_slot: Option<u32> = None;
    let mut missed_last = false;
    let mut pulse: Option<Pulse> = None;
    let mut last_adv: Option<u32> = None;
    // the transition window: the tick the old line went low / the new rose, for the current change
    let mut change_start: Option<u32> = None;
    let mut overlap = 0u32;
    let mut gap = 0u32;
    let horizon = o.warmup + (o.pulses as u32 + 50) * 400;
    let mut reload_at: Vec<u32> = reloads.to_vec();
    reload_at.sort_unstable();
    let mut ri = 0usize;
    let mut stop_after: Option<u32> = None;
    let dump_on = o.dump.is_some();
    let push_dump = |r: &mut Run, t: u32, s: &str| {
        let _ = writeln!(r.dump, "t={t} gt={t} {s}");
    };
    let stopped = clock::sim::drive_live(&cc.clock, &[], horizon, from, to, |t, w| {
        r.ticks = t;
        while ri < reload_at.len() && reload_at[ri] <= t {
            if reload_at[ri] == t {
                redsim::reload::reload_chunks(w, &chunks);
            }
            ri += 1;
        }
        // the input: the judge's switch, 2 gt (its repeater) ahead of the input cell's edge
        if let Some(p) = pulse {
            if t + 2 == p.rise {
                w.set_block(swp, on_block, redsim::world::flags::ALL);
            }
            if t + 2 == p.fall {
                w.set_block(swp, redsim::Block::Air, redsim::world::flags::ALL);
            }
            if t == p.rise && dump_on && t < o.dump_ticks {
                push_dump(&mut r, t, &format!("mark pulse golem (width {})", p.fall - p.rise));
            }
        }
        // the lines
        let now: Vec<bool> = probes.iter().map(|q| w.best_neighbor_signal(*q) > 0).collect();
        for k in 0..7 {
            if now[k] != lines[k] {
                lines[k] = now[k];
                if dump_on && t < o.dump_ticks {
                    push_dump(&mut r, t, &format!("signal {}={}", names[k], if now[k] { "on" } else { "off" }));
                }
            }
        }
        let high: Vec<usize> = (0..7).filter(|&k| lines[k]).collect();
        if t >= o.warmup / 2 {
            match high.len() {
                0 => gap += 1,
                1 => {}
                _ => overlap += 1,
            }
            if high.len() != 1 && change_start.is_none() {
                change_start = Some(t);
            }
        }
        let violate = |r: &mut Run, rule: &str, lane: &str, text: String| {
            r.violations.push((t, rule.to_string(), lane.to_string(), text.clone()));
            let _ = writeln!(r.dump, "t={t} gt={t} VIOLATION {rule} {lane}: {text}");
        };
        if high.len() == 1 {
            let k = high[0];
            match cur {
                None if t >= o.warmup / 2 => {
                    cur = Some(k);
                    last_adv = Some(t);
                    next_slot = Some((t + o.ready).div_ceil(100) * 100);
                }
                Some(c0) if c0 != k => {
                    // an advance
                    r.overlap_max = r.overlap_max.max(overlap);
                    r.gap_max = r.gap_max.max(gap);
                    overlap = 0;
                    gap = 0;
                    change_start = None;
                    let by = match pulse {
                        // (an advance a few gt after the rise cannot be this pulse's: the counter
                        // takes about T_c; it is the fallback the pulse came on top of)
                        Some(p) if !p.answered && t >= p.rise + o.tc_min => By::Golem,
                        _ => By::Fallback,
                    };
                    match succ[c0] {
                        None => succ[c0] = Some(k),
                        Some(s) if s != k => violate(&mut r, "order", &names[k], format!("{} -> {} but the cycle goes {} -> {}", names[c0], names[k], names[c0], names[s])),
                        _ => {}
                    }
                    match by {
                        By::Golem => {
                            let p = pulse.as_mut().unwrap();
                            p.answered = true;
                            let tc = t - p.rise;
                            r.tc.push(tc);
                            if tc > o.tc {
                                violate(&mut r, "late", &names[k], format!("T_c {tc} gt > {} after the golem pulse at {}", o.tc, p.rise));
                            }
                        }
                        By::Fallback => {
                            r.fallbacks += 1;
                            let m = last_adv.map(|l| t as i64 - l as i64 - 100).unwrap_or(0);
                            // (before the first golem advance the design is still starting: its
                            // own first edges, the monostable's first lap; nothing to check yet)
                            let started = r.tc.len() > 0;
                            if started {
                                r.m.push(m);
                            }
                            if started && (m < o.m.0 || m > o.m.1) {
                                violate(&mut r, "fallback", &names[k], format!("a fallback advance {} gt after the previous advance: m {m} outside {}..{}", m + 100, o.m.0, o.m.1));
                            }
                            if dump_on && t < o.dump_ticks {
                                push_dump(&mut r, t, "mark pulse fallback");
                            }
                        }
                    }
                    r.advances.push(Advance { t, from: c0, to: k, by });
                    cur = Some(k);
                    last_adv = Some(t);
                    next_slot = Some((t + o.ready).div_ceil(100) * 100);
                }
                _ => {}
            }
        }
        // a golem pulse no advance answered in time
        if let Some(p) = pulse
            && !p.answered
            && t == p.rise + o.tc + 1
        {
            // contract (c), ironspec 2026-11-10: a golem pulse within 95 gt after a fallback
            // advance is out of contract (the farm summons only at slots); it happens here after
            // long miss runs, when the fallback's drift reaches the next slot: the counter must
            // still not skip or double (the order check), but it may absorb the pulse
            let absorbed = r.advances.last().is_some_and(|a| a.by == By::Fallback && p.rise < a.t + 95);
            if absorbed {
                r.absorbed += 1;
            } else {
                violate(&mut r, "missed", "pulse", format!("no advance within {} gt of the golem pulse at {}", o.tc, p.rise));
            }
        }
        if let Some(p) = pulse
            && (p.answered || t > p.rise + o.tc)
            && t > p.fall
        {
            pulse = None;
        }
        // a stall: no advance for 100 + m + tc
        if let Some(l) = last_adv
            && t == l + 100 + o.m.1 as u32 + o.tc
        {
            violate(&mut r, "stall", "counter", format!("no advance since {l}"));
        }
        // the farm's slots: decided 3 gt ahead (the judge's switch leads the input by its
        // repeater's 2 gt, and the input is driven at the top of the next callbacks)
        if let Some(s) = next_slot
            && t + 3 == s
            && pulse.is_none()
        {
            next_slot = None;
            let miss = rng.unit() < if missed_last { o.burst.max(o.miss) } else { o.miss };
            if miss && r.golems > 0 {
                missed_last = true;
                r.misses += 1;
                if dump_on && s < o.dump_ticks {
                    let _ = writeln!(r.dump, "t={s} gt={s} mark slot missed");
                }
            } else {
                missed_last = false;
                let rise = s + rng.range(0, o.jitter);
                let width = rng.range(o.width.0, o.width.1);
                pulse = Some(Pulse { rise, fall: rise + width, answered: false });
                r.pulses.push((rise, rise + width));
                r.golems += 1;
            }
        }
        if r.golems >= o.pulses && stop_after.is_none() {
            stop_after = Some(t + 300);
        }
        if r.violations.len() >= 20 {
            return false;
        }
        stop_after.is_none_or(|s| t < s)
    });
    match stopped {
        Ok(None) => {}
        Ok(Some((_, why))) if why.is_empty() => {}
        Ok(Some((t, why))) => r.stopped = Some(format!("the run stopped at tick {t}: {why}")),
        Err((_, why)) => bail!("{why}"),
    }
    Ok(r)
}

fn spread(v: &[u32]) -> String {
    if v.is_empty() {
        return "none".into();
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    format!("{}..{} (median {})", s[0], s[s.len() - 1], s[s.len() / 2])
}

/// `mctas circuit memory counter-bench`.
pub fn bench(design: &Path, o: &Opts) -> Result<bool> {
    let c = Circuit::load(design)?;
    let t0 = std::time::Instant::now();
    let base = run_once(&c, o, &[])?;
    if let Some(path) = &o.stimulus_out {
        let tokens: Vec<String> = base.pulses.iter().map(|(a, b)| format!("{a}+ {b}-")).collect();
        let name = c.inputs.first().map(|i| i.name.clone()).unwrap_or_else(|| "in".into());
        let stim = format!("stimulus: {name} {}", tokens.join(" "));
        // the design's own text with the stimulus after its `name:` line (or first)
        let text = std::fs::read_to_string(design)?;
        let mut out = String::new();
        let mut put = false;
        for l in text.lines() {
            out.push_str(l);
            out.push('\n');
            if !put && l.starts_with("name:") {
                out.push_str(&stim);
                out.push('\n');
                put = true;
            }
        }
        if !put {
            out = format!("{stim}\n{out}");
        }
        std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    }
    let mut ok = base.violations.is_empty() && base.stopped.is_none();
    let mut restart_line = String::from("no restart run");
    if o.restarts > 0 && ok {
        let mut rng = Rng(o.seed ^ 0x5EED_0F_FA11);
        let span = base.ticks.max(o.warmup + 1);
        let at: Vec<u32> = (0..o.restarts).map(|_| rng.range(o.warmup, span - 1)).collect();
        let again = run_once(&c, o, &at)?;
        let same = again.advances == base.advances && again.violations.is_empty();
        restart_line = if same {
            format!("{} restarts (chunk reloads at random ticks): the same {} advances, tick for tick", o.restarts, base.advances.len())
        } else {
            ok = false;
            let first = base.advances.iter().zip(again.advances.iter()).position(|(a, b)| a != b).unwrap_or(base.advances.len().min(again.advances.len()));
            format!(
                "{} restarts: DIFFERENT from advance {first} ({:?} vs {:?}){}",
                o.restarts,
                base.advances.get(first),
                again.advances.get(first),
                again.violations.first().map(|v| format!("; first violation at {}: {} {} {}", v.0, v.1, v.2, v.3)).unwrap_or_default()
            )
        };
    }
    // the same run on the design turned a quarter, half and three quarters (the assembly may
    // build it any way round; redstone's update order can depend on the direction)
    let mut turn_line = String::new();
    if ok && !o.turns.is_empty() {
        let mut same_all = true;
        let mut why = String::new();
        for &k in &o.turns {
            let tc = c.turned_pure(k)?;
            let again = run_once(&tc, o, &[])?;
            if again.advances != base.advances || !again.violations.is_empty() {
                same_all = false;
                let first = base.advances.iter().zip(again.advances.iter()).position(|(a, b)| a != b).unwrap_or(base.advances.len().min(again.advances.len()));
                why = format!(
                    "turned {k}: DIFFERENT from advance {first} ({:?} vs {:?}){}",
                    base.advances.get(first),
                    again.advances.get(first),
                    again.violations.first().map(|v| format!("; first violation at {}: {} {} {}", v.0, v.1, v.2, v.3)).unwrap_or_default()
                );
                break;
            }
        }
        turn_line = if same_all {
            format!("; turned {}: the same advances, tick for tick", o.turns.iter().map(|k| format!("{}", u32::from(*k) * 90)).collect::<Vec<_>>().join(", ") + " degrees")
        } else {
            ok = false;
            format!("; {why}")
        };
    }
    let restart_line = format!("{restart_line}{turn_line}");
    let ms: Vec<i64> = base.m.clone();
    let mline = if ms.is_empty() { "no fallback".to_string() } else { format!("m {}..{}", ms.iter().min().unwrap(), ms.iter().max().unwrap()) };
    println!(
        "{}: {} golem pulses, {} missed slots ({} fallback advances, {} golem pulses absorbed within 95 gt after a fallback: out of contract (c)), {} gt; T_c {}; transition window: up to {} gt with two lines high, up to {} gt with none; fallback {}; {}; {}{}",
        design.display(),
        base.golems,
        base.misses,
        base.fallbacks,
        base.absorbed,
        base.ticks,
        spread(&base.tc),
        base.overlap_max,
        base.gap_max,
        mline,
        restart_line,
        if base.violations.is_empty() { "no violation".to_string() } else { format!("{} violations, first at {}: {} {}: {}", base.violations.len(), base.violations[0].0, base.violations[0].1, base.violations[0].2, base.violations[0].3) },
        base.stopped.as_ref().map(|s| format!("; {s}")).unwrap_or_default()
    );
    println!("  ({:.1} s)", t0.elapsed().as_secs_f64());
    if let Some(p) = &o.dump {
        std::fs::write(p, &base.dump)?;
        println!("  trace: {} (mctas mobfarm timeline --from-dump {} --out DIR)", p.display(), p.display());
    }
    Ok(ok)
}
