//! (tickexact 2026-11-13, ironspec 19:22 (1) / 19:21 bars: the refill candidates judged in the game) drive-replay's
//! R_1 override: with `MCTAS_REPLAY_R1=<rule>` the drive's own R-line writes (the per-lane levers (1,17,1),
//! (5,17,1), (9,17,1), each feeding its lane's R_1 powered rail (x,17,1) through the delay-1 repeater at
//! (x+1,17,1)) are dropped and the override writes the same cells, same states, by command each tick, by the
//! candidate's rule: real game physics and real AI riders, only the waveform emulated. Rules:
//! - `win:A-B[,C-D...]`: the lever on from act tick rise + A to rise + B (exclusive) for each window (`win:0-10`
//!   = today's window = the CONTROL; `win:0-20` = (iii), ironfarm's `--r-pulse 10 --r-second 9,19`);
//! - `arrival:S`: (ii) plain: on from the rise until the new slot-1 cart's arrival + S, the arrival being its
//!   first row at z < 2 ("arrival by position; the string's press offset unmeasured"), and in any case off 2 gt
//!   before the lane's next trapdoor opens (ironspec 19:45 (a), G8c's source-led cut).
//! `MCTAS_REPLAY_R1_SKIP=t,...` drops the rises at those act ticks (the forced-depth variant: one release
//! missed). A lever state written at iteration t shows on R_1 from row t + 1 (the drive's own rows: on at act
//! 158 -> powered rows 159..168 with the off at act 168), so `arrival:S` writes the off at iteration A + S and R_1's
//! last powered row is A + S; R_1's powered rows are READ from the rail every row.
//! Each refill (per lane) is watched from its rise to the row before that lane's next trapdoor open (the drive's
//! own trapdoor writes) and prints, against ironspec's 19:21 bars:
//! - (a) the margin: R_1's last powered row - the new slot-1 cart's arrival row;
//! - (b′) at rest: every slope cart's z at the row before the next open, its offset from its slot (1.28 + 0.79 k),
//!   and whether every one is settled (z unchanged within 0.001 over the last 5 rows);
//! - (c′) the transient: the queue's carts (those on the slope in front of V, z < 5.64, at the rise, the launched
//!   one aside): their largest z (into V, with the rows) and whether a returning cart (a cart on the lane's slope
//!   at z >= 5.64 that was not in the queue) was there in the same rows; any queue cart leaving the slope toward
//!   the pocket (z < 1 or y < 16.9: past the pocket exit); every slope cart's rail each row (off-rail rows);
//! - diagnostics: the largest net up-slope move of any slope cart (z above its running minimum) and the cart.
//! V's edge 5.64 is ironfarm's lane-3 figure, applied to the carts' centres on every lane.

use anyhow::{Result, bail, ensure};
use mctas::vanilla::VanillaServer;
use std::collections::BTreeMap;

const DY: i32 = 80;
/// Each lane's R-line lever cell (the drive's), its R_1 rail cell, its slot-0 trapdoor cell and its cart x.
const LANES: [([i32; 3], [i32; 3], [i32; 3], f64); 3] = [
    ([1, 17, 1], [-1, 17, 1], [-1, 16, 0], -0.5),
    ([5, 17, 1], [3, 17, 1], [3, 16, 0], 3.5),
    ([9, 17, 1], [7, 17, 1], [7, 16, 0], 7.5),
];
const V_EDGE: f64 = 5.64;
const CAP: u32 = 90;
const SLOT1: f64 = 1.28;
const PITCH: f64 = 0.79;
/// (V3) the hold: slot 0's cart on its shut trapdoor (the scene's t0 slot-0 carts rest at frame y 16.19, z 0.49)
const HOLD_Y: f64 = 16.19;
const HOLD_Z: f64 = 0.49;

enum Rule {
    Win(Vec<(u32, u32)>),
    Arrival(u32),
}

#[derive(Default)]
struct Track {
    lo: f64,
    up: f64,
}

struct Refill {
    lane: usize,
    rise: u32,
    /// the row before the lane's next trapdoor open (the rests are read there)
    end: u32,
    /// the row before the next rise (V1's window: the returning cart's V entry comes late on lane 7)
    end_v: u32,
    slot1: String,
    newcart: String,
    queue: Vec<String>,
    depth0: usize,
    arrival: Option<u32>,
    powered: Vec<u32>,
    v_rows: Vec<(u32, String, f64, bool)>,
    /// (V1) rows with a queue cart (the launched one aside) at z >= 5.64, and rows with a returning cart there
    q_rows: Vec<u32>,
    r_rows: Vec<u32>,
    pocket: Vec<(u32, String)>,
    off_rail: Vec<(u32, String)>,
    tracks: BTreeMap<String, Track>,
    last_rows: Vec<Vec<(String, f64)>>,
    /// (V3) the launched cart's position over the last 5 rows before the lane's next trapdoor open
    hold_rows: Vec<[f64; 3]>,
}

/// Consecutive rows as (first, last) intervals.
fn intervals(rows: &[u32]) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = Vec::new();
    for &r in rows {
        match v.last_mut() {
            Some(last) if last.1 + 1 == r => last.1 = r,
            _ => v.push((r, r)),
        }
    }
    v
}

pub(crate) struct R1Override {
    rule: Rule,
    label: String,
    rises: Vec<u32>,
    skip: Vec<u32>,
    opens: [Vec<u32>; 3],
    on_state: [String; 3],
    on: [bool; 3],
    refills: Vec<Refill>,
    pub dropped: usize,
    pub dropped_expects: usize,
    /// the previous row's cart positions (every row from the first rise on), to tell a resting queue cart in V
    /// (depth 6) from a returning cart in transit
    prev: Vec<(String, [f64; 3])>,
    /// the last row watched (a refill whose next open falls after it is not judged on V3)
    last_row: u32,
}

fn pos(srv: &mut VanillaServer, tag: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1] - DY as f64, v[2]]) } else { None })
}

/// Every cart on lane `lane`'s slope (16.9 <= y <= 23.6, z >= 1) with its position, sorted by z.
fn slope(all: &[(String, [f64; 3])], lane: usize) -> Vec<(String, [f64; 3])> {
    let mut v: Vec<(String, [f64; 3])> = all.iter().filter(|(_, p)| (p[0] - LANES[lane].3).abs() < 0.3 && (16.9..=23.6).contains(&p[1]) && p[2] >= 1.0).cloned().collect();
    v.sort_by(|a, b| a.1[2].total_cmp(&b.1[2]));
    v
}

fn lane_name(lane: usize) -> i32 {
    (LANES[lane].3 - 0.5) as i32
}

impl R1Override {
    /// The override from the environment; the drive's rises and trapdoor opens read from its own writes.
    pub(crate) fn from_env(r_acts: &[(u32, [i32; 3], String)]) -> Result<Option<Self>> {
        let Ok(spec) = std::env::var("MCTAS_REPLAY_R1") else { return Ok(None) };
        let rule = if let Some(w) = spec.strip_prefix("win:") {
            let mut v = Vec::new();
            for p in w.split(',') {
                let (a, b) = p.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_R1=win:A-B[,C-D], got {spec}"))?;
                v.push((a.trim().parse()?, b.trim().parse()?));
            }
            Rule::Win(v)
        } else if let Some(s) = spec.strip_prefix("arrival:") {
            let s: u32 = s.trim().parse()?;
            ensure!(s >= 1, "MCTAS_REPLAY_R1=arrival:S needs S >= 1 (the off is written the iteration after the arrival row at the earliest)");
            Rule::Arrival(s)
        } else {
            bail!("MCTAS_REPLAY_R1=win:A-B[,C-D] or arrival:S, got {spec}");
        };
        let skip: Vec<u32> = std::env::var("MCTAS_REPLAY_R1_SKIP").map(|s| s.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse()).collect::<Result<_, _>>()).unwrap_or(Ok(Vec::new()))?;
        let mut on_state: [String; 3] = Default::default();
        let mut per_lane: [Vec<u32>; 3] = Default::default();
        let mut opens: [Vec<u32>; 3] = Default::default();
        for (t, cell, st) in r_acts {
            if let Some(l) = LANES.iter().position(|x| x.0 == *cell)
                && st.contains("powered=true")
            {
                per_lane[l].push(*t);
                on_state[l] = st.clone();
            }
            if let Some(l) = LANES.iter().position(|x| x.2 == *cell)
                && st.contains("open=true")
            {
                opens[l].push(*t);
            }
        }
        ensure!(per_lane.iter().all(|v| !v.is_empty()), "MCTAS_REPLAY_R1: the drive has no R-line lever writes on every lane (cells (1,17,1), (5,17,1), (9,17,1))");
        ensure!(per_lane[0] == per_lane[1] && per_lane[1] == per_lane[2], "MCTAS_REPLAY_R1: the drive's three lanes rise at different ticks: {per_lane:?}");
        ensure!(opens.iter().all(|v| !v.is_empty()), "MCTAS_REPLAY_R1: the drive opens no slot-0 trapdoor on some lane");
        for s in &skip {
            ensure!(per_lane[0].contains(s), "MCTAS_REPLAY_R1_SKIP={s}: not one of the drive's rises {:?}", per_lane[0]);
        }
        Ok(Some(Self { rule, label: spec, rises: per_lane[0].clone(), skip, opens, on_state, on: [false; 3], refills: Vec::new(), dropped: 0, dropped_expects: 0, prev: Vec::new(), last_row: 0 }))
    }

    /// Whether the drive's act at `cell` is replaced by the override.
    pub(crate) fn replaces(&self, cell: [i32; 3]) -> bool {
        LANES.iter().any(|l| l.0 == cell)
    }

    fn latest_rise(&self, t: u32) -> Option<u32> {
        self.rises.iter().copied().filter(|r| *r <= t && !self.skip.contains(r)).max()
    }

    fn want_on(&self, lane: usize, t: u32) -> bool {
        match &self.rule {
            Rule::Win(ws) => self.rises.iter().filter(|r| !self.skip.contains(r)).any(|&r| ws.iter().any(|&(a, b)| t >= r + a && t < r + b)),
            Rule::Arrival(s) => {
                let Some(r) = self.latest_rise(t) else { return false };
                let arrival = self.refills.iter().rev().find(|f| f.lane == lane && f.rise == r).and_then(|f| f.arrival);
                // (ironspec 19:45 (a): G8c's source-led cut) R_1 off 2 gt before the lane's next trapdoor opens:
                // the lever off by iteration open - 3, so R_1 reads unpowered from row open - 2
                let cap = self.opens[lane].iter().copied().find(|o| *o > r).map(|o| o.saturating_sub(3)).unwrap_or(r + CAP);
                t < cap && arrival.is_none_or(|a| t < a + s)
            }
        }
    }

    fn read_all(srv: &mut VanillaServer, carts: &[String]) -> Result<Vec<(String, [f64; 3])>> {
        let mut all = Vec::new();
        for c in carts {
            if let Some(p) = pos(srv, c)? {
                all.push((c.clone(), p));
            }
        }
        Ok(all)
    }

    /// Before iteration t's acts: open each lane's refill at a rise (its queue from the rows so far), then write
    /// every lane's lever to the rule's state.
    pub(crate) fn before_acts(&mut self, srv: &mut VanillaServer, t: u32, carts: &[String]) -> Result<()> {
        if self.rises.contains(&t) && !self.skip.contains(&t) {
            let all = Self::read_all(srv, carts)?;
            for lane in 0..3 {
                let s = slope(&all, lane);
                // the queue: the lane's slope carts at rest (moved under 0.05 since the previous row, against a
                // returning cart's 0.1+ per tick in transit; a 6th cart resting in V at depth 6 included), a cart still
                // moving being a returning one; with no previous row (the first rise), the carts in front of V
                let queue: Vec<String> = if self.prev.is_empty() {
                    s.iter().filter(|(_, p)| p[2] < V_EDGE).map(|(c, _)| c.clone()).collect()
                } else {
                    s.iter().filter(|(c, p)| self.prev.iter().find(|(d, _)| d == c).is_some_and(|(_, q)| (q[2] - p[2]).abs() < 0.05 && (q[1] - p[1]).abs() < 0.05)).map(|(c, _)| c.clone()).collect()
                };
                ensure!(queue.len() >= 2, "R_1 override t{t}: lane {} has {} queue carts at its rise (needs a slot 1 and a slot 2)", lane_name(lane), queue.len());
                let end = self.opens[lane].iter().copied().find(|o| *o > t).map(|o| o - 1).unwrap_or(t + 60);
                let end_v = self.rises.iter().copied().find(|r| *r > t && !self.skip.contains(r)).map(|r| r - 1).unwrap_or(t + 99).max(end);
                self.refills.push(Refill {
                    lane,
                    rise: t,
                    end,
                    end_v,
                    slot1: queue[0].clone(),
                    newcart: queue[1].clone(),
                    depth0: queue.len(),
                    queue,
                    arrival: None,
                    powered: Vec::new(),
                    v_rows: Vec::new(),
                    q_rows: Vec::new(),
                    r_rows: Vec::new(),
                    pocket: Vec::new(),
                    off_rail: Vec::new(),
                    tracks: BTreeMap::new(),
                    last_rows: Vec::new(),
                    hold_rows: Vec::new(),
                });
            }
        }
        for lane in 0..3 {
            let want = self.want_on(lane, t);
            if want != self.on[lane] {
                let c = LANES[lane].0;
                let st = if want { self.on_state[lane].clone() } else { "minecraft:air".to_string() };
                let out = srv.cmd(&format!("setblock {} {} {} {st}", c[0], c[1] + DY, c[2]))?;
                ensure!(out.contains("Changed the block"), "R_1 override t{t}: the lever write at ({},{},{}) failed: {}", c[0], c[1], c[2], out.trim());
                self.on[lane] = want;
            }
        }
        Ok(())
    }

    /// After iteration t's step (row t): each open refill's R_1 state, the new slot-1 cart's arrival, the queue's
    /// transient (V, pocket exit, rails), the up-slope tracker and the last rows before the next open.
    pub(crate) fn after_step(&mut self, srv: &mut VanillaServer, t: u32, carts: &[String]) -> Result<()> {
        if !self.refills.iter().any(|f| t >= f.rise && t <= f.end_v) {
            return Ok(());
        }
        let all = Self::read_all(srv, carts)?;
        for f in self.refills.iter_mut().filter(|f| t >= f.rise && t <= f.end_v) {
            let s = slope(&all, f.lane);
            // (V1) who holds V this row: a queue cart (the launched one aside) and / or a returning cart
            if s.iter().any(|(c, p)| f.queue.contains(c) && *c != f.slot1 && p[2] >= V_EDGE) {
                f.q_rows.push(t);
            }
            if s.iter().any(|(c, p)| !f.queue.contains(c) && p[2] >= V_EDGE) {
                f.r_rows.push(t);
            }
            if t > f.end {
                continue;
            }
            let rail = LANES[f.lane].1;
            if crate::strict::test(srv, &format!("execute if block {} {} {} minecraft:powered_rail[powered=true]", rail[0], rail[1] + DY, rail[2]))? {
                f.powered.push(t);
            }
            if f.arrival.is_none()
                && let Some((_, p)) = all.iter().find(|(c, _)| *c == f.newcart)
                && p[2] < 2.0
            {
                f.arrival = Some(t);
            }
            let returning = s.iter().any(|(c, p)| !f.queue.contains(c) && p[2] >= V_EDGE);
            for (c, p) in all.iter().filter(|(c, _)| f.queue.contains(c) && *c != f.slot1) {
                if p[2] >= V_EDGE {
                    f.v_rows.push((t, c.clone(), p[2], returning));
                }
                if p[2] < 1.0 || p[1] < 16.9 {
                    f.pocket.push((t, c.clone()));
                }
            }
            for (c, p) in &s {
                // (AbstractMinecart's own rail lookup: the cell at the cart's floor, or the one below it when that
                // is a rail; a cart high on a slope cell sits above its rail's cell)
                let cell = [p[0].floor() as i32, p[1].floor() as i32 + DY, p[2].floor() as i32];
                let on_rail = crate::strict::test(srv, &format!("execute if block {} {} {} #minecraft:rails", cell[0], cell[1], cell[2]))?
                    || crate::strict::test(srv, &format!("execute if block {} {} {} #minecraft:rails", cell[0], cell[1] - 1, cell[2]))?;
                if !on_rail {
                    f.off_rail.push((t, c.clone()));
                }
                let tr = f.tracks.entry(c.clone()).or_insert(Track { lo: p[2], up: 0.0 });
                tr.lo = tr.lo.min(p[2]);
                tr.up = tr.up.max(p[2] - tr.lo);
            }
            if t + 5 > f.end {
                f.last_rows.push(s.iter().map(|(c, p)| (c.clone(), p[2])).collect());
                if let Some((_, p)) = all.iter().find(|(c, _)| *c == f.slot1) {
                    f.hold_rows.push(*p);
                }
            }
        }
        self.prev = all;
        self.last_row = t;
        Ok(())
    }

    /// The per-refill rows and the summary.
    pub(crate) fn report(&self) -> Result<()> {
        println!(
            "  R_1 OVERRIDE `{}` (real physics and AI riders, the waveform by command; the drive's {} R-line writes dropped, {} of them carrying `# expect` lines){}",
            self.label,
            self.dropped,
            self.dropped_expects,
            if self.skip.is_empty() { String::new() } else { format!("; rises skipped (forced depth): {:?}", self.skip) }
        );
        let mut margins: Vec<i64> = Vec::new();
        let mut v1_least: Vec<i64> = Vec::new();
        let mut v3_bad = 0usize;
        let mut v4: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for f in &self.refills {
            let last = f.powered.iter().copied().max();
            let margin = match (last, f.arrival) {
                (Some(l), Some(a)) => {
                    margins.push(l as i64 - a as i64);
                    format!("{:+}", l as i64 - a as i64)
                }
                (_, None) => format!("NOT JUDGED: no arrival (z < 2) by row {}", f.end),
                (None, Some(_)) => "R_1 never read powered".into(),
            };
            let pw = match (f.powered.first(), last) {
                (Some(a), Some(b)) => format!("{a}..{b} ({} rows)", f.powered.len()),
                _ => "never".into(),
            };
            let rest = f.last_rows.last().cloned().unwrap_or_default();
            // (b′) the refill's queue (the rise's queue, the launched cart aside): settled = every one of them
            // unmoved within 0.001 over the last 5 rows; a returning cart still moving is printed apart
            let members: Vec<&String> = f.queue.iter().filter(|c| **c != f.slot1).collect();
            let moved = |want: &dyn Fn(&String) -> bool| -> f64 {
                let mut m = 0.0f64;
                for r in &f.last_rows {
                    for (c, z) in r.iter().filter(|(c, _)| want(c)) {
                        if let Some((_, z1)) = rest.iter().find(|(d, _)| d == c) {
                            m = m.max((z - z1).abs());
                        }
                    }
                }
                m
            };
            let q_moved = moved(&|c: &String| members.contains(&c));
            let r_moved = moved(&|c: &String| !f.queue.contains(c));
            let settled = f.last_rows.len() >= 5 && q_moved < 0.001;
            // (ironspec 19:45 (b)) slot 1 at 1.28 + 0.05 at most (V's depth-6 margin sets it)
            let slot1_ok = rest.iter().find(|(c, _)| f.queue.contains(c)).map(|(_, z)| *z <= SLOT1 + 0.05);
            let offs: Vec<String> = rest
                .iter()
                .enumerate()
                .map(|(k, (c, z))| format!("{z:.3} ({:+.3}){}", z - (SLOT1 + PITCH * k as f64), if f.queue.contains(c) { "" } else { " r" }))
                .collect();
            let vmax = f.v_rows.iter().map(|r| r.2).fold(f64::MIN, f64::max);
            let v_txt = if f.v_rows.is_empty() {
                "no queue cart past 5.64".to_string()
            } else {
                let with_ret = f.v_rows.iter().filter(|r| r.3).count();
                format!(
                    "queue cart past 5.64 in {} cart-rows (max {vmax:.3}, rows {}..{}), {with_ret} of them while a returning cart was at z >= 5.64{}",
                    f.v_rows.len(),
                    f.v_rows.first().map(|r| r.0).unwrap_or(0),
                    f.v_rows.last().map(|r| r.0).unwrap_or(0),
                    if with_ret > 0 { " (FAILS (c′))" } else { "" }
                )
            };
            let (up_c, up) = f.tracks.iter().map(|(c, t)| (c.clone(), t.up)).fold((String::new(), 0.0f64), |acc, x| if x.1 > acc.1 { x } else { acc });
            println!(
                "    lane {:>2} rise t{} (queue {}): launched {} | new slot 1 {} arrives row {} | R_1 powered rows {pw} | (a) margin {margin}",
                lane_name(f.lane),
                f.rise,
                f.depth0,
                f.slot1,
                f.newcart,
                f.arrival.map(|a| a.to_string()).unwrap_or_else(|| "-".into())
            );
            println!(
                "      (b′) at row {} (before the lane's next trapdoor open): {}; {} [{}] | (c′) {v_txt}; past the pocket exit {}; off-rail rows {} | diag: largest up-slope {up:.3}{}",
                f.end,
                if settled { format!("queue settled (max move {q_moved:.4} over the last 5 rows)") } else { format!("queue NOT SETTLED (moved {q_moved:.4} within the last 5 rows)") },
                match slot1_ok {
                    Some(true) => "slot 1 <= 1.33 HOLDS".to_string(),
                    Some(false) => "slot 1 > 1.33 FAILS".to_string(),
                    None => "slot 1 not found".to_string(),
                },
                offs.join(", "),
                if f.pocket.is_empty() { "none".to_string() } else { format!("{} cart-rows (first row {} {})", f.pocket.len(), f.pocket[0].0, f.pocket[0].1) },
                if f.off_rail.is_empty() { "none".to_string() } else { format!("{} (first row {} {})", f.off_rail.len(), f.off_rail[0].0, f.off_rail[0].1) },
                if up > 0.0 { format!(" ({up_c}{})", if f.queue.contains(&up_c) { "" } else { ", returning" }) } else { String::new() }
            );
            if r_moved >= 0.001 {
                println!("      (b′) a returning cart (r) still moving at row {}: {r_moved:.3} within the last 5 rows", f.end);
            }
            // (V1, ironspec 20:00) V's occupancy over rise .. the row before the next rise: the least gap between the
            // queue's tail leaving V (z <= 5.64) and the next returning cart reaching z >= 5.64, and the reverse
            let (qi, ri) = (intervals(&f.q_rows), intervals(&f.r_rows));
            let overlap: Vec<u32> = f.q_rows.iter().copied().filter(|r| f.r_rows.contains(r)).collect();
            let mut gaps: Vec<(i64, String)> = Vec::new();
            for &(_, qe) in &qi {
                if let Some(&(rs, _)) = ri.iter().find(|(rs, _)| *rs > qe) {
                    gaps.push((rs as i64 - (qe as i64 + 1), format!("queue tail out of V at row {} -> returning cart in at row {rs}", qe + 1)));
                }
            }
            for &(_, re) in &ri {
                if let Some(&(qs, _)) = qi.iter().find(|(qs, _)| *qs > re) {
                    gaps.push((qs as i64 - (re as i64 + 1), format!("returning cart out of V at row {} -> queue cart in at row {qs}", re + 1)));
                }
            }
            let v1 = if !overlap.is_empty() {
                v1_least.push(-1);
                format!("OVERLAP: a queue cart and a returning cart both at z >= 5.64 on rows {:?} (FAILS)", &overlap[..overlap.len().min(6)])
            } else if let Some((g, why)) = gaps.iter().min_by_key(|g| g.0) {
                v1_least.push(*g);
                format!("least gap {g} gt ({why}){}", if *g - 1 >= 10 { "" } else { " (UNDER 10 after k + j = 1: FAILS)" })
            } else if qi.is_empty() {
                format!("the queue never in V this window (returning cart in V from row {})", ri.first().map(|r| r.0.to_string()).unwrap_or_else(|| "-".into()))
            } else {
                format!("the queue in V on rows {}..{}, no returning cart in V this window", qi[0].0, qi.last().map(|x| x.1).unwrap_or(0))
            };
            // (V3) the launched cart at the lane's hold (slot 0 on its shut trapdoor) before the next open
            let v3 = match f.hold_rows.last() {
                _ if f.end > self.last_row => format!("{}: NOT JUDGED (the run ended at row {} before the lane's next open)", f.slot1, self.last_row),
                Some(h) => {
                    // (ironspec 20:12 (1)) V3 = within 0.05 of the hold's rest on every one of the last 5 rows; the
                    // per-row motion printed, not judged (AI riders creep)
                    let at_hold = f.hold_rows.len() >= 5 && f.hold_rows.iter().all(|p| (p[1] - HOLD_Y).abs() < 0.05 && (p[2] - HOLD_Z).abs() < 0.05);
                    let motion = f.hold_rows.iter().map(|p| (p[0] - h[0]).abs().max((p[1] - h[1]).abs()).max((p[2] - h[2]).abs())).fold(0.0f64, f64::max);
                    if !at_hold {
                        v3_bad += 1;
                    }
                    format!(
                        "{} at ({:.3},{:.4},{:.4}){} (its largest move over the last 5 rows {motion:.4}, printed)",
                        f.slot1,
                        h[0],
                        h[1],
                        h[2],
                        if at_hold { " = within 0.05 of the hold on all 5 rows" } else { " NOT within 0.05 of the hold on every one of the 5 rows (FAILS)" }
                    )
                }
                None => {
                    v3_bad += 1;
                    format!("{} not found before the open (FAILS)", f.slot1)
                }
            };
            println!("      (V1) {v1} | (V3) {v3}");
            if let Some(rest) = f.last_rows.last() {
                // (V4) the resting carts only: the queue (the launched one aside) and any returning cart that has
                // come to rest (unmoved within 0.01 over the last 5 rows); a cart still sliding in is left out
                let resting: Vec<f64> = rest
                    .iter()
                    .filter(|(c, z)| {
                        *c != f.slot1 && (f.queue.contains(c) || f.last_rows.iter().all(|r| r.iter().find(|(d, _)| d == c).is_some_and(|(_, w)| (w - z).abs() < 0.01)))
                    })
                    .map(|(_, z)| *z)
                    .collect();
                for (k, z) in resting.iter().enumerate() {
                    v4.entry(k + 1).or_default().push(*z);
                }
            }
        }
        // (V4) the queue's rest per slot over every refill, at the row before each lane's next trapdoor open
        for (k, zs) in v4.iter_mut() {
            zs.sort_by(f64::total_cmp);
            println!("    (V4) slot {k}: n {} rest z min {:.3} median {:.3} max {:.3}", zs.len(), zs[0], zs[zs.len() / 2], zs[zs.len() - 1]);
        }
        if !v1_least.is_empty() {
            println!(
                "    (V1) least gap over {} refills with both a queue cart and a returning cart in V: {} gt; refills under 10 after k + j = 1 (or overlapping): {}",
                v1_least.len(),
                v1_least.iter().min().unwrap_or(&0),
                v1_least.iter().filter(|g| **g - 1 < 10).count()
            );
        }
        println!("    (V3) refills whose launched cart did not come to rest at the hold before the next open: {v3_bad} of {}", self.refills.len());
        if !margins.is_empty() {
            println!("    (a) margins (last powered row - arrival row): min {:+}, max {:+} over {} refills", margins.iter().min().unwrap_or(&0), margins.iter().max().unwrap_or(&0), margins.len());
        }
        ensure!(!self.refills.is_empty(), "R_1 override: no refill opened (no rise in the run)");
        // (G1 on AF) every refill's R_1 rows as READ must be the rule's own waveform: a rule that wrote the wrong
        // waveform fails here. MCTAS_REPLAY_R1_PLANT=1 extends each expected waveform by one row (must fail).
        let plant = std::env::var_os("MCTAS_REPLAY_R1_PLANT").is_some();
        let mut checked = 0usize;
        for f in &self.refills {
            let mut want: Vec<u32> = match &self.rule {
                Rule::Win(ws) => (f.rise..=f.end).filter(|row| ws.iter().any(|&(a, b)| *row >= f.rise + a + 1 && *row <= f.rise + b)).collect(),
                Rule::Arrival(s) => {
                    let cap = self.opens[f.lane].iter().copied().find(|o| *o > f.rise).map(|o| o.saturating_sub(3)).unwrap_or(f.rise + CAP);
                    let off = f.arrival.map(|a| (a + s).min(cap)).unwrap_or(cap);
                    (f.rise + 1..=off.min(f.end)).collect()
                }
            };
            if plant && let Some(&last) = want.last() {
                want.push(last + 1);
            }
            ensure!(
                f.powered == want,
                "R_1 override: lane {} rise t{}: R_1 read powered on rows {:?}, the rule `{}` writes {:?}{}",
                lane_name(f.lane),
                f.rise,
                f.powered,
                self.label,
                want,
                if plant { " (PLANTED: one row added)" } else { "" }
            );
            checked += 1;
        }
        println!("    the rule's waveform: R_1's rows as read equal the rule's own rows on {checked} of {} refills", self.refills.len());
        Ok(())
    }
}
