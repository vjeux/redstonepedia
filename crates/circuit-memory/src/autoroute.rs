//! A dust router for the generators (column 3 (c), day-1 lane 2026-11-11): the shortest path of
//! redstone dust between two cells through a design's free cells, under the rules
//! `sidinggen::route_d` lays by (one horizontal step at a time, a stair rises or falls one level
//! with its lower cell's top clear, no foreign dust it would join), and clear of every foreign
//! component it could power or be powered by (repeaters, comparators, torches, rails, mechanisms,
//! and the blocks they power). It finds the cells; the caller lays them (`route_d` or a link with
//! its repeaters placed for a given delay).

use crate::schedgen::{B, P3};
use anyhow::{Result, bail};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

/// The search's limits: the box the path stays in (both corners included), cells it must not use
/// (nor put a support in), and cells next to which it may run although they hold components (the
/// route's own source and sink).
pub(crate) struct Limits {
    pub lo: P3,
    pub hi: P3,
    pub avoid: BTreeSet<P3>,
    pub near_ok: BTreeSet<P3>,
}

fn free(b: &B, p: &P3) -> bool {
    b.blocks.get(p).is_none_or(|s| s.ends_with(":air"))
}

fn is_dust(s: &str) -> bool {
    s.contains("redstone_wire")
}

/// A block that takes or gives power (anything but plain stone, glass and dust).
fn component(s: &str) -> bool {
    ["repeater", "comparator", "torch", "rail", "piston", "observer", "lever", "button", "lamp", "bulb", "dropper", "dispenser", "hopper", "daylight_detector", "note_block", "target", "redstone_block", "door", "trapdoor", "fence_gate"].iter().any(|k| s.contains(k))
}

/// A block that powers the solid block it points into or sits under strongly (so that block
/// powers a dust beside it): dust only powers a block weakly, which powers no dust.
fn strong_src(s: &str) -> bool {
    ["repeater", "comparator", "torch", "observer", "lever", "button", "detector_rail", "redstone_block"].iter().any(|k| s.contains(k))
}

/// Does the block `u` at `r` power the solid block at `q` strongly (so that block powers a dust
/// beside it)? Dust only powers a block weakly, which powers no dust. A repeater or comparator
/// (its `facing` is its input side) powers the block it points into; a torch the block over it; a
/// detector rail the block under it; levers, buttons, observers and redstone blocks any neighbour
/// (kept wide).
fn strongly_powers(u: &str, r: P3, q: P3) -> bool {
    let facing = u.split(['[', ',', ']']).find_map(|t| t.strip_prefix("facing=")).unwrap_or("");
    let dir = |f: &str| -> P3 {
        match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            _ => [0, -1, 0],
        }
    };
    if u.contains("repeater") || u.contains("comparator") {
        let d = dir(facing);
        return q == [r[0] - d[0], r[1] - d[1], r[2] - d[2]];
    }
    if u.contains("torch") {
        return q == add(r, [0, 1, 0]);
    }
    if u.contains("detector_rail") {
        return q == add(r, [0, -1, 0]);
    }
    ["observer", "lever", "button", "redstone_block"].iter().any(|k| u.contains(k))
}

/// Every cell a minecart's rider passes through: the two cells over each rail (a villager in a
/// cart stands about 2 blocks tall over the rail's cell). No route may take one (a block there
/// suffocates the rider and stops the cart).
pub(crate) fn rider_space(blocks: &std::collections::BTreeMap<P3, String>) -> BTreeSet<P3> {
    let mut s = BTreeSet::new();
    for (p, b) in blocks {
        if b.contains("rail[") || b.ends_with("rail") {
            s.insert(add(*p, [0, 1, 0]));
            s.insert(add(*p, [0, 2, 0]));
        }
    }
    s
}

/// A world holding these blocks, for line-of-sight tests (a block redsim cannot parse is left out).
fn los_world(blocks: &std::collections::BTreeMap<P3, String>, lo: P3, hi: P3) -> redsim::world::World {
    let min = redsim::pos::BlockPos::new(lo[0] - 1, lo[1] - 1, lo[2] - 1);
    let size = [hi[0] - lo[0] + 3, hi[1] - lo[1] + 3, hi[2] - lo[2] + 3];
    let mut w = redsim::world::World::new(min, size);
    let parsed: Vec<(redsim::pos::BlockPos, redsim::block::Block)> = blocks.iter().filter_map(|(p, s)| redsim::block::Block::parse(s).ok().map(|b| (redsim::pos::BlockPos::new(p[0], p[1], p[2]), b))).collect();
    w.place_raw(&parsed);
    w
}

/// The cells that rider-to-zombie sight lines clear in `base` cross (the lines `sight_cuts` tests):
/// a full block in one cuts a line, so a route keeps its supports out of them (a dust in one is
/// fine: it has no collision shape).
pub(crate) fn sight_corridor(base: &std::collections::BTreeMap<P3, String>, zombies: &[[f64; 3]]) -> BTreeSet<P3> {
    use redsim::entity::Vec3;
    let (lo, hi) = base.keys().fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])]));
    let wb = los_world(base, lo, hi);
    let mut out = BTreeSet::new();
    for (r, s) in base {
        if !(s.contains("rail[") || s.ends_with("rail")) {
            continue;
        }
        for z in zombies {
            let feet = [r[0] as f64 + 0.5, r[1] as f64 + 0.25, r[2] as f64 + 0.5];
            let d2 = (feet[0] - z[0]).powi(2) + (feet[1] - z[1]).powi(2) + (feet[2] - z[2]).powi(2);
            if d2 >= 64.0 {
                continue;
            }
            for h in [1.4, 1.7, 2.0, 2.3] {
                for zh in [1.5, 1.9] {
                    let from = Vec3::new(feet[0], r[1] as f64 + h, feet[2]);
                    let to = Vec3::new(z[0], z[1] + zh, z[2]);
                    if !redsim::entity::mob::los::clear(&wb, from, to) {
                        continue;
                    }
                    let len = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
                    let steps = (len / 0.05).ceil() as usize;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let p = [from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t, from.z + (to.z - from.z) * t];
                        out.insert([p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
                    }
                }
            }
        }
    }
    out
}

/// Rider-to-zombie sight lines that `fin`'s extra blocks cut: every rider over a rail of `base`
/// (its eye 1.6 or 1.9 over the rail's cell) against every zombie of `zombies` (its feet; its eye
/// 1.7 over them) within 8 blocks, the villagers' hostile range (feet to feet), by redsim's line of
/// sight (`LivingEntity.hasLineOfSight`: collision shapes stop it, glass included) in `base` and in
/// `fin`. Returns the lines checked and, for each line clear in `base` and stopped in `fin`, the
/// rider's rail, the zombie and the stopping block. (Found 2026-11-11: a route's stair at z 6 hid
/// the station zombie from the queue lane; one member per 7 slots then reached the station calm.)
pub(crate) fn sight_cuts(base: &std::collections::BTreeMap<P3, String>, fin: &std::collections::BTreeMap<P3, String>, zombies: &[[f64; 3]]) -> (usize, Vec<(P3, [f64; 3], P3)>) {
    use redsim::entity::Vec3;
    let (lo, hi) = fin.keys().chain(base.keys()).fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])]));
    let wb = los_world(base, lo, hi);
    let wf = los_world(fin, lo, hi);
    let mut n = 0;
    let mut cuts = Vec::new();
    for (r, s) in base {
        if !(s.contains("rail[") || s.ends_with("rail")) {
            continue;
        }
        for z in zombies {
            let feet = [r[0] as f64 + 0.5, r[1] as f64 + 0.25, r[2] as f64 + 0.5];
            let d2 = (feet[0] - z[0]).powi(2) + (feet[1] - z[1]).powi(2) + (feet[2] - z[2]).powi(2);
            if d2 >= 64.0 {
                continue;
            }
            for h in [1.4, 1.7, 2.0, 2.3] {
                let mut cut = None;
                for zh in [1.5, 1.9] {
                    let from = Vec3::new(feet[0], r[1] as f64 + h, feet[2]);
                    let to = Vec3::new(z[0], z[1] + zh, z[2]);
                    n += 1;
                    if redsim::entity::mob::los::clear(&wb, from, to) {
                        if let Some(p) = redsim::entity::mob::los::first_hit(&wf, from, to) {
                            cut = Some([p.x, p.y, p.z]);
                            break;
                        }
                    }
                }
                if let Some(p) = cut {
                    cuts.push((*r, *z, p));
                    break;
                }
            }
        }
    }
    (n, cuts)
}

fn solid_support(s: &str) -> bool {
    ["minecraft:stone", "minecraft:smooth_stone", "minecraft:glass", "minecraft:cobblestone"].iter().any(|k| s == *k)
}

const H4: [P3; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const N6: [P3; 6] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]];

fn add(a: P3, d: P3) -> P3 {
    [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
}

fn inside(l: &Limits, p: &P3) -> bool {
    (0..3).all(|i| l.lo[i] <= p[i] && p[i] <= l.hi[i])
}

/// May a dust sit at `n` (its support below it, placed if free)?
fn cell_ok(b: &B, l: &Limits, n: P3, no_path: &BTreeSet<P3>) -> bool {
    !no_path.contains(&n) && why_not(b, l, n).is_none()
}

/// Why a dust may not sit at `n`, or None.
fn why_not(b: &B, l: &Limits, n: P3) -> Option<String> {
    if !inside(l, &n) {
        return Some("outside the limits".into());
    }
    if l.avoid.contains(&n) {
        return Some("avoided".into());
    }
    if !free(b, &n) {
        return Some(format!("holds {:?}", b.blocks.get(&n)));
    }
    let s = add(n, [0, -1, 0]);
    let support_ok = if free(b, &s) { inside(l, &s) && !l.avoid.contains(&s) } else { b.blocks.get(&s).is_some_and(|t| solid_support(t)) };
    if !support_ok {
        return Some(format!("support {s:?} is {:?}", b.blocks.get(&s)));
    }
    // no foreign component beside the dust or its support, nor a block beside the dust that a
    // foreign component powers strongly (it would power the dust)
    for d in N6 {
        let q = add(n, d);
        if l.near_ok.contains(&q) {
            continue;
        }
        if let Some(t) = b.blocks.get(&q) {
            if component(t) {
                return Some(format!("next to {t} at {q:?}"));
            }
            if !t.ends_with(":air") && !is_dust(t) {
                for e in N6 {
                    let r = add(q, e);
                    if r == n || l.near_ok.contains(&r) {
                        continue;
                    }
                    if let Some(u) = b.blocks.get(&r).filter(|u| strongly_powers(u, r, q)) {
                        return Some(format!("next to the block {q:?} that {u} at {r:?} powers"));
                    }
                }
            }
        }
    }
    for d in N6 {
        let q = add(s, d);
        if q == n || l.near_ok.contains(&q) {
            continue;
        }
        if let Some(t) = b.blocks.get(&q).filter(|t| strongly_powers(t, q, s)) {
            return Some(format!("its support {s:?} powered by {t} at {q:?}"));
        }
    }
    // no foreign dust it would join (route_d's rule: a diagonal is cut by a block over the lower
    // dust's side)
    for d in H4 {
        for dy in [-1, 0, 1] {
            let q = [n[0] + d[0], n[1] + dy, n[2] + d[2]];
            if l.near_ok.contains(&q) {
                continue;
            }
            if !b.blocks.get(&q).is_some_and(|t| is_dust(t)) {
                continue;
            }
            let cut = match dy {
                -1 => Some([n[0] + d[0], n[1], n[2] + d[2]]),
                1 => Some([n[0], n[1] + 1, n[2]]),
                _ => None,
            };
            if cut.is_some_and(|k| !free(b, &k)) {
                continue;
            }
            return Some(format!("would join the dust at {q:?}"));
        }
    }
    None
}

#[allow(dead_code)]
fn cell_ok_old(b: &B, l: &Limits, n: P3) -> bool {
    if !inside(l, &n) || l.avoid.contains(&n) || !free(b, &n) {
        return false;
    }
    let s = add(n, [0, -1, 0]);
    let support_ok = if free(b, &s) { inside(l, &s) && !l.avoid.contains(&s) } else { b.blocks.get(&s).is_some_and(|t| solid_support(t)) };
    if !support_ok {
        return false;
    }
    // no foreign component beside the dust or its support, nor a block beside the dust that a
    // foreign component or dust powers (it would power the dust)
    for d in N6 {
        let q = add(n, d);
        if l.near_ok.contains(&q) {
            continue;
        }
        if let Some(t) = b.blocks.get(&q) {
            if component(t) {
                return false;
            }
            if !t.ends_with(":air") && !is_dust(t) {
                for e in N6 {
                    let r = add(q, e);
                    if r == n || l.near_ok.contains(&r) {
                        continue;
                    }
                    if b.blocks.get(&r).is_some_and(|u| strong_src(u)) {
                        return false;
                    }
                }
            }
        }
    }
    for d in N6 {
        let q = add(s, d);
        if q == n || l.near_ok.contains(&q) {
            continue;
        }
        if b.blocks.get(&q).is_some_and(|t| component(t)) {
            return false;
        }
    }
    // no foreign dust it would join (route_d's rule: a diagonal is cut by a stone over the lower
    // dust's... here: any non-air over the lower cell cuts it)
    for d in H4 {
        for dy in [-1, 0, 1] {
            let q = [n[0] + d[0], n[1] + dy, n[2] + d[2]];
            if l.near_ok.contains(&q) {
                continue;
            }
            if !b.blocks.get(&q).is_some_and(|t| is_dust(t)) {
                continue;
            }
            let cut = match dy {
                -1 => Some([n[0] + d[0], n[1], n[2] + d[2]]),
                1 => Some([n[0], n[1] + 1, n[2]]),
                _ => None,
            };
            if cut.is_some_and(|k| !free(b, &k)) {
                continue;
            }
            return false;
        }
    }
    true
}

/// The shortest dust path from `start` to `end` (both included), preferring straight flat runs (a
/// repeater needs one), as a list of cells. Fails when there is none inside the limits. A path
/// that would sit a cell on its own support, cover its own stair or touch itself out of sequence
/// is searched again without the offending cell (up to 40 times).
pub(crate) fn find(b: &B, start: P3, end: P3, l: &Limits) -> Result<Vec<P3>> {
    // (the riders' space kept off every routed line by default, its dust and the supports it would
    // add: the two cells over every rail (ironspec Nov 11 15:06, W's first full night: a route's
    // support over the exit climb hurt every visiting rider); a line's own ends excepted)
    let mut avoid = l.avoid.clone();
    // (`MCTAS_NO_ROUTER_RIDER_SPACE`: the old router, for control rows only)
    if std::env::var("MCTAS_NO_ROUTER_RIDER_SPACE").is_err() {
        avoid.extend(rider_space(&b.blocks).into_iter().filter(|c| *c != start && *c != end));
    }
    let lim = Limits { lo: l.lo, hi: l.hi, avoid, near_ok: l.near_ok.clone() };
    let mut no_path: BTreeSet<P3> = BTreeSet::new();
    for _ in 0..40 {
        let path = find_once(b, start, end, &lim, &no_path)?;
        match self_conflict(&path) {
            None => return Ok(path),
            Some(cs) => {
                let Some(c) = cs.into_iter().find(|c| *c != start && *c != end) else {
                    bail!("autoroute: every path from {start:?} to {end:?} conflicts with its own ends");
                };
                no_path.insert(c);
            }
        }
    }
    bail!("autoroute: no self-consistent path from {start:?} to {end:?} after 40 tries")
}

/// The cells of the path that break it (any one of them, blocked, removes the conflict): a cell that
/// is another's support, a stair whose clear cell holds a cell or a support, a cell touching a
/// non-neighbour of the path.
fn self_conflict(path: &[P3]) -> Option<Vec<P3>> {
    let cells: BTreeSet<P3> = path.iter().copied().collect();
    let supports: BTreeSet<P3> = path.iter().map(|c| add(*c, [0, -1, 0])).collect();
    for (i, &c) in path.iter().enumerate() {
        let s = add(c, [0, -1, 0]);
        if cells.contains(&s) {
            return Some(vec![s, c]);
        }
        if i > 0 {
            let p = path[i - 1];
            if p[1] != c[1] {
                let lower = if c[1] > p[1] { p } else { c };
                let over = add(lower, [0, 1, 0]);
                if cells.contains(&over) {
                    return Some(vec![over, p, c]);
                }
                // (a stone under a later cell would fill the stair's clear cell)
                if supports.contains(&over) {
                    return Some(vec![add(over, [0, 1, 0]), p, c]);
                }
            }
        }
        for j in i + 2..path.len() {
            let d = path[j];
            let h = (c[0] - d[0]).abs() + (c[2] - d[2]).abs();
            if h == 1 && (c[1] - d[1]).abs() <= 1 {
                return Some(vec![d, c]);
            }
        }
    }
    None
}

#[allow(dead_code)]
fn self_conflict_old(path: &[P3]) -> Option<P3> {
    let cells: BTreeSet<P3> = path.iter().copied().collect();
    let supports: BTreeSet<P3> = path.iter().map(|c| add(*c, [0, -1, 0])).collect();
    for (i, &c) in path.iter().enumerate() {
        let s = add(c, [0, -1, 0]);
        if cells.contains(&s) {
            return Some(s);
        }
        if i > 0 {
            let p = path[i - 1];
            if p[1] != c[1] {
                let lower = if c[1] > p[1] { p } else { c };
                let over = add(lower, [0, 1, 0]);
                if cells.contains(&over) {
                    return Some(over);
                }
                // (a stone under a later cell would fill the stair's clear cell)
                if supports.contains(&over) {
                    return Some(add(over, [0, 1, 0]));
                }
            }
        }
        for j in i + 2..path.len() {
            let d = path[j];
            let h = (c[0] - d[0]).abs() + (c[2] - d[2]).abs();
            if h == 1 && (c[1] - d[1]).abs() <= 1 {
                return Some(d);
            }
        }
    }
    None
}

/// Opt-in for [`find`]: every path keeps a repeater spot (a flat step followed by a flat step) at
/// most 12 dust cells apart, so `sidinggen::route`'s repeaters can keep it live across long stairs
/// (sidinggen's auto-routed under-stand lines set it around their search; off, the search is as it
/// was for every other caller).
pub(crate) static LANDINGS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn find_once(b: &B, start: P3, end: P3, l: &Limits, no_path: &BTreeSet<P3>) -> Result<Vec<P3>> {
    for (p, what) in [(start, "start"), (end, "end")] {
        if let Some(why) = why_not(b, l, p) {
            bail!("autoroute: the {what} {p:?} cannot hold a dust: {why}");
        }
    }
    let landings = LANDINGS.load(std::sync::atomic::Ordering::Relaxed);
    // state: (cell, the step's horizontal direction index, or 4 at the start; with `landings`, plus
    // 5 x (2 x the dust since the last repeater spot + whether the step was flat))
    type S = (P3, usize);
    let mut dist: HashMap<S, u32> = HashMap::new();
    let mut prev: HashMap<S, S> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let s0: S = (start, 4);
    dist.insert(s0, 0);
    heap.push(Reverse((0u32, s0)));
    let mut done: Option<S> = None;
    while let Some(Reverse((c, st))) = heap.pop() {
        if dist.get(&st).is_some_and(|&d| d < c) {
            continue;
        }
        if st.0 == end {
            done = Some(st);
            break;
        }
        let (p, code) = st;
        let dir = code % 5;
        let (run, was_flat) = (code / 10, (code / 5) % 2 == 1);
        for (di, d) in H4.iter().enumerate() {
            for dy in [0, -1, 1] {
                let n = [p[0] + d[0], p[1] + dy, p[2] + d[2]];
                if !cell_ok(b, l, n, no_path) {
                    continue;
                }
                if dy != 0 {
                    let lower = if dy > 0 { p } else { n };
                    let over = add(lower, [0, 1, 0]);
                    if !free(b, &over) || l.avoid.contains(&over) {
                        continue;
                    }
                }
                // (never back onto the path's own previous cells' neighbourhood: a straight
                // reversal is excluded by the direction; a U-turn beside itself is rare in a
                // shortest path and caught by the caller's lay)
                let mut cost = c + 10;
                if dir != 4 && dir != di {
                    cost += 3;
                }
                if dy != 0 {
                    cost += 6;
                }
                let ns: S = if landings {
                    let flat = dy == 0;
                    let run2 = if flat && was_flat { 0 } else { run + 1 };
                    if run2 > 12 {
                        continue;
                    }
                    (n, di + 5 * (run2 * 2 + usize::from(flat)))
                } else {
                    (n, di)
                };
                if dist.get(&ns).is_none_or(|&d| cost < d) {
                    dist.insert(ns, cost);
                    prev.insert(ns, st);
                    heap.push(Reverse((cost, ns)));
                }
            }
        }
    }
    let Some(mut st) = done else {
        bail!("autoroute: no path from {start:?} to {end:?} inside {:?}..{:?}", l.lo, l.hi);
    };
    let mut path = vec![st.0];
    while let Some(&p) = prev.get(&st) {
        path.push(p.0);
        st = p;
    }
    path.reverse();
    Ok(path)
}
