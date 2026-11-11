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

/// `release-sweep` (the parent 22:23: sweeps and surveys from one Rust command, never a shell
/// loop): the wired farm generated once, or once per value when `knob` names one of the
/// generator's environment knobs (e.g. `MCTAS_K2_KICK`), then `run` (a command line, split on
/// spaces, no shell; `{design}` stands for the generated file, `{value}` for the value) once per
/// value, one after the other; per value it prints the runner's problems and visits lines and every
/// line holding one of `show`'s strings.
#[allow(clippy::too_many_arguments)]
pub fn release_sweep(loop_file: &Path, offdelay: &Path, siding: Option<&Path>, siding_ctl: Option<&Path>, pad: Option<&Path>, knob: Option<&str>, values: &str, run: &str, show: &[String]) -> Result<()> {
    let vals: Vec<&str> = values.split('|').map(|v| v.trim()).filter(|v| !v.is_empty()).collect();
    if vals.is_empty() {
        bail!("--values: give v1|v2|...");
    }
    let dir = std::env::temp_dir().join(format!("release-sweep-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let make = |out: &Path| -> Result<()> { crate::releasegen::generate_loop(out, loop_file, offdelay, siding, siding_ctl, pad) };
    let once = dir.join("design.txt");
    if knob.is_none() {
        make(&once)?;
    }
    let mut rows: Vec<String> = Vec::new();
    for (i, v) in vals.iter().enumerate() {
        let design = match knob {
            Some(k) => {
                // (the generator reads its knobs from the environment, as `release-gen` does)
                unsafe { std::env::set_var(k, v) };
                let out = dir.join(format!("design-{i}.txt"));
                make(&out)?;
                out
            }
            None => once.clone(),
        };
        let args: Vec<String> = run.split_whitespace().map(|t| t.replace("{design}", &design.display().to_string()).replace("{value}", v)).collect();
        let Some((prog, rest)) = args.split_first() else { bail!("--run is empty") };
        let o = std::process::Command::new(prog).args(rest).output()?;
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let mut row = format!("== {v}");
        for l in text.lines() {
            let t = l.trim_start();
            let key = t.ends_with(" problems:") || t.ends_with(" problems") || t.starts_with("visits:") || t.contains("seeds clean") || t.contains("riders hurt");
            if key || show.iter().any(|s| l.contains(s.as_str())) {
                row.push_str("\n   ");
                row.push_str(&l.chars().take(240).collect::<String>());
            }
        }
        println!("{row}");
        rows.push(row);
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// `price` (the fold's pricing, the parent and ironspec 2026-11-10 23:50: "compact: volume
/// decides", dig depth priced in build minutes, horizontal over vertical the tie-breaker): per
/// design, its blocks, box and volume; the box's depth under the ground layer and its dug cells
/// (footprint x depth: the judge's site footprint, `redsim::entity::mob::site::footprint`); the
/// dig's minutes at the stage's price; the void bill's build minutes (the judge's `build minutes`,
/// `circuit::mobfarm::site_bill` with no site) and its `place:edge` share (blocks placed in the
/// air, which a dug build places from its dug floor instead).
pub fn price(designs: &[std::path::PathBuf], ground: i32, stage: &str) -> Result<()> {
    let st = redsim::entity::mob::prices::stage_ix(stage);
    let dig = redsim::entity::mob::prices::Prices::get().map_err(|e| anyhow!(e))?.price("dig", st).map_err(|e| anyhow!(e))?;
    println!("design\tblocks\tbox\tsize\tvolume\tbelow\tdug (box)\tdug (own cells)\tdig min (box)\tbuild min\tof it mid-air\tbuild + dig (box)");
    for d in designs {
        let c = Circuit::load(d)?;
        let (lo, hi) = c.clock.design.bounds().unwrap_or(([0; 3], [0; 3]));
        let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let volume = size.iter().map(|s| *s as i64).product::<i64>();
        let below = (ground - lo[1]).max(0);
        let dug = size[0] as i64 * size[2] as i64 * below as i64;
        // (the lower bound: only the design's own cells under the ground layer, as a farm dug
        // into rock is priced (`site::spots_for_spec`); its air under the ground stays natural
        // there, which a redstone build cannot always keep)
        let own = c.clock.design.block_list().iter().filter(|(p, _)| p[1] < ground).count();
        let bill = circuit::mobfarm::site_bill(&c, None, st).map_err(|e| anyhow!(e))?;
        let edge = bill.items.get("place:edge").map_or(0.0, |x| x.1);
        let (bm, dm) = (bill.seconds / 60.0, dug as f64 * dig / 60.0);
        println!(
            "{}\t{}\t[{},{},{}]..[{},{},{}]\t{}x{}x{}\t{volume}\t{below}\t{dug}\t{own}\t{dm:.1}\t{bm:.1}\t{:.1}\t{:.1}",
            d.file_name().map_or_else(|| d.display().to_string(), |f| f.to_string_lossy().into_owned()),
            c.clock.design.blocks.len(),
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2],
            size[0], size[1], size[2],
            edge / 60.0,
            bm + dm
        );
    }
    Ok(())
}

/// `bed-distance` (ironspec 2026-11-11: the shared bed >= 16 + margin from every place a
/// non-sleeping villager can be at night): the least Euclidean distance from a bed block's centre
/// to a rider's place on a rail (the rail cell's centre, 0.35 up: a villager in a minecart) inside
/// the riders' box (the loop's carts carry the riders while the bed is taken; the junctions carry
/// only the napper itself), the five nearest with where.
pub fn bed_distance(design: &Path, riders: &str) -> Result<()> {
    let c = Circuit::load(design)?;
    let v: Vec<i32> = riders.split(',').map(|n| n.trim().parse()).collect::<std::result::Result<_, _>>().map_err(|e| anyhow!("--riders {riders}: {e}"))?;
    if v.len() != 6 {
        bail!("--riders {riders}: want x0,y0,z0,x1,y1,z1");
    }
    let inside = |p: &[i32; 3]| (v[0]..=v[3]).contains(&p[0]) && (v[1]..=v[4]).contains(&p[1]) && (v[2]..=v[5]).contains(&p[2]);
    let blocks = c.clock.design.block_list();
    let id = |s: &str| s.split('[').next().unwrap_or("").trim_start_matches("minecraft:").to_string();
    let beds: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| id(s).ends_with("_bed")).map(|(p, _)| *p).collect();
    if beds.is_empty() {
        bail!("{}: no bed", design.display());
    }
    let mut near: Vec<(f64, [i32; 3], [i32; 3])> = Vec::new();
    for (r, s) in blocks.iter().filter(|(p, s)| id(s).ends_with("rail") && inside(p)) {
        let _ = s;
        let (rx, ry, rz) = (r[0] as f64 + 0.5, r[1] as f64 + 0.35, r[2] as f64 + 0.5);
        let best = beds.iter().map(|b| (((b[0] as f64 + 0.5 - rx).powi(2) + (b[1] as f64 + 0.5 - ry).powi(2) + (b[2] as f64 + 0.5 - rz).powi(2)).sqrt(), *b)).fold((f64::MAX, [0; 3]), |a, x| if x.0 < a.0 { x } else { a });
        near.push((best.0, *r, best.1));
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    match near.first() {
        Some((d, r, b)) => println!("{}: the bed's nearest rider place {d:.2} (rail {},{},{}, bed {},{},{}); margin over 16: {:+.2}", design.display(), r[0], r[1], r[2], b[0], b[1], b[2], d - 16.0),
        None => println!("{}: no rail inside the riders' box", design.display()),
    }
    for (d, r, _) in near.iter().skip(1).take(4) {
        println!("  {d:.2} at rail {},{},{}", r[0], r[1], r[2]);
    }
    Ok(())
}

/// `watch-events`: a `queue-loop-run --watch` log (lines `cS +T: cart | cart ... | gates ... |
/// x,y,z state | ...`) as events, in gt after slot `from`'s start: each watched cell's state
/// changes (its `power=`/`powered=`/`lit=`/`triggered=` and shape fields), each watched cart's
/// starts (speed above 0.001 after rest) and stops, and the watched cells' columns it rides over
/// (the cart's block cell equal to a watched cell or the one above it).
pub fn watch_events(trace: &Path, from: u64) -> Result<()> {
    let text = std::fs::read_to_string(trace)?;
    let mut last_cell: std::collections::BTreeMap<String, String> = Default::default();
    let mut moving: Vec<bool> = Vec::new();
    let mut on_cell: Vec<Option<String>> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('c') else { continue };
        let Some((head, body)) = rest.split_once(": ") else { continue };
        let Some((slot, off)) = head.split_once(" +") else { continue };
        let (Ok(slot), Ok(off)) = (slot.parse::<u64>(), off.parse::<u64>()) else { continue };
        let t = (slot * 100 + off) as i64 - (from * 100) as i64;
        let parts: Vec<&str> = body.split(" | ").collect();
        let gates = parts.iter().position(|p| p.starts_with("gates")).unwrap_or(parts.len());
        let cells: Vec<(String, String)> = parts[gates + 1..].iter().filter_map(|p| p.split_once(' ').map(|(a, b)| (a.to_string(), b.trim_start_matches("minecraft:").to_string()))).collect();
        for (k, cart) in parts[..gates].iter().enumerate() {
            if moving.len() <= k {
                moving.push(false);
                on_cell.push(None);
            }
            let f: Vec<&str> = cart.split_whitespace().collect();
            let pos: Vec<f64> = f.first().map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect()).unwrap_or_default();
            let v: f64 = f.get(1).and_then(|s| s.strip_prefix('v')).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            if pos.len() != 3 {
                continue;
            }
            let m = v > 0.001;
            if m != moving[k] {
                println!("{t:+6} cart {k}: {} at {:.2},{:.2},{:.2}", if m { "starts" } else { "stops" }, pos[0], pos[1], pos[2]);
                moving[k] = m;
            }
            let c = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
            let hit = cells.iter().map(|(p, _)| p.clone()).find(|p| {
                let q: Vec<i32> = p.split(',').filter_map(|n| n.parse().ok()).collect();
                q.len() == 3 && q[0] == c[0] && q[2] == c[2] && (q[1] == c[1] || q[1] + 1 == c[1])
            });
            if hit != on_cell[k] {
                if let Some(h) = &hit {
                    println!("{t:+6} cart {k}: over watched cell {h} at {:.2} b/t", v);
                }
                on_cell[k] = hit;
            }
        }
        for (p, s) in cells {
            let key: String = s.split(['[', ',', ']']).filter(|f| ["power=", "powered=", "lit=", "triggered=", "shape="].iter().any(|k| f.starts_with(k))).collect::<Vec<_>>().join(",");
            let prev = last_cell.insert(p.clone(), key.clone());
            if prev.as_deref() != Some(key.as_str()) && prev.is_some() {
                println!("{t:+6} cell {p}: {} -> {key}", prev.unwrap_or_default());
            }
        }
    }
    Ok(())
}

/// Capped stairs: a dust D with a dust one up and one across (a climb), where the cell over D is a
/// conductor, so the two never connect (redstone wire climbs only past a transparent cell). Prints
/// each per file and a total; files with none print their count 0.
pub fn stair_lint(designs: &[std::path::PathBuf], plants: &[String], climbs: bool) -> Result<()> {
    let conductor = |s: &str| {
        let n = s.split('[').next().unwrap_or(s);
        ["minecraft:stone", "minecraft:smooth_stone", "minecraft:cobblestone", "minecraft:stone_bricks", "minecraft:dirt", "minecraft:grass_block", "minecraft:target", "minecraft:dropper", "minecraft:copper_bulb", "minecraft:waxed_copper_bulb", "minecraft:oak_planks", "minecraft:deepslate"].contains(&n)
    };
    let dir = |f: &str| -> Option<[i32; 3]> {
        Some(match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            _ => return None,
        })
    };
    let facing = |s: &str| s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).map(|v| v.to_string());
    let add = |a: [i32; 3], b: [i32; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let six = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]];
    let mut total = 0usize;
    let mut broken_total = 0usize;
    for path in designs {
        let mut d = mctas::design::Design::load(path)?;
        // (--plant: blocks placed in memory first, the planted-break control)
        for p in plants {
            let (at, block) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("--plant wants X,Y,Z=BLOCK, got {p}"))?;
            let v: Vec<i32> = at.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--plant wants X,Y,Z=BLOCK, got {p}");
            let was = d.blocks.insert([v[0], v[1], v[2]], block.to_string());
            println!("{}: planted {block} at {at} (was {was:?})", path.display());
        }
        let dust = |p: &[i32; 3]| d.blocks.get(p).is_some_and(|s| s.starts_with("minecraft:redstone_wire"));
        let cond = |p: &[i32; 3]| d.blocks.get(p).is_some_and(|s| conductor(s));
        // (nets by the game's rules: side by side, up a stair past a transparent cell over the
        // lower dust, down past a transparent cell beside the upper one)
        let dusts: Vec<[i32; 3]> = d.blocks.keys().copied().filter(|p| dust(p)).collect();
        let idx: std::collections::HashMap<[i32; 3], usize> = dusts.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let mut parent: Vec<usize> = (0..dusts.len()).collect();
        fn find(p: &mut Vec<usize>, i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut j = i;
            while p[j] != r {
                let n = p[j];
                p[j] = r;
                j = n;
            }
            r
        }
        let hs = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
        let mut open_climbs: Vec<([i32; 3], [i32; 3])> = Vec::new();
        for (i, p) in dusts.iter().enumerate() {
            for h in hs {
                let side = add(*p, h);
                let up = add(side, [0, 1, 0]);
                let link = if dust(&side) {
                    Some(side)
                } else if dust(&up) && !cond(&add(*p, [0, 1, 0])) {
                    open_climbs.push((*p, up));
                    Some(up)
                } else {
                    None
                };
                if let Some(q) = link {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, idx[&q]));
                    parent[a] = b;
                }
            }
        }
        // drivers and consumers per net
        let mut driven: std::collections::HashSet<usize> = Default::default();
        let mut consumed: std::collections::HashSet<usize> = Default::default();
        let net_of = |parent: &mut Vec<usize>, p: &[i32; 3]| idx.get(p).map(|&i| find(parent, i));
        // (a conductor block's dusts: the ones on it or beside it, which can power it)
        let block_dusts = |q: [i32; 3]| -> Vec<[i32; 3]> { [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]].iter().map(|nb| add(q, *nb)).collect() };
        // (ports: an input is driven from outside the file (the bench's block on its cell), any
        // other port is read from outside)
        for (name, at) in &d.ports {
            for q in std::iter::once(*at).chain(six.iter().map(|nb| add(*at, *nb))) {
                if let Some(k) = net_of(&mut parent, &q) {
                    if name.starts_with("input.") {
                        driven.insert(k);
                    } else {
                        consumed.insert(k);
                    }
                }
            }
        }
        for (c, s) in &d.blocks {
            let n = s.split('[').next().unwrap_or(s);
            if n.ends_with("repeater") || n.ends_with("comparator") {
                let Some(f) = facing(s).and_then(|f| dir(&f)) else { continue };
                let input = add(*c, f);
                let output = [c[0] - f[0], c[1] - f[1], c[2] - f[2]];
                if let Some(k) = net_of(&mut parent, &output) {
                    driven.insert(k);
                } else if cond(&output) {
                    // (a block it powers strongly powers the dust beside, on or under it)
                    for q in block_dusts(output).into_iter().chain(std::iter::once([output[0], output[1] - 1, output[2]])) {
                        if let Some(k) = net_of(&mut parent, &q) {
                            driven.insert(k);
                        }
                    }
                }
                if let Some(k) = net_of(&mut parent, &input) {
                    consumed.insert(k);
                } else if cond(&input) {
                    // (a block a dust on it or pointing in powers, read as the diode's input)
                    for q in block_dusts(input) {
                        if let Some(k) = net_of(&mut parent, &q) {
                            consumed.insert(k);
                        }
                    }
                }
                if n.ends_with("comparator") {
                    for side in [[f[2], 0, f[0]], [-f[2], 0, -f[0]]] {
                        if let Some(k) = net_of(&mut parent, &add(*c, side)) {
                            consumed.insert(k);
                        }
                    }
                }
            } else if n == "minecraft:redstone_torch" || n == "minecraft:redstone_wall_torch" {
                for nb in six {
                    if let Some(k) = net_of(&mut parent, &add(*c, nb)) {
                        driven.insert(k);
                    }
                }
                // (a torch strongly powers the block over it: that block drives the dust beside,
                // on or under it)
                let over = [c[0], c[1] + 1, c[2]];
                if cond(&over) {
                    for q in block_dusts(over) {
                        if let Some(k) = net_of(&mut parent, &q) {
                            driven.insert(k);
                        }
                    }
                }
                // (a torch reads the block it hangs on: the dusts that power that block feed it)
                let att = if n == "minecraft:redstone_torch" {
                    Some([c[0], c[1] - 1, c[2]])
                } else {
                    facing(s).and_then(|f| dir(&f)).map(|f| [c[0] - f[0], c[1], c[2] - f[2]])
                };
                if let Some(att) = att.filter(|a| cond(a)) {
                    for q in block_dusts(att) {
                        if let Some(k) = net_of(&mut parent, &q) {
                            consumed.insert(k);
                        }
                    }
                }
            } else if ["minecraft:redstone_block", "minecraft:lever", "minecraft:detector_rail", "minecraft:daylight_detector", "minecraft:stone_button", "minecraft:observer", "minecraft:tripwire_hook", "minecraft:trapped_chest", "minecraft:sculk_sensor"].contains(&n) || n.ends_with("pressure_plate") {
                for nb in six {
                    if let Some(k) = net_of(&mut parent, &add(*c, nb)) {
                        driven.insert(k);
                    }
                }
            } else if ["piston", "powered_rail", "activator_rail", "redstone_lamp", "dropper", "dispenser", "hopper", "copper_bulb", "door", "fence_gate", "note_block", "tnt"].iter().any(|m| n.contains(m)) {
                for nb in six {
                    let q = add(*c, nb);
                    if let Some(k) = net_of(&mut parent, &q) {
                        consumed.insert(k);
                    } else if cond(&q) {
                        // (a mechanism beside a block a dust powers)
                        for r in block_dusts(q) {
                            if let Some(k) = net_of(&mut parent, &r) {
                                consumed.insert(k);
                            }
                        }
                    }
                }
            }
        }
        let tag = |k: usize| match (driven.contains(&k), consumed.contains(&k)) {
            (true, true) => "driven+read",
            (true, false) => "driven only",
            (false, true) => "READ, NO DRIVER",
            (false, false) => "neither",
        };
        let mut found: Vec<String> = Vec::new();
        let mut broken = 0usize;
        for p in &dusts {
            for h in hs {
                let up = [p[0] + h[0], p[1] + 1, p[2] + h[2]];
                let over = [p[0], p[1] + 1, p[2]];
                if dust(&up) && cond(&over) {
                    let (a, b) = (find(&mut parent, idx[p]), find(&mut parent, idx[&up]));
                    let class = if a == b {
                        "redundant (same net)".to_string()
                    } else {
                        // (broken: a net that is read but has no driver, its only way to one
                        // being this capped step into a driven net)
                        let starved = |x: usize, y: usize| consumed.contains(&x) && !driven.contains(&x) && driven.contains(&y);
                        if starved(a, b) || starved(b, a) {
                            broken += 1;
                            format!("BROKEN ROUTE (lower {}, upper {})", tag(a), tag(b))
                        } else {
                            format!("isolation (lower {}, upper {})", tag(a), tag(b))
                        }
                    };
                    found.push(format!("{p:?} -> {up:?} capped by {} at {over:?}: {class}", d.blocks[&over]));
                }
            }
        }
        println!("{}: {} capped stairs, {} broken routes", path.display(), found.len(), broken);
        for f in &found {
            println!("  {f}");
        }
        if climbs {
            println!("  open climbs ({}):", open_climbs.len());
            for (p, up) in &open_climbs {
                let k = find(&mut parent, idx[p]);
                println!("    {p:?} -> {up:?} (net {})", tag(k));
            }
        }
        total += found.len();
        broken_total += broken;
    }
    println!("total: {total} capped, {broken_total} broken routes");
    Ok(())
}

/// `trace-diff` (the fold's check: which control lines moved, and by how much): two `release-trace`
/// outputs, per port: identical, every edge shifted by the same gt (and that shift), or the first
/// edge that differs.
pub fn trace_diff(a: &Path, b: &Path) -> Result<()> {
    let read = |p: &Path| -> Result<Vec<(String, Vec<String>)>> {
        Ok(std::fs::read_to_string(p)?
            .lines()
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                let k = w.next()?.to_string();
                Some((k, w.map(String::from).collect()))
            })
            .collect())
    };
    let (ra, rb) = (read(a)?, read(b)?);
    let num = |s: &str| -> Option<i64> { s.trim_start_matches(['+', '-']).split('@').last()?.parse().ok() };
    let (mut same, mut moved) = (0, 0);
    for (k, ea) in &ra {
        let Some((_, eb)) = rb.iter().find(|(n, _)| n == k) else {
            println!("{k:24} only in {}", a.display());
            continue;
        };
        if ea == eb {
            same += 1;
            continue;
        }
        moved += 1;
        let shifts: Vec<i64> = ea.iter().zip(eb).filter_map(|(x, y)| Some(num(y)? - num(x)?)).collect();
        let kinds_match = ea.len() == eb.len() && ea.iter().zip(eb).all(|(x, y)| x.chars().next() == y.chars().next());
        if kinds_match && !shifts.is_empty() && shifts.iter().all(|&s| s == shifts[0]) {
            println!("{k:24} every edge {:+} gt ({} edges)", shifts[0], ea.len());
        } else {
            let i = ea.iter().zip(eb).position(|(x, y)| x != y).unwrap_or(ea.len().min(eb.len()));
            let (lo, hi) = (shifts.iter().min().copied().unwrap_or(0), shifts.iter().max().copied().unwrap_or(0));
            println!(
                "{k:24} differs from edge {i} ({} vs {}); edges {} vs {}; shifts {lo:+}..{hi:+}",
                ea.get(i).map(|s| s.as_str()).unwrap_or("-"),
                eb.get(i).map(|s| s.as_str()).unwrap_or("-"),
                ea.len(),
                eb.len()
            );
        }
    }
    for (k, _) in &rb {
        if !ra.iter().any(|(n, _)| n == k) {
            println!("{k:24} only in {}", b.display());
        }
    }
    println!("{same} lines identical, {moved} moved");
    Ok(())
}

/// `nap-report` (ironspec 21:08, the night-1 lines): from one `queue-loop-run --visits-observe`
/// log (one seed), each villager's first nap (its cart `eN`: a villager rides its own cart for
/// life) and each batch's first fully napped slot (the batch at the stand during slot S is
/// S mod 7: the one a visit in S takes its cart from), and the startup's slot cost: the slots from
/// power-on (slot 0) whose stand batch had a rider not yet napped (no summon: it needs its three
/// riders willing, and a rider that never slept is not).
pub fn nap_report(log: &Path) -> Result<()> {
    use std::collections::BTreeMap;
    let text = std::fs::read_to_string(log)?;
    let num = |s: &str| -> Option<i64> { s.trim_matches(|c: char| !c.is_ascii_digit() && c != '-').parse().ok() };
    // (slot, stand cart) -> (cart entity, day time); (slot, stand cart) -> asleep
    let mut who: BTreeMap<(i64, i64), (i64, i64)> = BTreeMap::new();
    let mut asleep: BTreeMap<(i64, i64), bool> = BTreeMap::new();
    let mut last_slot = 0i64;
    for line in text.lines() {
        if let Some(r) = line.strip_prefix("visit (observed): slot ") {
            let w: Vec<&str> = r.split_whitespace().collect();
            let slot = num(w[0]).unwrap_or(0);
            let day = r.split("day time ").nth(1).and_then(|s| num(s.split(')').next().unwrap_or(""))).unwrap_or(0);
            let cart = r.split("the stand's cart ").nth(1).and_then(|s| num(s.split_whitespace().next().unwrap_or(""))).unwrap_or(0);
            let e = r.split("(e").nth(1).and_then(|s| num(s.split(')').next().unwrap_or(""))).unwrap_or(-1);
            who.insert((slot, cart), (e, day));
        } else if let Some(r) = line.strip_prefix("visit slot ") {
            let w: Vec<&str> = r.split_whitespace().collect();
            if w.len() >= 3 {
                let slot = num(w[0]).unwrap_or(0);
                let cart = num(w[2]).unwrap_or(0);
                asleep.insert((slot, cart), r.contains("rider asleep"));
            }
        } else if let Some(r) = line.trim_start().strip_prefix("slot ") {
            if let Some(s) = num(r.split(':').next().unwrap_or("")) {
                last_slot = last_slot.max(s);
            }
        }
    }
    // each villager: its batch and its naps
    let mut batch: BTreeMap<i64, i64> = BTreeMap::new();
    let mut first: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for (&(slot, cart), &(e, day)) in &who {
        batch.entry(e).or_insert(slot.rem_euclid(7));
        last_slot = last_slot.max(slot);
        if asleep.get(&(slot, cart)) == Some(&true) {
            first.entry(e).or_insert((slot, day));
        }
    }
    println!("{} villagers visited, {} napped; first naps (cart: slot, day time, batch):", batch.len(), first.len());
    for (e, b) in &batch {
        match first.get(e) {
            Some((s, d)) => println!("  e{e}: slot {s}, day time {d}, batch {b}"),
            None => println!("  e{e}: no nap in the log, batch {b}"),
        }
    }
    let mut lost = 0i64;
    let mut done_all = 0i64;
    for b in 0..7 {
        let riders: Vec<i64> = batch.iter().filter(|(_, bb)| **bb == b).map(|(e, _)| *e).collect();
        let full = riders.iter().map(|e| first.get(e).map(|f| f.0).unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
        let upto = full.min(last_slot + 1);
        let turns = (0..upto).filter(|s| s.rem_euclid(7) == b).count() as i64;
        lost += turns;
        done_all = done_all.max(full);
        let shown = if full == i64::MAX { "never in the log".to_string() } else { format!("slot {full}") };
        println!("  batch {b}: riders {:?}, fully napped from {shown}; stand turns before it: {turns}", riders.iter().map(|e| format!("e{e}")).collect::<Vec<_>>());
    }
    println!(
        "startup slot cost: {lost} stand slots from power-on with a rider not yet napped (no summon); every batch fully napped from {}",
        if done_all == i64::MAX { "never in the log".to_string() } else { format!("slot {done_all}") }
    );
    Ok(())
}
