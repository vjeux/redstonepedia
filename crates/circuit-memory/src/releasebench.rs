//! `mctas circuit memory release-bench <design>`: the release controller (release-gen) driven alone
//! (day-1 lane 2026-11-10). The golem input is driven directly (10-gt pulses: a redstone block, low
//! = stone); `output.group` and `output.adv` are read every tick. The schedule draws, after each
//! release, one of: a golem in time (94..99 gt after it, before the fallback), an early golem (62..86: held), a missed
//! slot (no golem: the fallback), plus extra pulses 10..44 gt after a release (double detections:
//! merged). An exact model predicts every group window's start; the bench checks each window (start,
//! length 30..36), the three advance pulses (group start + 38, + 58, + 78, 7..12 gt, identical), that
//! nothing else lights, and with `--restarts` that chunk reloads change nothing.

use crate::seqbench::Rng;
use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use std::path::Path;

pub struct Opts {
    pub pulses: u32,
    pub seed: u64,
    pub miss: f64,
    pub early: f64,
    pub double: f64,
    pub restarts: u32,
    pub game: u32,
    pub turns: Vec<u8>,
    /// Place the design raw (ironspec 11:40, "works as placed"): written states, block entities
    /// merged, no shape or neighbour updates, no settle.
    pub raw: bool,
}

/// `clock::sim::drive_live` with the design placed RAW: every block in its written state, the block
/// entities merged, no shape or neighbour updates and no settle ticks; then the ticks, `each`
/// called before each one (false stops the run).
pub fn drive_raw(c: &clock::Clock, ticks: u32, from: [i32; 3], to: [i32; 3], mut each: impl FnMut(u32, &mut redsim::world::World) -> bool) -> Result<Option<(u32, String)>, String> {
    let mut blocks = clock::sim::parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let min = redsim::pos::BlockPos::new(from[0], from[1], from[2]);
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut w = redsim::world::World::new(min, size);
    w.place_raw(&blocks);
    let lines: Vec<(redsim::pos::BlockPos, &str)> = c.design.block_entities.iter().map(|(p, s)| (redsim::pos::BlockPos::new(p[0], p[1], p[2]), s.as_str())).collect();
    redsim::redstone::container::apply_block_entities(&mut w, lines).map_err(|e| format!("nbt: {e}"))?;
    for t in 0..=ticks {
        if !each(t, &mut w) {
            return Ok(Some((t, String::new())));
        }
        if t < ticks {
            w.tick();
        }
    }
    Ok(None)
}

/// The circuit's constants (sim-measured on release-gen v1): group = golem + 4 when idle; a held
/// golem releases at the previous group + 92; the fallback at the previous group + 106.
const IDLE: u64 = 4;
const HOLD: u64 = 92;
const FALLBACK: u64 = 106;

pub fn bench(design: &Path, o: &Opts) -> Result<bool> {
    let mut recorded = Vec::new();
    let mut trace = Vec::new();
    let mut ok = run(design, o, &mut recorded, &[], &mut trace)?;
    let ticks = trace.last().map(|t| t.0 + 200).unwrap_or(1000);
    if o.restarts > 0 {
        let mut rng = Rng(o.seed ^ 0x0e10_ad5);
        let mut reloads: Vec<u32> = (0..o.restarts).map(|_| rng.range(100, ticks - 100)).collect();
        reloads.sort_unstable();
        let (mut rec2, mut trace2) = (Vec::new(), Vec::new());
        run(design, o, &mut rec2, &reloads, &mut trace2)?;
        if trace2 == trace {
            println!("{} restarts (chunk reloads at random ticks): the same {} output changes, tick for tick", o.restarts, trace.len());
        } else {
            let i = trace.iter().zip(&trace2).position(|(a, b)| a != b).unwrap_or(trace.len().min(trace2.len()));
            println!("RESTARTS DIVERGE at output change {i}: {:?} vs {:?}", trace.get(i), trace2.get(i));
            ok = false;
        }
    }
    if o.game > 0 {
        let probes: Vec<String> = Circuit::load(design)?.clock.design.ports.keys().filter(|k| k.starts_with("output.")).cloned().collect();
        ok = crate::schedbench::game_check_named(design, o.game.min(ticks), &recorded, &probes)? && ok;
    }
    if !o.turns.is_empty() {
        // the same run turned (redstone's update order can depend on the direction): the same
        // output changes, tick for tick
        let c = Circuit::load(design)?;
        for &k in &o.turns {
            let (mut rec3, mut trace3) = (Vec::new(), Vec::new());
            let tc = c.turned_pure(k)?;
            let fine = run_circuit(&tc, design, o, &mut rec3, &[], &mut trace3, false)?;
            if trace3 == trace && fine {
                println!("turned {} degrees: the same {} output changes, tick for tick", u32::from(k) * 90, trace.len());
            } else {
                let i = trace.iter().zip(&trace3).position(|(a, b)| a != b).unwrap_or(trace.len().min(trace3.len()));
                println!("TURNED {} DEGREES DIFFERS at output change {i}: {:?} vs {:?}", u32::from(k) * 90, trace.get(i), trace3.get(i));
                ok = false;
            }
        }
    }
    Ok(ok)
}

fn run(design: &Path, o: &Opts, recorded: &mut Vec<(u32, clock::Action)>, reloads: &[u32], trace: &mut Vec<(u32, u8)>) -> Result<bool> {
    run_circuit(&Circuit::load(design)?, design, o, recorded, reloads, trace, true)
}

fn run_circuit(c: &Circuit, design: &Path, o: &Opts, recorded: &mut Vec<(u32, clock::Action)>, reloads: &[u32], trace: &mut Vec<(u32, u8)>, print: bool) -> Result<bool> {
    let c = c.clone();
    let find = |name: &str| -> Result<redsim::pos::BlockPos> {
        let p = c.clock.design.ports.get(name).ok_or_else(|| anyhow!("{}: no port {name}", design.display()))?;
        Ok(redsim::pos::BlockPos::new(p[0], p[1], p[2]))
    };
    let golem = find("input.golem")?;
    let group = find("output.group")?;
    let adv = find("output.adv")?;
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    let chunks = redsim::reload::box_chunks([from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)]);
    let on = redsim::Block::parse("minecraft:redstone_block").map_err(|e| anyhow!(e))?;
    let off = redsim::Block::parse("minecraft:stone").map_err(|e| anyhow!(e))?;
    let mut rng = Rng(o.seed);
    // nothing moves at power-on (the fallback waits for the first golem): the first golem at 200
    let mut prev_group: u64 = 0;
    let mut pulses: Vec<u64> = Vec::new(); // golem rises still to come, sorted
    let mut expect: u64; // the next group start the model predicts
    let (mut n_pulses, mut n_idle, mut n_hold, mut n_fall, mut n_double) = (0u32, 0u32, 0u32, 0u32, 0u32);
    // draw the slot after the release whose group opened at `g`: returns the predicted next group
    // start and its kind (0 a golem in time, 1 a held golem, 2 the fallback)
    let mut draw = |g: u64, _kind: u8, rng: &mut Rng, pulses: &mut Vec<u64>, counts: &mut (u32, u32, u32, u32, u32)| -> (u64, u8) {
        if rng.unit() < o.double {
            // a double detection 10..44 gt after the release: it ends before the earliest real slot
            // (60), since a pulse overlapping it would be one input pulse
            pulses.push(g + rng.range(10, 44) as u64);
            counts.4 += 1;
        }
        let u = rng.unit();
        let next = if u < o.miss {
            counts.3 += 1;
            (g + FALLBACK, 2)
        } else if u < o.miss + o.early {
            let at = g + rng.range(60, 84) as u64;
            pulses.push(at);
            counts.2 += 1;
            (g + HOLD, 1)
        } else {
            // in time: before the fallback (a golem after it is merged into the fallback's release)
            let at = g + rng.range(94, 99) as u64;
            pulses.push(at);
            counts.1 += 1;
            (at + IDLE, 0)
        };
        pulses.sort_unstable();
        counts.0 += 1;
        next
    };
    let mut counts = (0u32, 0u32, 0u32, 0u32, 0u32);
    pulses.push(200);
    expect = 200 + IDLE;
    let mut expect_fb: u8 = 0;
    let mut recent: Vec<u64> = Vec::new();
    let ticks_cap = (o.pulses as u64 * 140 + 1000) as u32;
    let (mut group_was, mut adv_was) = (false, false);
    let (mut group_rise, mut adv_rise) = (0u64, 0u64);
    let mut advs: Vec<(u64, u64)> = Vec::new(); // this release's advance pulses (start, width)
    let mut bad = 0u32;
    let mut ri = 0usize;
    let mut released = 0u32;
    let mut done_at: Option<u32> = None;
    let mut report = |msg: String, bad: &mut u32| {
        if *bad < 12 {
            println!("{msg}");
        }
        *bad += 1;
    };
    let cb = |t: u32, w: &mut redsim::world::World| -> bool {
        let g = t as u64;
        while ri < reloads.len() && reloads[ri] <= t {
            if reloads[ri] == t {
                redsim::reload::reload_chunks(w, &chunks);
            }
            ri += 1;
        }
        let high = pulses.iter().any(|&p| g >= p && g < p + 10) || (pulses.first().is_some_and(|&p| p > g + 100_000));
        if pulses.first().is_some_and(|&p| p == g) {
            recent.push(g);
            if recent.len() > 6 {
                recent.remove(0);
            }
        }
        let is_on = matches!(w.get(golem), redsim::Block::RedstoneBlock);
        if is_on != high {
            w.set_block(golem, if high { on } else { off }, redsim::world::flags::ALL);
            recorded.push((t, clock::Action::Place { pos: [golem.x, golem.y, golem.z], state: if high { "minecraft:redstone_block".into() } else { "minecraft:stone".into() }, nbt: None }));
        }
        pulses.retain(|&p| p + 10 > g);
        let gv = w.best_neighbor_signal(group) > 0;
        let av = w.best_neighbor_signal(adv) > 0;
        if gv != group_was {
            trace.push((t, 0 | gv as u8));
        }
        if av != adv_was {
            trace.push((t, 2 | av as u8));
        }
        if t > 20 {
            if gv && !group_was {
                // a release: check the previous release's advance pulses, then this start
                if released > 0 {
                    let want: Vec<u64> = [38, 58, 78].iter().map(|d| prev_group + d).collect();
                    let starts: Vec<u64> = advs.iter().map(|a| a.0).collect();
                    let widths: Vec<u64> = advs.iter().map(|a| a.1).collect();
                    if starts != want || widths.iter().any(|w| !(7..=12).contains(w)) || widths.windows(2).any(|p| p[0] != p[1]) {
                        report(format!("t={g}: release at {prev_group}: advance pulses {advs:?}, want starts {want:?}, widths 7..12 identical"), &mut bad);
                    }
                }
                advs.clear();
                let ok_t = g == expect;
                let _ = expect_fb;
                if !ok_t && expect != u64::MAX {
                    report(format!("t={g}: group opened, the model wanted {expect} (previous group {prev_group}; recent golem pulses {recent:?})"), &mut bad);
                }
                released += 1;
                prev_group = g;
                group_rise = g;
                if counts.0 < o.pulses {
                    let (e, fb) = draw(g, expect_fb, &mut rng, &mut pulses, &mut counts);
                    expect = e;
                    expect_fb = fb;
                } else {
                    expect = u64::MAX;
                    done_at.get_or_insert(t + 200);
                }
            }
            if !gv && group_was && group_rise > 0 {
                let len = g - group_rise;
                if !(30..=36).contains(&len) {
                    report(format!("t={g}: group window {group_rise}..{g} is {len} gt, want 30..36"), &mut bad);
                }
            }
            if av && !adv_was {
                adv_rise = g;
            }
            if !av && adv_was {
                advs.push((adv_rise, g - adv_rise));
            }
            if g > expect + 2 && expect != u64::MAX {
                report(format!("t={g}: no group opened at {expect} (previous group {prev_group})"), &mut bad);
                expect = u64::MAX;
            }
        }
        group_was = gv;
        adv_was = av;
        !done_at.is_some_and(|d| t >= d)
    };
    let res = if o.raw { drive_raw(&cc.clock, ticks_cap, from, to, cb) } else { clock::sim::drive_live(&cc.clock, &[], ticks_cap, from, to, cb).map_err(|(_, why)| why) };
    match res {
        Err(why) => bail!("the run failed: {why}"),
        Ok(Some((t, why))) if !why.is_empty() => bail!("the run stopped at tick {t}: {why}"),
        _ => {}
    }
    (n_pulses, n_idle, n_hold, n_fall, n_double) = counts;
    if print {
        let (lo, hi) = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
        println!("{}: {} blocks, box {}x{}x{}", design.display(), c.clock.design.blocks.len(), hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1);
        println!(
            "{released} releases checked over {} slots: {n_idle} golems in time (group at golem + {IDLE}), {n_hold} early golems held (group at previous + {HOLD}), {n_fall} missed slots (fallback at previous + {FALLBACK}), {n_double} double detections merged; {bad} violations",
            n_pulses
        );
    }
    Ok(bad == 0 && released > 0)
}

/// `release-trace`: golem pulses at the given ticks (10 gt each), then every `output.*` port's on
/// intervals (the loop-wired probe file's controls), relative to the first pulse. `faults`:
/// `x,y,z@T+N=BLOCK`, the cell replaced by BLOCK over ticks T..T+N, then put back.
pub fn trace(design: &Path, at: &[u64], det: &[u64], ticks: u32, faults: &[String], watch: &[String]) -> Result<()> {
    let c = Circuit::load(design)?;
    for (k, e) in trace_run(&c, at, det, ticks, faults, watch)? {
        println!("{k:24} {}", e.join(" "));
    }
    Ok(())
}

/// One trace: each output (and watched) port's edges, as `trace` prints them.
fn trace_run(c: &Circuit, at: &[u64], det: &[u64], ticks: u32, faults: &[String], watch: &[String]) -> Result<Vec<(String, Vec<String>)>> {
    let p = c.clock.design.ports.get("input.golem").ok_or_else(|| anyhow!("no input.golem"))?;
    let golem = redsim::pos::BlockPos::new(p[0], p[1], p[2]);
    let mut fs: Vec<(redsim::pos::BlockPos, u64, u64, redsim::Block, Option<redsim::Block>)> = Vec::new();
    // (`reload@T+N`: every chunk unloaded at T and loaded again at T + N (tick-exact's act: a player
    // leaving and coming back); no golem pulse and no other fault while away)
    let mut reloads: Vec<(u64, u64)> = Vec::new();
    for f in faults {
        if let Some(r) = f.strip_prefix("reload@") {
            let (t, n) = r.split_once('+').ok_or_else(|| anyhow!("--fault {f}: reload@T+N"))?;
            reloads.push((t.parse()?, n.parse()?));
            continue;
        }
        let (cell, rest) = f.split_once('@').ok_or_else(|| anyhow!("--fault {f}: x,y,z@T+N=BLOCK"))?;
        let (tn, blk) = rest.split_once('=').ok_or_else(|| anyhow!("--fault {f}: x,y,z@T+N=BLOCK"))?;
        let (t, n) = tn.split_once('+').ok_or_else(|| anyhow!("--fault {f}: x,y,z@T+N=BLOCK"))?;
        let v: Vec<i32> = cell.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            bail!("--fault {f}: x,y,z@T+N=BLOCK");
        }
        let b = redsim::Block::parse(blk).map_err(|e| anyhow!("--fault {f}: {e}"))?;
        fs.push((redsim::pos::BlockPos::new(v[0], v[1], v[2]), t.parse()?, n.parse()?, b, None));
    }
    let mut outs: Vec<(String, redsim::pos::BlockPos)> = c.clock.design.ports.iter().filter(|(k, _)| k.starts_with("output.")).map(|(k, p)| (k.clone(), redsim::pos::BlockPos::new(p[0], p[1], p[2]))).collect();
    for wc in watch {
        let v: Vec<i32> = wc.split(',').map(|s| s.trim().parse()).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            bail!("--watch {wc}: x,y,z");
        }
        outs.push((format!("watch.{wc}"), redsim::pos::BlockPos::new(v[0], v[1], v[2])));
    }
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    let on = redsim::Block::parse("minecraft:redstone_block").map_err(|e| anyhow!(e))?;
    let off = redsim::Block::parse("minecraft:stone").map_err(|e| anyhow!(e))?;
    let mut was = vec![false; outs.len()];
    let mut lvl = vec![0i32; outs.len()];
    let mut edges: Vec<Vec<String>> = vec![Vec::new(); outs.len()];
    let base = at.first().copied().unwrap_or(0) as i64;
    let res = clock::sim::drive_live(&cc.clock, &[], ticks, from, to, |t, w| {
        let g = t as u64;
        for &(rt, rn) in &reloads {
            if g == rt {
                redsim::reload::unload_all(w);
            } else if g == rt + rn {
                redsim::reload::load_all(w);
            }
        }
        if reloads.iter().any(|&(rt, rn)| g >= rt && g < rt + rn) {
            return true;
        }
        // (a fault's swap or put-back that fell while away happens now, at the load: the day window's
        // detector stand-in is the sky, which changes while the player is away)
        let back = reloads.iter().find(|&&(rt, rn)| g == rt + rn).copied();
        if let Some((rt, _)) = back {
            for f in fs.iter_mut() {
                if f.1 >= rt && f.1 < g {
                    f.4 = Some(w.get(f.0));
                    w.set_block(f.0, f.3, redsim::world::flags::ALL);
                }
                if f.2 > 0 && f.1 + f.2 >= rt && f.1 + f.2 < g {
                    if let Some(orig) = f.4.take() {
                        w.set_block(f.0, orig, redsim::world::flags::ALL);
                    }
                }
            }
        }
        let high = at.iter().any(|&a| g >= a && g < a + 10);
        // the release run's detector rail (-3,1,4) stood in by a redstone block, 4 gt from each
        // `det` tick (the loop probe file has no rail)
        if !det.is_empty() {
            let dp = redsim::pos::BlockPos::new(-3, 1, 4);
            let dh = det.iter().any(|&a| g >= a && g < a + 4);
            if matches!(w.get(dp), redsim::Block::RedstoneBlock) != dh {
                w.set_block(dp, if dh { on } else { redsim::Block::Air }, redsim::world::flags::ALL);
            }
        }
        if matches!(w.get(golem), redsim::Block::RedstoneBlock) != high {
            w.set_block(golem, if high { on } else { off }, redsim::world::flags::ALL);
        }
        for f in fs.iter_mut() {
            if g == f.1 {
                f.4 = Some(w.get(f.0));
                w.set_block(f.0, f.3, redsim::world::flags::ALL);
            } else if f.2 > 0 && g == f.1 + f.2 {
                // (N = 0: set and left, the circuit carrying on from it, as a toggled bulb)
                if let Some(orig) = f.4.take() {
                    w.set_block(f.0, orig, redsim::world::flags::ALL);
                }
            }
        }
        for (i, (_, q)) in outs.iter().enumerate() {
            // (`output.lvl_*` ports: the analog level itself, at each change)
            if outs[i].0.starts_with("output.lvl_") {
                let lv = w.best_neighbor_signal(*q) as i32;
                if lv != lvl[i] {
                    edges[i].push(format!("{}@{}", lv, t as i64 - base));
                    lvl[i] = lv;
                }
                continue;
            }
            let v = w.best_neighbor_signal(*q) > 0;
            if v != was[i] {
                edges[i].push(format!("{}{}", if v { "+" } else { "-" }, t as i64 - base));
                was[i] = v;
            }
        }
        true
    });
    if let Err((_, why)) = res {
        bail!("the run failed: {why}");
    }
    Ok(outs.into_iter().map(|(k, _)| k).zip(edges).collect())
}

/// `fault-survey` (ironspec's self-recovery rule): golem pulses every 100 gt from 100 for `n`
/// pulses, the fault-free trace first, then each scenario of `spec` (lines `name | fault; fault
/// | golem edits`: faults as `--fault` takes them, golem edits `-T` a pulse missed, `+T` an extra
/// one; `#` comments). For each: the first tick a control (visit divert, kick, F, T1, T2, REL)
/// departs from the fault-free run, the cycles until every one of them runs again as the
/// fault-free run shifted by a whole number of releases (and that shift), or never; and the
/// visits, kicks and F pulses gained or lost (fault-free run's last 500 gt left out).
pub fn fault_survey(design: &Path, spec: &Path, n: u64) -> Result<()> {
    let c = Circuit::load(design)?;
    let at0: Vec<u64> = (1..=n).map(|k| 100 * k).collect();
    let ticks = (100 * n + 400) as u32;
    let key = ["output.divert", "output.ka", "output.force", "output.t1", "output.t2", "output.rel"];
    let parse = |r: &[(String, Vec<String>)]| -> Vec<Vec<(bool, i64)>> {
        key.iter()
            .map(|k| {
                r.iter().find(|(n, _)| n == k).map(|(_, e)| e.iter().filter_map(|s| {
                    let (on, t) = if let Some(t) = s.strip_prefix('+') { (true, t) } else { (false, s.strip_prefix('-')?) };
                    t.parse::<i64>().ok().map(|t| (on, t))
                }).collect()).unwrap_or_default()
            })
            .collect()
    };
    let mut base_faults: Vec<String> = Vec::new();
    let mut base_edits: Vec<String> = Vec::new();
    let text = std::fs::read_to_string(spec)?;
    // (`base | faults | edits`: applied to the fault-free run and to every scenario: a day window
    // for the night latch's edges, say)
    for line in text.lines().map(|l| l.trim()).filter(|l| l.starts_with("base")) {
        let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        base_faults = parts.get(1).map(|s| s.split(';').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect()).unwrap_or_default();
        base_edits = parts.get(2).map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
    }
    let edit = |at: &mut Vec<u64>, eds: &[String]| -> Result<()> {
        for ed in eds {
            let t: u64 = ed[1..].parse()?;
            if ed.starts_with('-') {
                at.retain(|&a| a != t);
            } else {
                at.push(t);
            }
        }
        at.sort_unstable();
        Ok(())
    };
    let mut at0 = at0;
    edit(&mut at0, &base_edits)?;
    let base = parse(&trace_run(&c, &at0, &[], ticks, &base_faults, &[])?);
    let end = (100 * n) as i64 - 500;
    let count = |v: &[Vec<(bool, i64)>], i: usize| v[i].iter().filter(|(on, t)| *on && *t > 0 && *t < end).count() as i64;
    println!("{:28} {:>10} {:>26} {:>8} {:>8} {:>8}", "scenario", "departs", "recovered", "visits", "kicks", "F");
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("base") {
            continue;
        }
        let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        let name = parts[0];
        let mut faults: Vec<String> = base_faults.clone();
        faults.extend(parts.get(1).map(|s| s.split(';').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect::<Vec<_>>()).unwrap_or_default());
        let mut at = at0.clone();
        edit(&mut at, &parts.get(2).map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>()).unwrap_or_default())?;
        // (times stay relative to the first fault-free pulse, as `trace` prints them)
        let mut at_shift = at.clone();
        if at_shift.first() != Some(&100) {
            at_shift.insert(0, 100);
            at_shift.dedup();
        }
        let r = parse(&trace_run(&c, &at_shift, &[], ticks, &faults, &[])?);
        let departs = (0..key.len()).filter_map(|i| {
            let (a, b) = (&base[i], &r[i]);
            (0..a.len().max(b.len())).find(|&j| a.get(j) != b.get(j)).map(|j| a.get(j).map(|e| e.1).unwrap_or(i64::MAX).min(b.get(j).map(|e| e.1).unwrap_or(i64::MAX)))
        }).min();
        let recovered = match departs {
            None => "unchanged".to_string(),
            Some(d) => {
                // the first tick T (in 100s from the departure) from which every control's edges in
                // [T, end) are the fault-free ones shifted by s releases
                let mut found = None;
                'outer: for k in 0..((end - d) / 100).max(0) {
                    let t0 = d + 100 * k;
                    for s in -4i64..=4 {
                        let ok = (0..key.len()).all(|i| {
                            let a: Vec<(bool, i64)> = base[i].iter().map(|&(o, t)| (o, t + 100 * s)).filter(|&(_, t)| t >= t0 && t < end).collect();
                            let b: Vec<(bool, i64)> = r[i].iter().copied().filter(|&(_, t)| t >= t0 && t < end).collect();
                            a == b
                        });
                        if ok {
                            found = Some((k, s));
                            break 'outer;
                        }
                    }
                }
                match found {
                    Some((k, s)) => format!("after {k} cycles, shift {s:+}"),
                    None => "NEVER".to_string(),
                }
            }
        };
        println!(
            "{:28} {:>10} {:>26} {:>+8} {:>+8} {:>+8}",
            name,
            departs.map(|d| format!("+{d}")).unwrap_or_else(|| "-".into()),
            recovered,
            count(&r, 0) - count(&base, 0),
            count(&r, 1) - count(&base, 1),
            count(&r, 2) - count(&base, 2)
        );
    }
    Ok(())
}
