//! ctl-pick: the outbound PICK RULE at ring level (ironspec 22:41 / 22:49 / 23:06 (3) / 23:21 / 23:34 / 23:45 / 23:48 /
//! 23:57 / 00:01, G1 22:40 / 23:18 / 23:22 / 23:37 / 23:45 / 23:58): each lane's carts circulate FIFO, one per REL; a
//! pick sends the lane's REL cart (its villager) on the nap detour INSTEAD of its batch cart; the bar is on VILLAGERS:
//! every villager naps every night (24 distinct) and every nap-to-nap interval < 24,000 gt - margin, robust to a late
//! visit, a skipped pick, a missed REL, two consecutive late picks and the day length +-10 RELs.
//!
//! THE DESIGN (judged): THE POINTER FORM: every pick fires when the lane's front cart is at the pointer's ring position
//! AND >= P = 11 REAL RELs have passed since the lane's last pick AND the lane's HELD is clear; the pointer then advances
//! Delta = 5 ring positions (coprime with 7 / 8 / 9) and is HELD THROUGH THE DAY (each night continues last night's
//! sequence: oldest nap first, independent of day length); the order-preserving merge rejoins a napper behind its
//! predecessor at the FIRST predecessor entry after its return (the rejoin counted as evidence); THREE IN FLIGHT, ONE PER
//! LANE, the stop a single server (the bed: arrival pick + 200 gt, 333 gt asleep); HELD's spans as measured (264 / 284 /
//! 313 gt: [3, 3, 4] RELs). Printed beside: today's rule (the control), the positional rules, FRESH / NAPPED (the
//! fallback), the second-entry rejoin, today's one-in-flight contract.
//!
//! The model: time in RELs (100 gt each), a day 240 RELs, the window the first 120 (REST, ~12,010 gt, tick-exact 23:38);
//! a batch circuit 2 RELs; a napper's absence 9 RELs (lane 7 + 1, a late visit + 1: cc767b78 23:20's tap-quantized
//! absences); the pointer uses the cart's identity for the ring position the circuit's counter must track (the counter's
//! netlist is OWED: positions on the REL pulse, the absent register, the rejoin evidence, state in latches).

use crate::verdict::Verdict;
use std::collections::VecDeque;

pub const DAY: i64 = 240;
pub const CIRCUIT: i64 = 2;
pub const N0: usize = 8;
pub const MARGIN: i64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Merge {
    Back,
    Order,
    /// FRESH / NAPPED (ironspec 22:49 (3)): picks draw from FRESH; a returned napper parks in its lane's NAPPED queue;
    /// the batch draws from FRESH, from NAPPED only when FRESH is empty; every batch return goes to FRESH; dawn merges
    /// NAPPED behind FRESH. The mark lives in the SLOT (which queue a cart waits in).
    TwoTrack,
}

#[derive(Clone, Debug)]
pub struct Cfg {
    pub name: String,
    pub period: i64,
    pub window: i64,
    pub visit: i64,
    pub merge: Merge,
    pub repick: bool,
    /// TwoTrack's dawn merge puts NAPPED ahead of FRESH (the night's first nappers lead the next night).
    pub napped_first: bool,
    /// The order-preserving merge's rejoin: at the predecessor's entry number `rejoin_at` after the pick (1: the first
    /// chance; 2: ironspec 23:24 (3), always one rotation late).
    pub rejoin_at: i64,
    /// ironspec 23:34's lead (b): the PICK POINTER held through the day: the night's first pick waits for the cart Delta
    /// = P + 2 ring positions after the last picked one (oldest nap first falls out); then every P RELs.
    pub pointer: bool,
    /// Per day: the day's length in RELs relative to DAY (golem-limited RELs: +-10).
    pub day_jitter: Vec<i64>,
    /// The station's ONE-IN-FLIGHT contract (HELD): after any pick, no lane picks for this many RELs (pick -> queue
    /// departure ~320-360 gt: 4 RELs); a pointer pick due while HELD is DEFERRED (its target comes round again). 0: off.
    pub held: i64,
    /// HELD's capacity: how many picked nappers may be in flight at once (today's contract: 1).
    pub held_cap: usize,
    /// HELD's span per picking lane in RELs (cc767b78 23:59, r12's normal night: pick -> HELD clear 264 / 284 / 313 gt,
    /// so a pick 300 gt later passes after lanes -1 / 3 and is blocked after lane 7: [3, 3, 4]); None: `held` for all.
    pub held_lane: Option<[i64; 3]>,
    /// The pointer's step in ring positions (D = 5).
    pub ptr_step: usize,
    /// The ONE BED modelled as a single server (arrival pick + 200 gt, 333 gt asleep); a wait lengthens the absence.
    pub bed: bool,
    /// (day, window index, length): a multi-REL HALT (no REL for `length` slots; G1 23:44 / ironspec 23:50 (2)).
    pub halts: Vec<(i64, i64, i64)>,
    /// (day, lane, positions): the lane's position counter SLIPS by that many ring positions at the day's start (a
    /// counter miss: G1 23:37 (1) / ironspec 23:48 (2)(1)).
    pub slip: Vec<(i64, usize, usize)>,
    /// (day, lane, pick number in the night): that pick is a LATE visit (+ 1 REL); two consecutive ones plant G1's 23:26 (3).
    pub late_picks: Vec<(i64, usize, usize)>,
    pub days: i64,
    /// (day, window index) a REL that doesn't happen.
    pub missed: Vec<(i64, i64)>,
    /// (day, lane) that lane's first pick of the day is a LATE visit (+ 8 RELs).
    pub late: Vec<(i64, usize)>,
    /// (day, lane) that lane's first pick of the day fails (no nap; the cart rides a normal circuit).
    pub skipped: Vec<(i64, usize)>,
    /// (day, from lane, to lane) one cart moved between lanes at the day's start (the compression fault).
    pub moved: Vec<(i64, usize, usize)>,
    /// (day, lane, pick number in the night, delay gt, a gate-G SKIP): that napper's ride to the way-in leg is delayed
    /// (ironspec 01:05 / G1 00:57: a world open 3, the stack hold 26, a planted stall > 46, a gate-G skip of lane 7's
    /// intake 100); the FIFO releases in ARRIVAL order, so a delay over the 46-gt margin REORDERS arrivals.
    pub ride_delay: Vec<(i64, usize, usize, i64, Ride)>,
    /// ironspec 01:05 (2): on a gate-G skip of a napper-carrying lane-7 intake, lane 7's record entry moves behind the
    /// entries written at the next REL (+ ~10 blocks): the record stays in arrival order.
    pub skip_fix: bool,
    /// (day, lane, pick number): that napper is LOST between its pick and P1 (its cart broken by a player): its record
    /// entry is never popped by its own release (ironspec 01:05 (b): a desync the + 10 can't see).
    pub lost_napper: Vec<(i64, usize, usize)>,
    /// ironspec 01:05 (2)(b): THE RECORD's RESYNC: cleared on evidence that no napper is in flight (P1, the front and the
    /// stop empty, and >= 3 RELs since the last pick: a napper reaches P1 within 2 RELs of its pick).
    pub resync: bool,
    /// THE STOP STALLED (ironspec 06:23 / 06:31, G1 06:26: b8949c96's way-in overflow row): (gt start, length): no bed
    /// start inside the window (a token stall: the queue holds its nappers).
    pub stop_stall: Vec<(i64, i64)>,
    /// THE WAY-IN ROOM TERM (ironspec 06:31 (1)): a pick only while the nappers DISPATCHED and not yet departed from the
    /// front (a count by evidence: the pick edges up, the front's departure presses down) number fewer than this
    /// (the way-in's dry places, P1 + 2 = 3). 0: off.
    pub wayin_cap: usize,
    /// ironspec 06:28 (B): RS_READY's EVIDENCE MOVED TO THE BALL EXIT (7,-3,4): each visit's cycle grows by the previous
    /// rider's travel from the R string to the ball exit (gt).
    pub rs_extra: i64,
    /// G1 06:35 (a): the departure presses (their index over the run, 1-based) the room count MISSES (no down-count).
    pub missed_dep: Vec<usize>,
    /// G1 06:35 (b): EXTRA departure presses (gt): the room count goes down once more (clamped at >= 0).
    pub extra_dep: Vec<i64>,
}

impl Cfg {
    pub fn new(name: &str, period: i64, window: i64, merge: Merge) -> Cfg {
        Cfg { name: name.to_string(), period, window, visit: 9, merge, repick: true, napped_first: false, rejoin_at: 2, pointer: false, day_jitter: vec![], held: 0, held_cap: 1, held_lane: None, ptr_step: 5, bed: false, halts: vec![], slip: vec![], late_picks: vec![], days: 8, missed: vec![], late: vec![], skipped: vec![], moved: vec![], ride_delay: vec![], skip_fix: true, lost_napper: vec![], resync: true, stop_stall: vec![], wayin_cap: 0, rs_extra: 0, missed_dep: vec![], extra_dep: vec![] }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Out {
    /// Per villager: its nap ticks (gt).
    pub naps: Vec<Vec<i64>>,
    /// Per villager: the day of each nap.
    pub nap_day: Vec<Vec<i64>>,
    /// Per day: the distinct villagers napped.
    pub distinct: Vec<usize>,
    pub stalls: usize,
    /// The largest nap-to-nap interval (gt) from day 2 on, and its villager.
    pub max_gap: (i64, usize),
    /// Per lane: the measured s (the predecessor's pops between a pick and the napper's rejoin), as a set.
    pub s_seen: [Vec<i64>; 3],
    /// Per lane: the distinct villagers napped per night.
    pub lane_distinct: Vec<[usize; 3]>,
    /// Per lane: the least queue depth (carts waiting) at any REL (ironspec 23:24 (3)(d)).
    pub min_depth: [usize; 3],
    /// Per lane: the longest wait in R_l (RELs, from the napper's arrival to its rejoin).
    pub max_wait: [i64; 3],
    /// Per lane: the most carts away at once (picked, not yet rejoined).
    pub max_away: [usize; 3],
    /// Per night: the picks DEFERRED by HELD (due, but the station's one napper still in flight).
    pub deferred: Vec<usize>,
    /// Per lane: the latest first pick of a night (window RELs into REST: the first-pick wait).
    pub first_wait: [i64; 3],
    /// Per night: the picks made (all lanes).
    pub picks: Vec<usize>,
    /// The nights holding an interval >= 24,000 - MARGIN (the later nap's night).
    pub bad_nights: Vec<i64>,
    /// The longest wait at the bed (gt), with `bed`.
    pub max_bed_wait: i64,
    /// THE WAY-IN's OCCUPANCY: the most nappers at once between the leg (9,25,9) and their bed start (P1, the queue
    /// behind it, the front), and the most DISPATCHED and not yet departed (the room count's reading).
    pub max_wayin: usize,
    pub max_open: usize,
    /// Picks deferred by THE WAY-IN ROOM TERM.
    pub wayin_deferred: usize,
    /// THE ROOM COUNT (the hardware's reading: up per pick, down per departure press, clamped >= 0): its most, its
    /// resyncs (at the record's quiet point: the stop free, no pick for 3 RELs), and the RELs carrying two or more picks
    /// (gt, the count before, after).
    pub max_room: i64,
    pub room_resyncs: usize,
    pub multi_pick: Vec<(i64, i64, i64)>,
    /// The RELs at which the room term deferred a pick (gt).
    pub deferred_at: Vec<i64>,
    /// The most nappers WAITING for the bed at once (the queue depth, not in flight: ironspec 00:01), with `bed`.
    pub max_queue: usize,
    /// With `bed`: the longest wait on a lane's branch hold and at the front (gt), and the bed's busy fraction over the
    /// windows (per mille).
    pub max_hold_wait: i64,
    pub max_front_wait: i64,
    pub bed_busy_permille: i64,
    /// RELs whose batch carried two / three nappers (two or three lanes picking in the same REL: ironspec 01:00 (3)).
    pub rels_two: usize,
    pub rels_three: usize,
    /// THE 3-DEEP LANE RECORD (ironspec 01:05 (2)): written at each pick (pick order), its head popped at each napper's
    /// release into the stop (arrival order); each release where the head names another lane: (day, the napper's lane,
    /// the head's lane) — the napper is then sent home to the HEAD's lane (its slope tap and RETURN bit).
    pub record_mismatch: Vec<(i64, usize, usize)>,
    /// The largest arrival inversion (gt): an earlier pick reaching the front after a later one.
    pub max_inversion: i64,
    /// The most entries in the record at once (the 3-deep record's depth bound).
    pub record_depth: usize,
    /// The fewest RELs from a lane-7 pick to the next lane -1 / 3 pick (a gate-G skip (+100) reorders only when that
    /// next pick comes ONE REL later).
    pub min_after7: i64,
    /// Nappers lost between their pick and P1 (planted), and the record's resyncs (cleared with entries left).
    pub lost: usize,
    pub resyncs: usize,
    /// Each planted ride delay read back: (night, lane, pick, planted, measured leg delay, shared).
    pub delay_reads: Vec<(i64, usize, usize, i64, i64, bool)>,
}

/// Where a napper's ride is delayed (ironspec 01:05 / G1 00:57): carts never overtake on the shared track, so a delay
/// AFTER the merge (the lift's stack hold, a world open) holds every later cart of the same batch behind it; a gate-G
/// SKIP drops lane 7's cart behind the next batch's carts (the record fix applies); a stall BEFORE the merge (on the
/// lane's own path to it) delays that cart alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ride {
    Shared,
    GateGSkip,
    Own,
}

/// The lanes' offsets inside the period (interleaved picks, one bed).
const OFFSET: [i64; 3] = [0, 4, 7];
/// The lanes' x (for the prints).
const LANES: [i32; 3] = [-1, 3, 7];
/// ironfarm 00:57 (bench, the last stretch extrapolated): a napper riding its batch cart reaches the way-in leg
/// (9,25,9) at REL + 132 / 154 / 186 by lane (its release offset included).
const LEG: [i64; 3] = [132, 154, 186];
/// cc767b78 23:20 (r10, every night): the napper's ABSENCE is quantized to its lane's slope tap: 8.50 / 8.85 / 10.31 RELs
/// (lanes -1 / 3 / 7), + exactly 1 REL on a late visit; the model's visit is 9 RELs, lane 7 + 1, a late visit + 1.
const LANE_EXTRA: [i64; 3] = [0, 0, 1];

pub fn run(cfg: &Cfg) -> Out {
    let mut lanes: Vec<VecDeque<usize>> = (0..3).map(|l| (0..N0).map(|i| l * N0 + i).collect()).collect();
    let mut napped: Vec<VecDeque<usize>> = vec![VecDeque::new(); 3];
    let nv = 3 * N0;
    let mut o = Out { naps: vec![Vec::new(); nv], nap_day: vec![Vec::new(); nv], min_depth: [usize::MAX; 3], ..Out::default() };
    // (in transit: (back at REL, lane, cart, order-preserving predecessor if any))
    let mut transit: Vec<(i64, usize, usize, Option<usize>)> = Vec::new();
    // (waiting in R_l: (lane, cart, predecessor, arrival REL))
    let mut siding: Vec<(usize, usize, Option<usize>, i64)> = Vec::new();
    // (s's count: (lane, napper, predecessor, the predecessor's pops so far))
    let mut away: Vec<(usize, usize, usize, i64, i64)> = Vec::new();
    // (the ring's cyclic predecessor of each lane's first cart is its last one: the first napper rejoins in order too)
    let mut last_pop: [Option<usize>; 3] = [Some(N0 - 1), Some(2 * N0 - 1), Some(3 * N0 - 1)];
    let mut repick: [Option<usize>; 3] = [None; 3];
    let offs = [0, cfg.period / 3, (2 * cfg.period) / 3];
    let mut next_pick: [i64; 3] = offs;
    let mut last_idx: [Option<usize>; 3] = [None; 3];
    let mut target: [Option<usize>; 3] = [None; 3];
    let mut last_pick_k: [i64; 3] = [-cfg.period; 3];
    // (the spacing gate counts REAL RELs (pops) since the lane's last pick: a halt delays it (G1 23:44 (2)))
    let mut since: [i64; 3] = [cfg.period; 3];
    // (the picked nappers in flight: each one's HELD end (REL))
    let mut in_flight: Vec<i64> = Vec::new();
    let mut bed_free: i64 = i64::MIN;
    let mut bed_waits: Vec<(i64, i64)> = Vec::new();
    let mut prev_start: i64 = i64::MIN;
    let mut bed_busy: i64 = 0;
    // (the FIFO in ARRIVAL order (G1 00:50 / ironspec 00:58): a picked napper's bed start waits until no later pick can
    // reach the front before it: (leg arrival, lane, cart, pick REL t, visit v, predecessor, day))
    let mut pending: Vec<(i64, usize, usize, i64, i64, Option<usize>, i64)> = Vec::new();
    // (every napper's (leg arrival, bed start) once its start is fixed: the way-in's occupancy and the room count)
    let mut assigned: Vec<(i64, i64)> = Vec::new();
    // (THE ROOM COUNT as the hardware keeps it: up per pick edge, down per departure press (a bed start: the front's
    // departure over (9,-11,-3)), clamped >= 0, reset at the record's quiet point; `deps`: departures not yet counted)
    let mut room: i64 = 0;
    let mut deps: Vec<i64> = Vec::new();
    let mut dep_no = 0usize;
    // (THE 3-DEEP LANE RECORD, ironspec 01:05 (2): lanes written at each pick; with the skip fix a gate-G-skipped lane-7
    // entry is held and written behind the next REL's entries: (lane, the REL t it was picked at))
    let mut record: VecDeque<usize> = VecDeque::new();
    let mut rec_hold: Vec<(usize, i64)> = Vec::new();
    let mut max_leg: i64 = 0;
    // (a record mismatch sends the napper home to the head's lane; when the head's own napper is in flight, the two
    // trade places: each rides home on the other's slope tap into the other's lane, taking the RING POSITION its partner
    // left (the rejoin is counted on that lane's predecessor): positions are kept, the two VILLAGERS swap lanes (G1
    // 01:04 (e)); `who` maps a ring slot to the villager in it; the swap lands after the pair's second nap)
    let mut who: Vec<usize> = (0..nv).collect();
    let mut pair: Option<(usize, usize)> = None;
    let mut gone: Vec<bool> = vec![false; nv];
    let mut last_pick_t: i64 = i64::MIN / 2;
    let mut last7: Option<i64> = None;
    o.min_after7 = i64::MAX;
    // (the pointer's step in RING positions: a design constant, D = 5 (coprime with 7 / 8 / 9: ironspec 23:45 (2)),
    // independent of the rejoin rule)
    let delta = cfg.ptr_step;
    let mut gt: i64 = 0;
    let mut t: i64 = 0;
    for day in 0..cfg.days {
        for &(d, l, by) in &cfg.slip {
            if d == day {
                if let Some(tg) = target[l] {
                    target[l] = Some(l * N0 + (tg - l * N0 + by) % N0);
                }
            }
        }
        let mut deferred_today = 0usize;
        let mut picks_today = 0usize;
        let mut pick_no: [usize; 3] = [0; 3];
        for &(d, a, b) in &cfg.moved {
            if d == day {
                if let Some(c) = lanes[a].pop_back() {
                    lanes[b].push_back(c);
                }
            }
        }
        // (dawn, TwoTrack: NAPPED merges behind FRESH, or ahead of it with `napped_first`)
        for l in 0..3 {
            if cfg.napped_first {
                while let Some(c) = napped[l].pop_back() {
                    lanes[l].push_front(c);
                }
            } else {
                while let Some(c) = napped[l].pop_front() {
                    lanes[l].push_back(c);
                }
            }
        }
        let mut first: [bool; 3] = [true; 3];
        let mut k_rel = 0i64;
        let day_len = DAY + cfg.day_jitter.get(day as usize).copied().unwrap_or(0);
        for k in 0..day_len {
            gt += 100;
            t += 1;
            // (THE ROOM COUNT: the departure presses since the last REL count down (a planted miss does not), a planted
            // extra press counts down once more; clamped at >= 0)
            deps.sort();
            while deps.first().is_some_and(|&d| d <= gt) {
                deps.remove(0);
                dep_no += 1;
                if !cfg.missed_dep.contains(&dep_no) {
                    room -= 1;
                }
            }
            room -= cfg.extra_dep.iter().filter(|&&x| x > gt - 100 && x <= gt).count() as i64;
            room = room.max(0);
            // (its RESYNC on evidence at the record's quiet point: the stop free (the bed's last visit over, no napper
            // waiting) and no pick for 3 RELs: nothing is dispatched and not departed, so the count reads 0)
            if cfg.resync && pending.is_empty() && gt >= bed_free && t - last_pick_t >= 3 && room != 0 {
                o.room_resyncs += 1;
                room = 0;
            }
            // (THE RECORD's RESYNC, ironspec 01:05 (2)(b): no napper in flight (none pending, the stop free: the last
            // napper's visit over) and >= 3 RELs since the last pick: the record must be empty; any entry left is cleared)
            if cfg.resync && pending.is_empty() && gt >= bed_free && t - last_pick_t >= 3 && (!record.is_empty() || !rec_hold.is_empty()) {
                o.resyncs += 1;
                record.clear();
                rec_hold.clear();
            }
            // (the record: a held (gate-G-skipped) lane-7 entry is written once the next REL's picks are in)
            rec_hold.retain(|&(l, th)| {
                if th + 2 <= t {
                    record.push_back(l);
                    false
                } else {
                    true
                }
            });
            // (the FIFO, in ARRIVAL order: every pending napper that no pick from this REL on can overtake (the earliest
            // pick from now reaches the front at gt + 132 + 60) takes the bed in arrival order; the record's head names
            // the lane it is released for: its slope tap and RETURN bit send it HOME to that lane)
            loop {
                let Some(i) = (0..pending.len()).filter(|&i| pending[i].0 + 60 < gt + 192).min_by_key(|&i| (pending[i].0, pending[i].3)) else {
                    break;
                };
                let (leg, l, c, tp, mut v, pred, pday) = pending.remove(i);
                let home = match record.pop_front() {
                    Some(h) if h != l => {
                        o.record_mismatch.push((pday, l, h));
                        h
                    }
                    _ => l,
                };
                let arrive = leg + 60;
                // (the FIFO's gate: P1 (9,22,-3), reached 15 gt after the leg, released by the previous napper's departure
                // from the front (its bed start); the front 45 gt below P1)
                let release = (leg + 15).max(prev_start);
                let at_front = arrive.max(release + 45);
                let mut start = at_front.max(bed_free);
                // (THE STOP STALLED: no bed start inside a stall window)
                for &(s0, len) in &cfg.stop_stall {
                    if start >= s0 && start < s0 + len {
                        start = s0 + len;
                    }
                }
                assigned.push((leg, start));
                deps.push(start);
                o.max_hold_wait = o.max_hold_wait.max(release - (leg + 15));
                o.max_front_wait = o.max_front_wait.max(start - at_front);
                prev_start = start;
                bed_busy += 333;
                bed_free = start + 333 + cfg.rs_extra;
                let wait = start - arrive;
                o.max_bed_wait = o.max_bed_wait.max(wait);
                if wait > 0 {
                    bed_waits.push((arrive, start));
                }
                v += (wait + 99) / 100;
                // (the nap is the villager's in the cart now; a mismatch's pair swaps villagers after its second nap)
                o.naps[who[c]].push(start);
                o.nap_day[who[c]].push(pday);
                let mut send = l;
                if home != l {
                    match pair {
                        Some((a, b)) if b == c => {
                            who.swap(a, b);
                            pair = None;
                        }
                        _ => match pending.iter().find(|p| p.1 == home) {
                            Some(p) => pair = Some((c, p.2)),
                            // (no napper of the head's lane in flight: this one goes to that lane's back, the lanes'
                            // counts change)
                            None => send = home,
                        },
                    }
                }
                if send == l {
                    transit.push((tp + v, l, c, match cfg.merge {
                        Merge::Order => pred,
                        Merge::TwoTrack => Some(usize::MAX),
                        Merge::Back => None,
                    }));
                } else {
                    away.retain(|a| a.1 != c);
                    transit.push((tp + v, send, c, None));
                }
            }
            // returns: carts back from their circuit or detour (an order-preserving napper goes to its siding; a TwoTrack
            // napper to NAPPED); the ORDER-PRESERVING merge (ironspec 23:21 (2)): a waiting napper rejoins only AT its
            // predecessor's TAIL ENTRY (it enters right behind it, in the same move), and a chain behind it follows
            let mut keep = Vec::new();
            let mut arrived: Vec<(usize, usize)> = Vec::new();
            for &(at, l, c, pred) in &transit {
                if at <= t {
                    match (cfg.merge, pred) {
                        (Merge::Order, Some(p)) => siding.push((l, c, Some(p), t)),
                        (Merge::TwoTrack, Some(_)) => napped[l].push_back(c),
                        _ => {
                            lanes[l].push_back(c);
                            arrived.push((l, c));
                        }
                    }
                } else {
                    keep.push((at, l, c, pred));
                }
            }
            transit = keep;
            // (each tail entry of a predecessor counts for its waiting nappers; a napper rejoins behind it at the entry
            // numbered `rejoin_at` (1: its first chance; 2: ironspec 23:24 (3), "always one rotation late") or the first
            // entry after its own return if later)
            while let Some((l, c)) = arrived.pop() {
                // (an entry counts once the predecessor has popped again after the pick: its entry right after the pick
                // (from the circuit it was already on) is not a chance)
                for a in away.iter_mut().filter(|a| a.0 == l && a.2 == c && a.3 >= 1) {
                    a.4 += 1;
                }
                let ready = away.iter().position(|a| a.0 == l && a.2 == c && a.4 >= cfg.rejoin_at && siding.iter().any(|s| s.0 == l && s.1 == a.1));
                if let Some(j) = ready {
                    let x = away[j].1;
                    let s = away[j].3;
                    if !o.s_seen[l].contains(&s) {
                        o.s_seen[l].push(s);
                    }
                    away.remove(j);
                    if let Some(i) = siding.iter().position(|s| s.0 == l && s.1 == x) {
                        let wait = t - siding[i].3;
                        o.max_wait[l] = o.max_wait[l].max(wait);
                        siding.remove(i);
                    }
                    lanes[l].push_back(x);
                    arrived.push((l, x));
                }
            }
            for l in 0..3 {
                o.min_depth[l] = o.min_depth[l].min(lanes[l].len() + napped[l].len());
                o.max_away[l] = o.max_away[l].max(away.iter().filter(|a| a.0 == l).count());
            }
            // (a napper whose predecessor is itself away or waiting re-enters behind the predecessor's predecessor: the
            // ideal keeps the ORDER; a lone napper whose predecessor is the lane's only other... falls back to the back)
            let in_window = k < cfg.window;
            let halted = cfg.halts.iter().any(|&(d, s, len)| d == day && k_rel >= s && k_rel < s + len);
            let missed = (cfg.missed.contains(&(day, k_rel)) || halted) && in_window;
            if (0..3).any(|l| lanes[l].is_empty() && napped[l].is_empty()) || missed {
                o.stalls += 1;
                if missed {
                    k_rel += 1;
                }
                continue;
            }
            let picks_before = picks_today;
            let room_before = room;
            // (THE WAY-IN's OCCUPANCY at this REL: arrived at the leg and not yet in the bed; the room count: dispatched
            // and not yet departed from the front)
            assigned.retain(|a| a.1 + 1_000 > gt);
            let in_wayin = pending.iter().filter(|p| p.0 <= gt).count() + assigned.iter().filter(|a| a.0 <= gt && a.1 > gt).count();
            o.max_wayin = o.max_wayin.max(in_wayin);
            let mut shared_floor: Option<(i64, usize)> = None;
            for l in 0..3 {
                let c = match lanes[l].pop_front() {
                    Some(c) => c,
                    None => napped[l].pop_front().unwrap(),
                };
                let pred = last_pop[l];
                for a in away.iter_mut().filter(|a| a.0 == l && a.2 == c) {
                    a.3 += 1;
                }
                last_pop[l] = Some(c);
                since[l] += 1;
                let due_repick = cfg.repick && repick[l] == Some(c);
                // (the full POINTER form: every pick fires when the lane's front cart is the pointer's target AND at least
                // `period` REAL RELs have passed since the lane's last pick AND HELD is clear; the target then advances
                // DELTA ring positions)
                // (a lost cart's position is stepped over: the pointer counts the carts that are there)
                while let Some(tg) = target[l] {
                    if !gone[tg] {
                        break;
                    }
                    target[l] = Some(l * N0 + (tg - l * N0 + delta) % N0);
                }
                let spaced = since[l] >= cfg.period;
                let by_target = cfg.pointer && target[l] == Some(c) && spaced;
                let mut due = in_window && ((!cfg.pointer || target[l].is_none()) && k_rel == next_pick[l] || due_repick || by_target);
                in_flight.retain(|&e| e > t);
                let open = pending.len() + assigned.iter().filter(|a| a.1 > gt).count();
                o.max_open = o.max_open.max(open);
                if due && cfg.wayin_cap > 0 && room >= cfg.wayin_cap as i64 {
                    // (THE WAY-IN ROOM TERM: the way-in's dry places are all spoken for: the pick is DEFERRED like HELD's)
                    o.wayin_deferred += 1;
                    o.deferred_at.push(gt);
                    if !by_target && !due_repick {
                        next_pick[l] += cfg.period;
                    }
                    due = false;
                }
                if due && cfg.held > 0 && in_flight.len() >= cfg.held_cap {
                    // (HELD: the station's one napper is still in flight: the pick is DEFERRED; a pointer's target comes
                    // round again, a scheduled pick moves to the next period)
                    deferred_today += 1;
                    if !by_target && !due_repick {
                        next_pick[l] += cfg.period;
                    }
                    due = false;
                }
                if due {
                    let skip = first[l] && cfg.skipped.contains(&(day, l));
                    pick_no[l] += 1;
                    let late = (first[l] && cfg.late.contains(&(day, l))) || cfg.late_picks.contains(&(day, l, pick_no[l]));
                    first[l] = false;
                    if pick_no[l] == 1 {
                        o.first_wait[l] = o.first_wait[l].max(k_rel);
                    }
                    if skip && !due_repick {
                        // (the pick fails: a normal circuit; the re-pick fix waits for this cart's next pass)
                        transit.push((t + CIRCUIT, l, c, None));
                        if cfg.repick {
                            repick[l] = Some(c);
                        }
                        next_pick[l] += cfg.period;
                        continue;
                    }
                    if due_repick {
                        repick[l] = None;
                    } else if by_target {
                        next_pick[l] = k_rel + cfg.period;
                    } else {
                        next_pick[l] += cfg.period;
                    }
                    if c >= l * N0 && c < (l + 1) * N0 {
                        last_idx[l] = Some(c - l * N0);
                        if cfg.pointer {
                            target[l] = Some(l * N0 + (c - l * N0 + delta) % N0);
                            last_pick_k[l] = k_rel;
                        }
                    }
                    since[l] = 0;
                    if cfg.held > 0 {
                        in_flight.push(t + cfg.held_lane.map_or(cfg.held, |h| h[l]));
                    }
                    picks_today += 1;
                    room += 1;
                    o.max_room = o.max_room.max(room);
                    let v = cfg.visit + LANE_EXTRA[l] + if late { 1 } else { 0 };
                    let nap_at = gt + v * 50;
                    if cfg.bed {
                        // (THE ONE BED as a single server behind the FIFO (P1 + the front), released in ARRIVAL order: the
                        // outbound napper rides WITH ITS BATCH (gate list G11: pocket -> lift -> the split row in PASS
                        // shape -> (8,25,9) -> the leg); its MEASURED route to the way-in leg (9,25,9): REL + 132 / 154 /
                        // 186 by lane (ironfarm 00:57, bench, the last stretch extrapolated), then ~60 gt down the leg,
                        // the line and the column to the front (assumed); a 54-gt spread inside the 100-gt REL spacing:
                        // arrival order = (REL, slot) = pick order unless one napper's ride is delayed > 46 gt
                        // (`ride_delay`: a gate-G skip of the lane-7 intake + 100, a planted stall; a SHARED delay (after
                        // the merge) holds the same batch's later nappers behind it: carts never overtake)
                        let dly = cfg.ride_delay.iter().find(|r| r.0 == day && r.1 == l && r.2 == pick_no[l]).copied();
                        let mut leg = gt + LEG[l] + dly.map_or(0, |r| r.3);
                        if let Some((fl, fl_l)) = shared_floor {
                            leg = leg.max(fl + LEG[l] - LEG[fl_l]);
                        }
                        // (G1 CHANGES 6 B5: each planted delay read back: the napper's leg arrival against its normal one)
                        if let Some(r) = dly {
                            o.delay_reads.push((day, l, pick_no[l], r.3, leg - (gt + LEG[l]), r.4 == Ride::Shared));
                        }
                        if dly.is_some_and(|r| r.4 == Ride::Shared) || shared_floor.is_some() {
                            shared_floor = Some((leg, l));
                        }
                        o.max_inversion = o.max_inversion.max(max_leg - leg);
                        max_leg = max_leg.max(leg);
                        if dly.is_some_and(|r| r.4 == Ride::GateGSkip) && cfg.skip_fix {
                            rec_hold.push((l, t));
                        } else {
                            record.push_back(l);
                        }
                        o.record_depth = o.record_depth.max(record.len() + rec_hold.len());
                        if l == 2 {
                            last7 = Some(t);
                        } else if let Some(t7) = last7.take() {
                            o.min_after7 = o.min_after7.min(t - t7);
                        }
                        if let (Merge::Order, Some(p)) = (cfg.merge, pred) {
                            away.push((l, c, p, 0, 0));
                        }
                        last_pick_t = t;
                        if cfg.lost_napper.contains(&(day, l, pick_no[l])) {
                            // (lost before P1: no bed, no return; its lane runs one short; its away entry is dropped; the
                            // pointer steps over its position from now on)
                            away.retain(|a| a.1 != c);
                            gone[c] = true;
                            o.lost += 1;
                            continue;
                        }
                        pending.push((leg, l, c, t, v, pred, day));
                        continue;
                    }
                    o.naps[c].push(nap_at);
                    o.nap_day[c].push(day);
                    if let (Merge::Order, Some(p)) = (cfg.merge, pred) {
                        away.push((l, c, p, 0, 0));
                    }
                    transit.push((t + v, l, c, match cfg.merge {
                        Merge::Order => pred,
                        Merge::TwoTrack => Some(usize::MAX),
                        Merge::Back => None,
                    }));
                } else {
                    transit.push((t + CIRCUIT, l, c, None));
                }
            }
            if picks_today - picks_before >= 2 {
                o.multi_pick.push((gt, room_before, room));
            }
            match picks_today - picks_before {
                2 => o.rels_two += 1,
                3 => o.rels_three += 1,
                _ => {}
            }
            if in_window {
                k_rel += 1;
            }
        }
        // (the window's schedule restarts each night: the first pick of each lane at its offset)
        next_pick = offs;
        if cfg.pointer {
            // (the pointer persists through the day: tonight's first pick waits for its target; the spacing restarts)
            last_pick_k = [-cfg.period; 3];
        }
        let napped_today = (0..nv).filter(|&v| o.nap_day[v].contains(&day)).count();
        let mut per_lane = [0usize; 3];
        for (l, pl) in per_lane.iter_mut().enumerate() {
            *pl = (l * N0..(l + 1) * N0).filter(|&v| o.nap_day[v].contains(&day)).count();
        }
        o.lane_distinct.push(per_lane);
        o.distinct.push(napped_today);
        o.deferred.push(deferred_today);
        o.picks.push(picks_today);
    }
    let mut mg = (0i64, 0usize);
    for (v, ns) in o.naps.iter().enumerate() {
        for (i, w) in ns.windows(2).enumerate() {
            if w[0] >= DAY * 100 && w[1] - w[0] > mg.0 {
                mg = (w[1] - w[0], v);
            }
            if w[0] >= DAY * 100 && w[1] - w[0] >= 24_000 - MARGIN {
                let d = o.nap_day[v][i + 1];
                if !o.bad_nights.contains(&d) {
                    o.bad_nights.push(d);
                }
            }
        }
    }
    o.bad_nights.sort();
    o.bed_busy_permille = bed_busy * 1000 / (cfg.window * 100 * cfg.days).max(1);
    // (the queue depth: the most [arrive, start) waits overlapping at one instant)
    for &(a, _) in &bed_waits {
        let depth = bed_waits.iter().filter(|&&(a2, s2)| a2 <= a && a < s2).count();
        o.max_queue = o.max_queue.max(depth);
    }
    o.max_gap = mg;
    o
}

fn line(cfg: &Cfg, o: &Out) -> String {
    format!(
        "{}: period {} RELs per lane, window {} RELs, merge {:?}{}: distinct per night {:?} (per lane, night 3: {:?}), s measured per lane {:?}, the largest interval {} gt (villager {}), stalls {}",
        cfg.name,
        cfg.period,
        cfg.window,
        cfg.merge,
        if cfg.repick { ", re-pick" } else { "" },
        o.distinct,
        o.lane_distinct.get(3),
        o.s_seen,
        o.max_gap.0,
        o.max_gap.1,
        o.stalls
    )
}

pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec![format!("ctl-pick (RING LEVEL: each lane's carts FIFO, one per REL; a batch circuit {CIRCUIT} RELs; a visit 9 RELs (lane 7 + 1, late + 1: cc767b78's measured absences); the day {DAY} RELs; picks interleaved at offsets {OFFSET:?}; the bar 24 distinct per night from night 2 and every interval < 24,000 - {MARGIN})")];
    let ok = |o: &Out| o.distinct.iter().skip(1).all(|&d| d == 3 * N0) && o.max_gap.0 < 24_000 - MARGIN;
    // (today's rule, as the control: period 10, the 80-REL window, the merge at the back)
    let today = Cfg::new("TODAY (control)", 10, 80, Merge::Back);
    let o = run(&today);
    let c = v.control("ctl-pick CONTROL: today's rule (period 10, the 80-REL window, R_l merging at the lane's back) misses villagers or the interval bar", 1, !ok(&o)).to_string();
    lines.push(format!("  {}: {c}", line(&today, &o)));
    // (the robustness rows, each scheme: (A) positional, period 11, the whole REST, the order-preserving merge, re-pick;
    // (B) FRESH / NAPPED (the mark in the slot), the whole REST, period 11; judged: (B); printed: (A), the evidence that a
    // positional rule fails when the napper's absence s varies (lane 7's longer trip: G1 23:18))
    let disturb = |name: &str, period: i64, merge: Merge| -> Vec<Cfg> {
        let mut out = vec![Cfg::new(&format!("{name}, normal"), period, 120, merge)];
        let mut c1 = Cfg::new(&format!("{name}, a missed REL (night 3, window REL 30)"), period, 120, merge);
        c1.missed = vec![(3, 30)];
        out.push(c1);
        let mut c2 = Cfg::new(&format!("{name}, a late visit (night 3, lane 7's first pick)"), period, 120, merge);
        c2.late = vec![(3, 2)];
        out.push(c2);
        let mut c3 = Cfg::new(&format!("{name}, a skipped pick (night 3, lane 3's first)"), period, 120, merge);
        c3.skipped = vec![(3, 1)];
        out.push(c3);
        let mut c4 = Cfg::new(&format!("{name}, a cart moved from lane 7 to lane -1 on night 3 (n 9 / 7)"), period, 120, merge);
        c4.moved = vec![(3, 2, 0)];
        out.push(c4);
        out
    };
    for cfg in disturb("PRINTED (B) FRESH / NAPPED", 11, Merge::TwoTrack) {
        let o = run(&cfg);
        lines.push(format!("  {} -> {}", line(&cfg, &o), if ok(&o) { "meets the bars" } else { "MISSES the bars" }));
    }
    for cfg in disturb("PRINTED (A) positional, period 11, order-preserving merge", 11, Merge::Order) {
        let o = run(&cfg);
        lines.push(format!("  {} -> {}", line(&cfg, &o), if ok(&o) { "meets the bars" } else { "MISSES the bars" }));
    }
    for (period, nf) in [(11, true), (10, false), (10, true), (9, false), (9, true), (8, false), (8, true)] {
        let mut cfg = Cfg::new(&format!("PRINTED (B) FRESH / NAPPED, period {period}, the dawn merge {}", if nf { "NAPPED first" } else { "NAPPED last" }), period, 120, Merge::TwoTrack);
        cfg.napped_first = nf;
        let o = run(&cfg);
        lines.push(format!("  {} -> {}", line(&cfg, &o), if ok(&o) { "meets the bars" } else { "MISSES the bars" }));
    }
    // (G1 23:22 / ironspec 23:21: s and the coverage per lane for P = 10, 11, 12, 13, 16 (the positional rule with the
    // order-preserving merge, rejoining only at the predecessor's tail entry))
    for period in [10, 11, 12, 13, 16] {
        for rj in [1, 2] {
            let mut cfg = Cfg::new(&format!("PRINTED (A) positional, period {period}, rejoin at the predecessor's entry {rj}"), period, 120, Merge::Order);
            cfg.rejoin_at = rj;
            let o = run(&cfg);
            lines.push(format!("  {} -> {}; least queue depth per lane {:?}", line(&cfg, &o), if ok(&o) { "meets the bars" } else { "MISSES the bars" }, o.min_depth));
        }
    }
    // (the interval: a search over the period (P + 2 coprime with 7 / 8 / 9: 9, 11) and the window, both schemes)
    let mut srch = Vec::new();
    for merge in [Merge::Order, Merge::TwoTrack] {
        for period in [9, 11] {
            for window in [100, 110, 120, 130, 140, 160] {
                let mut cfg = Cfg::new("search", period, window, merge);
                cfg.days = 10;
                let o = run(&cfg);
                let d = o.distinct.iter().skip(1).min().copied().unwrap_or(0);
                srch.push(format!("{merge:?} P {period} W {window}: least distinct {d}, max interval {}", o.max_gap.0));
            }
        }
    }
    lines.push(format!("  THE INTERVAL SEARCH (printed): {}", srch.join(" | ")));
    // (ironspec 23:34 / 23:45 / 23:48: THE POINTER FORM, every pick pointer-driven (the front at the target, >= P REAL
    // RELs since the lane's last pick, HELD clear), Delta = P + 2 = 13 = 5 ring positions, the pointer held through the
    // day; HELD 4 RELs; both REJOIN RULES (first predecessor entry after the napper is back / the second entry); the
    // robustness rows, day length +-10, two consecutive late picks, multi-REL halts, a counter slip)
    let mut prow = disturb("POINTER", 11, Merge::Order);
    let mut dj = Cfg::new("POINTER, the day length +10 / -10 RELs alternating", 11, 120, Merge::Order);
    dj.day_jitter = (0..10).map(|d| if d % 2 == 0 { 10 } else { -10 }).collect();
    prow.push(dj);
    let mut dj2 = Cfg::new("POINTER, the day length -10 RELs every day", 11, 120, Merge::Order);
    dj2.day_jitter = vec![-10; 10];
    prow.push(dj2);
    let mut dj3 = Cfg::new("POINTER, the day length +10 RELs every day", 11, 120, Merge::Order);
    dj3.day_jitter = vec![10; 10];
    prow.push(dj3);
    let mut l2 = Cfg::new("POINTER, two consecutive late picks (night 3, lane 7's first and second)", 11, 120, Merge::Order);
    l2.late_picks = vec![(3, 2, 1), (3, 2, 2)];
    prow.push(l2);
    for len in [5, 10, 20] {
        let mut h = Cfg::new(&format!("POINTER, a {len}-REL HALT mid-REST (night 3, window REL 40)"), 11, 120, Merge::Order);
        h.halts = vec![(3, 40, len)];
        prow.push(h);
    }
    let mut sl = Cfg::new("POINTER, a COUNTER SLIP (+1 ring position, lane 3, night 3's dawn)", 11, 120, Merge::Order);
    sl.slip = vec![(3, 1, 1)];
    prow.push(sl);
    // (ironspec 01:05 (2) / G1 00:57 / 01:04: the delayed rides: the normal-class delays (a world open on one napper's
    // ride, the stack hold) can't reorder; a gate-G SKIP of a napper-carrying lane-7 intake (+100) does, removed at its
    // cause by the +10 record fix (judged: the record exact); without the fix, and under a planted STALL the fix can't
    // see (> 46 gt), the record's head vs each napper's lane is printed with where each napper went home)
    for (name, rd, fix) in [
        ("a world open on lane 3's first napper's ride (+3 gt, night 3: SHARED, every later cart held with it)", vec![(3i64, 1usize, 1usize, 3i64, Ride::Shared)], true),
        ("the stack hold on lane -1's first napper's ride (+26 gt at the lift top, night 3: SHARED)", vec![(3, 0, 1, 26, Ride::Shared)], true),
        ("a gate-G SKIP of lane 7's first napper-carrying intake (+100 gt, night 3), the +10 record fix", vec![(3, 2, 1, 100, Ride::GateGSkip)], true),
        ("gate-G SKIPS of lane 7's first two napper-carrying intakes (night 3), the +10 record fix", vec![(3, 2, 1, 100, Ride::GateGSkip), (3, 2, 2, 100, Ride::GateGSkip)], true),
        ("gate-G SKIPS of EVERY lane-7 napper-carrying intake on EVERY night, the +10 record fix", (0..10).flat_map(|d| (1..=11).map(move |p| (d, 2, p, 100, Ride::GateGSkip))).collect(), true),
        ("gate-G SKIPS of EVERY lane-7 napper-carrying intake on EVERY night WITHOUT the record fix", (0..10).flat_map(|d| (1..=11).map(move |p| (d, 2, p, 100, Ride::GateGSkip))).collect(), false),
        ("a gate-G SKIP of lane 7's first napper-carrying intake (+100 gt, night 3) WITHOUT the record fix", vec![(3, 2, 1, 100, Ride::GateGSkip)], false),
        ("a planted STALL before the merge delaying lane -1's first napper 60 gt (night 3; the record fix can't see it)", vec![(3, 0, 1, 60, Ride::Own)], true),
        ("a planted STALL before the merge delaying lane -1's first napper 250 gt (night 3; the record fix can't see it)", vec![(3, 0, 1, 250, Ride::Own)], true),
        ("a planted STALL before the merge delaying lane 3's second napper 250 gt (night 3; the record fix can't see it)", vec![(3, 1, 2, 250, Ride::Own)], true),
    ] {
        let mut c = Cfg::new(&format!("POINTER, {name}"), 11, 120, Merge::Order);
        c.ride_delay = rd;
        c.skip_fix = fix;
        prow.push(c);
    }
    // (ironspec 01:05 (2)(b): a desync the + 10 can't see: a napper LOST between its pick and P1 (a player breaks its
    // cart): its record entry outlives it; without THE RESYNC every later release is one entry off)
    for rs in [false, true] {
        let mut c = Cfg::new(&format!("POINTER, a napper LOST between its pick and P1 (night 3, lane 3's first){}", if rs { ", THE RECORD's RESYNC (cleared when no napper is in flight)" } else { " WITHOUT the record's resync" }), 11, 120, Merge::Order);
        c.lost_napper = vec![(3, 1, 1)];
        c.resync = rs;
        prow.push(c);
    }
    // (ironspec 23:57 / 00:01: THREE IN FLIGHT, ONE PER LANE (the stop a single server) and the FIRST-entry rejoin with
    // the rejoin counted are the DESIGN, judged; HELD spans as measured (cc767b78 23:59: 264 / 284 / 313 gt -> [3, 3, 4]
    // RELs); the bed modelled (one occupant); printed beside: today's ONE in flight on the measured spans (ironspec
    // 23:59c) and the second-entry rejoin (modelled until the composed night measures the stamp))
    for (cap, rj) in [(3usize, 1i64), (3, 2), (1, 1), (1, 2)] {
        for base in &prow {
            let mut cfg = base.clone();
            cfg.name = format!(
                "{} [{}; rejoin: {}]",
                base.name,
                match cap {
                    1 => "ONE in flight (today's contract), HELD measured",
                    _ => "THREE in flight, ONE PER LANE (ruled 23:57)",
                },
                if rj == 2 { "SECOND entry" } else { "FIRST entry after the return, counted" }
            );
            cfg.pointer = true;
            cfg.days = 10;
            cfg.held = 4;
            cfg.held_lane = Some([3, 3, 4]);
            cfg.held_cap = cap;
            cfg.rejoin_at = rj;
            cfg.bed = true;
            let o = run(&cfg);
            let wv = o.max_gap.1;
            let fault = cfg.name.contains("moved") || cfg.name.contains("SLIP") || cfg.name.contains("HALT") || cfg.name.contains("STALL") || cfg.name.contains("WITHOUT the record fix") || cfg.name.contains("LOST");
            let design = cap == 3 && rj == 1;
            // (G1 CHANGES 6 B5: on the design config the record's twins are JUDGED CONTROLS and the resync row a judged bar;
            // every planted delay is read back (the leg arrival = normal + the delay; a shared delay at least it))
            if design && !cfg.ride_delay.is_empty() {
                let unread: Vec<String> = cfg.ride_delay.iter().filter(|p| !o.delay_reads.iter().any(|r| r.0 == p.0 && r.1 == p.1 && r.2 == p.2 && (r.4 == p.3 || (r.5 && r.4 >= p.3)))).map(|p| format!("night {} lane {} pick {} +{}", p.0, LANES[p.1], p.2, p.3)).collect();
                let w = v.bar_n(&format!("ctl-pick: {}: every planted ride delay read back (the leg arrival = normal + the delay)", cfg.name), cfg.ride_delay.len(), unread.is_empty());
                lines.push(format!("  {}: {} delays read back {}: {w}", cfg.name, o.delay_reads.len(), if unread.is_empty() { "all".to_string() } else { format!("(NOT read: {})", unread.join("; ")) }));
            }
            if design && cfg.name.contains("EVERY night WITHOUT the record fix") {
                let c = v.control(&format!("ctl-pick CONTROL: {}: the record's mismatches appear (the + 10 fix is load-bearing)", cfg.name), 1, !o.record_mismatch.is_empty()).to_string();
                lines.push(format!("  CONTROL ({}): {} mismatches: {c}", cfg.name, o.record_mismatch.len()));
            }
            if design && cfg.name.contains("WITHOUT the record's resync") {
                let c = v.control(&format!("ctl-pick CONTROL: {}: the record desyncs (mismatches appear without THE RESYNC)", cfg.name), 1, !o.record_mismatch.is_empty()).to_string();
                lines.push(format!("  CONTROL ({}): {} mismatches, distinct {:?}: {c}", cfg.name, o.record_mismatch.len(), o.distinct.iter().max()));
            }
            if design && cfg.name.contains("THE RECORD's RESYNC") {
                let w = v.bar(&format!("ctl-pick: {}: THE LANE RECORD EXACT after the loss (0 mismatches; the lost villager's lane one short: 23 distinct)", cfg.name), o.record_mismatch.is_empty() && o.resyncs >= 1);
                lines.push(format!("  {}: {} mismatches, {} resyncs, distinct up to {:?}: {w}", cfg.name, o.record_mismatch.len(), o.resyncs, o.distinct.iter().max()));
            }
            if design && cfg.name.contains("first napper-carrying intake (+100 gt, night 3)") {
                lines.push(format!("  NOTE ({}): the single skip leaves 0 mismatches with or without the fix (inversion {}): it does not exercise the fix (the every-night row does)", cfg.name, o.max_inversion));
            }
            let verdict = if fault {
                format!(
                    "PRINTED (a fault path: halts > 1 missed REL are fault class (ironspec 23:50); the slip's cause is a fault, a world-open slip is owed as a strict row; a stall or an unfixed skip reorders the FIFO): nights holding an interval >= 23,000: {:?}, the record's mismatches {} -> {}",
                    o.bad_nights,
                    o.record_mismatch.len(),
                    if ok(&o) { "meets the bars" } else { "misses the bars" }
                )
            } else if design {
                v.bar(&format!("ctl-pick: {}: 24 distinct every night from night 2, every interval < 24,000 - {MARGIN}, 0 picks deferred by HELD, THE LANE RECORD EXACT (every release's head names the released napper's lane)", cfg.name), ok(&o) && o.deferred.iter().all(|&d| d == 0) && o.record_mismatch.is_empty()).to_string()
            } else {
                format!("PRINTED (beside the design) -> {}", if ok(&o) && o.deferred.iter().all(|&d| d == 0) { "meets the bars" } else { "MISSES the bars" })
            };
            let wait_gt: Vec<i64> = o.max_wait.iter().map(|w| w * 100).collect();
            lines.push(format!(
                "  {}: {verdict}; THE LANE RECORD: mismatches (night, the napper's lane, the head's lane: a pair trades places, the two villagers swap lanes, positions kept) {:?}, the most entries at once {}, the largest arrival inversion {} gt, the fewest RELs from a lane-7 pick to the next lane -1 / 3 pick {} (a gate-G skip reorders only at 1), nappers lost {}, the record's resyncs {}; picks per night {:?}, deferred by HELD per night {:?}, the bed's longest wait {} gt, the most nappers WAITING for the bed at once {}, under the FIFO's gate (P1 (9,22,-3) released at the previous napper's front departure; arrival in (REL, slot) order): the longest wait at P1 {} gt and at the front {} gt, the bed busy {} per mille of the windows; batches carrying two nappers {} and three {} (two or three lanes picking in one REL: every napper's D press G11-emulated, their arrival order the slot order); the longest R_l wait per lane {:?} gt (an additive bound on the stamp: r8's 530 + the wait = {:?}; the stamp arms at the first partial HOME path, ironspec 23:57 (3): cc767b78 measures), the most away per lane {:?}, the latest first pick per lane {:?} RELs into REST; the worst villager {wv}'s naps (gt) {:?}",
                line(&cfg, &o),
                o.record_mismatch.iter().map(|&(d, l, h)| (d, LANES[l], LANES[h])).collect::<Vec<_>>(),
                o.record_depth,
                o.max_inversion,
                o.min_after7,
                o.lost,
                o.resyncs,
                o.picks,
                o.deferred,
                o.max_bed_wait,
                o.max_queue,
                o.max_hold_wait,
                o.max_front_wait,
                o.bed_busy_permille,
                o.rels_two,
                o.rels_three,
                wait_gt,
                wait_gt.iter().map(|w| 530 + w).collect::<Vec<_>>(),
                o.max_away,
                o.first_wait,
                o.naps[wv]
            ));
        }
    }
    // (ironspec 06:23 / 06:31 (1), G1 06:26 (2): THE WAY-IN OVERFLOW (b8949c96's token-stall row: the 4th napper on spills
    // onto the split row / lane 7's R / the station's side): THE DESIGN's DISPATCH IS BOUNDED BY THE WAY-IN's DRY PLACES
    // (P1 + 2 = 3) BY EVIDENCE: THE WAY-IN ROOM TERM (a pick only while dispatched-minus-departed < 3: the pick edges count
    // up, the front's departure presses (9,-11,-3) count down: never merged (one per visit, ~333 gt apart); two lanes
    // picking in one REL are two G11 outputs, counted as two). Rows: the normal nights (its margin vs 3, 0 deferred);
    // THE STOP STALLED for 2,000 / 6,000 gt at every phase of night 3's window (every 250 gt): the way-in never holds a
    // 4th; the control: the room term off: A 4th NAPPER DISPATCHED INTO A FULL WAY-IN by name. HELD's in-flight count is
    // a TIMER (its spans expire whether or not the napper left): it is not this bound)
    {
        let design = |name: &str, cap: usize, stall: Option<(i64, i64)>| {
            let mut c = Cfg::new(name, 11, 120, Merge::Order);
            c.pointer = true;
            c.days = 10;
            c.held = 4;
            c.held_lane = Some([3, 3, 4]);
            c.held_cap = 3;
            c.rejoin_at = 1;
            c.bed = true;
            c.wayin_cap = cap;
            if let Some(s) = stall {
                c.stop_stall = vec![s];
            }
            c
        };
        let on = run(&design("normal, the room term on", 3, None));
        let w = v.bar("ctl-pick: THE WAY-IN ROOM TERM never defers a pick on the normal nights (the design unchanged: 24 distinct, every interval < 24,000 - margin) and the way-in holds at most 3", ok(&on) && on.wayin_deferred == 0 && on.max_wayin <= 3);
        lines.push(format!("  THE WAY-IN on the normal nights (the room term on): at most {} nappers in the way-in at once (margin {} vs its 3 dry places), the most dispatched-not-departed {}, picks deferred by it {}: {w}", on.max_wayin, 3i64 - on.max_wayin as i64, on.max_open, on.wayin_deferred));
        let night = 3 * DAY * 100;
        let (mut worst, mut worst_at, mut n, mut miss) = (0usize, String::new(), 0usize, Vec::new());
        let (mut worst_off, mut off_at) = (0usize, String::new());
        for len in [2_000i64, 6_000] {
            for ph in (0..12_000).step_by(250) {
                let st = (night + ph, len);
                let o = run(&design(&format!("stop stalled {len} gt at night 3 + {ph}"), 3, Some(st)));
                n += 1;
                if o.max_wayin > worst {
                    worst = o.max_wayin;
                    worst_at = format!("{len} gt at + {ph}");
                }
                if o.max_wayin > 3 && miss.len() < 3 {
                    miss.push(format!("A 4th NAPPER IN THE WAY-IN: the stop stalled {len} gt at night 3 + {ph}: {} at once", o.max_wayin));
                }
                let of = run(&design(&format!("stop stalled {len} gt at night 3 + {ph}, NO room term"), 0, Some(st)));
                if of.max_wayin > worst_off {
                    worst_off = of.max_wayin;
                    off_at = format!("{len} gt at + {ph}");
                }
            }
        }
        let w = v.bar_n("ctl-pick: THE STOP STALLED 2,000 / 6,000 gt at every phase of a night (every 250 gt): THE WAY-IN NEVER HOLDS A 4th NAPPER (the room term: dispatched-minus-departed < 3 by evidence)", n, miss.is_empty() && worst <= 3);
        lines.push(format!("  THE WAY-IN with the stop stalled ({n} rows): at most {worst} nappers in the way-in ({worst_at}){}: {w}", if miss.is_empty() { String::new() } else { format!(" ({})", miss.join(" | ")) }));
        let c = v.control("ctl-pick CONTROL (ironspec 06:31 (1)): the way-in room term off, the stop stalled: A 4th NAPPER DISPATCHED INTO A FULL WAY-IN by name (b8949c96's spill)", 1, worst_off > 3).to_string();
        lines.push(format!("  CONTROL (no room term): at most {worst_off} nappers in the way-in ({off_at}): {c}"));
        // (G1 06:35's three rows on the room count (an ORDER MACHINE, ironspec 06:43 (1): its resync on evidence at the
        // record's quiet point): (a) A MISSED DEPARTURE PRESS: the count reads one high, picks defer early (safe), the
        // resync puts it back: the way-in still <= 3, the nights still meet the bars, the count's resyncs printed;
        // (b) AN EXTRA DEPARTURE PRESS while the count is full (a rollback over (9,-11,-3), which b8949c96's no-return
        // rows show can't happen; or THE EMPTY-AT-FRONT CHAIN: a player's empty departing on R1): the count reads one low
        // (clamped >= 0), so ONE napper too many can be dispatched: the overflow printed (the R_1 guard (7,19,1) turns it
        // into a stall held dry); (c) TWO PICKS IN ONE REL count two (the count before / after printed))
        let base = run(&design("room count, normal", 3, None));
        let mut ra = design("room count, the 30th departure press missed", 3, None);
        ra.missed_dep = vec![30];
        let oa = run(&ra);
        let wa = v.bar("ctl-pick: THE ROOM COUNT with a MISSED departure press (the 30th): the way-in still <= 3, the nights still meet the bars (24 distinct, every interval < 24,000 - margin), the count resynced at a quiet point", oa.max_wayin <= 3 && ok(&oa) && oa.room_resyncs >= 1);
        lines.push(format!("  THE ROOM COUNT, a MISSED departure press (the 30th): picks deferred {} (normal {}), the count's resyncs {} (normal {}), the most in the way-in {}, the worst interval {}: {wa}", oa.wayin_deferred, base.wayin_deferred, oa.room_resyncs, base.room_resyncs, oa.max_wayin, oa.max_gap.0));
        // (b): the extra press planted at each REL of night 3's window while the stop is stalled (the count full)
        let night3 = 3 * DAY * 100;
        let (mut over, mut over_at) = (0usize, String::new());
        for ph in (0..6_000).step_by(500) {
            let mut rb = design("room count, an extra departure press", 3, Some((night3 + ph, 2_000)));
            rb.extra_dep = vec![night3 + ph + 1_000];
            let ob = run(&rb);
            if ob.max_wayin > over {
                over = ob.max_wayin;
                over_at = format!("the stop stalled 2,000 gt at night 3 + {ph}, the extra press 1,000 gt in");
            }
        }
        lines.push(format!("  THE ROOM COUNT, an EXTRA departure press while full (a rollback over (9,-11,-3): b8949c96's no-return rows; or a player's empty departing on R1: the empty-at-front chain): the count clamped at >= 0, the most in the way-in {over} ({over_at}){}", if over > 3 { ": ONE TOO MANY: the R_1 guard (7,19,1) (dfb89499, priced) turns the overflow into a stall held dry" } else { "" }));
        // (c)
        let two: Vec<String> = base.multi_pick.iter().take(4).map(|(g, b, a)| format!("gt {g}: {b} -> {a}")).collect();
        let w2 = v.bar("ctl-pick: TWO PICKS IN ONE REL count TWO on the room count (every such REL: after = before + the picks)", base.multi_pick.iter().all(|(_, b, a)| a - b >= 2) && !base.multi_pick.is_empty());
        lines.push(format!("  THE ROOM COUNT, two or more picks in one REL: {} such RELs, each counting every pick ({}): {w2}", base.multi_pick.len(), two.join(", ")));
        // (ironspec 06:28 (B), G1 06:29: RS_READY AT THE BALL EXIT (7,-3,4): its cost per visit (the previous rider's
        // travel from the R string (4,0,5) to the ball exit, 58f770cc measuring) against the nap margin: the design's
        // worst nap-to-nap interval with each visit's cycle grown by 0..120 gt, printed (the bar's margin 1,000))
        let mut costs = Vec::new();
        for extra in [0i64, 20, 40, 60, 80, 120] {
            let mut c = design(&format!("RS_ready at the ball exit, + {extra} gt per visit"), 3, None);
            c.rs_extra = extra;
            let o = run(&c);
            costs.push(format!("+ {extra}: the worst interval {} gt (to 24,000: {}){}", o.max_gap.0, 24_000 - o.max_gap.0, if ok(&o) { "" } else { " MISSES the bars" }));
        }
        lines.push(format!("  RS_READY AT THE BALL EXIT (ironspec 06:28 (B)), each visit's cycle grown by the R-string-to-ball-exit travel: {}", costs.join("; ")));
    }
    lines.push("  OWED (before anything is laid, ironspec 23:48 (2)): the COUNTER's netlist (a ring-position counter of each lane's front, counting on the REL pulse in the same tick, the picked-not-rejoined positions in a per-lane register, the rejoin counted as evidence, state in latches; a world-open-mid-count row passing STRICTLY, value read back); the per-lane holds' MECHANISM (the waiting cell per lane; slot-1 supply, F1 and REL while held; each batch's make-up and credit; the gate = the previous napper's departure from the front (the RS_ready reset); the order arbiter releasing EXACTLY ONE, the OLDEST PICK, per departure event, with planted rows of two and three holds at once); the 219-row credit sweep under this schedule; cc767b78's composed night (each waiting napper's stamp age, the bed's busy fraction, golems lost)".into());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_prints() {
        // (the grid: rejoin x HELD x capacity x the pointer's step; one summary line each)
        for rj in [2, 1] {
            let mut cfg = Cfg::new("grid", 11, 120, Merge::Order);
            cfg.pointer = true;
            cfg.days = 10;
            cfg.rejoin_at = rj;
            cfg.held = 4;
            cfg.held_lane = Some([3, 3, 4]);
            for bed in [false, true] {
                cfg.bed = bed;
                let o = run(&cfg);
                println!(
                    "GRID-MEASURED rejoin {rj} ONE in flight, HELD [3,3,4] (264/284/313 gt) bed {bed}: distinct {:?}, max interval {} (v {}), picks {:?}, deferred {:?}, max bed wait {}",
                    o.distinct, o.max_gap.0, o.max_gap.1, o.picks, o.deferred, o.max_bed_wait
                );
            }
        }
        for rj in [2, 1] {
            for (held, cap, step) in [(0, 1, 5), (3, 1, 5), (4, 1, 5), (4, 2, 5), (4, 3, 5), (4, 1, 3), (4, 1, 7)] {
                for bed in [false, true] {
                let mut cfg = Cfg::new("grid", 11, 120, Merge::Order);
                cfg.pointer = true;
                cfg.days = 10;
                cfg.rejoin_at = rj;
                cfg.held = held;
                cfg.held_cap = cap;
                cfg.ptr_step = step;
                cfg.bed = bed;
                let o = run(&cfg);
                println!(
                    "GRID rejoin {rj} held {held} cap {cap} step {step} bed {bed}: distinct {:?}, max interval {} (v {}), picks {:?}, deferred {:?}, max wait {:?}, max away {:?}, max bed wait {}",
                    o.distinct, o.max_gap.0, o.max_gap.1, o.picks, o.deferred, o.max_wait, o.max_away, o.max_bed_wait
                );
                }
            }
        }
        let mut cfg = Cfg::new("dbg", 11, 120, Merge::Order);
        cfg.pointer = true;
        cfg.days = 4;
        cfg.rejoin_at = 1;
        let o = run(&cfg);
        let mut ev: Vec<(i64, usize)> = Vec::new();
        for v in 0..N0 {
            for &g in &o.naps[v] {
                ev.push((g, v));
            }
        }
        ev.sort();
        println!("lane 0 picks, first-entry rejoin (gt, villager): {:?}", ev);
    }

    #[test]
    fn the_bench_prints() {
        // (the pinned verdict: the ruled design (the pointer form, three in flight, the first-entry rejoin counted, the bed
        // a single server) passes every judged row; today's rule's control bites; the CLI exits 0)
        let mut v = Verdict::new("ctl-pick-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
        println!("ctl-pick fails: {:?}", v.fails());
        assert!(v.fails().is_empty(), "ctl-pick-bench: the pinned verdict changed: {:?}", v.fails());
        assert!(v.finish(false).is_ok());
    }
}
