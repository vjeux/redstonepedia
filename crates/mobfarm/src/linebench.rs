//! A controller line run source to consumer on the cells of a design of record (day-1, Nov 12 2026;
//! ironspec 21:40 PT (3b) / 21:49 (1): every controller line of record7 benched on its built cells, each with
//! a positive control). The design is loaded unchanged (its entities left out: no cart moves under the
//! line), the `--set` candidate cells laid over it, and the line's inputs driven by declared stand-ins: each
//! input a cell whose block the bench switches between an ON and an OFF state. The bench prints, per
//! combination of the inputs (each from a fresh run, settled 40 gt), whether the consumer reads powered,
//! against the expected function; the latency from each input's edge to the consumer's rise and fall (the
//! stand-in switched before tick t0, the consumer read after each tick: a change after tick t0 + k is k + 1
//! gt after the edge); the probes' states with the line enabled; and the control: the same table with the
//! control's cells laid, which must leave the consumer dead in every row.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

pub type P = [i32; 3];

/// One input: its name, its stand-in's cell, and the states for ON and OFF.
#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub cell: P,
    pub on: String,
    pub off: String,
}

/// `NAME=x,y,z=ON_STATE/OFF_STATE`.
pub fn parse_input(s: &str) -> Result<Input, String> {
    let (name, rest) = s.split_once('=').ok_or_else(|| format!("--input {s}: NAME=x,y,z=ON/OFF"))?;
    let (cell, states) = rest.split_once('=').ok_or_else(|| format!("--input {s}: NAME=x,y,z=ON/OFF"))?;
    let (on, off) = states.split_once('/').ok_or_else(|| format!("--input {s}: the states are ON/OFF"))?;
    Ok(Input { name: name.trim().to_string(), cell: parse_cell(cell)?, on: on.trim().to_string(), off: off.trim().to_string() })
}

pub fn parse_cell(s: &str) -> Result<P, String> {
    let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{s}: {e}"))).collect::<Result<_, _>>()?;
    match v[..] {
        [x, y, z] => Ok([x, y, z]),
        _ => Err(format!("{s}: x,y,z")),
    }
}

/// `x,y,z=STATE`.
pub fn parse_set(s: &str) -> Result<(P, String), String> {
    let (c, st) = s.split_once('=').ok_or_else(|| format!("{s}: x,y,z=STATE"))?;
    Ok((parse_cell(c)?, st.trim().to_string()))
}

/// The expected function: a conjunction of literals (`REST&!DAY&!NR`): (name, wanted ON).
pub fn parse_expect(s: &str) -> Vec<(String, bool)> {
    s.split('&').map(str::trim).filter(|t| !t.is_empty()).map(|t| match t.strip_prefix('!') {
        Some(n) => (n.trim().to_string(), false),
        None => (t.to_string(), true),
    }).collect()
}

fn block(s: &str) -> Result<Block, String> {
    let full = if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") };
    Block::parse(&full).map_err(|e| format!("{s}: {e}"))
}

/// The bench's world: the design's blocks with `sets` laid over them, no entities. (ironspec 04:07 PT) Each override is
/// returned with its cell's state before and after; the caller fails the run on a no-op.
fn world_spec(base: &FarmSpec, sets: &[(P, String)]) -> Result<(FarmSpec, Vec<(P, String, String)>), String> {
    let mut sp = base.clone();
    sp.entities.clear();
    let mut ov = Vec::new();
    for (c, st) in sets {
        let b = block(st)?;
        let p = BlockPos::new(c[0], c[1], c[2]);
        let before = sp.blocks.iter().find(|(q, _)| *q == p).map_or("minecraft:air".to_string(), |(_, x)| x.to_string());
        ov.push((*c, before, b.to_string()));
        match sp.blocks.iter_mut().find(|(q, _)| *q == p) {
            Some(slot) => slot.1 = b,
            None => sp.blocks.push((p, b)),
        }
    }
    Ok((sp, ov))
}

/// (G1's audit hole 19; ironspec's chain audit) The consumer's reading from its state: the first power property it
/// carries (`powered`, `lit`, `triggered`, `extended`, a hopper's `enabled` inverted, a dust's `power` > 0). A consumer
/// with none of them is refused, never read as off.
pub fn reading(state: &str) -> Result<bool, String> {
    let prop = |k: &str| -> Option<String> {
        let inner = state.split_once('[')?.1.trim_end_matches(']');
        inner.split(',').find_map(|kv| kv.split_once('=').filter(|(a, _)| a.trim() == k).map(|(_, b)| b.trim().to_string()))
    };
    for k in ["powered", "lit", "triggered", "extended"] {
        if let Some(v) = prop(k) {
            return match v.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(format!("the consumer {state}: {k}={v} is not a boolean")),
            };
        }
    }
    if state.contains("hopper")
        && let Some(v) = prop("enabled")
    {
        return Ok(v == "false");
    }
    if let Some(v) = prop("power") {
        return v.parse::<i32>().map(|n| n > 0).map_err(|e| format!("the consumer {state}: power={v}: {e}"));
    }
    Err(format!("the consumer {state} carries no power property (powered, lit, triggered, extended, a hopper's enabled, power): it cannot be read"))
}

fn powered(run: &Run, c: P) -> Result<bool, String> {
    reading(&run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string())
}

/// One fresh run with the inputs at `state` (true = ON), settled `settle` gt; the consumer's reading.
fn settled(sp: &FarmSpec, inputs: &[Input], state: &[bool], read: P, settle: i64) -> Result<(bool, Run), String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(sp, &ro)?;
    while run.settle_tick() {}
    for (i, inp) in inputs.iter().enumerate() {
        let b = block(if state[i] { &inp.on } else { &inp.off })?;
        run.world.set_block(BlockPos::new(inp.cell[0], inp.cell[1], inp.cell[2]), b, redsim::world::flags::ALL);
    }
    for _ in 0..settle {
        run.tick();
    }
    let r = powered(&run, read)?;
    Ok((r, run))
}

/// The truth table: per combination, (the inputs, the consumer, the expected).
fn table(sp: &FarmSpec, inputs: &[Input], expect: &[(String, bool)], read: P, settle: i64) -> Result<Vec<(Vec<bool>, bool, bool)>, String> {
    let n = inputs.len();
    let mut rows = Vec::new();
    for m in 0..(1u32 << n) {
        let state: Vec<bool> = (0..n).map(|i| m & (1 << i) != 0).collect();
        let (got, _) = settled(sp, inputs, &state, read, settle)?;
        let want = expect.iter().all(|(name, on)| inputs.iter().position(|x| &x.name == name).is_some_and(|i| state[i] == *on));
        rows.push((state, got, want));
    }
    Ok(rows)
}

/// (day-1's drive-lines slice, Nov 14 11:34) The PULSE rows' options: each input driven as a pulse of `width` gt;
/// `reload` adds a world open at every game tick of the pulse in every reload row; `target_at` (the interface's time of
/// the consumer's rise, e.g. 67 for REL + 67) prints the back-computed time the input must rise at.
#[derive(Clone, Copy, Debug)]
pub struct PulseOpts {
    pub width: i64,
    pub reload: bool,
    pub target_at: Option<i64>,
}

/// One pulse run's record: the consumer powered before the pulse, its rises and falls (game ticks counted from the
/// settle's end: the tick in which it changed), and the world open's evidence when the run restarted.
struct Pulse {
    before: bool,
    rises: Vec<i64>,
    falls: Vec<i64>,
    ev: Option<crate::liveness::RestartEvidence>,
}

/// One PULSE run, keyed on GAME time as ctlfeed's (a world open's idle ticks advance game time with nothing ticking):
/// the inputs at `enable` but input `k` at its disabling state, settled `settle` gt; input `k` at its enabling state
/// from game tick `t0` (counted from the settle's end; switched before that tick runs) until `t0 + width`; a world open
/// `restart` at its game tick (a switch the open's idle ticks skip over happens before the first tick after them); the
/// consumer read after every tick.
#[allow(clippy::too_many_arguments)]
fn pulse_run(sp: &FarmSpec, inputs: &[Input], enable: &[bool], k: usize, read: P, settle: i64, t0: i64, width: i64, restart: Option<(i64, crate::liveness::Restart)>) -> Result<Pulse, String> {
    let mut state = enable.to_vec();
    state[k] = !enable[k];
    let (before, mut run) = settled(sp, inputs, &state, read, settle)?;
    let inp = &inputs[k];
    let cell = BlockPos::new(inp.cell[0], inp.cell[1], inp.cell[2]);
    let lit = block(if enable[k] { &inp.on } else { &inp.off })?;
    let dark = block(if enable[k] { &inp.off } else { &inp.on })?;
    let g0 = run.world.time;
    let mut out = Pulse { before, rises: Vec::new(), falls: Vec::new(), ev: None };
    let (mut lit_done, mut dark_done, mut rs_done, mut was) = (false, false, false, before);
    loop {
        let t = run.world.time - g0;
        if t >= t0 + width + settle {
            break;
        }
        if let Some((rt, how)) = restart
            && !rs_done
            && t >= rt
        {
            rs_done = true;
            out.ev = Some(crate::liveness::restart_ev(&mut run.world, how)?);
        }
        let t = run.world.time - g0;
        if !lit_done && t >= t0 {
            lit_done = true;
            run.world.set_block(cell, lit.clone(), redsim::world::flags::ALL);
        }
        if !dark_done && t >= t0 + width {
            dark_done = true;
            run.world.set_block(cell, dark.clone(), redsim::world::flags::ALL);
        }
        run.tick();
        let t = run.world.time - g0 - 1;
        let now = powered(&run, read)?;
        if now && !was {
            out.rises.push(t);
        }
        if !now && was {
            out.falls.push(t);
        }
        was = now;
    }
    Ok(out)
}

/// (day-1's drive-lines slice, Nov 14 11:34, the pass bars: "each line's rise at its target against the interface's
/// timing, at every phase; a world reload at every phase of each pulse") Each input driven as a PULSE of `o.width` gt
/// (the other inputs at their enabling states) at `phases` phases: the consumer rises once and falls once at every
/// phase, at one latency L (the gt from the input's change to the consumer's, the latency rows' k + 1), its pulse as
/// wide as the input's; `o.target_at` prints the back-computed time the input must rise at. With `o.reload`, a world open
/// (each `liveness::Restart::rows()` row: the save order, its reverse and 8 seeded orders, each with 1 and 2 idle ticks)
/// at every game tick from the pulse's start through the consumer's fall + 1: one rise and one fall, each shifted by
/// 0 ..= the open's idle ticks (nothing ticks in them, so a scheduled tick due inside them fires at the first ticking
/// tick: ironspec 23:23), and every open's own evidence that it happened. Controls: the positive control's world never
/// powers the consumer; a no-op restart's evidence reads "not happened".
#[allow(clippy::too_many_arguments)]
fn pulse_rows(sp: &FarmSpec, csp: &FarmSpec, inputs: &[Input], enable: &[bool], read: P, settle: i64, phases: i64, label: &str, o: &PulseOpts, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    use std::collections::BTreeMap;
    if o.width < 1 {
        return Err(format!("--pulse {}: at least 1 gt", o.width));
    }
    let h = |m: &BTreeMap<i64, u32>| m.iter().map(|(k, n)| format!("{k}:{n}")).collect::<Vec<_>>().join(" ");
    let mut lines = Vec::new();
    for (k, inp) in inputs.iter().enumerate() {
        let (mut rise_d, mut fall_d) = (BTreeMap::<i64, u32>::new(), BTreeMap::<i64, u32>::new());
        let mut odd: Vec<String> = Vec::new();
        for ph in 0..phases {
            let p = pulse_run(sp, inputs, enable, k, read, settle, ph, o.width, None)?;
            match (p.before, &p.rises[..], &p.falls[..]) {
                (false, [r], [f]) => {
                    *rise_d.entry(r - ph + 1).or_default() += 1;
                    *fall_d.entry(f - ph - o.width + 1).or_default() += 1;
                }
                _ => odd.push(format!("phase {ph}: {}rises {:?}, falls {:?}", if p.before { "powered before the pulse; " } else { "" }, p.rises, p.falls)),
            }
        }
        let once = v.bar_n(&format!("{label}: {}'s {}-gt pulse: the consumer rises once and falls once at every phase", inp.name, o.width), phases as usize, odd.is_empty());
        let one = v.bar(&format!("{label}: {}'s pulse: one rise latency and one fall latency at every phase", inp.name), rise_d.len() == 1 && fall_d.len() == 1);
        let wide = v.bar(&format!("{label}: the consumer's pulse as wide as {}'s ({} gt)", inp.name, o.width), rise_d.len() == 1 && rise_d.keys().eq(fall_d.keys()));
        let d = if rise_d.len() == 1 { rise_d.keys().next().copied() } else { None };
        let back = match (o.target_at, d) {
            (Some(t), Some(d)) => format!("; the back-computation: the consumer's rise at {t} needs {}'s rise at {t} - {d} = {}", inp.name, t - d),
            (Some(t), None) => format!("; the back-computation for {t}: no single latency (FAIL above)"),
            _ => String::new(),
        };
        lines.push(format!(
            "  the {}-gt PULSE of {} at {phases} phases (the other inputs enabling; L = the gt from the input's change to the consumer's, the latency rows' k + 1: the stand-in switched before tick t0, the consumer changed after tick t0 + L - 1): rise L (L:phases) {}, fall L {} (once per phase: {once}; one latency: {one}; as wide: {wide}){}{back}",
            o.width,
            inp.name,
            h(&rise_d),
            h(&fall_d),
            if odd.is_empty() { String::new() } else { format!("; odd phases: {}", odd.join("; ")) }
        ));
        let mut live = 0usize;
        for ph in 0..phases {
            let p = pulse_run(csp, inputs, enable, k, read, settle, ph, o.width, None)?;
            live += usize::from(p.before || !p.rises.is_empty());
        }
        let cw = v.control(&format!("{label}: the positive control kills {}'s pulse at every phase", inp.name), phases as usize, live == 0);
        lines.push(format!("    the positive control's world: the consumer powered in {live} of {phases} phases ({cw})"));
        if !o.reload {
            continue;
        }
        let (Some(dr), Some(df)) = (d, if fall_d.len() == 1 { fall_d.keys().next().copied() } else { None }) else {
            v.fail(&format!("{label}: {}'s world opens: NOT JUDGED: no single baseline pulse to compare", inp.name));
            lines.push("    the world opens: NOT JUDGED: no single baseline pulse to compare (FAIL)".to_string());
            continue;
        };
        let (rise_t, fall_t) = (dr - 1, o.width + df - 1);
        let rows = crate::liveness::Restart::rows();
        let (mut runs, mut not_open) = (0usize, 0usize);
        let mut wrong: Vec<String> = Vec::new();
        let mut shifts = BTreeMap::<(i64, i64), u32>::new();
        for how in &rows {
            let n = how.load_ticks();
            for rt in 0..=fall_t + 1 {
                let p = pulse_run(sp, inputs, enable, k, read, settle, 0, o.width, Some((rt, *how)))?;
                runs += 1;
                not_open += usize::from(!p.ev.as_ref().is_some_and(|e| e.happened()));
                match (p.before, &p.rises[..], &p.falls[..]) {
                    (false, [r], [f]) if (0..=n).contains(&(r - rise_t)) && (0..=n).contains(&(f - fall_t)) => *shifts.entry((r - rise_t, f - fall_t)).or_default() += 1,
                    _ => wrong.push(format!("{} at tick {rt}: rises {:?}, falls {:?}", how.label(), p.rises, p.falls)),
                }
            }
        }
        let ww = v.bar_n(&format!("{label}: a world open at every tick of {}'s pulse in every reload row: one rise and one fall, each shifted by at most the open's idle ticks", inp.name), runs, wrong.is_empty());
        let wo = v.bar_n(&format!("{label}: every world open in {}'s rows happened (chunks out and back, every scheduled tick back)", inp.name), runs, not_open == 0);
        let noop = crate::liveness::with_noop_restart(|| pulse_run(sp, inputs, enable, k, read, settle, 0, o.width, Some((rise_t, rows[0]))))?;
        let nc = v.control(&format!("{label}: a no-op restart in {}'s pulse reads not happened", inp.name), 1, !noop.ev.as_ref().is_some_and(|e| e.happened()));
        lines.push(format!(
            "    the world opens ({} reload rows x game ticks 0..={} of the pulse = {runs} runs; the baseline's rise in tick {rise_t}, fall in tick {fall_t}): (rise shift, fall shift):runs {}; {} wrong ({ww}); every open happened: {wo} (CONTROL a no-op restart: {nc}){}",
            rows.len(),
            fall_t + 1,
            shifts.iter().map(|((a, b), n)| format!("({a},{b}):{n}")).collect::<Vec<_>>().join(" "),
            wrong.len(),
            if wrong.is_empty() { String::new() } else { format!("; the first wrong: {}", wrong.iter().take(5).cloned().collect::<Vec<_>>().join("; ")) }
        ));
    }
    Ok(lines)
}

fn row_text(inputs: &[Input], state: &[bool]) -> String {
    inputs.iter().zip(state).map(|(i, &s)| format!("{}{}", if s { "" } else { "!" }, i.name)).collect::<Vec<_>>().join(" ")
}

#[allow(clippy::too_many_arguments)]
pub fn bench(base: &FarmSpec, label: &str, inputs: &[Input], read: P, sets: &[(P, String)], control: &[(P, String)], expect: &str, probes: &[P], reach: &[P], declared: &[P], settle: i64, phases: i64, pulse: Option<&PulseOpts>, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let ex = parse_expect(expect);
    for (name, _) in &ex {
        if !inputs.iter().any(|i| &i.name == name) {
            return Err(format!("--expect names {name}, which is no --input"));
        }
    }
    // (G1's audit hole 19: every input is judged; an input the expected function does not name is refused, never
    // skipped; the control is required; the phases and the settle are at least 1)
    if let Some(i) = inputs.iter().find(|i| !ex.iter().any(|(n, _)| n == &i.name)) {
        return Err(format!("--input {} is not in --expect {expect}: every input must be judged", i.name));
    }
    if inputs.is_empty() {
        return Err("no --input: the line has nothing to judge".into());
    }
    if control.is_empty() {
        return Err("--control is required (G1's audit hole 19): the positive control that must kill the line".into());
    }
    if phases < 1 || settle < 1 {
        return Err(format!("--phases {phases} / --settle {settle}: each at least 1"));
    }
    let (sp, ov) = world_spec(base, sets)?;
    // (G1's audit hole 20: the bench's world checked, so a cell outside the world's box is refused, never dropped)
    let sp = crate::mirror::checked(&sp)?;
    let mut lines = vec![format!(
        "line bench: {label}: the design's cells unchanged but the candidate cells {} (entities left out); the inputs' stand-ins {}; the consumer {:?} read from its power property; expected powered exactly for {expect}",
        if sets.is_empty() { "none".to_string() } else { sets.iter().map(|(c, s)| format!("({},{},{}) {s}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ") },
        inputs.iter().map(|i| format!("{} at ({},{},{}) ON {} / OFF {}", i.name, i.cell[0], i.cell[1], i.cell[2], i.on, i.off)).collect::<Vec<_>>().join("; "),
        read
    )];
    // (the consumer must be readable before anything is judged)
    let cstate = sp.blocks.iter().find(|(q, _)| *q == BlockPos::new(read[0], read[1], read[2])).map_or("minecraft:air".to_string(), |(_, b)| b.to_string());
    reading(&cstate)?;
    // (ironspec 04:07 PT: every override with its cell's state before and after; an override that leaves its cell as it
    // was fails the run; a stand-in whose ON and OFF states both equal the design's cell changes nothing either way)
    let ov_w: Vec<String> = ov.iter().map(|(c, a, b)| v.over(&format!("the candidate cell ({},{},{})", c[0], c[1], c[2]), a, b)).collect();
    let mut stand: Vec<String> = Vec::new();
    for i in inputs {
        let p = BlockPos::new(i.cell[0], i.cell[1], i.cell[2]);
        let before = sp.blocks.iter().find(|(q, _)| *q == p).map_or("minecraft:air".to_string(), |(_, x)| x.to_string());
        let (on, off) = (block(&i.on)?.to_string(), block(&i.off)?.to_string());
        let w_on = v.over(&format!("the stand-in {} ON", i.name), &off, &on);
        let w_base = if before == on && before == off { v.over(&format!("the stand-in {} against the design's cell", i.name), &before, &on) } else { format!("the design's cell {before}") };
        stand.push(format!("{} ({},{},{}): {w_base}; {w_on}", i.name, i.cell[0], i.cell[1], i.cell[2]));
    }
    lines.push(format!(
        "  the overrides (ironspec 04:07 PT, each cell's state before -> after): {}; the stand-ins {}",
        if ov_w.is_empty() { "none".to_string() } else { ov_w.join("; ") },
        stand.join("; ")
    ));
    let rows = table(&sp, inputs, &ex, read, settle)?;
    let bad = rows.iter().filter(|r| r.1 != r.2).count();
    let tt = v.bar_n(&format!("{label}: the truth table as expected"), rows.len(), bad == 0);
    lines.push(format!(
        "  the truth table ({} combinations, each a fresh run settled {settle} gt): {}; {bad} rows wrong ({tt})",
        rows.len(),
        rows.iter().map(|(s, got, want)| format!("[{}] -> {}{}", row_text(inputs, s), if *got { "ON" } else { "off" }, if got == want { "" } else { " (WRONG)" })).collect::<Vec<_>>().join("; "),
    ));
    // (the latency: from the enabling assignment, each input's edge that turns the consumer off and back on, at
    // every phase: the edge after settle + 0..phases gt; a NEVER at any phase fails)
    let enable: Vec<bool> = inputs.iter().map(|i| ex.iter().find(|(n, _)| n == &i.name).is_none_or(|(_, on)| *on)).collect();
    let mut lat = Vec::new();
    for (k, inp) in inputs.iter().enumerate() {
        let (mut falls, mut rises) = (std::collections::BTreeMap::<String, u32>::new(), std::collections::BTreeMap::<String, u32>::new());
        let (mut dead, mut never, mut n) = (false, 0u32, 0usize);
        for ph in 0..phases {
            let (on0, mut run) = settled(&sp, inputs, &enable, read, settle + ph)?;
            if !on0 {
                dead = true;
                break;
            }
            n += 1;
            let toggle = |run: &mut Run, to_enable: bool| -> Result<(), String> {
                let want_on = if to_enable { enable[k] } else { !enable[k] };
                let b = block(if want_on { &inp.on } else { &inp.off })?;
                run.world.set_block(BlockPos::new(inp.cell[0], inp.cell[1], inp.cell[2]), b, redsim::world::flags::ALL);
                Ok(())
            };
            toggle(&mut run, false)?;
            let mut fall = None;
            for t in 0..settle {
                run.tick();
                if fall.is_none() && !powered(&run, read)? {
                    fall = Some(t + 1);
                }
            }
            toggle(&mut run, true)?;
            let mut rise = None;
            for t in 0..settle {
                run.tick();
                if rise.is_none() && powered(&run, read)? {
                    rise = Some(t + 1);
                }
            }
            never += u32::from(fall.is_none()) + u32::from(rise.is_none());
            *falls.entry(fall.map_or("NEVER".to_string(), |x| x.to_string())).or_default() += 1;
            *rises.entry(rise.map_or("NEVER".to_string(), |x| x.to_string())).or_default() += 1;
        }
        if dead {
            v.fail(&format!("{label}: the consumer is not powered with the line enabled ({} edge)", inp.name));
            lat.push(format!("{}: the consumer is not powered with the line enabled (FAIL)", inp.name));
            continue;
        }
        let w = v.bar_n(&format!("{label}: {}'s edges turn the consumer off and on at every phase (no NEVER)", inp.name), n, never == 0);
        let h = |m: &std::collections::BTreeMap<String, u32>| m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ");
        lat.push(format!(
            "{} {} -> the consumer off after (gt:phases) {}; back {} -> on after {} ({w})",
            inp.name,
            if enable[k] { "OFF" } else { "ON" },
            h(&falls),
            if enable[k] { "ON" } else { "OFF" },
            h(&rises)
        ));
    }
    lines.push(format!("  the latency from each input's edge, from the enabling assignment, at {phases} phases (the stand-in switched before tick t0, the consumer read after each tick: k + 1 = changed after tick t0 + k): {}", lat.join("; ")));
    if !probes.is_empty() {
        let (_, run) = settled(&sp, inputs, &enable, read, settle)?;
        lines.push(format!(
            "  the probes with the line enabled ({}): {}",
            v.scope("the probes", "printed for the reader, not a bar"),
            probes.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"))).collect::<Vec<_>>().join("; ")
        ));
    }
    // (ironspec 21:40 (2): POWER REACH: each listed cell's six neighbours in the bench's world, a mechanism
    // component among them flagged; the declared consumer is the line's own; G1's audit hole 19: `--declare` names an
    // intended mechanism neighbour, which prints; an undeclared one fails)
    if !reach.is_empty() {
        let (_, run) = settled(&sp, inputs, &enable, read, settle)?;
        let mech = |s: &str| ["rail", "piston", "trapdoor", "_door", "hopper", "dropper", "dispenser", "note_block", "lamp", "bell", "observer", "fence_gate", "tnt", "crafter"].iter().any(|k| s.contains(k));
        let (mut flagged, mut decl) = (0usize, 0usize);
        let mut parts = Vec::new();
        let mut used: std::collections::BTreeSet<P> = std::collections::BTreeSet::new();
        for c in reach {
            let mut nb = Vec::new();
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let q = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                let s = run.world.get(BlockPos::new(q[0], q[1], q[2])).to_string();
                let is_mech = mech(&s) && q != read;
                let is_decl = is_mech && declared.contains(&q);
                if is_decl {
                    used.insert(q);
                }
                flagged += usize::from(is_mech && !is_decl);
                decl += usize::from(is_decl);
                nb.push(format!("({},{},{}) {}{}", q[0], q[1], q[2], s.trim_start_matches("minecraft:"), if is_decl { " MECHANISM (declared)" } else if is_mech { " MECHANISM (undeclared)" } else { "" }));
            }
            parts.push(format!("({},{},{}) {} [{}]", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"), nb.join(", ")));
        }
        let w = v.bar(&format!("{label}: no undeclared mechanism component beside the reach cells"), flagged == 0);
        // (G1's Y20: a `--declare` cell never found beside a reach cell as a mechanism component is STALE, a failure; G1's
        // Z5 on 10bcdf661: one predicate for the bar and its control, the control planting a declaration no reach cell
        // touches (the read cell + 64 in x))
        let stale_of = |decl: &[P]| -> Vec<String> { decl.iter().filter(|q| !used.contains(*q)).map(|q| format!("({},{},{})", q[0], q[1], q[2])).collect() };
        let stale = stale_of(declared);
        let ws = v.bar(&format!("{label}: every --declare cell found (none stale)"), stale.is_empty());
        let mut planted: Vec<P> = declared.to_vec();
        planted.push([read[0] + 64, read[1], read[2]]);
        let wc = v.control(&format!("{label}: a planted declaration beside no reach cell reads stale"), 1, !stale_of(&planted).is_empty());
        lines.push(format!(
            "  power reach (each cell's six neighbours, the line enabled): {}; {flagged} undeclared and {decl} declared mechanism neighbours ({w}); stale declarations {} ({ws}; CONTROL a planted declaration: {wc})",
            parts.join("; "),
            if stale.is_empty() { "none".to_string() } else { stale.join(", ") }
        ));
    } else if !declared.is_empty() {
        v.fail(&format!("{label}: --declare given with no --reach: every declaration stale"));
        lines.push("  --declare given with no --reach cells: every declaration STALE (FAIL)".to_string());
    }
    let mut all: Vec<(P, String)> = sets.to_vec();
    all.extend(control.iter().cloned());
    let (csp, cov) = world_spec(base, &all)?;
    let csp = crate::mirror::checked(&csp)?;
    // (ironspec 04:07 PT: the control's own cells must change their cells too)
    let cov_w: Vec<String> = cov.iter().skip(sets.len()).map(|(c, a, b)| v.over(&format!("the control cell ({},{},{})", c[0], c[1], c[2]), a, b)).collect();
    let crows = table(&csp, inputs, &ex, read, settle)?;
    let live = crows.iter().filter(|r| r.1).count();
    let ctl = v.control(&format!("{label}: the control kills the line"), crows.len(), live == 0);
    let mut ctl_probe = String::new();
    if !probes.is_empty() {
        let (_, run) = settled(&csp, inputs, &enable, read, settle)?;
        ctl_probe = format!(
            "; its probes with the line enabled: {}",
            probes.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"))).collect::<Vec<_>>().join("; ")
        );
    }
    lines.push(format!(
        "  the positive control (the cells laid over the candidate: {}): the consumer powered in {live} of {} combinations ({ctl}){ctl_probe}",
        cov_w.join("; "),
        crows.len()
    ));
    if let Some(o) = pulse {
        lines.extend(pulse_rows(&sp, &csp, inputs, &enable, read, settle, phases, label, o, v)?);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-repeater line: the stand-in A (0,1,0) -> repeater[facing=west] (1,1,0) -> dust (2,1,0) -> repeater[facing=west]
    /// (3,1,0) -> its strongly powered slime (4,1,0) -> the powered rail (4,2,0) on it (the drive lines' line A in small).
    fn line() -> FarmSpec {
        let b = BlockPos::new;
        let mut cells: Vec<(BlockPos, &str)> = (-1..=6).map(|x| (b(x, 0, 0), "minecraft:stone")).collect();
        cells.push((b(1, 1, 0), "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"));
        cells.push((b(2, 1, 0), "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"));
        cells.push((b(3, 1, 0), "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"));
        cells.push((b(4, 1, 0), "minecraft:slime_block"));
        cells.push((b(4, 2, 0), "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]"));
        let blocks = cells.iter().map(|(p, s)| (*p, Block::parse(s).unwrap())).collect();
        FarmSpec { blocks, entities: Vec::new(), output: b(6, 3, 1), lo: b(-2, -1, -2), hi: b(7, 4, 2), block_entities: Vec::new(), rock: false, nether: None, land: false }
    }

    fn a() -> Vec<Input> {
        vec![parse_input("A=0,1,0=redstone_block/air").unwrap()]
    }

    /// (day-1's drive-lines slice, Nov 14 11:34) The 12-gt pulse reaches the rail once per phase at one latency (two
    /// repeaters: 4 gt), as wide as the input's; the back-computation prints 67 - 4; a world open at every tick of the
    /// pulse in every reload row keeps one rise and one fall, each shifted by at most the open's idle ticks; the
    /// controls bite (the dust removed kills the pulse; a no-op restart reads not happened).
    #[test]
    fn a_pulse_reaches_the_consumer_once_at_one_latency_and_every_world_open_keeps_it() {
        let mut v = crate::verdict::Verdict::new("t");
        let o = PulseOpts { width: 12, reload: true, target_at: Some(67) };
        let lines = bench(&line(), "t", &a(), [4, 2, 0], &[], &[([2, 1, 0], "air".to_string())], "A", &[], &[], &[], 20, 3, Some(&o), &mut v).unwrap();
        let all = lines.join("\n");
        assert!(v.fails().is_empty(), "{:?}\n{all}", v.fails());
        assert!(all.contains("rise L (L:phases) 4:3, fall L 4:3"), "{all}");
        assert!(all.contains("needs A's rise at 67 - 4 = 63"), "{all}");
        assert!(all.contains("= 340 runs") && all.contains("0 wrong (pass)"), "{all}");
        assert!(all.contains("(CONTROL a no-op restart: the control bites)"), "{all}");
    }

    /// The width bar can fail: a 1-gt pulse comes out of a delay-1 repeater 2 gt wide.
    #[test]
    fn a_pulse_shorter_than_a_repeater_fails_the_width_bar() {
        let mut v = crate::verdict::Verdict::new("t");
        let o = PulseOpts { width: 1, reload: false, target_at: None };
        let lines = bench(&line(), "t", &a(), [4, 2, 0], &[], &[([2, 1, 0], "air".to_string())], "A", &[], &[], &[], 20, 3, Some(&o), &mut v).unwrap();
        assert!(v.fails().iter().any(|f| f.contains("as wide as A's (1 gt)")), "{:?}\n{}", v.fails(), lines.join("\n"));
    }
}
