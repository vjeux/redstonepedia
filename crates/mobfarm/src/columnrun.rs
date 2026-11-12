//! `mctas mobfarm column-run` (onechunk, Nov 11; farmopt 19:14, ironspec 19:16): a gravity column
//! run from its own file. The **frame** is design-agnostic: the file built as placed (its rails,
//! carts, riders, posts), the run cells named in its header, the summon frame (T at every world
//! time multiple of 100), and the measures: every new cart-cart contact with both stored speeds the
//! tick before, every gate shut onto a cart, each cart's passes over the named detector rails (its
//! lap), and at each T every villager inside a stand summoner's box grown by 10. The **controller**
//! is per design (`run-controller: v4`): every signal it drives is EMULATED (a gate block set open
//! or shut, a powered rail's source block set to a redstone block or back) and is labelled so in the
//! report; the detector rails it reads are the real ones.
//!
//! Header lines (written by `mobfarm column --run-gate/--run-cell/--run-controller`, in the file's
//! own frame):
//! - `run-gates: NAME x,y,z[/sx,sy,sz]; ...` a fence gate, and the block that powers the powered
//!   rail under it (set to a redstone block while the controller powers it);
//! - `run-cells: NAME x,y,z; ...` detector rails (read: their real powered state) and rest cells
//!   (read: a cart's box over the cell);
//! - `run-controller: NAME`.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

fn p3(s: &str) -> Result<P, String> {
    let n: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{s}: {e}"))).collect::<Result<_, _>>()?;
    if n.len() != 3 {
        return Err(format!("{s}: want x,y,z"));
    }
    Ok([n[0], n[1], n[2]])
}

/// The run cells of a file's header.
#[derive(Clone, Debug, Default)]
pub struct Cells {
    pub gates: BTreeMap<String, (P, Option<P>)>,
    pub cells: BTreeMap<String, P>,
    pub controller: String,
}

pub fn cells_of(meta: &BTreeMap<String, String>) -> Result<Cells, String> {
    let mut c = Cells::default();
    for item in meta.get("run-gates").map(String::as_str).unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, rest) = item.split_once(' ').ok_or_else(|| format!("run-gates: {item}: want NAME x,y,z[/sx,sy,sz]"))?;
        let (g, src) = match rest.trim().split_once('/') {
            Some((g, s)) => (p3(g)?, Some(p3(s)?)),
            None => (p3(rest.trim())?, None),
        };
        c.gates.insert(name.to_string(), (g, src));
    }
    for item in meta.get("run-cells").map(String::as_str).unwrap_or("").split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, rest) = item.split_once(' ').ok_or_else(|| format!("run-cells: {item}: want NAME x,y,z"))?;
        c.cells.insert(name.to_string(), p3(rest.trim())?);
    }
    c.controller = meta.get("run-controller").cloned().unwrap_or_default().trim().to_string();
    if c.controller.is_empty() {
        return Err("no `run-controller:` line (the design's controller: v4)".into());
    }
    Ok(c)
}

/// One cart as the controller and the measures see it.
#[derive(Clone, Debug)]
pub struct Cart {
    pub id: u32,
    pub pos: [f64; 3],
    pub v: [f64; 3],
    pub bb: [f64; 6],
    pub rider: Option<u32>,
}

fn cell_box(c: P) -> [f64; 6] {
    [c[0] as f64, c[1] as f64, c[2] as f64, c[0] as f64 + 1.0, c[1] as f64 + 1.0, c[2] as f64 + 1.0]
}

fn meets(a: &[f64; 6], b: &[f64; 6], tol: f64) -> bool {
    a[0] <= b[3] + tol && b[0] <= a[3] + tol && a[1] <= b[4] + tol && b[1] <= a[4] + tol && a[2] <= b[5] + tol && b[2] <= a[5] + tol
}

/// What the controller reads each tick: the world time, each run cell's detector state (real) and
/// its rising edge, and which rest cells hold a cart.
pub struct View<'a> {
    pub t: i64,
    pub on: &'a BTreeMap<String, bool>,
    pub rise: &'a BTreeSet<String>,
    pub carts: &'a [Cart],
    pub cells: &'a Cells,
}

impl View<'_> {
    pub fn is_on(&self, n: &str) -> bool {
        self.on.get(n).copied().unwrap_or(false)
    }
    pub fn rose(&self, n: &str) -> bool {
        self.rise.contains(n)
    }
    /// A cart whose box meets the cell `n` (a rest cell's cart).
    pub fn cart_at(&self, n: &str) -> bool {
        self.cells.cells.get(n).is_some_and(|c| {
            let b = cell_box(*c);
            let inner = [b[0] + 0.2, b[1], b[2] + 0.2, b[3] - 0.2, b[4] + 0.5, b[5] - 0.2];
            self.carts.iter().any(|k| meets(&k.bb, &inner, 0.0))
        })
    }
    /// The carts whose boxes meet the cell `n` at all.
    pub fn carts_near(&self, n: &str) -> Vec<u32> {
        self.cells.cells.get(n).map_or(Vec::new(), |c| {
            let b = cell_box(*c);
            let inner = [b[0] + 0.05, b[1] - 0.5, b[2] + 0.05, b[3] - 0.05, b[4] + 0.5, b[5] - 0.05];
            self.carts.iter().filter(|k| meets(&k.bb, &inner, 0.0)).map(|k| k.id).collect()
        })
    }
    /// The carts whose boxes meet the gate `g`'s cell.
    pub fn carts_on_gate(&self, g: &str) -> Vec<u32> {
        self.cells.gates.get(g).map_or(Vec::new(), |(c, _)| {
            let b = cell_box(*c);
            self.carts.iter().filter(|k| meets(&k.bb, &b, -1e-6)).map(|k| k.id).collect()
        })
    }
    /// A cart whose box meets the cell `n` at all (a pocket's zone).
    pub fn cart_near(&self, n: &str) -> bool {
        self.cells.cells.get(n).is_some_and(|c| {
            let b = cell_box(*c);
            let inner = [b[0] + 0.05, b[1] - 0.5, b[2] + 0.05, b[3] - 0.05, b[4] + 0.5, b[5] - 0.05];
            self.carts.iter().any(|k| meets(&k.bb, &inner, 0.0))
        })
    }
    /// A cart at rest (horizontal speed under 0.02) over the rest cell `n`.
    pub fn cart_rest(&self, n: &str) -> bool {
        self.cells.cells.get(n).is_some_and(|c| {
            let b = cell_box(*c);
            let inner = [b[0] + 0.05, b[1] - 0.5, b[2] + 0.05, b[3] - 0.05, b[4] + 0.5, b[5] - 0.05];
            self.carts.iter().any(|k| meets(&k.bb, &inner, 0.0) && (k.v[0].powi(2) + k.v[2].powi(2)).sqrt() < 0.02)
        })
    }
    /// A cart whose box meets the gate `g`'s cell.
    pub fn cart_on_gate(&self, g: &str) -> bool {
        self.cells.gates.get(g).is_some_and(|(c, _)| {
            let b = cell_box(*c);
            self.carts.iter().any(|k| meets(&k.bb, &b, -1e-6))
        })
    }
}

/// A controller's wanted state of one gate: open, and its rail's source powered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Want {
    pub open: bool,
    pub power: bool,
}

pub trait Controller {
    /// The wanted state of every gate it drives, after reading the tick's view; `log` gets its
    /// control events (each an emulated signal).
    fn step(&mut self, v: &View, log: &mut Vec<String>) -> BTreeMap<String, Want>;
    /// The stand's summoner rest cells (the batch at T).
    fn summoner_cells(&self) -> Vec<String>;
    /// Where the carts are, by part (a line at each T).
    fn snapshot(&self, v: &View) -> String;
}

/// v4's controller (C′, the inverted gravity column): the hold's front-first fill and its release
/// at T+3 (shut once Ds counts the latched carts, cap 56), ironfarm's escapement (G0 12 gt on the
/// escapement sensor's edge; G_P 11 gt on the edge of staged AND fills < 3 AND ≥ 10 gt past the
/// release AND no hold-sensor edge for 6 gt), the stand's arrival fill (gate k lets k carts by,
/// shutting on its k-th cart's centre `close_at` past the gate's cell) and its departure at T+5
/// (S0 and S1 open, S2 2 gt later; S1's and S2's slopes powered `slope_delay` after their gates),
/// the receiver's catch (T1 and T2 open, T3 shut; on P3's edge T1 shuts at +1 and T2 at +2) and the
/// tail's pocket chain (a gate opens while its pocket holds a cart and the next is free, and shuts
/// on the next pocket's detector edge), the intake (T5 open while pocket 5 holds a cart and the
/// run's top detector is off, shut when it rises). Every drive is emulated.
pub struct V4 {
    rel: Option<(i64, usize, usize, Option<i64>)>,
    rel_pockets: [bool; 3],
    rel_b_open: Option<i64>,
    rel_rear: Option<i64>,
    latched: Vec<u32>,
    last_release: i64,
    last_hold_edge: i64,
    staged: bool,
    staged_at: i64,
    g0_until: i64,
    gp_until: i64,
    fills: usize,
    cond_was: bool,
    until: [i64; 6],
    last_feed: i64,
    held_at: i64,
    feeding: bool,
    adv_at: Option<i64>,
    adv: [bool; 3],
    left: [bool; 3],
    pulse: i64,
    ready: bool,
    t1_shut_at: i64,
    t2_shut_at: i64,
    stand: Stand,
    passed: [usize; 3],
    close_at: f64,
    slope_delay: i64,
    g2_plus: i64,
    pub hold_shut_delay: i64,
    want: BTreeMap<String, Want>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stand {
    Hold,
    Depart(i64),
    Arrive,
}

impl V4 {
    pub fn new(v: &View, close_at: f64, slope_delay: i64, g2_plus: i64, hold_shut_delay: i64, pulse: i64) -> V4 {
        let fills = ["Ra", "Rb", "Rc"].iter().filter(|n| v.cart_near(n)).count();
        let mut want = BTreeMap::new();
        for g in v.cells.gates.keys() {
            want.insert(g.clone(), Want::default());
        }
        V4 {
            rel: None,
            rel_pockets: [false; 3],
            rel_b_open: None,
            rel_rear: None,
            latched: Vec::new(),
            last_release: i64::MIN / 4,
            last_hold_edge: i64::MIN / 4,
            staged: v.cart_near("Pesc"),
            staged_at: i64::MIN / 4,
            g0_until: i64::MIN / 4,
            gp_until: i64::MIN / 4,
            fills,
            cond_was: false,
            ready: !(1..=3).any(|k| v.cart_near(&format!("R{k}"))),
            until: [i64::MIN / 4; 6],
            last_feed: i64::MIN / 4,
            held_at: i64::MIN / 4,
            feeding: false,
            adv_at: None,
            adv: [false; 3],
            left: [false; 3],
            pulse,
            t1_shut_at: i64::MIN / 4,
            t2_shut_at: i64::MIN / 4,
            stand: Stand::Hold,
            passed: [0; 3],
            close_at,
            slope_delay,
            g2_plus,
            hold_shut_delay,
            want,
        }
    }

    fn set(&mut self, g: &str, open: bool, power: bool) {
        self.want.insert(g.to_string(), Want { open, power });
    }
}

impl Controller for V4 {
    fn summoner_cells(&self) -> Vec<String> {
        vec!["S0r".into(), "S1r".into(), "S2r".into()]
    }

    fn snapshot(&self, v: &View) -> String {
        let c = |n: &str| v.carts_near(n).len();
        let stand = c("S0r") + c("S1r") + c("S2r");
        let hold = ["Ra", "Rb", "Rc"].iter().map(|n| c(n)).sum::<usize>();
        let tail: Vec<String> = (1..=5).map(|k| format!("{}", c(&format!("R{k}")))).collect();
        let (top, low) = (v.cells.cells.get("RUNTOP").copied(), v.cells.cells.get("Pesc").copied());
        let run = match (top, low) {
            (Some(tp), Some(lp)) => v.carts.iter().filter(|k| k.pos[0].floor() as i32 == tp[0] && k.pos[2] >= lp[2] as f64 && k.pos[2] < tp[2] as f64 + 1.0 && k.pos[1] > lp[1] as f64 - 0.5 && k.pos[1] < tp[1] as f64 + 1.5).count(),
            _ => 0,
        };
        format!("stand {stand}, hold {hold}, tail pockets 1..5 [{}], run {run} (top detector {}), elsewhere {}", tail.join(" "), if v.is_on("RUNTOP") { "on" } else { "off" }, v.carts.len() - stand - hold - run - (1..=5).map(|k| c(&format!("R{k}"))).sum::<usize>())
    }

    fn step(&mut self, v: &View, log: &mut Vec<String>) -> BTreeMap<String, Want> {
        let t = v.t;
        let phase = t.rem_euclid(100);
        // ---- the hold: its pockets' occupancy read from the carts (EMULATED: a cart at rest over a
        // rest cell, standing in for the latches on Pa..Pc), the release at T+3
        let near = |n: &str| v.cart_near(n);
        let (near_a, near_b) = (near("Pa") || near("Ra"), near("Pb") || near("Rb"));
        for n in ["Pa", "Pb", "Pc"] {
            if v.rose(n) {
                self.last_hold_edge = t;
            }
        }
        if v.rose("Ds")
            && let Some((_, _, ds, _)) = self.rel.as_mut()
        {
            *ds += 1;
            if self.rel_b_open.is_none() {
                // (cart 0's rising edge at Ds: Gb and Gc open, G_b, G_c and R*_b, R*_c powered,
                // same-tick latch)
                self.rel_b_open = Some(t);
                log.push(format!("t {t}: hold: cart 0's Ds edge: Gb, Gc open, G_b, G_c (and R*_b, R*_c along the line) powered (emulated)"));
            }
        }
        let rests = [v.cart_rest("Ra"), v.cart_rest("Rb"), v.cart_rest("Rc")];
        if phase == 3 && self.rel.is_none() && rests.iter().any(|o| *o) {
            let n = rests.iter().filter(|o| **o).count();
            self.rel = Some((t, n, 0, None));
            self.rel_pockets = rests;
            self.rel_b_open = None;
            self.rel_rear = None;
            self.latched = ["Ra", "Rb", "Rc"].iter().flat_map(|c| v.carts_near(c)).collect();
            self.last_release = t;
            self.fills = 0;
            log.push(format!("t {t}: hold release (T+3), {n} at rest: Ga open, G_a powered (emulated); Gb, Gc on cart 0's Ds edge (9fc3155f's R* stagger)"));
        }
        if let Some((at, n, ds, _)) = self.rel {
            // (the rear cart's own evidence: its P_b rise when pocket c was latched, its P_a rise
            // when b was the rear, cart 0's Ds edge when a was alone; Ga shuts 21 gt after it,
            // the rear having cleared Ga; the cap 90)
            if self.rel_rear.is_none()
                && let Some(b) = self.rel_b_open
            {
                let ev = if self.rel_pockets[2] {
                    v.rose("Pb") && t > b
                } else if self.rel_pockets[1] {
                    v.rose("Pa") && t > b
                } else {
                    true
                };
                if ev {
                    self.rel_rear = Some(t);
                }
            }
            let shut = self.rel_rear.is_some_and(|r| t >= r + 21 + self.hold_shut_delay) || t >= at + 90;
            if shut {
                self.rel = None;
                log.push(format!("t {t}: hold release ends ({}; Ds rose {ds} times for {n} carts): Ga shut (emulated)", if self.rel_rear.is_some() { "21 gt after the rear's evidence" } else { "the cap 90" }));
            }
        }
        match self.rel {
            Some(_) => {
                // (9fc3155f, Nov 11 20:59: Gc, with G_c and R*_c's power, closes on the rear
                // cart's own P_b rise, zero delay; Gb stays with Ga's close)
                let bc = self.rel_b_open.is_some();
                let c_open = bc && !(self.rel_pockets[2] && self.rel_rear.is_some());
                self.set("Ga", true, true);
                self.set("Gb", bc, bc);
                self.set("Gc", c_open, c_open);
            }
            None => {
                self.set("Ga", false, false);
                let (b, c) = (!near_a, !near_b);
                self.set("Gb", b, b);
                self.set("Gc", c, c);
            }
        }
        // ---- the escapement (ironfarm's): G0 on the escapement sensor's edge, G_P on staged
        if v.rose("ESC") {
            self.g0_until = t + 12;
            self.staged_at = t + 14;
            log.push(format!("t {t}: escapement sensor edge: G0 open 12 gt (emulated)"));
        }
        if t == self.staged_at {
            self.staged = true;
        }
        // (ironfarm's admission keyed on the release finished, ironspec Nov 11 20:58: Ga, Gb and Gc
        // shut after the rear cart's evidence; until ironfarm's own law lands)
        let cond = self.staged && self.fills < 3 && self.rel.is_none() && t >= self.last_release + 10 && t >= self.last_hold_edge + 6;
        if cond && !self.cond_was {
            self.gp_until = t + 11;
            self.staged = false;
            self.fills += 1;
            log.push(format!("t {t}: G_P open 11 gt (fill {} since the release, emulated)", self.fills));
        }
        self.cond_was = cond;
        let (g0, gp) = (t < self.g0_until, t < self.gp_until);
        self.set("G0e", g0, false);
        self.set("GP", gp, false);
        // ---- the tail: its pockets' occupancy read from the carts (EMULATED), the receiver's
        // catch keyed on P3's real edge (7b90c3f1's: T1 shuts at +1, T2 at +2), each later move a
        // pulse of `pulse` gt while its pocket holds a cart at rest and the next is empty, the feed
        // into pocket 4 at least 24 gt apart, the intake while the run's top detector is off
        let nearp = |k: usize| v.cart_near(&format!("P{k}")) || v.cart_near(&format!("R{k}"));
        // (a pocket holds a cart when one is over its rest cell, at rest or still settling: a
        // cart against a shut gate on a slope keeps a small stored speed)
        let rest = |k: usize| v.cart_near(&format!("R{k}"));
        if self.ready && v.rose("P3") {
            // (7b90c3f1's key, gq-bench a3d13d512e: the first P3 rise while idle is the batch's
            // cart 0: T1 shuts at the edge + 0, T2 at + 1; held 4 gt after the later shut)
            self.ready = false;
            self.t1_shut_at = t;
            self.t2_shut_at = t + 1;
            self.held_at = t + 5;
            self.feeding = false;
            self.adv_at = None;
            self.adv = [false; 3];
            self.left = [false; 3];
            log.push(format!("t {t}: receiver key: P3 edge, T1 shut, T2 at +1 (emulated)"));
        }
        let pulse = self.pulse;
        if v.rose("P1") {
            let occ: Vec<String> = (1..=5usize).map(|k| format!("{k}:{}", if rest(k) { "rest" } else if nearp(k) { "moving" } else { "-" })).collect();
            log.push(format!("t {t}: P1 edge (an arrival): pockets {} receiver {}", occ.join(" "), if self.ready { "idle" } else { "busy" }));
        }
        if !self.ready {
            // (the feed: T3 opens when held, no advance running, >= 24 gt since the last feed
            // start, a cart at G3's face; X's rise shuts it and starts the advance 4 gt later:
            // T2 and T1 open together, T2 shut on the next P3 rise, T1 on the next P2 rise)
            if !self.feeding && t >= self.held_at && self.adv_at.is_none() && !self.adv[1] && !self.adv[2] && t >= self.last_feed + 24 && rest(3) {
                self.feeding = true;
                self.last_feed = t;
            }
            if self.feeding && v.rose("X") {
                self.feeding = false;
                self.adv_at = Some(t + 4);
            }
            if self.adv_at == Some(t) {
                self.adv_at = None;
                self.adv[2] = nearp(2);
                self.adv[1] = nearp(1);
            }
            if self.adv[2] && v.rose("P3") {
                self.adv[2] = false;
                self.left[2] = true;
            }
            if self.adv[1] && v.rose("P2") {
                self.adv[1] = false;
                self.left[1] = true;
            }
            let quiet = !self.feeding && self.adv_at.is_none() && !self.adv[1] && !self.adv[2];
            if quiet && t > self.held_at && (1..=3).all(|k| !nearp(k)) && ["T1", "T2", "T3"].iter().all(|g| !v.cart_on_gate(g)) {
                self.ready = true;
                log.push(format!("t {t}: receiver idle (emulated)"));
            }
        }
        if self.ready {
            self.set("T1", true, true);
            self.set("T2", true, true);
            self.set("T3", false, false);
        } else {
            // (early reopen: T1 once its cart has left and its pocket is empty, T2 once its cart
            // has left and pockets 1 and 2 are empty)
            let t1 = t < self.t1_shut_at || self.adv[1] || (self.left[1] && !nearp(1));
            let t2 = t < self.t2_shut_at || self.adv[2] || (self.left[2] && !nearp(1) && !nearp(2));
            let t3 = self.feeding;
            self.set("T1", t1, t1);
            self.set("T2", t2, t2);
            self.set("T3", t3, t3);
        }
        if self.until[4] <= t && rest(4) && !nearp(5) {
            self.until[4] = t + pulse;
        }
        let t4 = t < self.until[4];
        self.set("T4", t4, t4);
        if self.until[5] <= t && rest(5) && !v.is_on("RUNTOP") {
            self.until[5] = t + 20;
        }
        if v.rose("RUNTOP") {
            self.until[5] = self.until[5].min(t);
        }
        // (the intake's G5 is a plain slope: a cart from rest takes ~18 gt to clear the gate's bar,
        // so the gate stays open while a cart is under it)
        let t5 = t < self.until[5] || (self.until[5] > i64::MIN / 4 && t < self.until[5] + 40 && v.cart_on_gate("T5"));
        self.set("T5", t5, false);
        // ---- the stand: departure at T+5, the arrival fill after it
        if phase == 5 && self.stand == Stand::Hold && ["S0r", "S1r", "S2r"].iter().any(|n| v.cart_at(n)) {
            self.stand = Stand::Depart(t);
            log.push(format!("t {t}: stand departure (T+5): S0, S1 open, S2 at +{}; S1's, S2's slopes powered {} gt after their gates (emulated)", self.g2_plus, self.slope_delay));
        }
        match self.stand {
            Stand::Hold => {
                self.set("S0", false, false);
                self.set("S1", false, false);
                self.set("S2", false, false);
            }
            Stand::Depart(at) => {
                self.set("S0", true, true);
                self.set("S1", true, t >= at + self.slope_delay);
                self.set("S2", t >= at + self.g2_plus, t >= at + self.g2_plus + self.slope_delay);
                let strip = v.cells.cells.get("STRIP0").copied();
                let strip1 = v.cells.cells.get("STRIP1").copied();
                let left = match (strip, strip1) {
                    (Some(a), Some(b)) => !v.carts.iter().any(|k| {
                        let c = [k.pos[0].floor() as i32, k.pos[1].floor() as i32, k.pos[2].floor() as i32];
                        (a[0]..=b[0]).contains(&c[0]) && (a[1]..=b[1]).contains(&c[1]) && (a[2]..=b[2]).contains(&c[2])
                    }),
                    _ => t >= at + 40,
                };
                if left && t >= at + 20 {
                    self.stand = Stand::Arrive;
                    self.passed = [0; 3];
                    log.push(format!("t {t}: stand clear: S0 shut, S1 and S2 open for the arrivals, their slopes powered (emulated)"));
                }
            }
            Stand::Arrive => {
                // (gate k from the front lets k carts by: S1 one, S2 two)
                for (k, g) in [(1usize, "S1"), (2usize, "S2")] {
                    let gx = v.cells.gates[g].0[0] as f64;
                    let edge = gx + self.close_at;
                    let n = v.carts.iter().filter(|c| c.pos[0] > edge && (c.pos[2] - (v.cells.gates[g].0[2] as f64 + 0.5)).abs() < 0.5 && (c.pos[1] - v.cells.gates[g].0[1] as f64).abs() < 3.0).count();
                    if n > self.passed[k] {
                        self.passed[k] = n;
                    }
                }
                let s1 = self.passed[1] < 1;
                let s2 = self.passed[2] < 2;
                self.set("S0", false, false);
                self.set("S1", s1, s1);
                self.set("S2", s2, s2);
                if !s1 && !s2 && v.cart_at("S0r") && v.cart_at("S1r") && v.cart_at("S2r") {
                    self.stand = Stand::Hold;
                    log.push(format!("t {t}: stand full: 3 at rest"));
                }
            }
        }
        self.want.clone()
    }
}

impl V4 {}

/// The golem chance per summon (ironspec Nov 11 20:27, vjeux waiting): for each villager riding a
/// cart over one of the `stand` cells (the batch at its rest positions, the file's carts and riders
/// placed and settled 20 gt), the game's summon search from its block position
/// (`golem_spawn_cells_from`: 289 columns dx, dz in -8..8, each scanned down from y+6 over the floors
/// y+5..y-7, the first floor passing LEGACY_IRON_GOLEM's canSpawnOn taken, then
/// `checkSpawnObstruction` there with the entities in place): n_i the columns landing on the pad
/// (`pad`, a box of spawn cells) that pass; off-pad passes (must be 0); off-pad landings that fail
/// (wasted attempts); pad landings that fail; columns with no floor at all. P = 1 - prod (1 -
/// n_i/289)^10 (each villager's 10 tries), and `per_day` x P.
pub fn summon_chance(spec: &FarmSpec, pad: (P, P), stand: &[P], per_day: f64) -> Result<String, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 400, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    for _ in 0..20 {
        run.tick();
    }
    let w = &run.world;
    let in_pad = |c: &BlockPos| c.x >= pad.0[0] && c.x <= pad.1[0] && c.y >= pad.0[1] && c.y <= pad.1[1] && c.z >= pad.0[2] && c.z <= pad.1[2];
    let mut s = format!("summon chance: pad spawn cells x {}..{} y {}..{} z {}..{} (file frame); each stand villager's 289 columns (the game's LEGACY_IRON_GOLEM scan from its block position, checkSpawnObstruction with the file's carts and riders in place, settled 20 gt)\n", pad.0[0], pad.1[0], pad.0[1], pad.1[1], pad.0[2], pad.1[2]);
    let mut miss = 1.0f64;
    let mut found = 0usize;
    let mut pad_cells: BTreeSet<[i32; 3]> = BTreeSet::new();
    for st in stand {
        let b = cell_box(*st);
        let inner = [b[0] + 0.05, b[1] - 0.5, b[2] + 0.05, b[3] - 0.05, b[4] + 0.5, b[5] - 0.05];
        let cart = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && meets(&[e.bb.min_x, e.bb.min_y, e.bb.min_z, e.bb.max_x, e.bb.max_y, e.bb.max_z], &inner, 0.0));
        let Some(cart) = cart else {
            s.push_str(&format!("  stand ({},{},{}): no cart\n", st[0], st[1], st[2]));
            continue;
        };
        let Some(vid) = cart.passengers.first().copied() else {
            s.push_str(&format!("  stand ({},{},{}): an empty cart\n", st[0], st[1], st[2]));
            continue;
        };
        let Some(vi) = redsim::entity::index_of(w, vid) else { continue };
        let v = &w.entities.list[vi];
        let bp = v.block_position();
        found += 1;
        let cols = redsim::entity::mob::spawn::golem_spawn_cells_from(w, bp);
        let (mut n, mut off_ok, mut off_fail, mut pad_fail, mut none) = (0usize, 0usize, 0usize, 0usize, 0usize);
        let mut pad_fail_why: BTreeMap<&str, usize> = BTreeMap::new();
        let mut off_ok_list: Vec<String> = Vec::new();
        for (cell, ok) in &cols {
            let landed = !matches!(ok, Err(e) if e.starts_with("no cell"));
            match (landed, in_pad(cell), ok.is_ok()) {
                (false, _, _) => none += 1,
                (true, true, true) => {
                    n += 1;
                    pad_cells.insert([cell.x, cell.y, cell.z]);
                }
                (true, true, false) => {
                    pad_fail += 1;
                    if let Err(e) = ok {
                        *pad_fail_why.entry(e).or_default() += 1;
                    }
                }
                (true, false, true) => {
                    off_ok += 1;
                    off_ok_list.push(format!("({},{},{})", cell.x, cell.y, cell.z));
                }
                (true, false, false) => off_fail += 1,
            }
        }
        miss *= (1.0 - n as f64 / 289.0).powi(10);
        s.push_str(&format!(
            "  villager over ({},{},{}) at ({:.2},{:.2},{:.2}), block position ({},{},{}): n = {n} of 289 columns land on the pad and pass ({:.1}%); off the pad and pass {off_ok}{}; off the pad and fail (wasted) {off_fail}; on the pad and fail {pad_fail}{}; no floor in the column {none}; its 10 tries summon with {:.1}%\n",
            st[0], st[1], st[2], v.pos.x, v.pos.y, v.pos.z, bp.x, bp.y, bp.z,
            100.0 * n as f64 / 289.0,
            if off_ok_list.is_empty() { String::new() } else { format!(" (FAIL: {})", off_ok_list.iter().take(8).cloned().collect::<Vec<_>>().join(" ")) },
            if pad_fail_why.is_empty() { String::new() } else { format!(" ({})", pad_fail_why.iter().map(|(k, v)| format!("{v}: {k}")).collect::<Vec<_>>().join("; ")) },
            100.0 * (1.0 - (1.0 - n as f64 / 289.0).powi(10)),
        ));
    }
    let p = 1.0 - miss;
    s.push_str(&format!("P (a summon at T succeeds: 1 - prod (1 - n_i/289)^10 over the {found} villagers) = {:.4}; x {per_day} = {:.1} golems a day; pad cells any of them reached: {}\n", p, p * per_day, pad_cells.len()));
    Ok(s)
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub cycles: i64,
    pub warm: i64,
    pub seed: i64,
    pub close_at: f64,
    pub slope_delay: i64,
    pub g2_plus: i64,
    pub hold_shut_delay: i64,
    pub pulse: i64,
    pub laps: usize,
    pub trace: bool,
}

/// One run: the report lines.
pub fn run(spec: &FarmSpec, meta: &BTreeMap<String, String>, o: &Opts) -> Result<String, String> {
    let cells = cells_of(meta)?;
    if cells.controller != "v4" {
        return Err(format!("run-controller {}: no such controller (v4)", cells.controller));
    }
    let ticks = o.warm + o.cycles * 100;
    let ro = RunOpts { ai: false, ..RunOpts::steady(o.seed, (ticks + 200) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let rsrc = Block::parse("minecraft:redstone_block")?;
    // (each gate's open and shut states, its source's own block, its closed bar: the collision
    // shape a shut gate puts across the track, 1.5 high, 4/16 thick across the cell's middle)
    let mut gate_b: BTreeMap<String, (Block, Block, Option<(BlockPos, Block)>)> = BTreeMap::new();
    let mut bars: BTreeMap<String, [f64; 6]> = BTreeMap::new();
    for (name, (g, src)) in &cells.gates {
        let gp = BlockPos::new(g[0], g[1], g[2]);
        let (open, shut) = match run.world.get(gp) {
            Block::FenceGate { facing, in_wall, .. } => {
                let (x, y, z) = (g[0] as f64, g[1] as f64, g[2] as f64);
                bars.insert(name.clone(), if matches!(facing, redsim::pos::Dir::North | redsim::pos::Dir::South) { [x, y, z + 0.375, x + 1.0, y + 1.5, z + 0.625] } else { [x + 0.375, y, z, x + 0.625, y + 1.5, z + 1.0] });
                (Block::FenceGate { facing, in_wall, open: true, powered: false }, Block::FenceGate { facing, in_wall, open: false, powered: false })
            }
            b => return Err(format!("run gate {name} at ({},{},{}): {b:?} is not a fence gate", g[0], g[1], g[2])),
        };
        let s = src.map(|s| {
            let sp = BlockPos::new(s[0], s[1], s[2]);
            (sp, run.world.get(sp))
        });
        gate_b.insert(name.clone(), (open, shut, s));
    }
    let mut cur: BTreeMap<String, Want> = cells.gates.keys().map(|g| (g.clone(), Want::default())).collect();
    let carts_of = |w: &redsim::world::World| -> Vec<Cart> {
        w.entities
            .list
            .iter()
            .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
            .map(|e| Cart { id: e.id, pos: [e.pos.x, e.pos.y, e.pos.z], v: [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z], bb: [e.bb.min_x, e.bb.min_y, e.bb.min_z, e.bb.max_x, e.bb.max_y, e.bb.max_z], rider: e.passengers.first().copied() })
            .collect()
    };
    let det_on = |w: &redsim::world::World, c: P| matches!(w.get(BlockPos::new(c[0], c[1], c[2])), Block::DetectorRail { powered: true, .. });
    let mut on: BTreeMap<String, bool> = cells.cells.iter().map(|(n, c)| (n.clone(), det_on(&run.world, *c))).collect();
    let carts0 = carts_of(&run.world);
    let rise0 = BTreeSet::new();
    let v0 = View { t: run.world.time, on: &on, rise: &rise0, carts: &carts0, cells: &cells };
    let mut ctl = V4::new(&v0, o.close_at, o.slope_delay, o.g2_plus, o.hold_shut_delay, o.pulse);
    let mut log: Vec<String> = Vec::new();
    let mut prev = carts0.clone();
    let mut touching: BTreeSet<(u32, u32)> = BTreeSet::new();
    let start = run.world.time;
    let measure_from = start + o.warm;
    let mut contacts: Vec<(i64, u32, u32, f64, f64, [f64; 3])> = Vec::new();
    let mut edges: Vec<String> = Vec::new();
    let mut laps: BTreeMap<u32, Vec<(i64, String)>> = BTreeMap::new();
    let mut summon_lines: Vec<String> = Vec::new();
    let mut summon_fail = 0usize;
    let mut opens: BTreeMap<String, usize> = BTreeMap::new();
    let mut ds_passes: BTreeMap<u32, usize> = BTreeMap::new();
    let mut max_v_contact = 0.0f64;
    let mut external: Vec<String> = Vec::new();
    let mut sensor_fail = 0usize;
    let mut dep_fail = 0usize;
    let mut sensor_lines: Vec<String> = Vec::new();
    let mut prev_batch: Vec<u32> = Vec::new();
    let mut reach_at: BTreeMap<u32, i64> = BTreeMap::new();
    let lock_y = cells.cells.get("PAD0").map(|p| p[1] as f64 + 20.54);
    let rider_of: BTreeMap<u32, Option<u32>> = carts0.iter().map(|c| (c.id, c.rider)).collect();
    for _ in 0..ticks {
        let t = run.world.time;
        if let Some(lock) = lock_y {
            for vid in &prev_batch {
                if !reach_at.contains_key(vid)
                    && let Some(i) = redsim::entity::index_of(&run.world, *vid)
                    && run.world.entities.list[i].pos.y >= lock
                {
                    reach_at.insert(*vid, t);
                }
            }
        }
        let carts = carts_of(&run.world);
        let mut rise = BTreeSet::new();
        for (n, c) in &cells.cells {
            let now = det_on(&run.world, *c);
            if now && !on[n] {
                rise.insert(n.clone());
            }
            on.insert(n.clone(), now);
        }
        // (laps: a detector's edge and the cart over it)
        for n in &rise {
            let b = cell_box(cells.cells[n]);
            for c in carts.iter().filter(|c| meets(&c.bb, &b, 0.0)) {
                laps.entry(c.id).or_default().push((t, n.clone()));
                if n == "Ds" && t >= measure_from {
                    *ds_passes.entry(c.id).or_default() += 1;
                }
            }
        }
        // (the summon frame: at each T after the warm-up, every villager inside a stand
        // summoner's box grown by 10)
        if t >= measure_from && t.rem_euclid(100) == 0 {
            let v = View { t, on: &on, rise: &rise, carts: &carts, cells: &cells };
            let mut batch: Vec<u32> = Vec::new();
            for n in ctl.summoner_cells() {
                if let Some(c) = cells.cells.get(&n) {
                    let b = cell_box(*c);
                    let inner = [b[0] + 0.2, b[1], b[2] + 0.2, b[3] - 0.2, b[4] + 0.5, b[5] - 0.2];
                    for k in carts.iter().filter(|k| meets(&k.bb, &inner, 0.0)) {
                        if let Some(r) = k.rider {
                            batch.push(r);
                        }
                    }
                }
            }
            let _ = v;
            let vill: Vec<(u32, [f64; 6], [f64; 3])> = run.world.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(_)) && e.vehicle.is_some_and(|vid| carts.iter().any(|c| c.id == vid))).map(|e| (e.id, [e.bb.min_x, e.bb.min_y, e.bb.min_z, e.bb.max_x, e.bb.max_y, e.bb.max_z], [e.pos.x, e.pos.y, e.pos.z])).collect();
            let mut others: BTreeSet<u32> = BTreeSet::new();
            for s in &batch {
                if let Some((_, sb, _)) = vill.iter().find(|x| x.0 == *s) {
                    let g = [sb[0] - 10.0, sb[1] - 10.0, sb[2] - 10.0, sb[3] + 10.0, sb[4] + 10.0, sb[5] + 10.0];
                    for (id, b, _) in &vill {
                        if !batch.contains(id) && meets(b, &g, 0.0) {
                            others.insert(*id);
                        }
                    }
                }
            }
            let list: Vec<String> = others.iter().filter_map(|id| vill.iter().find(|x| x.0 == *id)).map(|(_, _, p)| format!("({:.2},{:.2},{:.2})", p[0], p[1], p[2])).collect();
            if !others.is_empty() {
                summon_fail += 1;
            }
            // (the pad's golem sensor box at T, tick-exact's worst case: a rider senses a golem
            // whose feet are < 20.54 below or < 19.79 above its own and |dx|, |dz| < 18.84 to the
            // golem's centre; every rider outside the batch inside it, and the departing batch's
            // feet against the lock height pad feet + 20.54)
            if let (Some(p0), Some(p1)) = (cells.cells.get("PAD0"), cells.cells.get("PAD1")) {
                let g = p0[1] as f64;
                let inside: Vec<String> = vill.iter().filter(|(id, _, p)| {
                    !batch.contains(id) && p[1] - 20.54 < g && g < p[1] + 19.79 && {
                        let cx = (p[0]).clamp(p0[0] as f64 + 0.5, p1[0] as f64 + 0.5);
                        let cz = (p[2]).clamp(p0[2] as f64 + 0.5, p1[2] as f64 + 0.5);
                        (p[0] - cx).abs() < 18.84 && (p[2] - cz).abs() < 18.84
                    }
                }).map(|(_, _, p)| format!("({:.2},{:.2},{:.2})", p[0], p[1], p[2])).collect();
                if !inside.is_empty() {
                    sensor_fail += 1;
                }
                let lock = g + 20.54;
                let dep: Vec<String> = prev_batch.iter().map(|vid| vill.iter().find(|x| x.0 == *vid).map_or("gone".to_string(), |(_, _, p)| format!("{:.2}{}", p[1], if p[1] >= lock { "" } else { " LOW" }))).collect();
                let reached: Vec<String> = prev_batch.iter().map(|vid| reach_at.get(vid).map_or("never".to_string(), |r| format!("T{:+}", r - t))).collect();
                if !prev_batch.is_empty() && (prev_batch.iter().any(|vid| reach_at.get(vid).is_none_or(|r| *r > t - 5))) {
                    dep_fail += 1;
                }
                sensor_lines.push(format!("  T {}: riders outside the batch in a pad spot's sensor box: {}{}; the batch that left at the last T, feet now [{}] (lock {lock:.2}), first at the lock {}", t - start, inside.len(), if inside.is_empty() { String::new() } else { format!(" {}", inside.join(" ")) }, dep.join(", "), reached.join(", ")));
                prev_batch = batch.clone();
                for vid in &prev_batch {
                    reach_at.remove(vid);
                }
            }
            summon_lines.push(format!("  T {}: {} summoner(s) in the stand; other riders in a summoner's 10-box: {}{}", t - start, batch.len(), others.len(), if list.is_empty() { String::new() } else { format!(" {}", list.join(" ")) }));
            if o.trace {
                let vv = View { t, on: &on, rise: &rise, carts: &carts, cells: &cells };
                log.push(format!("T {}: {}", t - start, ctl.snapshot(&vv)));
            }
        }
        let v = View { t, on: &on, rise: &rise, carts: &carts, cells: &cells };
        // (a gate moved by the file's own redstone, not by the controller: listed)
        for (g, (gc, _)) in &cells.gates {
            let open_now = matches!(run.world.get(BlockPos::new(gc[0], gc[1], gc[2])), Block::FenceGate { open: true, .. });
            if let Some(c) = cur.get_mut(g)
                && open_now != c.open
            {
                external.push(format!("t {}: {g} {} by the file's own redstone", t - start, if open_now { "opened" } else { "shut" }));
                c.open = open_now;
            }
        }
        let mut events: Vec<String> = Vec::new();
        let want = ctl.step(&v, &mut events);
        for e in events {
            if o.trace {
                log.push(format!("{e} [t {}]", t - start));
            }
        }
        for (g, w) in &want {
            let c = cur.get(g).copied().unwrap_or_default();
            let Some((open_b, shut_b, src)) = gate_b.get(g) else { continue };
            let (gc, _) = cells.gates[g];
            if w.open != c.open {
                if !w.open {
                    // (a gate shut onto a cart: its box over the gate's closed bar)
                    let b = bars[g];
                    for k in carts.iter().filter(|k| meets(&k.bb, &b, -1e-6)) {
                        edges.push(format!("t {} ({}): {g} shut onto cart {} at ({:.3},{:.3},{:.3})", t - start, if t >= measure_from { "measured" } else { "warm-up" }, k.id, k.pos[0], k.pos[1], k.pos[2]));
                    }
                } else {
                    *opens.entry(g.clone()).or_default() += 1;
                }
                run.world.set_block(BlockPos::new(gc[0], gc[1], gc[2]), if w.open { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
            }
            if w.power != c.power
                && let Some((sp, own)) = src
            {
                run.world.set_block(*sp, if w.power { rsrc.clone() } else { own.clone() }, redsim::world::flags::ALL);
            }
            cur.insert(g.clone(), *w);
        }
        run.tick();
        // (contacts: a pair of carts whose boxes newly touch; both stored speeds the tick before)
        let now = carts_of(&run.world);
        let mut touch_now: BTreeSet<(u32, u32)> = BTreeSet::new();
        for i in 0..now.len() {
            for j in i + 1..now.len() {
                if meets(&now[i].bb, &now[j].bb, 1e-3) {
                    let key = (now[i].id.min(now[j].id), now[i].id.max(now[j].id));
                    touch_now.insert(key);
                    if !touching.contains(&key) && t >= measure_from {
                        let sp = |id: u32| prev.iter().find(|c| c.id == id).map_or(0.0, |c| (c.v[0].powi(2) + c.v[2].powi(2)).sqrt());
                        let (a, b) = (sp(key.0), sp(key.1));
                        max_v_contact = max_v_contact.max(a.max(b));
                        let at = now.iter().find(|c| c.id == key.0).map_or([0.0; 3], |c| c.pos);
                        contacts.push((t + 1 - start, key.0, key.1, a, b, at));
                    }
                }
            }
        }
        touching = touch_now;
        prev = now;
    }
    // ---- the report
    let mut s = String::new();
    s.push_str(&format!("column-run: controller {} (every gate and powered-rail source it drives is EMULATED: a gate block set open or shut, a source block set to a redstone block and back; the detector rails it reads are the real ones); {} carts ({} ridden); {} gt warm-up then {} cycles of 100 gt (T at world time multiples of 100); seed {}\n", cells.controller, carts0.len(), carts0.iter().filter(|c| c.rider.is_some()).count(), o.warm, o.cycles, o.seed));
    let hard: Vec<&(i64, u32, u32, f64, f64, [f64; 3])> = contacts.iter().filter(|c| c.3.max(c.4) > 0.2).collect();
    s.push_str(&format!("contacts (measured): {} new cart-cart contacts, the largest stored speed at contact {:.3}; {} above 0.2{}\n", contacts.len(), max_v_contact, hard.len(), if hard.is_empty() { " (PASS)" } else { " (FAIL)" }));
    for c in hard.iter().take(20) {
        // (the nearest run cell names where it happened)
        let near = cells.cells.iter().chain(cells.gates.iter().map(|(n, (g, _))| (n, g))).map(|(n, p)| (n, ((p[0] as f64 + 0.5 - c.5[0]).powi(2) + (p[1] as f64 + 0.5 - c.5[1]).powi(2) + (p[2] as f64 + 0.5 - c.5[2]).powi(2)).sqrt())).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(n, d)| format!("{n} +{d:.1}")).unwrap_or_default();
        s.push_str(&format!("  t {}: carts {} and {} at ({:.2},{:.2},{:.2}) [{near}], stored {:.3} / {:.3}\n", c.0, c.1, c.2, c.5[0], c.5[1], c.5[2], c.3, c.4));
    }
    let measured_edges = edges.iter().filter(|e| e.contains("(measured)")).count();
    s.push_str(&format!("gate shuts onto a cart: {} measured, {} in the warm-up{}\n", measured_edges, edges.len() - measured_edges, if measured_edges == 0 { " (PASS)" } else { " (FAIL)" }));
    for e in edges.iter().take(20) {
        s.push_str(&format!("  {e}\n"));
    }
    s.push_str(&format!("summon frame (at each T, every rider inside a stand summoner's box grown by 10; AI off, so wanting is not evaluated: every rider outside the batch counts): {} of {} T with a rider outside the batch{}\n", summon_fail, summon_lines.len(), if summon_fail == 0 { " (PASS)" } else { " (FAIL)" }));
    for l in &summon_lines {
        s.push_str(l);
        s.push('\n');
    }
    if !sensor_lines.is_empty() {
        s.push_str(&format!("pad sensor box at T (tick-exact's worst case, |dx|,|dz| < 18.84, golem feet < 20.54 below / < 19.79 above a rider's) and the departing batch (its rear at the lock, pad feet + 20.54, by the next T - 5): {} of {} T with a rider outside the batch inside a pad spot's box{}; {} T whose last batch missed the lock by T - 5{}\n", sensor_fail, sensor_lines.len(), if sensor_fail == 0 { " (PASS)" } else { " (FAIL)" }, dep_fail, if dep_fail == 0 { " (PASS)" } else { " (FAIL)" }));
        for l in &sensor_lines {
            s.push_str(l);
            s.push('\n');
        }
    }
    s.push_str(&format!("gate openings: {}\n", opens.iter().map(|(g, n)| format!("{g} {n}")).collect::<Vec<_>>().join(", ")));
    s.push_str(&format!("gates moved by the file's own redstone (not the controller): {}{}\n", external.len(), if external.is_empty() { String::new() } else { format!(": {}", external.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }));
    let lapped: Vec<String> = ds_passes.iter().map(|(id, n)| format!("{id}:{n}")).collect();
    s.push_str(&format!("hold releases per cart (Ds passes after the warm-up): {} carts passed Ds at least once: {}\n", ds_passes.len(), lapped.join(" ")));
    // (the laps of the first carts released after the warm-up)
    let mut firsts: Vec<(i64, u32)> = laps.iter().filter_map(|(id, l)| l.iter().find(|(t, n)| n == "Ds" && *t >= measure_from).map(|(t, _)| (*t, *id))).collect();
    firsts.sort();
    for (_, id) in firsts.iter().take(o.laps) {
        let l = &laps[id];
        let tl: Vec<String> = l.iter().filter(|(t, _)| *t >= measure_from).map(|(t, n)| format!("{n}@{}", t - start)).collect();
        s.push_str(&format!("  cart {id} ({}): {}\n", if rider_of.get(id).copied().flatten().is_some() { "ridden" } else { "empty" }, tl.join(" ")));
    }
    if o.trace {
        s.push_str("carts at the end (position, velocity, every detector pass):\n");
        for c in carts_of(&run.world) {
            let tl: Vec<String> = laps.get(&c.id).map(|l| l.iter().map(|(t, n)| format!("{n}@{}", t - start)).collect()).unwrap_or_default();
            s.push_str(&format!("  cart {} ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3}): {}\n", c.id, c.pos[0], c.pos[1], c.pos[2], c.v[0], c.v[1], c.v[2], tl.join(" ")));
        }
        s.push_str("control log:\n");
        for l in &log {
            s.push_str(&format!("  {l}\n"));
        }
    }
    Ok(s)
}
