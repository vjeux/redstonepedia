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
