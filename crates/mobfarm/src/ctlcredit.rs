//! ctl-credit, LOGIC LEVEL: G8 as (B') with THE REL-KEYED RING RESET (gate list rev 20:00; ironspec 18:55 / 19:03 /
//! 19:08 / 19:17 / 19:38 / 19:45; G1 19:25 / 19:27 / 19:40; day-1 19:21's ring finding): the station's batch-complete
//! evidence as a CREDIT COUNT in a counting hopper with a BALLAST item, the ring re-armed every batch by a pulse at
//! REL + 75, the outbound pick's HOLDs and emulated presses (G11), the returning napper's masks, at 1-gt resolution,
//! before the cells are laid (the real-cell bench replaces every DESIGNED delay below; tick-exact pins the hopper and
//! dropper lines, AD 8f081acbf).
//!
//! THE FORM:
//! - SEEN3: a latch SET by D(3)'s press (masked in K(3)'s window), RESET by its own batch's D(7) (+ RESET).
//! - THE CREDIT: D(7)'s rising edge AND SEEN3 (the AND reads SEEN3 before D(7)'s own reset lands) fires the credit
//!   dropper (4 gt) into the counting hopper; REL unlocks the hopper for ONE transfer back into the dropper (the items
//!   recycle). The hopper holds a BALLAST item: credits = items - 1; RE_ARMED = >= 2 items (the four-dust read).
//!   Power-up: 3 in the hopper, 2 in the dropper. An item entering an EMPTY hopper starts its cooldown (effective 7);
//!   a transfer starts it too (8; eject-then-suck under one cooldown); the cooldown is saved over a world open.
//! - THE IN-FLIGHT BYPASS (a flag): IN_FLIGHT SET by the AND's pulse, RESET by REL or the comparator's rise;
//!   RE_ARMED' = (>= 2 items) OR IN_FLIGHT.
//! - THE RING (G10): at X = REL + 75 a 4-gt pulse forces J(-1) ARMED and J(3) DISARMED (J(7) is on by default); D(-1)
//!   drives J(-1) off and arms J(3) (masked in K(-1)'s window); D(3) drives J(3) off. The latches are set-priority: a
//!   drive overlapping the reset pulse ends on whichever drive outlasts the other. D(7) touches nothing on the ring
//!   (the AND -> J(-1) link is DROPPED; the control puts it back without the reset).
//! - G11, per pick: lane -1: the emulated D(-1) at X + the pulse's width + 2, HOLD_3 / HOLD_7 from X to the picked
//!   cart's J(3) / J(7) arrival + k; lane 3: the emulated D(3) at slot 1's real D(-1) + 2, HOLD_7 from X; lane 7: the
//!   emulated D(7) at slot 2's real D(3) + delta7, HOLD_7 from X. J(y)'s drive = ARMED_y AND NOT HOLD_y.
//! - F1: REL = the pulse's request (every 100 gt; B held 100 from each start; a pulse during B latched) AND the holds
//!   full AND RE_ARMED; REL = the pulse + 6 when nothing waits.
//!
//! THE TIMELINE (onechunk's clock-locked figures, gt after the batch's REL): slot 1 (lane -1's cart) at J(-1) 101,
//! J(3) 116, J(7) 146; slot 2 (lane 3's) at 121 / 136 / 166; slot 3 (lane 7's) at 150 / 165 / 181; a returning napper
//! R6 (10) behind slot 3: 160 / 175 / 191. A cart diverts at the first J whose shape is branch when it arrives (the
//! returning napper at its own K(x) junction, the J's west of it held off); its D press 4 gt after the J.

use crate::verdict::Verdict;

/// The golem pulse's period (the station's cycle B).
pub const PERIOD: i64 = 100;
/// The pulse -> REL (2 into the pad portal + 4).
pub const REL_LAT: i64 = 6;
/// REL -> the hopper's unlock window, and its width (a short pulse: at most one transfer).
pub const UNLOCK: i64 = 2;
pub const UNLOCK_W: i64 = 4;
/// The hopper's cooldowns (tick-exact 19:16 / 19:25, accepted): an item entering an EMPTY hopper (input R -> entry
/// R + 4 -> pushable R + 11: 7 after the entry); a transfer (8, eject-then-suck under one cooldown).
pub const INSERT_COOLDOWN: i64 = 7;
pub const TRANSFER_COOLDOWN: i64 = 8;
/// The hopper's comparator -> the read (2 gt behind each change; the four dust add nothing).
pub const CMP: i64 = 2;
/// D(7)'s rising edge -> the AND's output (the edge comparator and the AND's input repeater into the OR block + its
/// torch).
pub const AND: i64 = 4;
/// The dropper's dispense after its input rises (vanilla: a 4-gt scheduled tick).
pub const DROPPER: i64 = 4;
/// ctl-junction's latch: set drive -> Q 4, reset drive -> Q's fall 6.
pub const SET: i64 = 4;
pub const RESET: i64 = 6;
/// The bypass's latch: the AND's pulse -> IN_FLIGHT's Q block (its set to Q 2).
pub const IN_FLIGHT_SET: i64 = 2;
/// THE FAST CREDIT PATH (ironspec 20:26: the credit is on the critical path, readable at REL_b + 196 against REL_{b+2}'s
/// strings ready at REL_b + 195 on cc767b78's night; cut >= 6 gt): ONE SUBTRACT-MODE COMPARATOR folds D(7)'s edge and
/// SEEN3 (D(7) on its rear; D(7) through a delay-2 repeater and NOT-SEEN3 ORed on its side): the credit pulse 2 gt after
/// D(7)'s rise, straight into the dropper and into IN_FLIGHT's set; RE_ARMED = the four-dust read OR IN_FLIGHT OR the
/// pulse: readable at D(7) + 2 (REL_b + 188 against + 195: margin 7).
pub const FAST: i64 = 2;
/// The measured strings-ready tick of REL_{b+2} after REL_b (cc767b78's t24 smoke night) and the measured D(7) - REL.
pub const STRINGS_READY: i64 = 195;
pub const D7_MEASURED: i64 = 186;
/// A D press -> its J's shape change (G10, G1 15:51): to branch (+6), back to pass (+8); a press's drive width.
pub const SWITCH: i64 = 6;
pub const RETURN: i64 = 8;
pub const PRESS_W: i64 = 4;
/// THE RING RESET (G10, ironspec 19:45 (1)): REL + 75, a 4-gt pulse.
pub const RING_X: i64 = 72;
pub const RESET_W: i64 = 4;
/// A J -> the cart's D press.
pub const J_TO_D: i64 = 4;
/// K(x)'s window around the napper's crossing of J(x).
pub const K_HALF: i64 = 10;
/// G11: the HOLDs' reset after the picked cart's arrival at the J (k), and the lane-7 pick's delta7.
pub const HOLD_K: i64 = 1;
pub const DELTA7: i64 = 10;
/// G11's HOLD SET tap (day-1 20:10, a finding of this bench): the HOLDs can't set at the ring reset X (REL + 75): the
/// previous batch's slot 3 reaches J(7) at REL + 81 and a HOLD_7 set at X holds it in pass (it rides every J and is
/// lost to the way-in); set at REL + 95: 14 behind the previous slot 3 at J(7), 30 behind it at J(3), and 21 / 51 / 71
/// ahead of the picked slot 1 / 2 / 3 at J(3) / J(7).
pub const HOLD_SET: i64 = 95;
/// The slots' arrivals at J(-1), J(3), J(7) after REL; the napper's.
pub const SLOT_T: [[i64; 3]; 3] = [[101, 116, 146], [121, 136, 166], [150, 165, 181]];
/// The returning napper's J arrivals, R6 (10) behind slot 3 on the REL + 51 stand-in (the hold rail at REL + 63).
/// NAPPER_SHIFT: ARM3 at (4,6,7)'s press (REL + 57) through the + 8 chain puts the hold rail at REL + 65 (ironspec 20:58
/// a: the tent's middle): the napper 2 gt later than the stand-in's.
pub const NAPPER_SHIFT: i64 = 2;
pub const NAPPER_T: [i64; 3] = [160 + NAPPER_SHIFT, 175 + NAPPER_SHIFT, 191 + NAPPER_SHIFT];
pub const LANES: [i32; 3] = [-1, 3, 7];
/// The power-up: the hopper's items (the ballast + 2 credits) and the dropper's stock.
pub const HOPPER0: i64 = 3;
pub const DROPPER0: i64 = 2;

/// One scripted run.
#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub hopper0: i64,
    pub dropper0: i64,
    pub t_max: i64,
    /// (batch, slot) a cart missing from its slot.
    pub missing: Vec<(usize, usize)>,
    /// (batch, lane index, extra lag) a returning napper behind the batch's slot 3.
    pub napper: Vec<(usize, usize, i64)>,
    /// Stray presses (tick, lane) by nothing on the ring's path.
    pub stray: Vec<(i64, i32)>,
    /// The holds not full over [a, b) (a REL delayed).
    pub holds_empty: Vec<(i64, i64)>,
    /// (batch, slot, gt) a batch cart late by that much (all its J arrivals).
    pub late: Vec<(usize, usize, i64)>,
    /// A world open (game tick, idle, the carts' extra lag after it).
    pub open: Option<(i64, i64, i64)>,
    /// (batch, lane index) an outbound pick: that slot's cart carries the napper to the way-in (G11).
    pub pick: Vec<(usize, usize)>,
    /// The in-flight bypass (a flag).
    pub bypass: bool,
    /// The misroute-surviving completion candidate (ironspec 19:56 (2)): a batch cart's REAL D(-1) press while SEEN3
    /// is set also completes (credits and resets SEEN3), K(-1)'s mask excluding the napper's.
    pub credit_dm1: bool,
    /// The ring reset's tap X (REL + X).
    pub ring_x: i64,
    /// The pick-dependent reset (ironspec 19:56 (4)): on a lane -1 pick the reset leaves J(-1) off and arms J(3)
    /// directly (no emulated D(-1)).
    pub pick_dep: bool,
    /// The fast credit path (the folded comparator and IN_FLIGHT; ironspec 20:26); false: the edge comparator + the
    /// AND into the dropper, readable only from the hopper's read (D(7) + 10).
    pub fast: bool,
}

impl Row {
    pub fn normal(name: &str) -> Row {
        Row { name: name.to_string(), hopper0: HOPPER0, dropper0: DROPPER0, t_max: 3000, missing: vec![], napper: vec![], stray: vec![], holds_empty: vec![], late: vec![], open: None, pick: vec![], bypass: false, credit_dm1: true, ring_x: RING_X, pick_dep: false, fast: true }
    }
}

/// Planted defects (one per control).
#[derive(Clone, Copy, Default, Debug)]
pub struct Defects {
    pub no_k3_mask: bool,
    pub no_km1_mask: bool,
    /// G8 as (A): RE_ARMED a single latch (the count capped at 1, reset at REL) and SEEN3 reset at REL.
    pub single_latch: bool,
    /// No ballast: RE_ARMED = the hopper >= 1 item (ironspec 19:03 (2)'s defect).
    pub no_ballast: bool,
    /// The old ring: J(-1) re-armed by G8's AND (D(7) AND SEEN3), no REL-keyed reset (day-1 19:21's finding).
    pub and_rearm: bool,
    /// Lane -1's emulated D(-1) at X + 2, inside the reset pulse (the set-priority fight).
    pub reset_order: bool,
    /// HOLD_3 removed (lane -1's pick).
    pub no_hold3: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Out {
    pub rels: Vec<i64>,
    /// Per REL: its slip past the pulse + REL_LAT.
    pub slips: Vec<i64>,
    /// Per REL: armed by IN_FLIGHT (the fast path) with the hopper still reading < 2.
    pub rel_inflight: Vec<bool>,
    /// Per REL: the hopper's items read by F1, after its unlock window, and the unlock's transfer tick (None: no transfer).
    pub at_rel: Vec<(i64, i64, Option<i64>)>,
    pub items_max: i64,
    pub items_min: i64,
    pub dropper_min: i64,
    /// Per credit: the dropper's input edges (rise, fall).
    pub dropper_edges: Vec<(i64, i64)>,
    /// The item total (the hopper + the dropper) whenever it differs from the power-up's: an invariant.
    pub total_changes: Vec<(i64, i64)>,
    /// Per REL: the dropper's stock and the total at its read.
    pub stock_at_rel: Vec<(i64, i64)>,
    /// Per credit: (the item's entry tick, the batch whose D(7) made it; usize::MAX for a stray).
    pub credits: Vec<(i64, usize)>,
    /// Per credit: its latency from the press to readable.
    pub credit_latency: Vec<i64>,
    /// Per credit: (the tick RE_ARMED can read it, the batch it completes).
    pub credit_ready: Vec<(i64, usize)>,
    /// The least margin of a credit's readable tick before REL_{b+2}'s MEASURED strings-ready tick (REL_b + 195; the
    /// press at the measured D(7) - REL 186 plus the path's latency).
    pub measured_margin: Option<i64>,
    /// Credits the dropper fired dry (no item).
    pub dry: Vec<i64>,
    /// Per batch: per slot, the lane it entered (None: missing, still riding, or passed every J).
    pub entered: Vec<[Option<i32>; 3]>,
    /// Per batch: per slot, its pass ticks at J(-1), J(3), J(7) (0: not reached).
    pub pass: Vec<[[i64; 3]; 3]>,
    /// Per batch: its D presses (tick, lane pressed, the slot's own lane; None for a napper or an emulated press).
    pub presses: Vec<Vec<(i64, i32, Option<i32>)>>,
    pub misroutes: Vec<String>,
    /// Napper presses that changed SEEN3, the credit or J(3) (not state-neutral).
    pub napper_effects: Vec<String>,
    /// The picked (outbound) carts that diverted into a lane.
    pub pick_diverted: Vec<String>,
    /// The least (REL_{b+2}'s F1 read) - (batch b's credit readable tick) over the batches.
    pub credit_lead_min: Option<i64>,
    /// The least distance of a batch D(-1) / D(3) press from a K(-1) / K(3) window.
    pub mask_margin: Option<i64>,
    /// Per batch: the ring reset's margins (J(-1)'s branch -> this batch's slot 1 at J(-1); J(3)'s return - the
    /// previous batch's slot 3 at J(3); J(-1) armed - the previous batch's slot 3 at J(-1)).
    pub reset_margins: Vec<(Option<i64>, Option<i64>, Option<i64>)>,
    /// Per lane -1 pick: the emulated D(-1)'s drive on J(-1) - the reset pulse's end (>= 1: no fight).
    pub order_margin: Vec<i64>,
    /// The hopper's items before and after the world open; the cooldown left before and after.
    pub open_items: Option<(i64, i64)>,
    pub open_cooldown: Option<(i64, i64)>,
    /// The read at the first REL (items, the fourth dust's level).
    pub first_read: Option<(i64, i64)>,
    /// Per lane -1 pick (emulated form): the picked slot 1's arrival at J(-1) - J(-1)'s turn-off.
    pub pick_margin: Vec<i64>,
    /// Per lane (-1, 3, 7): the net carts gained (a batch cart entering another lane moves a cart between sidings).
    pub lane_delta: [i64; 3],
    /// Credits made by the D(-1) completion candidate.
    pub dm1_credits: usize,
}

struct Press {
    at: i64,
    lane: i32,
    batch: usize,
    slot: Option<usize>,
    napper: bool,
}

struct Cart {
    batch: usize,
    slot: Option<usize>,
    ji: usize,
    arr: [i64; 3],
    nap: Option<usize>,
    picked: bool,
}

fn active(d: &[(i64, i64)], t: i64) -> bool {
    d.iter().any(|&(a, b)| t >= a && t < b)
}

/// The four dust after the hopper's comparator (non-stackable items: 3 a slot).
pub fn four_dust(items: i64) -> i64 {
    (3 * items - 4).max(0)
}

pub fn run(row: &Row, def: Defects) -> Out {
    let (t_open, idle, kj) = row.open.unwrap_or((i64::MAX, 0, 0));
    let n = (row.t_max + idle + kj + 600) as usize;
    let ballast = if def.no_ballast { 0 } else { 1 };
    let need = if def.single_latch { 1 } else { ballast + 1 };
    let mut o = Out { items_min: i64::MAX, dropper_min: i64::MAX, ..Out::default() };
    let mut items_hist = vec![0i64; n];
    let mut items = row.hopper0;
    let mut dropper = row.dropper0;
    let mut cool_until = 0i64;
    let mut unlock: Option<(i64, i64, usize)> = None;
    let mut dispense: Vec<(i64, usize)> = Vec::new();
    // the latches' drives (block domain): set / reset intervals
    let (mut jm1_set, mut jm1_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let (mut j3_set, mut j3_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let (mut s3_set, mut s3_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let (mut hold3, mut hold7): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let mut if_set: Vec<i64> = Vec::new();
    let (mut jm1, mut j3, mut seen3, mut in_flight) = (true, false, false, false);
    let mut resets: Vec<i64> = Vec::new();
    let mut pending = false;
    let mut last_rel: Option<i64> = None;
    let mut pulse_at = 0i64;
    let mut presses: Vec<Press> = Vec::new();
    let mut k_windows: Vec<(i64, i64, usize)> = Vec::new();
    let mut carts: Vec<Cart> = Vec::new();
    let shift = |t: i64| if t >= t_open { t + idle } else { t };
    let mut rel_of: Vec<i64> = Vec::new();
    for t in 0..(n as i64 - 1) {
        let ti = t as usize;
        if t == t_open {
            // the world open: the block domain freezes for `idle`, the carts resume `kj` later still; everything
            // scheduled from here on moves with its domain (the hopper's cooldown is saved in the block)
            o.open_cooldown = Some(((cool_until - t).max(0), 0));
            let sh = |d: &mut Vec<(i64, i64)>| {
                for x in d.iter_mut() {
                    if x.0 >= t_open {
                        x.0 += idle;
                    }
                    if x.1 > t_open {
                        x.1 += idle;
                    }
                }
            };
            for d in [&mut jm1_set, &mut jm1_rst, &mut j3_set, &mut j3_rst, &mut s3_set, &mut s3_rst, &mut hold3, &mut hold7] {
                sh(d);
            }
            for x in dispense.iter_mut().filter(|x| x.0 >= t_open) {
                x.0 += idle;
            }
            for x in if_set.iter_mut().filter(|x| **x >= t_open) {
                *x += idle;
            }
            for x in k_windows.iter_mut() {
                if x.0 >= t_open {
                    x.0 += idle;
                }
                if x.1 >= t_open {
                    x.1 += idle;
                }
            }
            if let Some(u) = unlock.as_mut() {
                if u.0 + u.1 > t_open {
                    u.0 += idle;
                }
            }
            if cool_until > t_open {
                cool_until += idle;
            }
            for c in carts.iter_mut() {
                for a in c.arr.iter_mut() {
                    if *a >= t_open {
                        *a += idle + kj;
                    }
                }
            }
            for p in presses.iter_mut().filter(|p| p.at >= t_open) {
                // (a cart's press moves with the cart; an emulated press is redstone)
                p.at += if p.slot.is_some() || p.napper { idle + kj } else { idle };
            }
            o.open_items = Some((items, items));
        }
        if t >= t_open && t < t_open + idle {
            items_hist[ti] = items;
            if t % PERIOD == 0 {
                pending = true;
                pulse_at = t;
            }
            continue;
        }
        if t == t_open + idle {
            if let Some(c) = o.open_items.as_mut() {
                c.1 = items;
            }
            if let Some(c) = o.open_cooldown.as_mut() {
                c.1 = (cool_until - t).max(0);
            }
        }
        // the dropper dispenses (one item into the hopper, or nothing if dry)
        for &(at, b) in &dispense {
            if at == t {
                if dropper > 0 {
                    dropper -= 1;
                    if items == 0 {
                        cool_until = t + INSERT_COOLDOWN;
                    }
                    items += 1;
                    if def.single_latch {
                        items = items.min(1);
                    }
                    o.credits.push((t, b));
                } else {
                    o.dry.push(t);
                }
            }
        }
        // the hopper's unlock window: at most one push back into the dropper
        if let Some((a, w, ri)) = unlock {
            if t >= a && t < a + w && o.at_rel[ri].2.is_none() && items > 0 && t >= cool_until {
                items -= 1;
                dropper += 1;
                cool_until = t + TRANSFER_COOLDOWN;
                o.at_rel[ri].2 = Some(t);
            }
            if t == a + w {
                o.at_rel[ri].1 = items;
                unlock = None;
                if def.single_latch {
                    items = 0;
                }
            }
        }
        items_hist[ti] = items;
        o.items_max = o.items_max.max(items);
        o.items_min = o.items_min.min(items);
        o.dropper_min = o.dropper_min.min(dropper);
        if items + dropper != row.hopper0 + row.dropper0 && o.total_changes.last().map(|c| c.1) != Some(items + dropper) {
            o.total_changes.push((t, items + dropper));
        }
        // the latches (set-priority)
        let latch = |q: bool, s: &[(i64, i64)], r: &[(i64, i64)]| if active(s, t) { true } else if active(r, t) { false } else { q };
        jm1 = latch(jm1, &jm1_set, &jm1_rst);
        j3 = latch(j3, &j3_set, &j3_rst);
        seen3 = latch(seen3, &s3_set, &s3_rst);
        let read = if t >= CMP { items_hist[(t - CMP) as usize] } else { items };
        if if_set.contains(&t) {
            in_flight = true;
        }
        if in_flight && read >= need {
            in_flight = false;
        }
        // F1 and REL
        if t % PERIOD == 0 {
            pending = true;
            pulse_at = t;
        }
        let b_ok = last_rel.is_none_or(|r| t >= r + PERIOD);
        let holds = !row.holds_empty.iter().any(|&(a, b)| t >= shift(a) && t < shift(b));
        let armed = read >= need || ((row.bypass || row.fast) && in_flight);
        if pending && t >= pulse_at + REL_LAT && b_ok && holds && armed {
            pending = false;
            let last_rel_prev = last_rel;
            last_rel = Some(t);
            in_flight = false;
            o.rels.push(t);
            // (a slip is throughput lost: the period over 100; a constant phase lag behind the golem pulse is none)
            o.slips.push(last_rel_prev.map_or(0, |p| (t - p - PERIOD).max(0)));
            o.rel_inflight.push(read < need);
            if o.first_read.is_none() {
                o.first_read = Some((read, four_dust(read)));
            }
            o.at_rel.push((read, items, None));
            o.stock_at_rel.push((dropper, items + dropper));
            unlock = Some((t + UNLOCK, UNLOCK_W, o.at_rel.len() - 1));
            if def.single_latch {
                s3_rst.push((t + RESET, t + RESET + PRESS_W));
            }
            let b = o.presses.len();
            rel_of.push(t);
            o.presses.push(Vec::new());
            o.entered.push([None; 3]);
            o.pass.push([[0; 3]; 3]);
            let lag = if t >= t_open { kj } else { 0 };
            let pick = row.pick.iter().find(|p| p.0 == b).map(|p| p.1);
            for (s, st) in SLOT_T.iter().enumerate() {
                if !row.missing.contains(&(b, s)) {
                    let lt = lag + row.late.iter().filter(|l| l.0 == b && l.1 == s).map(|l| l.2).sum::<i64>();
                    carts.push(Cart { batch: b, slot: Some(s), ji: 0, arr: [t + st[0] + lt, t + st[1] + lt, t + st[2] + lt], nap: None, picked: pick == Some(s) });
                }
            }
            for &(nb, lane, extra) in &row.napper {
                if nb == b {
                    let nt = [t + NAPPER_T[0] + extra + lag, t + NAPPER_T[1] + extra + lag, t + NAPPER_T[2] + extra + lag];
                    k_windows.push((nt[lane] - K_HALF, nt[lane] + K_HALF, lane));
                    carts.push(Cart { batch: b, slot: None, ji: 0, arr: nt, nap: Some(lane), picked: false });
                }
            }
            let x = t + row.ring_x;
            let dep = row.pick_dep && pick == Some(0);
            if !def.and_rearm {
                resets.push(x);
                if dep {
                    // (the pick-dependent reset: J(-1) left off, J(3) armed directly)
                    jm1_rst.push((x + RETURN, x + RETURN + RESET_W));
                    j3_set.push((x + SWITCH, x + SWITCH + RESET_W));
                } else {
                    jm1_set.push((x + SWITCH, x + SWITCH + RESET_W));
                    j3_rst.push((x + RETURN, x + RETURN + RESET_W));
                }
            }
            match pick {
                Some(0) => {
                    if !dep {
                        let e = if def.reset_order { x - 2 } else { x + RESET_W + 2 };
                        presses.push(Press { at: e, lane: -1, batch: b, slot: None, napper: false });
                        o.order_margin.push((e + RETURN) - (x + SWITCH + RESET_W));
                        // (the pick's own margin: J(-1) off against the picked slot 1's arrival at J(-1))
                        o.pick_margin.push((t + SLOT_T[0][0] + lag) - (e + RETURN));
                    }
                    if !def.no_hold3 {
                        hold3.push((t + HOLD_SET, t + SLOT_T[0][1] + lag + HOLD_K + 1));
                    }
                    hold7.push((t + HOLD_SET, t + SLOT_T[0][2] + lag + HOLD_K + 1));
                }
                Some(1) => hold7.push((t + HOLD_SET, t + SLOT_T[1][2] + lag + HOLD_K + 1)),
                Some(2) => hold7.push((t + HOLD_SET, t + SLOT_T[2][2] + lag + HOLD_K + 1)),
                _ => {}
            }
        }
        // carts reaching their next J
        let mut keep = Vec::new();
        for mut c in carts.drain(..) {
            if c.arr[c.ji] != t {
                keep.push(c);
                continue;
            }
            let branch = match c.nap {
                Some(lane) => c.ji == lane,
                None => match c.ji {
                    0 => jm1,
                    1 => j3 && !active(&hold3, t),
                    _ => !active(&hold7, t),
                },
            };
            if let Some(s) = c.slot {
                if c.batch < o.pass.len() {
                    o.pass[c.batch][s][c.ji] = t;
                }
            }
            if branch {
                presses.push(Press { at: t + J_TO_D, lane: LANES[c.ji], batch: c.batch, slot: c.slot, napper: c.nap.is_some() });
                if let Some(s) = c.slot {
                    if c.batch < o.entered.len() {
                        o.entered[c.batch][s] = Some(LANES[c.ji]);
                    }
                    if c.picked {
                        o.pick_diverted.push(format!("batch {}'s picked lane {} cart diverted into lane {} at t {t}", c.batch, LANES[s], LANES[c.ji]));
                    } else if s != c.ji {
                        o.misroutes.push(format!("batch {}'s lane {} cart into lane {} at t {t}", c.batch, LANES[s], LANES[c.ji]));
                        o.lane_delta[c.ji] += 1;
                        o.lane_delta[s] -= 1;
                    }
                }
            } else if c.ji < 2 {
                c.ji += 1;
                keep.push(c);
            } else if let Some(s) = c.slot {
                if !c.picked {
                    o.misroutes.push(format!("batch {}'s lane {} cart passed every J at t {t}", c.batch, LANES[s]));
                    o.lane_delta[s] -= 1;
                }
            }
        }
        carts = keep;
        for &(s, lane) in &row.stray {
            if shift(s) == t {
                presses.push(Press { at: t, lane, batch: usize::MAX, slot: None, napper: false });
            }
        }
        // presses landing now
        let in_k = |l: usize| k_windows.iter().any(|&(a, b2, kl)| kl == l && t >= a && t <= b2);
        let mut later = Vec::new();
        let mut emulated = Vec::new();
        for p in presses.drain(..) {
            if p.at != t {
                later.push(p);
                continue;
            }
            if p.batch < o.presses.len() {
                o.presses[p.batch].push((t, p.lane, p.slot.map(|s| LANES[s])));
            }
            let pick = row.pick.iter().find(|q| q.0 == p.batch).map(|q| q.1);
            match p.lane {
                -1 => {
                    // (K(-1)'s mask covers both of D(-1)'s ring outputs, ironspec 19:56 (3) / day-1 20:11: a lane -1
                    // napper's own D(-1) can land after the next batch's ring reset (R6 + 10 or later) and would turn
                    // the freshly armed J(-1) off under batch b+1's slot 1)
                    let masked = !def.no_km1_mask && in_k(0);
                    if !masked {
                        if p.napper && jm1 {
                            o.napper_effects.push(format!("t {t}: the lane -1 napper's D(-1) turned J(-1) off"));
                        }
                        jm1_rst.push((t + RETURN, t + RETURN + PRESS_W));
                    }
                    if !masked {
                        if p.napper && !j3 {
                            o.napper_effects.push(format!("t {t}: the lane -1 napper's D(-1) armed J(3)"));
                        }
                        j3_set.push((t + SWITCH, t + SWITCH + PRESS_W));
                    }
                    if pick == Some(1) && p.slot == Some(0) {
                        emulated.push(Press { at: t + 2, lane: 3, batch: p.batch, slot: None, napper: false });
                    }
                    // (the D(-1) completion, ironspec 20:17 (A): a batch cart's real D(-1) while SEEN3 is set completes
                    // the batch that set it; K(-1)'s mask covers this input too)
                    if row.credit_dm1 && p.slot.is_some() && seen3 && !masked {
                        let lat = if row.fast { FAST } else { AND };
                        dispense.push((t + lat + DROPPER, p.batch.saturating_sub(1)));
                        o.credit_latency.push(if row.fast { FAST } else { AND + DROPPER + CMP });
                        o.credit_ready.push((t + if row.fast { FAST } else { AND + DROPPER + CMP }, p.batch.saturating_sub(1)));
                        o.dropper_edges.push((t + lat, t + lat + PRESS_W));
                        if row.fast {
                            if_set.push(t + FAST);
                        }
                        s3_rst.push((t + RESET, t + RESET + PRESS_W));
                        o.dm1_credits += 1;
                    }
                }
                3 => {
                    j3_rst.push((t + RETURN, t + RETURN + PRESS_W));
                    if def.no_k3_mask || !in_k(1) {
                        if p.napper && !seen3 {
                            o.napper_effects.push(format!("t {t}: the lane-3 napper's D(3) set SEEN3"));
                        }
                        s3_set.push((t + SET, t + SET + PRESS_W));
                    }
                    if pick == Some(2) && p.slot == Some(1) {
                        emulated.push(Press { at: t + DELTA7, lane: 7, batch: p.batch, slot: None, napper: false });
                    }
                }
                _ => {
                    if seen3 {
                        if p.napper {
                            o.napper_effects.push(format!("t {t}: the lane-7 napper's D(7) made a credit"));
                        }
                        let lat = if row.fast { FAST } else { AND };
                        dispense.push((t + lat + DROPPER, p.batch));
                        o.credit_latency.push(if row.fast { FAST } else { AND + DROPPER + CMP });
                        o.credit_ready.push((t + if row.fast { FAST } else { AND + DROPPER + CMP }, p.batch));
                        o.dropper_edges.push((t + lat, t + lat + PRESS_W));
                        if row.fast {
                            if_set.push(t + FAST);
                        } else if row.bypass {
                            if_set.push(t + AND + IN_FLIGHT_SET);
                        }
                        if def.and_rearm {
                            jm1_set.push((t + AND + SWITCH, t + AND + SWITCH + PRESS_W));
                        }
                    }
                    if !def.single_latch {
                        s3_rst.push((t + RESET, t + RESET + PRESS_W));
                    }
                }
            }
        }
        later.extend(emulated);
        presses = later;
    }
    for &(ready, b) in &o.credit_ready {
        if b != usize::MAX && b + 2 < o.rels.len() {
            let lead = (o.rels[b + 2] - CMP) - ready;
            o.credit_lead_min = Some(o.credit_lead_min.map_or(lead, |m: i64| m.min(lead)));
        }
    }
    // (against the measured timing: the credit readable at REL_b + 186 + its latency vs REL_{b+2}'s strings ready at
    // REL_b + 195)
    if let Some(&l) = o.credit_latency.iter().max() {
        o.measured_margin = Some(STRINGS_READY - (D7_MEASURED + l));
    }
    for ps in &o.presses {
        for &(t, lane, own) in ps {
            if own.is_none() {
                continue;
            }
            let l = match lane {
                -1 => 0,
                3 => 1,
                _ => continue,
            };
            for &(a, b, kl) in &k_windows {
                if kl == l {
                    let d = if t < a { a - t } else if t > b { t - b } else { 0 };
                    o.mask_margin = Some(o.mask_margin.map_or(d, |m: i64| m.min(d)));
                }
            }
        }
    }
    // the ring reset's margins per batch (the reset of batch b's REL against batch b's slot 1 and batch b-1's slot 3)
    for (b, &x) in resets.iter().enumerate() {
        if b >= o.pass.len() {
            break;
        }
        let s1 = o.pass[b][0][0];
        let m1 = if s1 > 0 { Some(s1 - (x + SWITCH)) } else { None };
        let (m2, m3) = if b > 0 {
            let p3 = o.pass[b - 1][2];
            (if p3[1] > 0 { Some((x + RETURN) - p3[1]) } else { None }, if p3[0] > 0 { Some((x + SWITCH) - p3[0]) } else { None })
        } else {
            (None, None)
        };
        o.reset_margins.push((m1, m2, m3));
    }
    o
}

fn periods(o: &Out) -> Vec<i64> {
    o.rels.windows(2).map(|w| w[1] - w[0]).collect()
}

/// A batch in phase: every present slot entered its own lane (a picked slot passes through, not counted).
fn in_phase(o: &Out, row: &Row, b: usize) -> bool {
    (0..3).all(|s| {
        if row.missing.contains(&(b, s)) || row.pick.contains(&(b, s)) {
            return true;
        }
        o.entered.get(b).and_then(|e| e[s]) == Some(LANES[s])
    })
}

fn phase_line(o: &Out, row: &Row) -> String {
    (0..o.entered.len().min(14))
        .map(|b| {
            let e = o.entered[b];
            if in_phase(o, row, b) {
                "ok".to_string()
            } else {
                format!("[{}]", e.iter().map(|x| x.map_or("-".into(), |l| l.to_string())).collect::<Vec<_>>().join(","))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe(o: &Out, row: &Row) -> String {
    let p = periods(o);
    let no_transfer = o.at_rel.iter().filter(|r| r.2.is_none()).count();
    let slips: Vec<i64> = o.slips.iter().copied().filter(|&s| s > 0).collect();
    format!(
        "{} RELs, periods {}, slips {} ({} gt), the ring's phase per batch {}, the hopper {}..{} items (per REL read -> after [transfer] / the dropper / the total: {}), {} REL(s) with NO transfer, the dropper >= {}, the item total {}, credits {}, dry {}, misroutes {}, picked carts diverted {}, napper effects {}",
        o.rels.len(),
        if p.is_empty() { "-".into() } else { format!("{}..{}", p.iter().min().unwrap(), p.iter().max().unwrap()) },
        slips.len(),
        slips.iter().sum::<i64>(),
        phase_line(o, row),
        o.items_min,
        o.items_max,
        o.at_rel.iter().zip(o.stock_at_rel.iter()).take(3).map(|((a, b, x), (d, tot))| format!("{a}->{b}[{}]/{d}/{tot}", x.map_or("none".into(), |x| x.to_string()))).collect::<Vec<_>>().join(" "),
        no_transfer,
        o.dropper_min,
        if o.total_changes.is_empty() { "constant".to_string() } else { format!("CHANGED at {:?}", o.total_changes) },
        o.credits.len(),
        o.dry.len(),
        if o.misroutes.is_empty() { "none".into() } else { format!("{} ({})", o.misroutes.len(), o.misroutes.iter().take(2).cloned().collect::<Vec<_>>().join("; ")) },
        if o.pick_diverted.is_empty() { "none".into() } else { o.pick_diverted.join("; ") },
        if o.napper_effects.is_empty() { "none".into() } else { o.napper_effects.join("; ") }
    )
}

/// The store's bars (every row): the hopper <= 3 items, >= 2 at every REL's read, one transfer per REL, the total
/// constant, the dropper never dry.
fn store_bars(o: &Out) -> Vec<String> {
    let mut bad = Vec::new();
    if o.items_max > HOPPER0 {
        bad.push(format!("the hopper reached {} items (credits > 2)", o.items_max));
    }
    if o.at_rel.iter().zip(o.rel_inflight.iter()).any(|(r, &inf)| r.0 < 2 && !inf) {
        bad.push("a REL read fewer than 2 items".into());
    }
    if o.at_rel.iter().any(|r| r.2.is_none()) {
        bad.push(format!("{} REL(s) moved no item", o.at_rel.iter().filter(|r| r.2.is_none()).count()));
    }
    if !o.total_changes.is_empty() {
        bad.push(format!("the item total changed at {:?}", o.total_changes));
    }
    if !o.dry.is_empty() {
        bad.push(format!("credits fired dry at {:?}", o.dry));
    }
    bad
}

/// The design's bars on an unfaulted run: the store, P = 100, every batch in phase, no misroute, no picked cart
/// diverted, every napper press neutral.
fn design_bars(o: &Out, row: &Row, n_min: usize) -> Vec<String> {
    let mut bad = store_bars(o);
    if o.rels.len() < n_min {
        bad.push(format!("only {} RELs", o.rels.len()));
    }
    if periods(o).iter().any(|&p| p != PERIOD) {
        bad.push(format!("periods {:?}", periods(o)));
    }
    if !o.misroutes.is_empty() {
        bad.push(format!("misroutes: {}", o.misroutes.join("; ")));
    }
    if (0..o.entered.len().saturating_sub(2)).any(|b| !in_phase(o, row, b)) {
        bad.push(format!("the ring out of phase: {}", phase_line(o, row)));
    }
    if !o.pick_diverted.is_empty() {
        bad.push(format!("picked carts diverted: {}", o.pick_diverted.join("; ")));
    }
    if !o.napper_effects.is_empty() {
        bad.push(format!("napper presses not state-neutral: {}", o.napper_effects.join("; ")));
    }
    bad
}

/// The fault bars (ironspec 19:23 (3), G1 19:27 (c)): after a fault in batch `bf`, every batch from bf + 2 on is in its
/// SAME pre-fault phase (each slot into its own lane: another working phase fails by name), at least 5 of them; the
/// cadence back to P = 100 from REL_{bf+2}; no misroute after the recovery; the store's bars throughout.
fn recovery_bars(o: &Out, row: &Row, bf: usize) -> Vec<String> {
    let mut bad = store_bars(o);
    let from = bf + 2;
    let judged: Vec<usize> = (from..o.entered.len().saturating_sub(2)).collect();
    if judged.len() < 5 {
        bad.push(format!("only {} batches after the recovery point (REL count {})", judged.len(), o.rels.len()));
    }
    let out: Vec<usize> = judged.iter().copied().filter(|&b| !in_phase(o, row, b)).collect();
    if !out.is_empty() {
        bad.push(format!("THE RING DOES NOT RETURN TO ITS PHASE within 2 batches (out of phase at batches {out:?}: {})", phase_line(o, row)));
    }
    if o.rels.len() > from + 1 && o.rels[from..].windows(2).any(|w| w[1] - w[0] != PERIOD) {
        bad.push(format!("the cadence not back to P = 100 from REL_{from}: periods {:?}", periods(o)));
    }
    let late_mis: Vec<&String> = o.misroutes.iter().filter(|m| m.split_whitespace().nth(1).and_then(|x| x.trim_end_matches("'s").parse::<usize>().ok()).is_some_and(|b| b >= from)).collect();
    if !late_mis.is_empty() {
        bad.push(format!("misroutes after the recovery: {late_mis:?}"));
    }
    // (ironspec 19:56 (2) / 20:17 (A): NO ROW GAINS A CREDIT: the credits made never exceed the batches whose three
    // batch carts all pressed a D (a gain FAILS BY NAME; a lost one on a truly missing cart is a fault outcome))
    let completed = o.presses.iter().filter(|ps| ps.iter().filter(|p| p.2.is_some()).count() >= 3).count();
    let made = o.credits.iter().filter(|c| c.1 != usize::MAX).count();
    if made > completed {
        bad.push(format!("A CREDIT GAINED: {made} credits for {completed} completed batches"));
    }
    bad
}

/// The documented compression fault (ironspec 20:17 (B)(ii)): a cart moved between sidings, printed with its note.
fn moved_note(o: &Out) -> String {
    if o.lane_delta == [0; 3] {
        "no cart moved between sidings".into()
    } else {
        format!("THE DOCUMENTED COMPRESSION FAULT: a cart moved between sidings, the lanes' net carts (-1, 3, 7) {:?} (repair: move the extra cart, with its villager, back to the lane that is one short)", o.lane_delta)
    }
}

fn line(v: &mut Verdict, name: &str, bad: &[String], o: &Out, row: &Row, lines: &mut Vec<String>) {
    let w = v.bar(&format!("ctl-credit: {name}"), bad.is_empty());
    lines.push(format!("  {name}: {}: {w}{}", describe(o, row), if bad.is_empty() { String::new() } else { format!(" ({})", bad.join("; ")) }));
}

fn control(v: &mut Verdict, name: &str, bites: bool, o: &Out, row: &Row, lines: &mut Vec<String>) {
    let c = v.control(&format!("ctl-credit CONTROL: {name}"), 1, bites);
    lines.push(format!("  CONTROL ({name}): {}: {c}", describe(o, row)));
}

fn margins_line(o: &Out) -> String {
    let f = |sel: &dyn Fn(&(Option<i64>, Option<i64>, Option<i64>)) -> Option<i64>| {
        let v: Vec<i64> = o.reset_margins.iter().filter_map(sel).collect();
        if v.is_empty() { "-".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) }
    };
    format!("J(-1)'s branch ahead of this batch's slot 1 {}; J(3)'s return behind the previous slot 3 at J(3) {}; J(-1) armed behind the previous slot 3 at J(-1) {}", f(&|m| m.0), f(&|m| m.1), f(&|m| m.2))
}

pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec![format!(
        "ctl-credit (LOGIC LEVEL: designed delays; gate list rev 20:00: G8 as (B') with the ballast, G10's REL-keyed ring reset at REL + {RING_X}, G11's picks): SEEN3 set by D(3) (masked in K(3)), reset by its batch's D(7); a credit per D(7)'s edge AND SEEN3 (the dropper beside the counting hopper), one transfer per REL back into the dropper; RE_ARMED = >= 2 items (the four dust: 0 / 2 / 5 for 1 / 2 / 3 items); power-up {HOPPER0} items + {DROPPER0} in the dropper; the ring reset ({RESET_W}-gt pulse) forces J(-1) armed and J(3) disarmed; D(-1) arms J(3) (masked in K(-1)); the AND -> J(-1) link dropped. Delays: the edge + AND {AND}, the dropper {DROPPER}, the comparator {CMP}, the unlock REL + {UNLOCK} for {UNLOCK_W} gt, the cooldowns insert {INSERT_COOLDOWN} (an empty hopper) / transfer {TRANSFER_COOLDOWN}, the latch set {SET} / reset {RESET}, a D press -> its J {SWITCH} / {RETURN}, the pulse -> REL {REL_LAT}, B {PERIOD}."
    )];
    lines.push(format!("  the read: the four dust after the comparator give {} / {} / {} for 1 / 2 / 3 items (RE_ARMED = the fourth dust lit = >= 2 items)", four_dust(1), four_dust(2), four_dust(3)));
    let w = v.bar("ctl-credit: the four-dust read separates 1 item (0) from 2 (lit) and 3 (lit)", four_dust(1) == 0 && four_dust(2) >= 1 && four_dust(3) >= 1);
    lines.push(format!("    planted reads at 1 / 2 / 3 items: {w}"));
    // (1) the normal run with returning nappers in every lane and outbound picks of every lane
    let mut row = Row::normal("normal");
    row.napper = vec![(2, 0, 0), (4, 1, 0), (6, 2, 0), (8, 0, 20), (10, 1, 20), (12, 2, 20)];
    row.pick = vec![(3, 0), (7, 1), (11, 2), (15, 0), (19, 1), (23, 2)];
    let o = run(&row, Defects::default());
    let b = design_bars(&o, &row, 25);
    line(v, "the normal run (25+ batches; returning nappers in each lane at R6 10 and 30 behind slot 3; outbound picks of each lane twice): P = 100, every batch in phase, the store's bars, no picked cart diverted, every napper press neutral", &b, &o, &row, &mut lines);
    lines.push(format!("    the read at the first REL: {:?} (items, the fourth dust); the credit readable at D(7) + {} (the edge + AND {AND}, the dropper {DROPPER}, the comparator {CMP}); the least lead of batch b's credit over REL_(b+2)'s read {:?} gt (deterministic here: minus D(7)'s measured spread on the composed nights); the ring reset's margins per batch: {}; lane -1's emulated D(-1) behind the reset pulse's end {:?} gt (>= 1: no set-priority fight); the least distance of a batch D(-1) / D(3) from a K window {:?}", o.first_read, o.credit_latency.first().copied().unwrap_or(0), o.credit_lead_min, margins_line(&o), o.order_margin, o.mask_margin));
    let w = v.bar("ctl-credit: the ring reset's normal margins (J(-1)'s branch ahead of slot 1, J(3)'s return and J(-1)'s arming behind the previous slot 3) each >= 10 before the spread; lane -1's emulated D(-1) after the reset pulse", o.reset_margins.iter().all(|m| m.0.is_none_or(|x| x >= 10) && m.1.is_none_or(|x| x >= 10) && m.2.is_none_or(|x| x >= 10)) && o.order_margin.iter().all(|&m| m >= 1));
    lines.push(format!("    the reset's margins and the ordering: {w}"));
    // (the D(-1) completion candidate on the normal run: it must never fire in normal operation)
    let mut rd = row.clone();
    rd.credit_dm1 = true;
    let od = run(&rd, Defects::default());
    let bd = design_bars(&od, &rd, 25);
    let w = v.bar("ctl-credit: the D(-1) completion candidate on the normal run: never fires (0 credits from D(-1)), every design bar kept", bd.is_empty() && od.dm1_credits == 0);
    lines.push(format!("  the D(-1) completion candidate on the normal run: {} credits from D(-1){}: {w}", od.dm1_credits, if bd.is_empty() { String::new() } else { format!(" ({})", bd.join("; ")) }));
    // (X, ironspec 19:56 (4): the least of the reset's margins and the lane -1 pick's, per X, with and without the
    // pick-dependent reset)
    let mut xs = Vec::new();
    for x in [70, 72, 75, 76, 78, 80] {
        for dep in [false, true] {
            let mut rx = row.clone();
            rx.ring_x = x;
            rx.pick_dep = dep;
            let ox = run(&rx, Defects::default());
            let mut all: Vec<i64> = ox.reset_margins.iter().flat_map(|m| [m.0, m.1, m.2]).flatten().collect();
            all.extend(ox.pick_margin.iter().copied());
            let least = all.iter().min().copied();
            xs.push(format!("X {x}{}: least {:?}{}", if dep { " (pick-dependent)" } else { "" }, least, if design_bars(&ox, &rx, 25).is_empty() { "" } else { " (design bars FAIL)" }));
        }
    }
    lines.push(format!("  the reset's tap X: the least of J(-1)'s branch ahead of slot 1, J(3)'s return and J(-1)'s arming behind the previous slot 3, and (emulated form) the picked slot 1's arrival after J(-1)'s turn-off: {}", xs.join("; ")));
    // (the fast credit path against the measured timing, ironspec 20:26: margin >= 5 before REL_{b+2}'s strings ready;
    // the slow path (the edge comparator + the AND into the dropper, read from the hopper) as its control; the single
    // REL slip cap 29 gt (ironspec 20:24) and the night's total on the normal run)
    let w = v.bar("ctl-credit: the fast credit path's margin against the measured timing (the credit readable at REL_b + 186 + its latency before REL_{b+2}'s strings ready at REL_b + 195) >= 5", o.measured_margin.is_some_and(|m| m >= 5));
    lines.push(format!("  THE FAST CREDIT PATH (ironspec 20:26; one subtract-mode comparator folding D(7)'s edge and SEEN3, the pulse into the dropper and IN_FLIGHT, RE_ARMED = the read OR IN_FLIGHT OR the pulse): readable at D(7) + {FAST}: against the measured strings-ready (REL_b + {STRINGS_READY}, D(7) at + {D7_MEASURED}) margin {:?}: {w}", o.measured_margin));
    let mut rs = row.clone();
    rs.fast = false;
    let os = run(&rs, Defects::default());
    let c = v.control("ctl-credit CONTROL: the slow credit path (the edge comparator + the AND into the dropper, readable only from the hopper's read at D(7) + 10): against the measured strings-ready the margin is negative (every REL waits, compounding)", 1, os.measured_margin.is_some_and(|m| m < 0)).to_string();
    lines.push(format!("  CONTROL (the slow credit path: margin {:?} against the measured strings-ready): {c}", os.measured_margin));
    let max_slip = o.slips.iter().copied().max().unwrap_or(0);
    let total_slip: i64 = o.slips.iter().sum();
    let w = v.bar("ctl-credit: the normal run's REL slips: no single slip over the cap 29 gt (ironspec 20:24: V1's least gap 40 - 11), the night's total within the provisional 200", max_slip <= 29 && total_slip <= 200);
    lines.push(format!("  the normal run's REL slips: the largest single slip {max_slip} gt (cap 29), the total {total_slip} gt (bar 200 provisional): {w}"));
    let mut rb = row.clone();
    rb.fast = false;
    rb.bypass = true;
    let ob = run(&rb, Defects::default());
    lines.push(format!("  the in-flight bypass alone on the slow path (a flag, superseded by the fast path): margin against the measured strings-ready {:?}; design bars {}", ob.measured_margin.map(|_| STRINGS_READY - (D7_MEASURED + AND + IN_FLIGHT_SET)), if design_bars(&ob, &rb, 25).is_empty() { "pass" } else { "FAIL" }));
    // (2) single faults in batch 5, each judged on the same-phase recovery within 2 batches
    let bf = 5usize;
    let mut fault = |name: &str, r: &Row, lines: &mut Vec<String>| -> Out {
        // (the old form, the credit on D(7) AND SEEN3 alone, printed; the design, with the D(-1) completion
        // (ironspec 20:17 (A)), judged; a cart moved between sidings is the documented compression fault, printed)
        let mut r0 = r.clone();
        r0.credit_dm1 = false;
        let o0 = run(&r0, Defects::default());
        let b0 = recovery_bars(&o0, &r0, bf);
        lines.push(format!("  FINDING, the old form (the credit on D(7) alone), {name}: {}", if b0.is_empty() { "recovers".to_string() } else { format!("FAILS ({})", b0.join("; ")) }));
        let o = run(r, Defects::default());
        let bad = recovery_bars(&o, r, bf);
        line(v, &format!("FAULT ROW {name}: the ring back in its SAME phase within 2 batches, P = 100 from REL_7, no misroute after, no credit gained, the store's bars ({})", moved_note(&o)), &bad, &o, r, lines);
        o
    };
    for s in 0..3 {
        for late in (0..=60).step_by(10) {
            let mut r = Row::normal("late");
            r.late = vec![(bf, s, late)];
            fault(&format!("batch 5's lane {} cart {late} gt late", LANES[s]), &r, &mut lines);
        }
    }
    for s in 0..3 {
        let mut r = Row::normal("missing");
        r.missing = vec![(bf, s)];
        fault(&format!("batch 5's lane {} cart missing", LANES[s]), &r, &mut lines);
    }
    for lane in [-1, 3, 7] {
        let mut r = Row::normal("stray");
        r.stray = vec![(560, lane)];
        fault(&format!("a stray D({lane}) press at t 560 (inside batch 4's presses)"), &r, &mut lines);
    }
    // (the tolerances, per slot, lateness 0..60 at 1 gt: where a cart moves between sidings (the compression misroute:
    // a cart reaching a J inside the switching time of the cart ahead, or slot 3 reaching J(-1) after the next ring
    // reset armed it), and where the ruled form (the credit on D(7) alone) loses a credit for good)
    let ranges = |xs: &[i64]| -> String {
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        while i < xs.len() {
            let mut j = i;
            while j + 1 < xs.len() && xs[j + 1] == xs[j] + 1 {
                j += 1;
            }
            out.push(if i == j { xs[i].to_string() } else { format!("{}..{}", xs[i], xs[j]) });
            i = j + 1;
        }
        if out.is_empty() { "none".into() } else { out.join(", ") }
    };
    for s in 0..3 {
        let (mut moved, mut lost, mut lost_c) = (Vec::new(), Vec::new(), Vec::new());
        for late in 0..=60 {
            let mut r = Row::normal("tol");
            r.late = vec![(bf, s, late)];
            r.credit_dm1 = false;
            let o0 = run(&r, Defects::default());
            if o0.rels.len() > 9 && o0.rels[7..].windows(2).any(|w| w[1] - w[0] != PERIOD) {
                lost.push(late);
            }
            r.credit_dm1 = true;
            let o = run(&r, Defects::default());
            if o.lane_delta != [0; 3] {
                moved.push(late);
            }
            if o.rels.len() > 9 && o.rels[7..].windows(2).any(|w| w[1] - w[0] != PERIOD) {
                lost_c.push(late);
            }
        }
        lines.push(format!("  TOLERANCE, batch 5's lane {} cart late 0..60 gt (X {RING_X}): a cart moves between sidings (the documented compression fault) at {}; the old form (the credit on D(7) alone) loses a credit for good at {}; the design (the D(-1) completion) at {}", LANES[s], ranges(&moved), ranges(&lost), ranges(&lost_c)));
    }
    // (ironspec 20:17 (D): the delayed returning nappers: a lane-3 or lane-7 napper behind a late slot 3 (0..30 gt),
    // each still routed to its own lane by K's precedence (J(y) = (ARMED_y AND NOT HOLD_y AND NOT OFF14_y) OR K(y)), and
    // batch b+1's slot 1 printed: J(-1) branched in time, or the damage inside the two-batch bound)
    for lane in [1usize, 2] {
        let mut out = Vec::new();
        let mut all_ok = true;
        for late in (0..=30).step_by(5) {
            let mut r = Row::normal("delayed napper");
            r.late = vec![(bf, 2, late)];
            r.napper = vec![(bf, lane, late)];
            let o = run(&r, Defects::default());
            let bad = recovery_bars(&o, &r, bf);
            let s1 = o.entered.get(bf + 1).and_then(|e| e[0]);
            // (inside slot 3's tolerance (no misroute) the napper's presses must be neutral; past it, a napper press
            // completing the faulted next batch early is part of the accepted two-batch damage, printed)
            all_ok &= bad.is_empty() && (!o.misroutes.is_empty() || o.napper_effects.is_empty());
            out.push(format!("{late}: batch 6's slot 1 into lane {}{}{}", s1.map_or("-".into(), |l| l.to_string()), if bad.is_empty() { String::new() } else { format!(" ({})", bad.join("; ")) }, if o.napper_effects.is_empty() { String::new() } else { format!(" [napper effects: {}]", o.napper_effects.join("; ")) }));
        }
        let w = v.bar(&format!("ctl-credit: a lane {} napper behind batch 5's late slot 3 (both 0..30 gt late): routed to its own lane (K's precedence), batch 6's slot 1 printed, the ring and the cadence back within 2 batches, every napper press neutral", LANES[lane]), all_ok);
        lines.push(format!("  the delayed lane {} napper (late 0..30 with its slot 3): {}: {w}", LANES[lane], out.join("; ")));
    }
    // (the world open mid-batch: every tick of batch 5's span, idle 1 / 2, the carts 1 / 3 behind)
    let mut n_open = 0usize;
    let mut bad_open = Vec::new();
    let mut cool_kept = true;
    for (idle, kj) in [(1, 1), (2, 1), (1, 3), (2, 3)] {
        for t0 in 500..720 {
            let mut r = Row::normal("open");
            r.open = Some((t0, idle, kj));
            let o = run(&r, Defects::default());
            n_open += 1;
            if o.open_items.is_some_and(|(a, b)| a != b) || o.open_cooldown.is_some_and(|(a, b)| b < a - 0 && a > 0 && b + idle < a) {
                cool_kept = false;
            }
            let bad = recovery_bars(&o, &r, bf);
            if !bad.is_empty() && bad_open.len() < 2 {
                bad_open.push(format!("open at {t0} idle {idle} kj {kj}: {}; {}", bad.join("; "), describe(&o, &r)));
            }
        }
    }
    let w = v.bar_n("ctl-credit: FAULT ROW a world open at every tick of batch 5's span (t 500..719; idle 1 / 2, the carts 1 / 3 behind the redstone): the ring in its same phase within 2 batches, P = 100, the store's bars, the hopper's items and cooldown kept over the open", n_open, bad_open.is_empty() && cool_kept);
    lines.push(format!("  FAULT ROW a world open mid-batch, {n_open} runs: {w}{}", if bad_open.is_empty() { String::new() } else { format!(" ({})", bad_open.join(" | ")) }));
    // (the negative: the old ring, J(-1) re-armed by the AND with no reset, locks rotated: it must FAIL BY NAME)
    let mut r = Row::normal("old ring");
    r.late = vec![(bf, 2, 30)];
    let oc = run(&r, Defects { and_rearm: true, ..Default::default() });
    let bites = recovery_bars(&oc, &r, bf).iter().any(|b| b.contains("DOES NOT RETURN"));
    control(v, "the old ring (J(-1) re-armed by G8's AND, no REL-keyed reset), batch 5's lane-7 cart 30 gt late: the ring never returns to its phase (the rotation lock, day-1 19:21) and the recovery bar fails BY NAME", bites, &oc, &r, &mut lines);
    let mut r = Row::normal("old ring late 7 at 15");
    r.late = vec![(bf, 2, 15)];
    let oc = run(&r, Defects { and_rearm: true, ..Default::default() });
    let od = run(&r, Defects::default());
    let bites = !recovery_bars(&oc, &r, bf).is_empty() && recovery_bars(&od, &r, bf).is_empty();
    control(v, "the old ring, batch 5's lane-7 cart only 15 gt late (inside the reset's tolerance): the old ring rotates and loses the credit, the reset form recovers clean", bites, &oc, &r, &mut lines);
    // (3) G11's controls
    let mut r = Row::normal("order");
    r.pick = vec![(bf, 0)];
    let o = run(&r, Defects::default());
    line(v, "a lane -1 pick at batch 5: the picked cart passes J(-1) (the emulated D(-1) after the reset pulse), J(3) and J(7) (HOLD_3 / HOLD_7); the evidence emulated; the cadence kept", &design_bars(&o, &r, 25), &o, &r, &mut lines);
    let oc = run(&r, Defects { reset_order: true, ..Default::default() });
    control(v, "lane -1's emulated D(-1) at X - 2, its drive on J(-1) coinciding with the reset pulse: the set-priority latch keeps J(-1) armed and the picked cart diverts into lane -1", !oc.pick_diverted.is_empty(), &oc, &r, &mut lines);
    let oc = run(&r, Defects { no_hold3: true, ..Default::default() });
    control(v, "HOLD_3 removed on the lane -1 pick: the picked cart diverts into lane 3 at J(3) (armed by the emulated D(-1))", !oc.pick_diverted.is_empty(), &oc, &r, &mut lines);
    // (4) the masks
    let mut r = Row::normal("km1");
    r.napper = vec![(5, 0, 0)];
    let o = run(&r, Defects::default());
    line(v, "batch 5's lane -1 napper's D(-1): K(-1)'s mask keeps J(3) unarmed (the press state-neutral)", &design_bars(&o, &r, 25), &o, &r, &mut lines);
    let oc = run(&r, Defects { no_km1_mask: true, ..Default::default() });
    control(v, "K(-1)'s mask removed: the napper's D(-1) arms J(3) (not state-neutral)", !oc.napper_effects.is_empty(), &oc, &r, &mut lines);
    let mut r = Row::normal("k3");
    r.napper = vec![(5, 1, 20)];
    let o = run(&r, Defects::default());
    line(v, "batch 5's lane-3 napper 30 behind slot 3 (its D(3) after batch 5's D(7)): K(3)'s mask keeps SEEN3 reset (neutral)", &design_bars(&o, &r, 25), &o, &r, &mut lines);
    let oc = run(&r, Defects { no_k3_mask: true, ..Default::default() });
    control(v, "K(3)'s mask removed: the napper's D(3) sets SEEN3 for batch 6 (not state-neutral)", !oc.napper_effects.is_empty(), &oc, &r, &mut lines);
    // (5) the store
    for d in [90, 150, 300] {
        let mut r = Row::normal("delay");
        r.holds_empty = vec![(700, 700 + d)];
        let o = run(&r, Defects::default());
        let mut bad = store_bars(&o);
        let after: Vec<i64> = o.rels.iter().copied().filter(|&x| x >= 700 + d).collect();
        if after.len() < 6 || after.windows(2).skip(2).any(|w| w[1] - w[0] != PERIOD) {
            bad.push(format!("the cadence not back to P = 100 within 2 batches: periods {:?}", periods(&o)));
        }
        line(v, &format!("a REL delayed {d} gt (the holds not full over t 700..{}): the cadence back to P = 100 within 2 batches, the store's bars", 700 + d), &bad, &o, &r, &mut lines);
        if d == 150 {
            let oc = run(&r, Defects { single_latch: true, ..Default::default() });
            let after: Vec<i64> = oc.rels.iter().copied().filter(|&x| x >= 850).collect();
            control(v, "G8 as (A) (one latch reset at every REL), a REL delayed 150: the farm locks at half rate", after.windows(2).skip(2).any(|w| w[1] - w[0] != PERIOD), &oc, &r, &mut lines);
        }
    }
    let mut r = Row::normal("no ballast");
    r.hopper0 = 2;
    r.late = vec![(5, 2, 20)];
    let oc = run(&r, Defects { no_ballast: true, ..Default::default() });
    let od = run(&Row { hopper0: HOPPER0, ..r.clone() }, Defects::default());
    control(v, "no ballast (2 items, RE_ARMED = >= 1 item), batch 5's lane-7 cart 20 late: REL fires on the in-flight credit with the hopper EMPTY, the credit lands in an empty hopper (its insert cooldown 7) inside the unlock window, and that REL moves no item (the count gains one); with the ballast the same row keeps every store bar", od.at_rel.iter().all(|r| r.2.is_some()) && store_bars(&od).is_empty() && (oc.at_rel.iter().any(|r| r.2.is_none()) || !store_bars(&oc).is_empty()), &oc, &r, &mut lines);
    let mut r2 = Row::normal("p2");
    r2.hopper0 = 2;
    let o2 = run(&r2, Defects::default());
    control(v, "the power-up with 2 items (one credit): the farm runs one batch deep (periods > 100)", periods(&o2).iter().any(|&p| p > PERIOD), &o2, &r2, &mut lines);
    let mut r4 = Row::normal("p4");
    r4.hopper0 = 4;
    let o4 = run(&r4, Defects::default());
    control(v, "the power-up with 4 items (three credits): the hopper holds 4 (credits > 2 FAILS by name)", o4.items_max > HOPPER0, &o4, &r4, &mut lines);
    lines.push("  OWED: the cells on the station's placement with their measured delays (the dropper, the hopper's unlock, the comparator and its four dust, SEEN3, the masks, the ring reset's tap off the R line); D(7)'s normal spread from cc767b78's composed nights (every margin here is before it); the coverage slack row (the least planted total REL slip at which 24 / 24 first breaks: the credit-slip bar's final figure is half of it)".into());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_normal_run_keeps_the_cadence_and_the_phase() {
        let r = Row::normal("t");
        let o = run(&r, Defects::default());
        assert!(design_bars(&o, &r, 25).is_empty(), "{}", describe(&o, &r));
    }

    #[test]
    fn a_lane_7_cart_inside_the_resets_tolerance_recovers_within_two_batches() {
        let mut r = Row::normal("t");
        r.late = vec![(5, 2, 30)];
        let o = run(&r, Defects::default());
        assert!(recovery_bars(&o, &r, 5).is_empty(), "{:?} {}", recovery_bars(&o, &r, 5), describe(&o, &r));
    }

    #[test]
    fn the_d_minus_1_completion_keeps_the_cadence_after_a_misroute() {
        let mut r = Row::normal("t");
        r.late = vec![(5, 2, 40)];
        r.credit_dm1 = true;
        let o = run(&r, Defects::default());
        assert!(o.rels.len() > 9 && o.rels[7..].windows(2).all(|w| w[1] - w[0] == PERIOD), "{}", describe(&o, &r));
    }

    #[test]
    fn the_bench_prints() {
        let mut v = Verdict::new("ctl-credit-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
    }
}
