//! The calm trigger's catalog part (designs/briefs/farm-optimizer.md, "The calm's catalog
//! part"): the smallest archived timing part legal for the stage's materials whose output for the
//! bed observer's 2 gt pulse lasts at least L_min = 20 gt (`mobfarm::parts::timers`, the ironfarm
//! lane's), placed behind the observer (`parts::place`) and routed to the hide (`parts::route`), as
//! `day1::m2_front_timer` does for M2, at any module offset. One calm can drive a row of modules:
//! a repeater after each module's feed and a route to the next module's.

use crate::parts::Ports;
use anyhow::{Result, anyhow, bail};
use mobfarm::layouts::Builder;
use mobfarm::parts::{self as mp, Stage};

/// L_min (gt): every hostile-sensor phase sees the hide shut once (the brief's tick-order proof).
pub const L_MIN: u32 = 20;

/// A catalog timing part as the grammar uses it.
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    /// The repeater delay (redstone ticks) that stretches the observer's pulse to its min-pulse.
    pub stretch: Option<u8>,
    pub length: u32,
    pub blocks: u32,
    pub row: mp::Row,
}

/// The stage's materials as the parts API names them.
pub fn stage_of(stage: &str) -> Stage {
    match stage {
        "day-1" => Stage::Day1 { redstone: false },
        "day-1-dust" => Stage::Day1 { redstone: true },
        "late" => Stage::End,
        _ => Stage::Nether,
    }
}

/// Every part that meets the need for the stage's materials, fewest blocks first, the 4 smallest
/// kept (every part's output already covers L_min, so the farm differs only by the part's and its
/// wiring's blocks; the grammar keeps a few for the wiring to choose from). Empty: none, and the
/// observer cages leave the space; the front says so.
pub fn parts(stage: &str) -> Vec<Part> {
    let Ok(rows) = mp::catalog() else { return Vec::new() };
    mp::timers(&rows, L_MIN, 400, 2, stage_of(stage)).into_iter().take(4).map(|t| Part { name: t.row.name.clone(), stretch: t.stretch, length: t.length, blocks: t.blocks, row: t.row.clone() }).collect()
}

fn beside(a: [i32; 3], c: [i32; 3]) -> bool {
    (a[0] - c[0]).abs() + (a[1] - c[1]).abs() + (a[2] - c[2]).abs() == 1
}

/// A cell sharing a face with the hide's driver stone (west of `feed`) at its level, other than
/// the feed itself: dust there would point into the stone or connect to the driver's dust above it
/// (one up, one across) and turn it into an L that no longer points into the trapdoor. Diagonal
/// cells connect to neither.
fn near_driver(q: [i32; 3], feed: [i32; 3]) -> bool {
    q != feed && q[1] == feed[1] && (q[0] - (feed[0] - 1)).abs() + (q[2] - feed[2]).abs() == 1
}

/// The branch from a feed to the next module: a repeater on a free face of the feed (east, north
/// or south, never west: the driver), facing away from it; returns (the repeater, the route's
/// first cell beyond it).
fn branch(b: &mut Builder, feed: [i32; 3]) -> Option<([i32; 3], [i32; 3])> {
    for (d, facing) in [([1, 0, 0], "west"), ([0, 0, -1], "south"), ([0, 0, 1], "north")] {
        let r = [feed[0] + d[0], feed[1], feed[2] + d[2]];
        let s = [r[0] + d[0], r[1], r[2] + d[2]];
        if b.d.blocks.contains_key(&r) || b.d.blocks.contains_key(&s) || b.d.blocks.contains_key(&[r[0], r[1] - 1, r[2]]) {
            continue;
        }
        b.set(r[0], r[1] - 1, r[2], crate::parts::STONE);
        b.set(r[0], r[1], r[2], &format!("repeater[delay=1,facing={facing},locked=false,powered=false]"));
        return Some((r, s));
    }
    None
}

/// The climb onto the hide's stone: the route's last dust (`goal`, east of the stone under the
/// hide's dust) pointing west into it.
fn climb(b: &mut Builder, goal: [i32; 3]) {
    if let Some(s) = b.d.blocks.get(&goal).cloned() {
        b.set(goal[0], goal[1], goal[2], &s.replace("west=none", "west=side"));
    }
}

/// Place `part` behind a bed observer watching module `p`'s U1 and route its output to the hide
/// feed; of both observer places (east of U1's bed head, its pulse going east; north of it, going
/// north) and the part's 4 turns, keep the fewest blocks. `at`: the module's origin.
pub fn wire(b: &mut Builder, part: &Part, p: &Ports, at: [i32; 3]) -> Result<()> {
    let (Some(u1), Some(feed)) = (p.u1_bed, p.hide_feed) else { bail!("the cage has no U1 bed or hide feed") };
    let mut best: Option<Builder> = None;
    // the observer on any side of U1's bed head but the foot's: east (its pulse going east), west
    // (going west) or north (going north); the repeater that stretches its pulse faces back at it
    let sides = [([u1[0] + 1, u1[1], u1[2]], "west", [1, 0, 0], "west"), ([u1[0] - 1, u1[1], u1[2]], "east", [-1, 0, 0], "east"), ([u1[0], u1[1], u1[2] - 1], "south", [0, 0, -1], "south")];
    let (row_lo, row_hi) = (p.row.0 - at[0], p.row.1 - at[0]);
    let u1x = u1[0] - at[0];
    let west = u1x <= row_lo;
    for (obs, facing, out, back) in sides {
        if b.d.blocks.contains_key(&obs) {
            continue;
        }
        let mut base = Builder { d: b.d.clone() };
        base.set(obs[0], obs[1], obs[2], &format!("observer[facing={facing},powered=false]"));
        base.set(obs[0], obs[1] - 1, obs[2], crate::parts::STONE);
        let step = |q: [i32; 3], k: i32| [q[0] + k * out[0], q[1], q[2] + k * out[2]];
        let mut source = vec![obs];
        let input = match part.stretch {
            Some(d) => {
                let r = step(obs, 1);
                if base.d.blocks.contains_key(&r) {
                    continue;
                }
                base.set(r[0], r[1] - 1, r[2], crate::parts::STONE);
                base.set(r[0], r[1], r[2], &format!("repeater[delay={d},facing={back},locked=false,powered=false]"));
                source.push(r);
                step(obs, 2)
            }
            None => step(obs, 1),
        };
        let hx = feed[0] - at[0] - 2;
        for turns in 0..4u8 {
            let mut t = Builder { d: base.d.clone() };
            let Ok(placed) = mp::place(&mut t, &part.row, turns, input) else { continue };
            // the part in free cells beyond U1 (away from the row), or low between the pods and the
            // zombie (under the villagers' lines of sight), clear of the hide and the zombie's row
            let region = |c: &[i32; 3]| {
                let (x, y, z) = (c[0] - at[0], c[1] - at[1], c[2] - at[2]);
                let beyond = if west { x <= u1x - 2 } else { x >= u1x + 2 };
                (beyond && (-5..=8).contains(&z)) || (y <= 0 && (row_lo - 1..=row_hi + 1).contains(&x) && (-5..=-1).contains(&z))
            };
            let fits = placed.cells.iter().all(|c| (*c == input || !base.d.blocks.contains_key(c)) && region(c) && (*c == input || source.iter().all(|s| !beside(*s, *c))));
            if !fits {
                continue;
            }
            let Some(start) = mp::read_output(&mut t, &placed) else { continue };
            let forbid = |q: [i32; 3]| {
                let (x, z) = (q[0] - at[0], q[2] - at[2]);
                x < row_lo - 14 || x > row_hi + 14 || z < -6 || z > 10 || (z >= 0 && x >= row_lo - 1 && x <= row_hi + 1) || (z == -6 && x < hx + 2) || near_driver(q, feed)
            };
            // allowed beside the path: the part's output repeater, the hide's driver stone and the
            // driver's dust on it (the goal meets that dust on the diagonal: the climb)
            let Some(_path) = mp::route(&mut t, start, feed, &forbid, &placed.cells, &[placed.output, [feed[0] - 1, feed[1], feed[2]], [feed[0] - 1, feed[1] + 1, feed[2]]], 1) else { continue };
            climb(&mut t, feed);
            if best.as_ref().is_none_or(|bb| t.d.blocks.len() < bb.d.blocks.len()) {
                best = Some(t);
            }
        }
    }
    let t = best.ok_or_else(|| anyhow!("calm part {}: no placement and route fits", part.name))?;
    b.d = t.d;
    Ok(())
}

/// Whether a cell at the sight height (y 1..2 of the module) crosses a scared villager's line to
/// the zombie: every segment from a corner of a scared villager's cell to a corner of the zombie's
/// cell, in plan (the judge casts every ray from every corner of the villager's cell; below y 1 and
/// above y 2 a cell is under or over every eye line).
fn on_a_line(q: [i32; 3], xs: &[i32], zx: i32) -> bool {
    if !(1..=2).contains(&q[1]) {
        return false;
    }
    let (x0, x1, z0, z1) = (q[0] as f64, q[0] as f64 + 1.0, q[2] as f64, q[2] as f64 + 1.0);
    // Liang-Barsky: does the segment a-b cross the open square?
    let crosses = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, qv) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dz, a.1 - z0), (dz, z1 - a.1)] {
            if p.abs() < 1e-12 {
                if qv <= 0.0 {
                    return false;
                }
            } else {
                let r = qv / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        t0 < t1 - 1e-9
    };
    let zc = [(zx as f64, -7.0), (zx as f64 + 1.0, -7.0), (zx as f64, -6.0), (zx as f64 + 1.0, -6.0)];
    xs.iter().any(|&x| [(x as f64, 0.0), (x as f64 + 1.0, 0.0), (x as f64, 1.0), (x as f64 + 1.0, 1.0)].iter().any(|v| zc.iter().any(|z| crosses(*v, *z))))
}

/// Day 1's falling-edge pulse part (ironspec, catalog row 189): the smallest robust catalog part
/// of kind `falling-edge` made only of day-1 blocks (torches, dust, repeaters: no quartz, copper or
/// pistons) whose pulse covers L_min. None: none in the catalog.
pub fn pulse_part() -> Option<Part> {
    static PART: std::sync::OnceLock<Option<Part>> = std::sync::OnceLock::new();
    PART.get_or_init(|| {
        let rows = mp::catalog().ok()?;
        let day1 = |fam: &str| fam.split('+').all(|b| matches!(b, "redstone_torch" | "redstone_wire" | "repeater" | "stone" | "cobblestone"));
        let mut c: Vec<mp::Row> = rows.into_iter().filter(|r| r.kind == "falling-edge" && r.robustness == "robust" && day1(&r.family) && r.axis.trim_start_matches("L = ").trim_end_matches(" gt").parse::<u32>().is_ok_and(|l| l >= L_MIN)).collect();
        c.sort_by_key(|r| r.blocks);
        let r = c.into_iter().next()?;
        let length = r.axis.trim_start_matches("L = ").trim_end_matches(" gt").parse::<u32>().unwrap_or(L_MIN);
        Some(Part { name: r.name.clone(), stretch: None, length, blocks: r.blocks, row: r })
    })
    .clone()
}

/// The dust fork's calm (day 1): the falling-edge pulse part placed with its input on the stone
/// under U1's plate (the plate strongly powers it all day; U1 lying down at dusk is the falling
/// edge) and its output routed to the hide's feed; of the part's 4 turns, the fewest blocks. Its
/// cells go under the pods' level or beyond U1, clear of the hide and the zombie's row.
pub fn wire_plate(b: &mut Builder, part: &Part, p: &Ports, at: [i32; 3]) -> Result<()> {
    let (Some(u1), Some(feed)) = (p.u1_bed, p.hide_feed) else { bail!("the cage has no U1 bed or hide feed") };
    let input = [u1[0], u1[1] - 1, u1[2] - 1];
    let plate = [u1[0], u1[1], u1[2] - 1];
    let (row_lo, row_hi) = (p.row.0 - at[0], p.row.1 - at[0]);
    let u1x = u1[0] - at[0];
    let west = u1x <= row_lo;
    let hx = feed[0] - at[0] - 2;
    let mut best: Option<Builder> = None;
    for turns in 0..4u8 {
        let mut t = Builder { d: b.d.clone() };
        let Ok(placed) = mp::place(&mut t, &part.row, turns, input) else { continue };
        let region = |c: &[i32; 3]| {
            let (x, y, z) = (c[0] - at[0], c[1] - at[1], c[2] - at[2]);
            let beyond = if west { x <= u1x } else { x >= u1x };
            // (as the routed calm's: low between the pods and the zombie, under the standing lines)
            let under = y <= 0 && (row_lo - 12..=row_hi + 12).contains(&x) && (-5..=6).contains(&z);
            // (a regular row's U1 sleeps at the watchers' level: the part one lower, so it may
            // reach the zombie's rows beyond the row's end, under its slab's level)
            let low = u1[1] - at[1] <= 0 && beyond && (-9..=8).contains(&z) && y <= -1;
            (beyond && (-6..=8).contains(&z) && y <= 1) || under || low
        };
        let fits = placed.cells.iter().all(|c| (*c == input || !b.d.blocks.contains_key(c)) && region(c) && (*c == input || !beside(plate, *c)));
        if std::env::var("FARMOPT_DEBUG").is_ok() {
            let bad: Vec<_> = placed.cells.iter().filter(|c| !((**c == input || !b.d.blocks.contains_key(*c)) && region(c) && (**c == input || !beside(plate, **c)))).map(|c| ([c[0] - at[0], c[1] - at[1], c[2] - at[2]], b.d.blocks.get(c).cloned().unwrap_or_default())).take(4).collect();
            eprintln!("wire_plate turn {turns}: fits {fits} bad {bad:?}");
        }
        if !fits {
            continue;
        }
        // the part's output is its torch (T_out): the route starts on a free cell beside it, not
        // the part's own cells (dust there takes the torch's 15)
        let read = mp::read_output(&mut t, &placed).or_else(|| {
            let o = placed.output;
            [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().map(|d| [o[0] + d[0], o[1], o[2] + d[2]]).find(|q| !placed.cells.contains(q) && !t.d.blocks.contains_key(q) && !t.d.blocks.contains_key(&[q[0], q[1] - 1, q[2]]) || (!placed.cells.contains(q) && !t.d.blocks.contains_key(q) && placed.cells.contains(&[q[0], q[1] - 1, q[2]])))
        });
        let Some(start) = read else {
            if std::env::var("FARMOPT_DEBUG").is_ok() {
                eprintln!("wire_plate turn {turns}: no output read");
            }
            continue;
        };
        let forbid = |q: [i32; 3]| {
            let (x, z) = (q[0] - at[0], q[2] - at[2]);
            // (a regular row's U1 sleeps at the watchers' level: the dust may pass north of the
            // zombie's row too, below it)
            let zmin = if u1[1] - at[1] <= 0 { -9 } else { -6 };
            x < row_lo - 14 || x > row_hi + 14 || z < zmin || z > 10 || (z < -6 && q[1] - at[1] >= 0) || (z >= 0 && x >= row_lo - 1 && x <= row_hi + 1 && q[1] - at[1] >= 0) || (z == -6 && x < hx + 2) || near_driver(q, feed) || beside(plate, q) || q == plate
        };
        // the route on the part's output level; a regular row's part sits a level below the feed:
        // its dust ends one east of the feed and one lower, then climbs onto the block under the
        // feed's dust (and the feed's climb onto the driver stone follows)
        let low = start[1] == feed[1] - 1;
        let goal = if low { [feed[0] + 1, feed[1] - 1, feed[2]] } else { feed };
        let allow = [placed.output, [feed[0] - 1, feed[1], feed[2]], [feed[0] - 1, feed[1] + 1, feed[2]], feed, [feed[0], feed[1] - 1, feed[2]]];
        let Some(_path) = mp::route(&mut t, start, goal, &forbid, &placed.cells, &allow, 1) else { continue };
        if low {
            let under = [feed[0], feed[1] - 1, feed[2]];
            if t.d.blocks.get(&under).is_some_and(|s| !s.ends_with(":air") && !s.starts_with("air")) && !t.d.blocks.get(&under).is_some_and(|s| s.contains("stone")) {
                continue;
            }
            t.set(under[0], under[1], under[2], crate::parts::STONE);
            t.set(feed[0], feed[1], feed[2], "redstone_wire[east=up,north=none,power=0,south=none,west=side]");
            // the dust's step: the cell over its lower end open
            let over = [goal[0], goal[1] + 1, goal[2]];
            if !t.d.blocks.contains_key(&over) {
                t.set(over[0], over[1], over[2], "air");
            }
        }
        climb(&mut t, feed);
        let above = [feed[0], feed[1] + 1, feed[2]];
        if !t.d.blocks.contains_key(&above) {
            t.set(above[0], above[1], above[2], "air");
        }
        if best.as_ref().is_none_or(|bb| t.d.blocks.len() < bb.d.blocks.len()) {
            best = Some(t);
        }
    }
    let t = best.ok_or_else(|| anyhow!("pulse part {}: no placement and route fits under U1's plate", part.name))?;
    b.d = t.d;
    Ok(())
}

/// The calm placed jointly with its cage (the parent, 12:10: "the timer's input next to the bed
/// observer and its output next to the hide's trapdoor"): the timer's output repeater powers the
/// trapdoor directly from beside it (no dust), the timer behind it, the observer (and the
/// repeater that stretches its pulse) in line behind the timer's input, and U1's pod where that
/// line ends (its bed head watched), each cell clear of the scared villagers' lines to the zombie,
/// U1 within the cluster. Of every turn, side and line direction, the fewest blocks. `with_u1`
/// must have been false for the module (this places U1). Err: no placement fits.
pub fn wire_direct(b: &mut Builder, part: &Part, cage: &crate::parts::Cage, at: [i32; 3], place: crate::parts::Place) -> Result<()> {
    let (xs, zx) = crate::parts::scared_xs(cage);
    let hide = [at[0] + zx, at[1] + 1, at[2] - 6];
    // the routed calm's driver (stone + dust east of the hide) is not needed: the repeater replaces it
    for q in [[hide[0] + 1, hide[1] - 1, hide[2]], [hide[0] + 1, hide[1], hide[2]]] {
        b.d.blocks.remove(&q);
    }
    let local = |q: [i32; 3]| [q[0] - at[0], q[1] - at[1], q[2] - at[2]];
    let free = |bb: &Builder, q: [i32; 3]| bb.d.blocks.get(&q).is_none_or(|s| s.ends_with(":air"));
    // the part's shape per turn, from a placement at the origin
    let mut best: Option<Builder> = None;
    for turns in 0..4u8 {
        let mut probe = Builder::new("probe");
        let Ok(pl) = mp::place(&mut probe, &part.row, turns, [0, 0, 0]) else { continue };
        let o = pl.output;
        // the part cell beside its output cell: the output repeater's input side
        let sides = [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 0, 1], "south"), ([0, 0, -1], "north")];
        let beside: Vec<([i32; 3], &str)> = sides.iter().filter(|(d, _)| pl.cells.contains(&[o[0] + d[0], o[1], o[2] + d[2]])).copied().collect();
        let [(back, facing)] = beside.as_slice() else { continue };
        // the repeater east or west of the hide, outputting into it
        for (d_off, need) in [([1, 0, 0], "east"), ([-1, 0, 0], "west")] {
            if *facing != need {
                continue;
            }
            let _ = back;
            let dpos = [hide[0] + d_off[0], hide[1], hide[2]];
            // the part's input so its output cell lands on dpos
            let input = [dpos[0] - o[0], dpos[1] - o[1], dpos[2] - o[2]];
            let shift = |q: [i32; 3]| [q[0] + input[0], q[1] + input[1], q[2] + input[2]];
            let cells: Vec<[i32; 3]> = pl.cells.iter().map(|q| shift(*q)).collect();
            if std::env::var("FARMOPT_DEBUG").is_ok() {
                let bad: Vec<String> = cells.iter().filter(|q| !free(b, **q) || on_a_line(local(**q), &xs, zx)).map(|q| format!("{:?}{}", local(*q), if !free(b, *q) { " taken" } else { " on a line" })).collect();
                eprintln!("turn {turns} D {:?}: {} part cells, bad {:?}", local(dpos), cells.len(), bad);
            }
            if !cells.iter().all(|q| free(b, *q) && !on_a_line(local(*q), &xs, zx)) || !free(b, dpos) {
                continue;
            }
            // the line behind the input: (stretch repeater), observer, U1's bed head
            for (out_dir, obs_facing, back_name) in [([1, 0, 0], "west", "west"), ([-1, 0, 0], "east", "east"), ([0, 0, -1], "south", "south")] {
                let back_step = |q: [i32; 3], k: i32| [q[0] - k * out_dir[0], q[1], q[2] - k * out_dir[2]];
                let k0 = if part.stretch.is_some() { 1 } else { 0 };
                let rep = back_step(input, 1);
                let obs = back_step(input, 1 + k0);
                let head = back_step(input, 2 + k0);
                let mut t = Builder { d: b.d.clone() };
                // U1's pod at `head` (its bed facing north: the foot south of the head)
                let before: std::collections::BTreeSet<[i32; 3]> = t.d.blocks.keys().copied().collect();
                let u1_at = [head[0], head[1], head[2]];
                crate::parts::pod(&mut t, u1_at, place, 0, false);
                let pod_cells: Vec<[i32; 3]> = t.d.blocks.keys().copied().filter(|q| !before.contains(q)).collect();
                if std::env::var("FARMOPT_DEBUG").is_ok() {
                    let clash: Vec<String> = pod_cells.iter().filter(|q| cells.contains(q) || **q == dpos || on_a_line(local(**q), &xs, zx)).map(|q| format!("{:?}", local(*q))).collect();
                    eprintln!("  line {out_dir:?}: input {:?} head {:?}, pod {} cells, clashes {:?}", local(input), local(head), pod_cells.len(), clash);
                }
                if pod_cells.iter().any(|q| cells.contains(q) || *q == dpos || on_a_line(local(*q), &xs, zx)) || pod_cells.len() < 3 {
                    continue;
                }
                // the pod overwrote nothing (its cells were free) and U1 stays in the cluster
                if pod_cells.iter().any(|q| before.contains(q)) {
                    continue;
                }
                let ux = head[0] - at[0];
                if xs.iter().any(|&x| (ux - x).abs() > 10) || (head[2] - at[2]).abs() > 10 || (head[1] - at[1]).abs() > 3 {
                    continue;
                }
                // out of the zombie's attack box (with its boat), which the judge refuses: U1's cell
                // 3 or more blocks from the zombie's on x or z
                if (ux - zx).abs() < 3 && (head[2] - at[2] + 7).abs() < 3 {
                    continue;
                }
                let mut line = vec![obs];
                if k0 == 1 {
                    line.push(rep);
                }
                if line.iter().any(|q| !free(&t, *q) && !pod_cells.contains(q) || cells.contains(q) || on_a_line(local(*q), &xs, zx)) {
                    continue;
                }
                t.set(obs[0], obs[1], obs[2], &format!("observer[facing={obs_facing},powered=false]"));
                if free(&t, [obs[0], obs[1] - 1, obs[2]]) {
                    t.set(obs[0], obs[1] - 1, obs[2], crate::parts::STONE);
                }
                if let Some(d) = part.stretch {
                    t.set(rep[0], rep[1], rep[2], &format!("repeater[delay={d},facing={back_name},locked=false,powered=false]"));
                    if free(&t, [rep[0], rep[1] - 1, rep[2]]) {
                        t.set(rep[0], rep[1] - 1, rep[2], crate::parts::STONE);
                    }
                }
                let Ok(placed) = mp::place(&mut t, &part.row, turns, input) else { continue };
                // the output repeater on dpos, pointing into the trapdoor
                let ro = mp::read_output(&mut t, &placed);
                if std::env::var("FARMOPT_DEBUG").is_ok() {
                    eprintln!("    read_output {:?} (hide {:?}), {} blocks", ro.map(local), local(hide), t.d.blocks.len());
                }
                if ro != Some(hide) {
                    continue;
                }
                if best.as_ref().is_none_or(|bb| t.d.blocks.len() < bb.d.blocks.len()) {
                    best = Some(t);
                }
            }
        }
    }
    let t = best.ok_or_else(|| anyhow!("calm part {}: no joint placement beside the hide", part.name))?;
    b.d = t.d;
    Ok(())
}

/// One calm for a row of modules on x: from each module's feed a repeater on a free face of it
/// (facing away) and a route between the zombies' row and the pods to the next module's feed,
/// which then climbs onto its hide's stone.
pub fn share(b: &mut Builder, ports: &[Ports], ats: &[[i32; 3]]) -> Result<()> {
    for i in 1..ports.len() {
        let (Some(prev), Some(next)) = (ports[i - 1].hide_feed, ports[i].hide_feed) else { bail!("a module without a hide feed cannot share the calm") };
        let (r, start) = branch(b, prev).ok_or_else(|| anyhow!("no room for the branch repeater at module {}'s feed", i - 1))?;
        let a = ats[i];
        let forbid = |q: [i32; 3]| {
            let z = q[2] - a[2];
            q[0] < prev[0] - 1 || q[0] > next[0] + 3 || z < -10 || z > -1 || near_driver(q, next) || near_driver(q, prev) || (z == -6 && q[0] < next[0] && q[0] > prev[0] + 2)
        };
        let keep = std::collections::BTreeSet::new();
        mp::route(b, start, next, &forbid, &keep, &[r, [next[0] - 1, next[1], next[2]], [next[0] - 1, next[1] + 1, next[2]]], 1).ok_or_else(|| anyhow!("no route for the shared calm from module {} to {i}", i - 1))?;
        climb(b, next);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::{Cage, CalmKind, Kill, Module, Pad, Place};

    /// The joint placement fits beside the hide for the smallest Nether timer, with fewer blocks
    /// than the routed calm.
    #[test]
    fn the_joint_calm_fits_beside_the_hide() {
        let parts = parts("middle");
        let m = Module { cage: Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west: false, regular: false }, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Blade { k: -22 } };
        let mut routed = Builder::new("r");
        let p = crate::parts::module(&mut routed, [0, 0, 0], Place::Void, &m, true);
        let n0 = routed.d.blocks.len();
        wire(&mut routed, &parts[0], &p, [0, 0, 0]).unwrap();
        let nr = routed.d.blocks.len() - n0;
        let mut joint = Builder::new("j");
        crate::parts::module(&mut joint, [0, 0, 0], Place::Void, &m, false);
        let n1 = joint.d.blocks.len();
        let r = wire_direct(&mut joint, &parts[0], &m.cage, [0, 0, 0], Place::Void);
        assert!(r.is_ok(), "{r:?}");
        let nj = joint.d.blocks.len() as i64 - n1 as i64;
        eprintln!("calm blocks: joint {nj} (U1's pod, observer, repeater, timer, output repeater) vs routed {nr} (+ U1's pod in the cage)");
        assert!(nj < nr as i64 + 13, "joint {nj} (U1's pod included) vs routed {nr} (U1's pod not)");
    }
}
