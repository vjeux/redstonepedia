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

/// The bench's world: the design's blocks with `sets` laid over them, no entities.
fn world_spec(base: &FarmSpec, sets: &[(P, String)]) -> Result<FarmSpec, String> {
    let mut sp = base.clone();
    sp.entities.clear();
    for (c, st) in sets {
        let b = block(st)?;
        let p = BlockPos::new(c[0], c[1], c[2]);
        match sp.blocks.iter_mut().find(|(q, _)| *q == p) {
            Some(slot) => slot.1 = b,
            None => sp.blocks.push((p, b)),
        }
    }
    Ok(sp)
}

fn powered(run: &Run, c: P) -> bool {
    run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().contains("powered=true")
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
    let r = powered(&run, read);
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

fn row_text(inputs: &[Input], state: &[bool]) -> String {
    inputs.iter().zip(state).map(|(i, &s)| format!("{}{}", if s { "" } else { "!" }, i.name)).collect::<Vec<_>>().join(" ")
}

#[allow(clippy::too_many_arguments)]
pub fn bench(base: &FarmSpec, label: &str, inputs: &[Input], read: P, sets: &[(P, String)], control: &[(P, String)], expect: &str, probes: &[P], reach: &[P], settle: i64, phases: i64) -> Result<Vec<String>, String> {
    let ex = parse_expect(expect);
    for (name, _) in &ex {
        if !inputs.iter().any(|i| &i.name == name) {
            return Err(format!("--expect names {name}, which is no --input"));
        }
    }
    let sp = world_spec(base, sets)?;
    let mut lines = vec![format!(
        "line bench: {label}: the design's cells unchanged but the candidate cells {} (entities left out); the inputs' stand-ins {}; the consumer {:?} read as powered; expected powered exactly for {expect}",
        if sets.is_empty() { "none".to_string() } else { sets.iter().map(|(c, s)| format!("({},{},{}) {s}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ") },
        inputs.iter().map(|i| format!("{} at ({},{},{}) ON {} / OFF {}", i.name, i.cell[0], i.cell[1], i.cell[2], i.on, i.off)).collect::<Vec<_>>().join("; "),
        read
    )];
    let rows = table(&sp, inputs, &ex, read, settle)?;
    let bad = rows.iter().filter(|r| r.1 != r.2).count();
    lines.push(format!(
        "  the truth table ({} combinations, each a fresh run settled {settle} gt): {}; {}",
        rows.len(),
        rows.iter().map(|(s, got, want)| format!("[{}] -> {}{}", row_text(inputs, s), if *got { "ON" } else { "off" }, if got == want { "" } else { " (WRONG)" })).collect::<Vec<_>>().join("; "),
        if bad == 0 { "every row as expected (pass)".to_string() } else { format!("{bad} rows wrong (FAIL)") }
    ));
    // (the latency: from the enabling assignment, each input's edge that turns the consumer off and back on, at
    // every phase: the edge after settle + 0..phases gt)
    let enable: Vec<bool> = inputs.iter().map(|i| ex.iter().find(|(n, _)| n == &i.name).is_none_or(|(_, on)| *on)).collect();
    let mut lat = Vec::new();
    for (k, inp) in inputs.iter().enumerate() {
        if !ex.iter().any(|(n, _)| n == &inp.name) {
            continue;
        }
        let (mut falls, mut rises) = (std::collections::BTreeMap::<String, u32>::new(), std::collections::BTreeMap::<String, u32>::new());
        let mut dead = false;
        for ph in 0..phases.max(1) {
            let (on0, mut run) = settled(&sp, inputs, &enable, read, settle + ph)?;
            if !on0 {
                dead = true;
                break;
            }
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
                if fall.is_none() && !powered(&run, read) {
                    fall = Some(t + 1);
                }
            }
            toggle(&mut run, true)?;
            let mut rise = None;
            for t in 0..settle {
                run.tick();
                if rise.is_none() && powered(&run, read) {
                    rise = Some(t + 1);
                }
            }
            *falls.entry(fall.map_or("NEVER".to_string(), |x| x.to_string())).or_default() += 1;
            *rises.entry(rise.map_or("NEVER".to_string(), |x| x.to_string())).or_default() += 1;
        }
        if dead {
            lat.push(format!("{}: the consumer is not powered with the line enabled (FAIL)", inp.name));
            continue;
        }
        let h = |m: &std::collections::BTreeMap<String, u32>| m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ");
        lat.push(format!(
            "{} {} -> the consumer off after (gt:phases) {}; back {} -> on after {}",
            inp.name,
            if enable[k] { "OFF" } else { "ON" },
            h(&falls),
            if enable[k] { "ON" } else { "OFF" },
            h(&rises)
        ));
    }
    lines.push(format!("  the latency from each input's edge, from the enabling assignment, at {} phases (the stand-in switched before tick t0, the consumer read after each tick: k + 1 = changed after tick t0 + k): {}", phases.max(1), lat.join("; ")));
    if !probes.is_empty() {
        let (_, run) = settled(&sp, inputs, &enable, read, settle)?;
        lines.push(format!(
            "  the probes with the line enabled: {}",
            probes.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"))).collect::<Vec<_>>().join("; ")
        ));
    }
    // (ironspec 21:40 (2): POWER REACH: each listed cell's six neighbours in the bench's world, a mechanism
    // component among them flagged; the declared consumer is the line's own)
    if !reach.is_empty() {
        let (_, run) = settled(&sp, inputs, &enable, read, settle)?;
        let mech = |s: &str| ["rail", "piston", "trapdoor", "_door", "hopper", "dropper", "dispenser", "note_block", "lamp", "bell", "observer", "fence_gate", "tnt", "crafter"].iter().any(|k| s.contains(k));
        let mut flagged = 0usize;
        let mut parts = Vec::new();
        for c in reach {
            let mut nb = Vec::new();
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let q = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                let s = run.world.get(BlockPos::new(q[0], q[1], q[2])).to_string();
                let is_mech = mech(&s) && q != read;
                flagged += is_mech as usize;
                nb.push(format!("({},{},{}) {}{}", q[0], q[1], q[2], s.trim_start_matches("minecraft:"), if is_mech { " MECHANISM" } else { "" }));
            }
            parts.push(format!("({},{},{}) {} [{}]", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"), nb.join(", ")));
        }
        lines.push(format!(
            "  power reach (each cell's six neighbours, the line enabled): {}; {}",
            parts.join("; "),
            if flagged == 0 { "no mechanism component beside them but the declared consumer (pass)".to_string() } else { format!("{flagged} mechanism neighbours (FAIL unless declared)") }
        ));
    }
    if !control.is_empty() {
        let mut all: Vec<(P, String)> = sets.to_vec();
        all.extend(control.iter().cloned());
        let csp = world_spec(base, &all)?;
        let crows = table(&csp, inputs, &ex, read, settle)?;
        let live = crows.iter().filter(|r| r.1).count();
        let mut ctl_probe = String::new();
        if !probes.is_empty() {
            let (_, run) = settled(&csp, inputs, &enable, read, settle)?;
            ctl_probe = format!(
                "; its probes with the line enabled: {}",
                probes.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"))).collect::<Vec<_>>().join("; ")
            );
        }
        lines.push(format!(
            "  the positive control (the cells {} laid over the candidate): the consumer powered in {live} of {} combinations ({}){ctl_probe}",
            control.iter().map(|(c, s)| format!("({},{},{}) {s}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", "),
            crows.len(),
            if live == 0 { "dead in every row: the control bites" } else { "FAIL: the control does not kill the line" }
        ));
    }
    Ok(lines)
}
