//! Study 14: combination locks (`circuit: lock N`).
//!
//! The player's dials are N item frames holding an item (`port input.d0: x y z frame`, d0..d<N-1>):
//! a right-click turns a frame's item one step of 8, and a comparator reading the frame through
//! the block it hangs on gives rotation + 1 (`ItemFrame.getAnalogOutput`). The code is set the
//! same way, by N code frames (`port input.k0: x y z frame`, one per dial): the lock opens
//! when every dial shows its code frame's rotation.
//!
//! The judge turns the frames like a player: one right-click with an empty hand per step (the
//! bridge's `use_entity`, `ItemFrame.interact` -> `setRotation(r + 1)`, which updates the
//! comparators reading it; rewriting the frame's data would not). It tries EVERY combination of
//! the dials (an odometer, [`STEP`] ticks per combination), then sets a second code by turning
//! every code frame 3 steps and tries every combination again. The output must be on exactly at
//! the code and off everywhere else: in each combination it may change once, from what the
//! previous combination showed to what this one must show, and must have settled before the next
//! click (no flicker while turning, no false open on the way through).

use crate::check::{Outcome, Sizing};
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, InputHow};
use anyhow::{bail, Result};

/// Ticks each combination is held: the lock must settle within it (2 s: the coordinator's call,
/// compactness beats speed for a lock; a comparator bus takes about 2 gt per dial).
pub const STEP: u32 = 40;
/// Steps each code frame turns to set the second code.
const RECODE: u8 = 3;

/// The dials' and the code frames' input indices, in digit order.
fn frames(c: &Circuit, n: u32) -> (Vec<usize>, Vec<usize>) {
    let find = |p: &str, i: u32| c.inputs.iter().position(|x| x.name == format!("{p}{i}"));
    let dials = (0..n).filter_map(|i| find("d", i)).collect();
    let keys = (0..n).filter_map(|i| find("k", i)).collect();
    (dials, keys)
}

/// Ports of a `lock N`: dials d0..d<N-1> and code frames k0..k<N-1>, every one a `frame` input,
/// and one output.
pub fn check_ports(c: &Circuit, n: u32) -> Result<()> {
    if n == 0 || n > 6 {
        bail!("lock {n}: 1 to 6 dials (8^N combinations are tried)");
    }
    let (dials, keys) = frames(c, n);
    if dials.len() != n as usize || keys.len() != n as usize || c.inputs.len() != 2 * n as usize {
        let names: Vec<_> = c.inputs.iter().map(|i| i.name.as_str()).collect();
        bail!("lock {n}: needs inputs d0..d{} (the dials) and k0..k{} (their code frames), nothing else; has {names:?}", n - 1, n - 1);
    }
    for i in &c.inputs {
        if !matches!(i.how, InputHow::Frame { .. }) {
            bail!("lock {n}: input {} must be an item frame (`port input.{}: x y z frame`)", i.name, i.name);
        }
    }
    if c.outputs.len() != 1 {
        bail!("lock {n}: needs exactly one output (on while the lock is open)");
    }
    hidden_code(c, n)
}

/// An item frame input's `Facing` (0 down, 1 up, 2 north, 3 south, 4 west, 5 east) as a unit
/// vector: the direction its face (and item) looks.
fn frame_facing(c: &Circuit, i: usize) -> Option<[i32; 3]> {
    let pos = c.inputs[i].pos;
    let e = c.clock.design.entities.iter().find(|e| e.kind.trim_start_matches("minecraft:") == "item_frame" && (0..3).all(|k| e.pos[k].floor() as i32 == pos[k]))?;
    let nbt = e.nbt.as_deref().unwrap_or("");
    let f = nbt.split("Facing:").nth(1)?.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u8>().ok()?;
    Some(match f {
        0 => [0, -1, 0],
        1 => [0, 1, 0],
        2 => [0, 0, -1],
        3 => [0, 0, 1],
        4 => [-1, 0, 0],
        5 => [1, 0, 0],
        _ => return None,
    })
}

fn dot(a: [i32; 3], b: [i32; 3]) -> i32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The hidden-code rule (vjeux, 2026-10-21: "we can't hide the answer from the user in this
/// implementation... not super useful for a lock"). The user stands in front of the dials: every
/// dial faces one side F, and that half-space (at or beyond the dials' plane) is the user side.
/// The code must be neither visible nor reachable from there: every code frame either hangs
/// BEHIND the dials' plane with its face turned away from the user (facing against F, so a player
/// in front sees at most the back of the block it hangs on), or is ENCLOSED (all five cells around
/// it but its support are opaque full blocks).
pub fn hidden_code(c: &Circuit, n: u32) -> Result<()> {
    let (dials, keys) = frames(c, n);
    let face = |i: usize| frame_facing(c, i).ok_or_else(|| anyhow::anyhow!("lock {n}: frame {} has no Facing", c.inputs[i].name));
    let f = face(dials[0])?;
    for &d in &dials {
        if face(d)? != f {
            bail!("lock {n}: every dial must face the same side (the user side): {} faces {:?}, {} faces {:?}", c.inputs[dials[0]].name, f, c.inputs[d].name, face(d)?);
        }
    }
    // the dials' plane: the deepest dial cell along F; the user side is everything at or past it
    let plane = dials.iter().map(|&d| dot(c.inputs[d].pos, f)).min().unwrap_or(0);
    for &k in &keys {
        frame_hidden(c, k, f, plane).map_err(|why| anyhow::anyhow!("lock {n}: {why}"))?;
    }
    Ok(())
}

/// One code frame against the hidden-code rule: behind the user's plane (`plane` along `f`, the
/// user side's direction) with its face turned away, or enclosed by opaque blocks on the five
/// sides but its support. Err(why) otherwise (also the sequence locks', [`crate::seqlock`]).
pub fn frame_hidden(c: &Circuit, k: usize, f: [i32; 3], plane: i32) -> std::result::Result<(), String> {
    let opaque = |p: [i32; 3]| -> bool {
        c.clock.design.blocks.get(&p).and_then(|s| redsim::block::Block::parse(s).ok()).is_some_and(|b| {
            redsim::behavior::conductor_of(&b) || matches!(b, redsim::block::Block::Piston { .. })
        })
    };
    let g = frame_facing(c, k).ok_or_else(|| format!("frame {} has no Facing", c.inputs[k].name))?;
    let p = c.inputs[k].pos;
    let back = dot(g, f) < 0 && dot(p, f) < plane;
    let support = [p[0] - g[0], p[1] - g[1], p[2] - g[2]];
    let enclosed = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
        .iter()
        .map(|d| [p[0] + d[0], p[1] + d[1], p[2] + d[2]])
        .filter(|q| *q != support)
        .all(opaque);
    if !back && !enclosed {
        return Err(format!(
            "the code is visible from the user side: code frame {} at {:?} faces {:?} while the user faces {:?}. Hide every code frame behind the user's plane, facing away from the user, or enclose it in opaque blocks",
            c.inputs[k].name, p, g, f
        ));
    }
    Ok(())
}

fn rotation(c: &Circuit, i: usize) -> u8 {
    match c.inputs[i].how {
        InputHow::Frame { rotation } => rotation,
        _ => 0,
    }
}

fn digits(v: &[u8]) -> String {
    v.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("")
}

/// Every combination of the dials (twice: the built code, then a code reset by the code frames).
/// Each phase is one combination: `dials 3517 code 3517 open` / `... shut`.
pub fn stimulus(c: &Circuit, n: u32, _z: &Sizing) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let (dials, keys) = frames(c, n);
    let mut rot: Vec<u8> = (0..c.inputs.len()).map(|i| rotation(c, i)).collect();
    let mut t = c.clock.free_from() + 40;
    for round in 0..2 {
        if round == 1 {
            // the player sets a new code: every code frame turned RECODE steps, one click each
            for &k in &keys {
                for _ in 0..RECODE {
                    rot[k] = (rot[k] + 1) % 8;
                    s.levels.push((t, k, rot[k]));
                    t += 4;
                }
            }
            t += 40;
        }
        let code: Vec<u8> = keys.iter().map(|&k| rot[k]).collect();
        let start: Vec<u8> = dials.iter().map(|&d| rot[d]).collect();
        let total = 8u32.pow(n);
        for step in 0..total {
            if step > 0 {
                // odometer: dial 0 one step; a dial back at its start carries into the next one
                for (j, &d) in dials.iter().enumerate() {
                    rot[d] = (rot[d] + 1) % 8;
                    s.levels.push((t, d, rot[d]));
                    if rot[d] != start[j] {
                        break;
                    }
                }
            }
            let now: Vec<u8> = dials.iter().map(|&d| rot[d]).collect();
            let open = now == code;
            s.phase(t, t + STEP, format!("dials {} code {} {}", digits(&now), digits(&code), if open { "open" } else { "shut" }));
            t += STEP;
        }
        t += 20;
    }
    s.ticks = t;
    s
}

/// The output per combination: settled to its value before the next click, at most one change
/// in each combination (from the previous combination's value to this one's).
pub fn check(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let frames = t.frames();
    let on = |f: usize| f < frames && t.on(0, f);
    let mut table = Vec::new();
    let (mut open_lat, mut close_lat, mut tried, mut opened) = (0usize, 0usize, 0usize, 0usize);
    let mut prev: Option<bool> = None;
    let mut prev_end = 0u32;
    for (a, b, label) in &s.phases {
        let want = label.ends_with(" open");
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        if a >= b {
            return Outcome::fail(format!("the run ended before {label}"), table);
        }
        // a phase right after a gap (the code being set) starts from whatever the lock shows
        let from = if a as u32 == prev_end { prev } else { None };
        let seq: Vec<bool> = (a..b).map(on).collect();
        let changes = seq.windows(2).filter(|w| w[0] != w[1]).count();
        if seq.last() != Some(&want) {
            table.extend(states(c, t, b - 1));
            return Outcome::fail(
                format!("{label}: the output is {} at the end of the combination ({} gt after the click), want {}", onoff(*seq.last().unwrap_or(&false)), b - a, onoff(want)),
                table,
            );
        }
        let first = seq.first().copied().unwrap_or(false);
        let flicker = changes > 1 || (changes == 1 && from == Some(want)) || (from.is_some_and(|p| p != first) && first != want);
        if flicker {
            let at: Vec<String> = seq.iter().enumerate().filter(|(i, v)| *i == 0 || seq[i - 1] != **v).map(|(i, v)| format!("{} at +{i}", onoff(*v))).collect();
            return Outcome::fail(format!("{label}: the output flickers while turning ({})", at.join(", ")), table);
        }
        // latency: the first frame from which the output stays at its value
        let lat = seq.iter().rposition(|v| *v != want).map(|i| i + 1).unwrap_or(0);
        if want && from == Some(false) {
            open_lat = open_lat.max(lat);
            opened += 1;
            table.push(format!("{label} after {lat} gt"));
        }
        if !want && from == Some(true) {
            close_lat = close_lat.max(lat);
            table.push(format!("{label} after {lat} gt"));
        }
        tried += 1;
        prev = Some(want);
        prev_end = b as u32;
    }
    let codes = s.phases.iter().filter(|p| p.2.ends_with(" open")).count();
    if opened != codes || codes != 2 {
        return Outcome::fail(format!("lock {n}: opened at {opened} of {codes} codes (2 codes tried)"), table);
    }
    let metrics = vec![
        ("dials".to_string(), format!("{n}")),
        ("combinations tried".to_string(), format!("{tried} ({} per code, 2 codes)", 8usize.pow(n))),
        ("opens within".to_string(), format!("{open_lat} gt")),
        ("closes within".to_string(), format!("{close_lat} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// One dial of the `lock N` family as cells relative to its main row (z) at component level `h`:
/// (x, y, z, block letter of `alt_design`'s palette), plus its two frames (cell, code?). With
/// `mirror` the W row is north of the main row instead of south.
fn dial_cells(dx: i32, h: i32, z: i32, mirror: bool) -> (Vec<((i32, i32, i32), char)>, [(i32, i32, i32); 2]) {
    let s = h - 1;
    let w = if mirror { z - 1 } else { z + 1 };
    let c = vec![
        ((dx - 2, h, z), 'S'),
        ((dx - 1, h, z), 'T'),
        ((dx, h, z), 'w'),
        ((dx - 1, s, z), 'S'),
        ((dx, s, z), 'S'),
        ((dx + 1, s, z), 'A'),
        ((dx + 2, s, z), 'b'),
        ((dx + 2, s - 1, z), 'S'),
        ((dx - 3, h, w), 'S'),
        ((dx - 2, h, w), 'T'),
        ((dx - 1, h, w), 'w'),
        ((dx, h, w), 'T'),
        ((dx + 1, h, w), 'S'),
        ((dx + 2, h, w), 'A'),
        ((dx - 2, s, w), 'S'),
        ((dx - 1, s, w), 'S'),
        ((dx, s, w), 'S'),
        ((dx + 2, s, w), 'b'),
        ((dx + 2, s - 1, w), 'S'),
    ];
    (c, [(dx - 3, h, z), (dx - 4, h, w)])
}

/// A lock from dial placements (dx, h, z, mirror): the cells merged (a cell two dials share must be
/// stone in both), the output torch west of the first dial's bus-end support. `None` on a clash
/// (two different blocks in a cell, a frame cell taken, two frames in one cell).
pub fn placed_design(dials: &[(i32, i32, i32, bool)]) -> Option<(String, usize, i64)> {
    let codes = [3u8, 5, 1, 6, 2, 7];
    let mut cells: std::collections::BTreeMap<(i32, i32, i32), char> = std::collections::BTreeMap::new();
    let mut frames: Vec<((i32, i32, i32), u8)> = Vec::new();
    for (i, &(dx, h, z, m)) in dials.iter().enumerate() {
        let (c, f) = dial_cells(dx, h, z, m);
        for (p, b) in c {
            match cells.get(&p) {
                // two dials may share a stone, or a bus dust ('b': the bus is one OR network)
                Some(&o) if o != b || (b != 'S' && b != 'b') => return None,
                _ => {
                    cells.insert(p, b);
                }
            }
        }
        frames.push((f[0], 0));
        frames.push((f[1], codes[i % codes.len()]));
    }
    let (dx0, h0, z0, _) = dials[0];
    let o = (dx0 + 1, h0 - 2, z0);
    if cells.contains_key(&o) {
        return None;
    }
    cells.insert(o, 'O');
    for (k, (p, _)) in frames.iter().enumerate() {
        if cells.contains_key(p) || frames[..k].iter().any(|(q, _)| q == p) {
            return None;
        }
    }
    let n = dials.len();
    let lo = |f: fn(&(i32, i32, i32)) -> i32| cells.keys().map(f).min().unwrap_or(0);
    let hi = |f: fn(&(i32, i32, i32)) -> i32| cells.keys().map(f).max().unwrap_or(0);
    let (x0, y0, z0b) = (lo(|p| p.0), lo(|p| p.1), lo(|p| p.2));
    let (x1, y1, z1) = (hi(|p| p.0), hi(|p| p.1), hi(|p| p.2));
    let volume = ((x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0b + 1)) as i64;
    let mut out = format!("name: lock{n}-search\ncircuit: lock {n}\n");
    for (i, k) in (0..n).zip(frames.chunks(2)) {
        out += &format!("port input.d{i}: {} {} {} frame\nport input.k{i}: {} {} {} frame\n", k[0].0.0, k[0].0.1, k[0].0.2, k[1].0.0, k[1].0.1, k[1].0.2);
    }
    out += &format!("port output: {} {} {}\n", o.0, o.1, o.2);
    let item = "Item:{id:\"minecraft:arrow\",count:1}";
    for (p, r) in &frames {
        // a cell's center is x + 0.5 (-2.5 for x = -3, not "-3.5")
        out += &format!("entity: minecraft:item_frame {} {} {} {{Facing:4b,{item},ItemRotation:{r}b}}\n", p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5);
    }
    out += &format!("origin: {x0} 0 {z0b}\npalette:\n  S minecraft:stone\n  w minecraft:redstone_wire\n  T minecraft:comparator[facing=west,mode=compare,powered=false]\n  A minecraft:redstone_wall_torch[facing=east,lit=true]\n  O minecraft:redstone_wall_torch[facing=west,lit=true]\n");
    for y in y0..=y1 {
        out += &format!("layer {y}:\n");
        for z in z0b..=z1 {
            let row: String = (x0..=x1).map(|x| match cells.get(&(x, y, z)).copied().unwrap_or('.') { 'b' => 'w', c => c }).collect();
            let row = row.trim_end_matches('.');
            out += &format!("  {}\n", if row.is_empty() { "." } else { row });
        }
    }
    // blocks as the judge counts them: the cells plus the frames
    Some((out, cells.len() + frames.len(), volume))
}

fn onoff(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

/// The `lock N` family (clk-item, study 14): lock3-alt's pattern for any N. Each dial is 2 rows:
/// T = compare(dial frame, code) in the main row with its dust D1 (a torch on D1's support
/// catches "too low"), and W = compare(code, D1) beside the code dust (a torch on W's front block
/// catches "too high"). Both torches feed one dust bus under them. The code comparator K reads its
/// frame from the west, so all frames hang on the west wall. Odd dials sit one level lower, so a
/// comparator's side next to another dial faces a plain support block (no spacer rows); the bus
/// steps down and up diagonally. The output torch hangs on the support under the bus's north end.
pub fn alt_design(n: u32) -> String {
    let codes = [3u8, 5, 1, 6, 2, 7];
    let mut cells: std::collections::BTreeMap<(i32, i32, i32), char> = std::collections::BTreeMap::new();
    let mut ports = Vec::new();
    let mut ents = Vec::new();
    for i in 0..n as i32 {
        let z = 2 * i;
        let h = if i % 2 == 0 { 3 } else { 2 };
        let s = h - 1;
        for (p, c) in [
            ((-2, h, z), 'S'),
            ((-1, h, z), 'T'),
            ((0, h, z), 'w'),
            ((-1, s, z), 'S'),
            ((0, s, z), 'S'),
            ((1, s, z), 'A'),
            ((2, s, z), 'w'),
            ((2, s - 1, z), 'S'),
            ((-3, h, z + 1), 'S'),
            ((-2, h, z + 1), 'T'),
            ((-1, h, z + 1), 'w'),
            ((0, h, z + 1), 'T'),
            ((1, h, z + 1), 'S'),
            ((2, h, z + 1), 'A'),
            ((-2, s, z + 1), 'S'),
            ((-1, s, z + 1), 'S'),
            ((0, s, z + 1), 'S'),
            ((2, s, z + 1), 'w'),
            ((2, s - 1, z + 1), 'S'),
        ] {
            cells.insert(p, c);
        }
        ports.push(format!("port input.d{i}: -3 {h} {z} frame"));
        ports.push(format!("port input.k{i}: -4 {h} {} frame", z + 1));
        let item = "Item:{id:\"minecraft:arrow\",count:1}";
        ents.push(format!("entity: minecraft:item_frame -2.5 {} {} {{Facing:4b,{item},ItemRotation:0b}}", mctas::cell_centre(h), mctas::cell_centre(z)));
        ents.push(format!("entity: minecraft:item_frame -3.5 {} {} {{Facing:4b,{item},ItemRotation:{}b}}", mctas::cell_centre(h), mctas::cell_centre(z + 1), codes[i as usize % codes.len()]));
    }
    // the output torch on the west face of the support under the bus's north end
    cells.insert((1, 1, 0), 'O');
    let (y0, y1) = (cells.keys().map(|p| p.1).min().unwrap_or(0), cells.keys().map(|p| p.1).max().unwrap_or(0));
    let z1 = 2 * n as i32 - 1;
    let mut out = format!(
        "name: lock{n}-alt\n# Combination lock, {n} dial{} (study 14, `mctas circuit lock-gen --n {n}`): two comparators per dial, all frames on the west wall; the same 2-row pattern for any number of dials (21 blocks per dial, plus the output torch).\ncircuit: lock {n}\ngroup: lock-alt\ngroup title: Combination lock (any number of dials)\ngroup note: two comparators per dial (the dial at least the code, the code at least the dial), a torch for each failure on one dust bus; 21 blocks and 2 rows per dial (`mctas circuit lock-gen --n N`)\n",
        if n == 1 { "" } else { "s" }
    );
    for p in &ports {
        out += &format!("{p}\n");
    }
    out += "port output: 1 1 0\n";
    for e in &ents {
        out += &format!("{e}\n");
    }
    out += &format!("origin: -3 0 0\npalette:\n  S minecraft:stone\n  w minecraft:redstone_wire\n  T minecraft:comparator[facing=west,mode=compare,powered=false]\n  A minecraft:redstone_wall_torch[facing=east,lit=true]\n  O minecraft:redstone_wall_torch[facing=west,lit=true]\n");
    // `layer N` is relative to the origin, which stays at y = 0 so the labels are the frames' y
    for y in y0..=y1 {
        out += &format!("layer {y}:\n");
        for z in 0..=z1 {
            let row: String = (-3..=2).map(|x| cells.get(&(x, y, z)).copied().unwrap_or('.')).collect();
            // an all-air row keeps one '.': the parser skips blank lines (the rows would shift)
            let row = row.trim_end_matches('.');
            out += &format!("  {}\n", if row.is_empty() { "." } else { row });
        }
    }
    out
}

/// The hidden-code `lock N` family (clk-item, study 14, after the hidden-code rule): alt's logic
/// (T = compare(dial, code) with its dust D1 and a torch for "too low"; W = compare(code, D1) with
/// a torch for "too high"; one dust bus under all the torches), but every code frame hangs on the
/// BACK face facing away from the player. Its comparator K faces the back, so it powers a block B
/// beside the code dust X instead of feeding X head-on (X then carries the code undecayed to T's
/// side and W's rear). A dial takes 3 rows (T, W, K); dials pair up back to back (K rows touching,
/// the second dial mirrored), and a spacer row separates two pairs (two T rows side by side would
/// join their D1 dusts).
pub fn hidden_design(n: u32) -> String {
    let codes = [3u8, 5, 1, 6, 2, 7];
    let mut cells: std::collections::BTreeMap<(i32, i32, i32), char> = std::collections::BTreeMap::new();
    let mut ports = Vec::new();
    let mut ents = Vec::new();
    let item = "Item:{id:\"minecraft:arrow\",count:1}";
    let mut zlast = 0;
    for i in 0..n as i32 {
        let z0 = (i / 2) * 7;
        let (zt, zw, zk) = if i % 2 == 0 { (z0, z0 + 1, z0 + 2) } else { (z0 + 5, z0 + 4, z0 + 3) };
        for (p, c) in [
            ((-2, 2, zt), 'S'),
            ((-1, 2, zt), 'T'),
            ((0, 2, zt), 'w'),
            ((-1, 1, zt), 'S'),
            ((0, 1, zt), 'S'),
            ((1, 1, zt), 'A'),
            ((-1, 2, zw), 'w'),
            ((0, 2, zw), 'T'),
            ((1, 2, zw), 'S'),
            ((2, 2, zw), 'A'),
            ((-1, 1, zw), 'S'),
            ((0, 1, zw), 'S'),
            ((-1, 2, zk), 'S'),
            ((0, 2, zk), 'K'),
            ((1, 2, zk), 'S'),
            ((0, 1, zk), 'S'),
        ] {
            cells.insert(p, c);
        }
        zlast = zlast.max(zt.max(zw));
        ports.push(format!("port input.d{i}: -3 2 {zt} frame"));
        ports.push(format!("port input.k{i}: 2 2 {zk} frame"));
        ents.push(format!("entity: minecraft:item_frame -2.5 2.5 {} {{Facing:4b,{item},ItemRotation:0b}}", mctas::cell_centre(zt)));
        ents.push(format!("entity: minecraft:item_frame 2.5 2.5 {} {{Facing:5b,{item},ItemRotation:{}b}}", mctas::cell_centre(zk), codes[i as usize % codes.len()]));
    }
    // the bus under every torch, and the output torch on the west face of its first support
    for z in 0..=zlast {
        cells.insert((2, 1, z), 'w');
        cells.insert((2, 0, z), 'S');
    }
    cells.insert((1, 0, 0), 'O');
    let zmax = cells.keys().map(|p| p.2).max().unwrap_or(0);
    let mut out = format!(
        "name: lock{n}-hid\n# Combination lock, {n} dial{} with the code hidden (study 14, `mctas circuit lock-gen --hidden --n {n}`): the dials on the front (west) face, the code frames on the back (east) face facing away; two comparators per dial plus the code's reader.\ncircuit: lock {n}\ngroup: lock-hid\ngroup title: Combination lock with a hidden code (any number of dials)\ngroup note: the code frames hang on the back face, facing away from the player; per dial a code reader, two comparators (the dial at least the code, the code at least the dial) and a torch for each failure on one dust bus (`mctas circuit lock-gen --hidden --n N`)\n",
        if n == 1 { "" } else { "s" }
    );
    for p in &ports {
        out += &format!("{p}\n");
    }
    out += "port output: 1 0 0\n";
    for e in &ents {
        out += &format!("{e}\n");
    }
    out += "origin: -3 0 0\npalette:\n  S minecraft:stone\n  w minecraft:redstone_wire\n  T minecraft:comparator[facing=west,mode=compare,powered=false]\n  K minecraft:comparator[facing=east,mode=compare,powered=false]\n  A minecraft:redstone_wall_torch[facing=east,lit=true]\n  O minecraft:redstone_wall_torch[facing=west,lit=true]\n";
    for y in 0..=2 {
        out += &format!("layer {y}:\n");
        for z in 0..=zmax {
            let row: String = (-3..=2).map(|x| cells.get(&(x, y, z)).copied().unwrap_or('.')).collect();
            let row = row.trim_end_matches('.');
            out += &format!("  {}\n", if row.is_empty() { "." } else { row });
        }
    }
    out
}

/// The design's redstone components at frame `f` (a failure's evidence: what each comparator
/// read and gave).
fn states(c: &Circuit, t: &Trace, f: usize) -> Vec<String> {
    let mut v = vec![format!("components at frame {f}:")];
    for p in c.clock.design.blocks.keys() {
        if let Some(s) = t.state(f, *p).filter(|s| ["comparator", "repeater", "wire", "torch", "lamp"].iter().any(|k| s.contains(k))) {
            v.push(format!("  {p:?} {}", s.trim_start_matches("minecraft:")));
        }
    }
    v
}
