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
/// The token's recovery delay N (gt), READ from day-1's controller interface file ("recovery delay N = N gt"; ironspec
/// 09:00a (1): one N, stated once, read by both benches): the arrival detector unpowered continuously for N while A_SEEN
/// (counted after its 20-gt recheck hold) before a missed departure's token SET (and the room count's down-count).
pub fn n_rec() -> i64 {
    let f = include_str!("../../../designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-controller-interface.md");
    let key = "- The token's recovery delay N = ";
    let at = f.find(key).expect("the interface file states the recovery delay N") + key.len();
    f[at..].split(' ').next().and_then(|s| s.parse().ok()).expect("the recovery delay N is a number of gt")
}

/// THE WAY-IN AS BOUGHT (P1 OCCUPIED form (B), ironspec 09:50a / 10:05a): the leg's timings the model reads (provisional;
/// b8949c96 measures each): the leg -> the second hold (9,24,7); the hold -> the arrival detector (9,23,6)'s SET edge;
/// OCCUPIED's delay D after the SET (>= the (9,23,6) -> rest-on-P1 transit + margin); P1's release -> its exit detector
/// (9,22,3) (the RESET); P1's release -> the napper's box out of (9,23,6)'s; the detector rails' 20-gt recheck; the front's
/// clear time after a departure (b8949c96 09:43: 5 gt).
pub const LEG_TO_HOLD: i64 = 10;
pub const HOLD_TO_SET: i64 = 2;
pub const P1_TO_EXIT: i64 = 3;
pub const P1_LEAVE_DET: i64 = 2;
pub const DET_RECHECK: i64 = 20;
pub const FRONT_CLEAR: i64 = 5;

/// P1 OCCUPIED's delay D (gt), READ from day-1's controller interface file ("P1 OCCUPIED's delay D = N gt"; ironspec
/// 09:50a (d): stated once): OCCUPIED = the latch AND D elapsed since its SET.
pub fn d_occ() -> i64 {
    let f = include_str!("../../../designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-controller-interface.md");
    let key = "- P1 OCCUPIED's delay D = ";
    let at = f.find(key).expect("the interface file states P1 OCCUPIED's delay D") + key.len();
    f[at..].split(' ').next().and_then(|s| s.parse().ok()).expect("P1 OCCUPIED's delay D is a number of gt")
}

/// A detector rail pressed at `pressed` whose cart's box left it at `left` reads UNPOWERED from its first 20-gt recheck
/// at or after `left` (26.3 DetectorRailBlock: a pressed rail schedules a re-check every 20 gt while a cart is on it).
pub fn det_off(pressed: i64, left: i64) -> i64 {
    let k = ((left - pressed) + DET_RECHECK - 1).div_euclid(DET_RECHECK).max(1);
    pressed + k * DET_RECHECK
}

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
    /// ironspec 01:05 (2)(b): THE RECORD's RESYNC: cleared on SENSED evidence that no napper is in flight (P1 OCCUPIED's
    /// latch 0, the front's arrival detector unpowered, the stop free (RS_ready set), and >= 3 RELs since the last pick: a
    /// napper reaches P1 within 2 RELs of its pick).
    pub resync: bool,
    /// THE STOP STALLED (ironspec 06:23 / 06:31, G1 06:26: b8949c96's way-in overflow row): (gt start, length): no bed
    /// start inside the window (a token stall: the queue holds its nappers).
    pub stop_stall: Vec<(i64, i64)>,
    /// THE WAY-IN ROOM TERM (ironspec 06:31 (1)): a pick only while the nappers DISPATCHED and not yet departed from the
    /// front (the pick edges up; each TOKEN SET down: the front's departure press while A_SEEN, or the token's recovery)
    /// number fewer than this (3: the front + P1 + the second hold (9,24,7) under P1 OCCUPIED's form (B)). 0: off.
    pub wayin_cap: usize,
    /// ironspec 06:28 (B): RS_READY's EVIDENCE MOVED TO THE BALL EXIT (7,-3,4): each visit's cycle grows by the previous
    /// rider's travel from the R string to the ball exit (gt).
    pub rs_extra: i64,
    /// The bed's cycle per visit (gt): the front's departure to the next release's RS_ready: 333 (the exit-hold era);
    /// 58f770cc 06:55's no-hold re-runs: entry -> lie-down 27..37, the nap 56..76, wake -> RS_ready 68..84 (151..197).
    pub bed_cycle: i64,
    /// G1 06:35 (a): the departure presses (their index over the run, 1-based) the front's detector MISSES: no token set
    /// by the press; the token's RECOVERY (A_SEEN AND the arrival detector unpowered continuously for N) sets it.
    pub missed_dep: Vec<usize>,
    /// G1 06:35 (b): EXTRA departure presses (gt): a press while A_SEEN sets the token early (a spurious set; the napper's
    /// own departure then finds A_SEEN clear); a press outside A_SEEN is ignored by the gate.
    pub extra_dep: Vec<i64>,
    /// G1 on 16e8535f7 (5)'s control: THE A_SEEN GATE REMOVED: an extra departure press sets the token anywhere (a press
    /// while the previous napper is still in the column releases the next one behind it).
    pub gate_off: bool,
    /// ironspec 10:18a's control: TODAY's PRESS-BASED ROOM COUNT (the token set counts the room down) instead of the
    /// front emptying.
    pub press_count: bool,
    /// G1 08:26 (7)'s control: two picks in one REL MERGED into one count (a single edge for both G11 outputs).
    pub merge_picks: bool,
    /// Planted picks: (the REL's gt, lane): the lane picks at that REL (still under the room term and HELD).
    pub force_pick: Vec<(i64, usize)>,
    /// THE PICK-RATE RULE: at most this many picks per REL (0: no limit).
    pub max_picks_per_rel: usize,
    /// The lanes' leg times overridden (the napper's pick -> its leg arrival), e.g. b8949c96's measured 15-16 gt spacing.
    pub leg_override: Option<[i64; 3]>,
}

impl Cfg {
    pub fn new(name: &str, period: i64, window: i64, merge: Merge) -> Cfg {
        Cfg { name: name.to_string(), period, window, visit: 9, merge, repick: true, napped_first: false, rejoin_at: 2, pointer: false, day_jitter: vec![], held: 0, held_cap: 1, held_lane: None, ptr_step: 5, bed: false, halts: vec![], slip: vec![], late_picks: vec![], days: 8, missed: vec![], late: vec![], skipped: vec![], moved: vec![], ride_delay: vec![], skip_fix: true, lost_napper: vec![], resync: true, stop_stall: vec![], wayin_cap: 0, rs_extra: 0, bed_cycle: 333, missed_dep: vec![], extra_dep: vec![], gate_off: false, press_count: false, merge_picks: false, force_pick: vec![], max_picks_per_rel: 0, leg_override: None }
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
    /// THE ROOM COUNT (the hardware's reading: up per pick edge, down per TOKEN SET, clamped >= 0 (an item counter can't
    /// go below empty)): its most, the token sets made by the RECOVERY, the extra presses the A_SEEN gate ignored and those
    /// it took as a (spurious, early) set, and the RELs carrying two or more picks (gt, the count before, after).
    pub max_room: i64,
    pub room_recoveries: usize,
    pub extra_ignored: usize,
    pub token_spurious: usize,
    /// Releases into the column while the previous napper was still in it (the gate-off control's failure).
    pub double_release: usize,
    pub multi_pick: Vec<(i64, i64, i64)>,
    /// THE HARDWARE-ONLY CHECK (ironspec 10:02a (2)): each room-count change's (applied at, its sensed event's tick): the
    /// controller applies an event only at or after it happened (a negative lag = a read of a future tick, failing by
    /// name).
    pub event_lag: Vec<(i64, i64)>,
    /// Each napper's departure from the front (its bed start; the departure press's tick).
    pub departures: Vec<i64>,
    /// (B): the least lag from a napper's RESET (its exit press) to the next napper's SET, and the nappers that reached the
    /// hold while the previous one was still at it (waiting behind the hold, on the flat top).
    pub reset_set_lag: Option<i64>,
    pub behind_hold: usize,
    /// Picks deferred by THE PICK-RATE RULE.
    pub rate_deferred: usize,
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
    let n_rec_gt = n_rec();
    let d_occ_gt = d_occ();
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
    let mut bed_busy: i64 = 0;
    // (the FIFO in ARRIVAL order (G1 00:50 / ironspec 00:58): a picked napper's bed start waits until no later pick can
    // reach the front before it: (leg arrival, lane, cart, pick REL t, visit v, predecessor, day))
    let mut pending: Vec<(i64, usize, usize, i64, i64, Option<usize>, i64)> = Vec::new();
    // (every napper's (leg arrival, bed start) once its start is fixed: the way-in's occupancy and the room count)
    let mut assigned: Vec<(i64, i64)> = Vec::new();
    // (THE ROOM COUNT as the hardware keeps it: up per pick edge, down per TOKEN SET, clamped >= 0. `token_sets`: the sets
    // not yet applied (their tick, made by the recovery); each is made from sensed evidence only: the front's departure
    // press while A_SEEN, or the recovery (A_SEEN AND the arrival detector unpowered continuously for N))
    let mut room: i64 = 0;
    let mut token_sets: Vec<(i64, bool)> = Vec::new();
    let mut dep_count = 0usize;
    let mut extra_used: Vec<i64> = Vec::new();
    // (THE WAY-IN AS BOUGHT, (B): the previous napper's P1 release pulse and SET edge at (9,23,6); its front arrival (the
    // arrival detector (9,-9,-5)'s press), departure, and whether that departure's press was missed)
    let mut prev_rel: Option<i64> = None;
    let mut prev_set: i64 = i64::MIN / 4;
    let mut prev_front: Option<(i64, i64, i64, i64)> = None;
    let mut prev_clear: i64 = i64::MIN / 4;
    let mut prev2_clear: i64 = i64::MIN / 4;
    let mut prev_pass: i64 = i64::MIN / 4;
    // (each processed napper's SENSED windows: (its SET, its latch's RESET + 1), (its front arrival press, the front's
    // arrival detector falling), (its bed start, RS_ready's set after its visit): the record's resync reads the levels)
    let mut sensed: Vec<(i64, i64, i64, i64, i64, i64)> = Vec::new();
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
            // (THE ROOM COUNT (ironspec 07:38r / 08:41a, the hardware-only rule 10:02a (2)): it counts DOWN on each TOKEN
            // SET, applied at its tick (never before: the event-lag check); each set is made in the FIFO below from sensed
            // evidence only (the front's departure press while A_SEEN, or the RECOVERY: A_SEEN AND the arrival detector
            // unpowered continuously for N); an extra press OUTSIDE A_SEEN is ignored by the gate. No quiet-point resync
            // (G1 07:39 (1): an over-count is the safe direction))
            token_sets.sort();
            while token_sets.first().is_some_and(|d| d.0 <= gt) {
                let (d, by_rec) = token_sets.remove(0);
                room -= 1;
                if by_rec {
                    o.room_recoveries += 1;
                }
                o.event_lag.push((gt, d));
            }
            room = room.max(0);
            // (THE RECORD's RESYNC, ironspec 01:05 (2)(b), FROM SENSED LEVELS (G1 on 16e8535f7, the older oracle
            // `pending.is_empty() && gt >= bed_free` removed): no napper in flight = P1 OCCUPIED's latch 0 (no napper on P1
            // or let down to it), the front's arrival detector unpowered, the stop free (RS_ready set by the ball-exit edge
            // after the last visit), and >= 3 RELs since the last pick (a napper reaches P1 within 2 RELs of its pick, so a
            // napper still on the leg is ruled out by time, not seen); the levels at this tick are read from each napper's
            // sensed edges up to now: the record must be empty; any entry left is cleared)
            sensed.retain(|s| s.5 + 2_000 > gt);
            let latch_on = sensed.iter().any(|s| s.0 <= gt && gt < s.1);
            let front_on = sensed.iter().any(|s| s.2 <= gt && gt < s.3);
            let stop_busy = sensed.iter().any(|s| s.4 <= gt && gt < s.5);
            if cfg.resync && !latch_on && !front_on && !stop_busy && t - last_pick_t >= 3 && (!record.is_empty() || !rec_hold.is_empty()) {
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
                // (THE WAY-IN AS BOUGHT (P1 OCCUPIED form (B), ironspec 09:50a / 10:05a; the hardware-only rule 10:02a
                // (2)): every tick below is a sensed edge or level:
                // - the second hold (9,24,7) opens on NOT latch AND (9,23,6) unpowered AND (9,22,3) unpowered (the
                //   previous napper's RESET at its exit press, and both detectors' 20-gt rechecks after it left them);
                // - the napper passes the hold, its SET edge at (9,23,6) HOLD_TO_SET later; OCCUPIED rises D (read from the
                //   interface file) after it;
                // - the TOKEN was set by the previous napper's departure press at the front while A_SEEN (its arrival
                //   press set A_SEEN), or, that press missed, by the RECOVERY: the arrival detector unpowered (its recheck
                //   after the napper's box cleared, FRONT_CLEAR after the departure) continuously for N;
                // - FRONT CLEAR = the arrival detector unpowered;
                // - P1's release pulse: the rising edge of TOKEN AND FRONT CLEAR AND OCCUPIED; START-UP: the token free, the
                //   latch 0, so the first napper is released once it rests)
                let hold_open = match prev_rel {
                    None => i64::MIN / 4,
                    Some(pr) => {
                        let reset = pr + P1_TO_EXIT;
                        let u_det = det_off(prev_set, pr + P1_LEAVE_DET);
                        let u_exit = det_off(reset, reset + 1);
                        reset.max(u_det).max(u_exit)
                    }
                };
                let pass = (leg + LEG_TO_HOLD).max(hold_open);
                // (ironspec 10:18a (2) / G1: the lag from the previous napper's RESET to this napper's SET (> 0 at every
                // phase: the SET never lands while the RESET level holds); and a napper reaching the hold while the
                // previous one is still at it (it would wait BEHIND the hold, on the flat top (9,25,8)): counted)
                if let Some(pr) = prev_rel {
                    let lag = (pass + HOLD_TO_SET) - (pr + P1_TO_EXIT);
                    o.reset_set_lag = Some(o.reset_set_lag.map_or(lag, |m: i64| m.min(lag)));
                }
                if leg + LEG_TO_HOLD < prev_pass {
                    o.behind_hold += 1;
                }
                prev_pass = pass;
                let set = pass + HOLD_TO_SET;
                let occupied = set + d_occ_gt;
                // (prev_front: the previous napper's front arrival, its token set (the press, the recovery, or an extra press
                // inside A_SEEN), the arrival detector's falling edge (FRONT CLEAR), and its P1 release)
                let (token, front_clear) = match prev_front {
                    None => (i64::MIN / 4, i64::MIN / 4),
                    Some((_a, tok, u, _r)) => (tok, u),
                };
                let mut release = occupied.max(token).max(front_clear);
                // (the A_SEEN GATE's control: with the gate removed, an extra press while the previous napper is still in the
                // column (released, not yet arrived: the front reads clear) sets the token, and this napper is released
                // into the column behind it: A DOUBLE RELEASE)
                if cfg.gate_off {
                    if let Some((a, _tok, u0, r0)) = prev_front {
                        let _ = u0;
                        if let Some(x) = cfg.extra_dep.iter().copied().filter(|&x| x > r0 && x < a).min() {
                            let r = occupied.max(x).max(prev2_clear);
                            if r < a {
                                o.double_release += 1;
                                release = r;
                            }
                        }
                    }
                }
                let at_front = arrive.max(release + 45);
                let mut start = at_front.max(bed_free);
                // (THE STOP STALLED: no bed start inside a stall window)
                for &(s0, len) in &cfg.stop_stall {
                    if start >= s0 && start < s0 + len {
                        start = s0 + len;
                    }
                }
                assigned.push((leg, start));
                o.departures.push(start);
                // (this napper's TOKEN SET: its departure press at `start` (A_SEEN since its arrival press at `at_front`),
                // or the RECOVERY if that press is missed (A_SEEN AND the arrival detector unpowered continuously for N); an
                // EXTRA press inside A_SEEN [at_front, the set) sets it early (P1's next release still waits for FRONT
                // CLEAR); a press outside A_SEEN is ignored by the gate)
                dep_count += 1;
                let missed_here = cfg.missed_dep.contains(&dep_count);
                let u_front = det_off(at_front, start + FRONT_CLEAR);
                let tset = if missed_here { u_front + n_rec_gt } else { start };
                let early = cfg.extra_dep.iter().copied().filter(|&x| x >= at_front && x < tset && !extra_used.contains(&x)).min();
                let tok = match early {
                    Some(x) => {
                        extra_used.push(x);
                        o.token_spurious += 1;
                        x
                    }
                    None => {
                        if missed_here {
                            o.room_recoveries += 1;
                        }
                        tset
                    }
                };
                // (THE ROOM's DOWN-COUNT (bought, ironspec 10:18a): from THE FRONT EMPTYING: the arrival detector's falling
                // edge after its recheck (an R_SEEN latch per napper, set by the arrival edge, cleared and counted by the
                // fall), so an extra press moves nothing; the control `press_count`: today's press-based count (the token
                // set counts the room down))
                token_sets.push((if cfg.press_count { tok } else { u_front }, false));
                sensed.push((set, release + P1_TO_EXIT, at_front, u_front, start, start + cfg.bed_cycle + cfg.rs_extra));
                prev2_clear = prev_clear;
                prev_clear = u_front;
                prev_rel = Some(release);
                prev_set = set;
                prev_front = Some((at_front, tok, u_front, release));
                o.max_hold_wait = o.max_hold_wait.max(release - (leg + LEG_TO_HOLD + HOLD_TO_SET + d_occ_gt));
                o.max_front_wait = o.max_front_wait.max(start - at_front);
                bed_busy += cfg.bed_cycle;
                bed_free = start + cfg.bed_cycle + cfg.rs_extra;
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
                let forced = in_window && cfg.force_pick.contains(&(gt, l));
                let mut due = forced || (in_window && ((!cfg.pointer || target[l].is_none()) && k_rel == next_pick[l] || due_repick || by_target));
                in_flight.retain(|&e| e > t);
                let open = pending.len() + assigned.iter().filter(|a| a.1 > gt).count();
                o.max_open = o.max_open.max(open);
                if due && cfg.max_picks_per_rel > 0 && picks_today - picks_before >= cfg.max_picks_per_rel {
                    // (THE PICK-RATE RULE (priced for b8949c96 10:57 / G1 10:59, bunching at the second hold): at most this
                    // many picks per REL: a further due lane defers to the next REL (a pointer target comes round again; a
                    // scheduled pick moves one period))
                    o.rate_deferred += 1;
                    if !by_target && !due_repick {
                        next_pick[l] += cfg.period;
                    }
                    due = false;
                }
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
                    if !(cfg.merge_picks && picks_today - 1 > picks_before) {
                        room += 1;
                    }
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
                        let leg_t = cfg.leg_override.unwrap_or(LEG);
                        let mut leg = gt + leg_t[l] + dly.map_or(0, |r| r.3);
                        if let Some((fl, fl_l)) = shared_floor {
                            leg = leg.max(fl + leg_t[l] - leg_t[fl_l]);
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
    // (the extra departure presses the A_SEEN gate ignored: those that fell outside every napper's A_SEEN window)
    if !cfg.gate_off {
        o.extra_ignored = cfg.extra_dep.len() - extra_used.len();
    }
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
        lines.push(format!("  THE WAY-IN on the normal nights (the room term on): at most {} nappers in the way-in at once (margin {} vs its 3 places: the front + P1 + the second hold (9,24,7)), the most dispatched-not-departed {}, picks deferred by it {}: {w}", on.max_wayin, 3i64 - on.max_wayin as i64, on.max_open, on.wayin_deferred));
        // (the way-in's timings under (B), printed: the longest P1 wait past its earliest release (the token / front clear
        // / OCCUPIED), the longest front wait; the hardware-only check: every room-count change applied at or after its
        // sensed event (a negative lag = a read of a future tick, failing by name))
        let lag_bad = on.event_lag.iter().filter(|(at, ev)| at < ev).count();
        let w = v.bar_n("ctl-pick: THE HARDWARE-ONLY CHECK (ironspec 10:02a (2)): every room-count change on the normal nights applied at or after its sensed event's tick (no read of a future tick)", on.event_lag.len(), lag_bad == 0 && !on.event_lag.is_empty());
        lines.push(format!("  THE WAY-IN under (B): the longest P1 wait past its earliest release {} gt, the longest front wait {} gt; room-count changes {} (applied before their event: {lag_bad}): {w}", on.max_hold_wait, on.max_front_wait, on.event_lag.len()));
        let night = 3 * DAY * 100;
        let (mut worst, mut worst_at, mut n, mut miss) = (0usize, String::new(), 0usize, Vec::new());
        let (mut worst_off, mut off_at) = (0usize, String::new());
        let (mut least_lag, mut behind) = (on.reset_set_lag, on.behind_hold);
        for len in [2_000i64, 6_000] {
            for ph in (0..12_000).step_by(250) {
                let st = (night + ph, len);
                let o = run(&design(&format!("stop stalled {len} gt at night 3 + {ph}"), 3, Some(st)));
                n += 1;
                if let Some(l) = o.reset_set_lag {
                    least_lag = Some(least_lag.map_or(l, |m| m.min(l)));
                }
                behind += o.behind_hold;
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
        let w = v.bar_n("ctl-pick: THE STOP STALLED 2,000 / 6,000 gt at every phase of a night (every 250 gt): THE WAY-IN NEVER HOLDS A 4th NAPPER (the room term: dispatched-minus-departed < 3, the pick edges up, the front emptying down)", n, miss.is_empty() && worst <= 3);
        lines.push(format!("  THE WAY-IN with the stop stalled ({n} rows): at most {worst} nappers in the way-in ({worst_at}){}: {w}", if miss.is_empty() { String::new() } else { format!(" ({})", miss.join(" | ")) }));
        // ((B)'s two prints, judged (ironspec 10:18a (2) / (3), G1 10:11): THE RESET -> SET LAG (the next napper's SET after
        // the previous one's RESET press, over the normal nights and every stall row, the queued napper let down at the
        // hold's earliest opening): > 0 everywhere; A NAPPER WAITING BEHIND THE HOLD (reaching it while the previous one
        // is still there, so resting on the flat top (9,25,8)): none, the room cap (the front + P1 + the hold) keeping a
        // newly dispatched napper's arrival after P1's release has freed the hold)
        let w = v.bar_n("ctl-pick: (B)'s RESET -> SET LAG over the normal nights and every stall row is > 0 (the SET pulse never lands while the RESET level holds) and NO NAPPER WAITS BEHIND THE HOLD", n + 1, least_lag.is_some_and(|l| l > 0) && behind == 0);
        lines.push(format!("  (B): the least RESET -> SET lag {least_lag:?} gt; nappers waiting behind the hold {behind}: {w}"));
        let c = v.control("ctl-pick CONTROL (ironspec 06:31 (1)): the way-in room term off, the stop stalled: A 4th NAPPER DISPATCHED INTO A FULL WAY-IN by name (b8949c96's spill)", 1, worst_off > 3).to_string();
        lines.push(format!("  CONTROL (no room term): at most {worst_off} nappers in the way-in ({off_at}): {c}"));
        // (G1 06:35's three rows on the room count (an ORDER MACHINE) under the token rule as ruled (the count goes down
        // once per TOKEN SET: the departure press while A_SEEN, or the recovery; no quiet resync, G1 07:39 (1)), MODELLED
        // FROM SENSED EDGES AND LEVELS (G1 on 16e8535f7 (4) / (5)): (a) A MISSED DEPARTURE PRESS: the token's RECOVERY
        // (A_SEEN AND the arrival detector unpowered continuously for N, counted from its recheck after the napper's box
        // cleared) sets the token and counts the room down, once (the recovery path counted, not the plant); (b) AN EXTRA
        // DEPARTURE PRESS (a rollback over (9,-11,-3); a player's empty departing on R1): outside A_SEEN the gate ignores
        // it; INSIDE A_SEEN (a napper resting at the front) it sets the token early under the ruled rule: THE HAZARD,
        // pinned by name pending ironspec's ruling; (c) TWO PICKS IN ONE REL count two (the count before / after printed);
        // and G1 07:39 (1)'s HALT ROW: the stop stalled far longer than any quiet window (the stall rows above))
        let base = run(&design("room count, normal", 3, None));
        let mut ra = design("room count, the 30th departure press missed", 3, None);
        ra.missed_dep = vec![30];
        let oa = run(&ra);
        let wa = v.bar("ctl-pick: THE ROOM COUNT with a MISSED departure press (the 30th): the token's RECOVERY (A_SEEN AND the arrival detector unpowered continuously for N) counts it down exactly once, the way-in still <= 3, the nights still meet the bars (24 distinct, every interval < 24,000 - margin)", oa.max_wayin <= 3 && ok(&oa) && oa.room_recoveries == 1 && base.room_recoveries == 0);
        lines.push(format!("  THE ROOM COUNT, a MISSED departure press (the 30th): the recovery's token sets {} (normal {}; N {} gt read from the interface file, counted from the arrival detector's recheck to unpowered), picks deferred {} (normal {}), the most in the way-in {}, the worst interval {}: {wa}", oa.room_recoveries, base.room_recoveries, n_rec(), oa.wayin_deferred, base.wayin_deferred, oa.max_wayin, oa.max_gap.0));
        // (b): the extra press planted at each REL of night 3's window while the stop is stalled (the count full)
        let night3 = 3 * DAY * 100;
        // (b1) THE A_SEEN GATE (G1 on 16e8535f7 (5)): extra presses OUTSIDE A_SEEN, each 40 gt after a departure in the
        // 2,000 gt AFTER a 2,000-gt stall (the queue full behind it: the next napper released and still in the column, the
        // departures read from the same row run without presses): the gate ignores every one (no token set, no release
        // into the column); the control (the gate removed: a press sets the token anywhere): A DOUBLE RELEASE INTO THE
        // COLUMN by name
        let (mut over1, mut over1_at, mut ign1, mut n_press, mut dbl_on, mut dbl_off) = (0usize, String::new(), 0usize, 0usize, 0usize, 0usize);
        for ph in (0..6_000).step_by(500) {
            let s0 = night3 + ph;
            let r0 = design("room count, the reference stall", 3, Some((s0, 2_000)));
            let o0 = run(&r0);
            let presses: Vec<i64> = o0.departures.iter().filter(|&&d| d >= s0 + 2_000 && d < s0 + 4_000).map(|&d| d + 40).collect();
            n_press += presses.len();
            let mut rb = r0.clone();
            rb.extra_dep = presses;
            let ob = run(&rb);
            ign1 += ob.extra_ignored;
            dbl_on += ob.double_release;
            if ob.max_wayin > over1 {
                over1 = ob.max_wayin;
                over1_at = format!("the stop stalled 2,000 gt at night 3 + {ph}");
            }
            let mut rc = rb.clone();
            rc.gate_off = true;
            dbl_off += run(&rc).double_release;
        }
        let wb1 = v.bar("ctl-pick: THE A_SEEN GATE: extra departure presses OUTSIDE A_SEEN (40 gt after each departure in the 2,000 gt after a 2,000-gt stall, the next napper in the column) are all ignored: no release into the column, the way-in never holds a 4th", over1 <= 3 && ign1 == n_press && dbl_on == 0 && n_press >= 12);
        lines.push(format!("  THE A_SEEN GATE: {n_press} extra presses outside A_SEEN over 12 stall rows, {ign1} ignored, double releases {dbl_on}, the most in the way-in {over1} ({over1_at}): {wb1}"));
        let c = v.control("ctl-pick CONTROL (G1 on 16e8535f7 (5)): the A_SEEN gate removed (a press sets the token anywhere): A DOUBLE RELEASE INTO THE COLUMN by name (the next napper released while the previous one is still in it)", 1, dbl_off > 0).to_string();
        lines.push(format!("  CONTROL (the gate removed): {dbl_off} double releases: {c}"));
        // (b2) THE FRONT-REST HAZARD (found by this re-model, 10:15; the fix BOUGHT ironspec 10:18a): an extra press INSIDE
        // A_SEEN (the napper resting at the front through a stall, the press 1,000 gt in) sets the token early under the
        // ruled rule; with the room's down-count from THE FRONT EMPTYING (the arrival detector's falling edge, R_SEEN) it
        // moves nothing: the way-in never holds a 4th; the control (today's press-based count planted): A 4th NAPPER
        let (mut over, mut over_at, mut spur, mut over_press) = (0usize, String::new(), 0usize, 0usize);
        for ph in (0..6_000).step_by(500) {
            let mut rb = design("room count, an extra departure press inside A_SEEN", 3, Some((night3 + ph, 2_000)));
            rb.extra_dep = vec![night3 + ph + 1_000];
            let ob = run(&rb);
            spur += ob.token_spurious;
            if ob.max_wayin > over {
                over = ob.max_wayin;
                over_at = format!("the stop stalled 2,000 gt at night 3 + {ph}, the extra press 1,000 gt in");
            }
            let mut rp = rb.clone();
            rp.press_count = true;
            over_press = over_press.max(run(&rp).max_wayin);
        }
        let wb = v.bar("ctl-pick: THE ROOM COUNT with an EXTRA departure press INSIDE A_SEEN (a napper resting at the front; a rollback over (9,-11,-3) or a player's empty on R1), the down-count from the front emptying: the way-in never holds a 4th", over <= 3 && spur >= 12);
        lines.push(format!("  THE ROOM COUNT, an EXTRA press inside A_SEEN: {spur} spurious token sets over 12 rows (P1 still waits for FRONT CLEAR), the most in the way-in {over} ({over_at}): {wb}"));
        let c = v.control("ctl-pick CONTROL (ironspec 10:18a): today's PRESS-BASED room count planted (the token set counts the room down): A 4th NAPPER IN THE WAY-IN by name on the stalled-stop rows", 1, over_press > 3).to_string();
        lines.push(format!("  CONTROL (the press-based count): at most {over_press} nappers in the way-in: {c}"));
        // (c) (G1 08:26 (7) / 10:00: the count's increment per pick is the code's, so a bar on it can't fail; the normal
        // nights under the way-in as bought carry no REL with two picks, so two picks are PLANTED in one REL (lanes -1 and 3)
        // 300 gt before a 6,000-gt stall at every phase: counted as two, the way-in never holds a 4th (the design partner:
        // each lane's pick edge feeds the count on its own); the CONTROL, the two picks MERGED into one count: a 4th napper)
        let two: Vec<String> = base.multi_pick.iter().take(4).map(|(g, b, a)| format!("gt {g}: {b} -> {a}")).collect();
        lines.push(format!("  THE ROOM COUNT, two or more picks in one REL on the normal nights: {} such RELs ({})", base.multi_pick.len(), two.join(", ")));
        {
            let (mut worst_d, mut worst_m, mut n_two, mut counted_two) = (0usize, 0usize, 0usize, 0usize);
            for ph in (0..12_000).step_by(250) {
                let s0 = night + ph;
                let g = s0 - 300;
                let mut cd = design("room count, two picks planted in one REL", 3, Some((s0, 6_000)));
                cd.force_pick = vec![(g, 0), (g, 1)];
                let od = run(&cd);
                worst_d = worst_d.max(od.max_wayin);
                for m in od.multi_pick.iter().filter(|m| m.0 == g) {
                    n_two += 1;
                    if m.2 - m.1 >= 2 {
                        counted_two += 1;
                    }
                }
                let mut cm = cd.clone();
                cm.merge_picks = true;
                worst_m = worst_m.max(run(&cm).max_wayin);
            }
            let w = v.bar("ctl-pick: TWO PICKS PLANTED IN ONE REL (lanes -1 and 3, 300 gt before a 6,000-gt stall at every phase): each counted (the count up by two), the way-in never holds a 4th", worst_d <= 3 && n_two >= 1 && counted_two == n_two);
            lines.push(format!("  TWO PICKS PLANTED IN ONE REL: {n_two} RELs took both, {counted_two} counted two, at most {worst_d} in the way-in: {w}"));
            let c = v.control("ctl-pick CONTROL (G1 08:26 (7)): the planted two picks in one REL MERGED into one count, the stop stalled 6,000 gt at every phase: A 4th NAPPER IN THE WAY-IN by name", 1, worst_m > 3).to_string();
            lines.push(format!("  CONTROL (two picks merged into one count): at most {worst_m} nappers in the way-in: {c}"));
        }
        // (BUNCHING AT THE SECOND HOLD (b8949c96 10:57, G1 10:59): three nappers arriving 15-16 gt apart (three picks in one
        // REL with an empty way-in: the cold start, or any REL where all three lanes' pointer targets come due) put the
        // third behind the hold while the second waits at it: b8949c96's redsim run rests it on LANE 7's R HOLD (5,24,7), a
        // cross-lane rest. THE PICK-RATE RULE (day-1's pick, priced): AT MOST ONE PICK PER REL (a further due lane defers to
        // the next REL), so two nappers arrive >= 100 - the largest leg difference apart (46 gt on the model's legs) and the
        // second finds the hold open or waits alone at it. Rows: three picks planted in each of the first 10 RELs of the run
        // (the way-in empty), on the model's legs and on b8949c96's measured 16-gt spacing: no napper behind the hold; the
        // control (today's form, no rate limit, the measured spacing): A NAPPER BEHIND THE HOLD by name; its nap cost)
        {
            let (mut behind_d, mut behind_c, mut rows_b, mut wayin_d) = (0usize, 0usize, 0usize, 0usize);
            for legs in [None, Some([132i64, 148, 164])] {
                for k in 1..=10i64 {
                    let g = k * 100;
                    let mut cb = design("bunched picks, one pick per REL", 3, None);
                    cb.force_pick = vec![(g, 0), (g, 1), (g, 2)];
                    cb.leg_override = legs;
                    cb.max_picks_per_rel = 1;
                    let ob = run(&cb);
                    rows_b += 1;
                    behind_d += ob.behind_hold;
                    wayin_d = wayin_d.max(ob.max_wayin);
                    if legs.is_some() {
                        let mut cc = cb.clone();
                        cc.max_picks_per_rel = 0;
                        behind_c += run(&cc).behind_hold;
                    }
                }
            }
            let w = v.bar_n("ctl-pick: BUNCHED PICKS (three planted in one REL with the way-in empty, the model's legs and b8949c96's measured 16-gt spacing) under THE PICK-RATE RULE (one pick per REL): NO NAPPER WAITS BEHIND THE SECOND HOLD, the way-in never holds a 4th", rows_b, behind_d == 0 && wayin_d <= 3);
            lines.push(format!("  BUNCHED PICKS under one pick per REL ({rows_b} rows): nappers behind the hold {behind_d}, at most {wayin_d} in the way-in: {w}"));
            let c = v.control("ctl-pick CONTROL (b8949c96 10:57): today's form (no pick-rate rule), three picks in one REL on the measured 16-gt spacing: A NAPPER BEHIND THE SECOND HOLD by name (b8949c96's redsim rest on lane 7's R hold)", 1, behind_c > 0).to_string();
            lines.push(format!("  CONTROL (no pick-rate rule): {behind_c} nappers behind the hold: {c}"));
            let o1 = run(&{
                let mut c1 = design("normal, one pick per REL", 3, None);
                c1.max_picks_per_rel = 1;
                c1
            });
            let sc = v.scope("ctl-pick: the pick-rate rule's nap cost", "priced for ironspec's ruling on the bunching fix");
            lines.push(format!("  PRICED: ONE PICK PER REL on the normal nights: picks deferred by it {}, the worst nap interval {} gt{} (without it: {} gt): {sc}", o1.rate_deferred, o1.max_gap.0, if ok(&o1) { "" } else { " MISSES the nap bars" }, base.max_gap.0));
            // (the multi-pick RELs are timing-sensitive (ctl-pick before the sensed-edge re-model printed 54 in 10 nights;
            // the re-model's departure timing gives 0): the rule priced over the no-hold sweep's timings too, each point's
            // multi-pick RELs without the rule, its deferrals and worst interval with it)
            let mut pts = Vec::new();
            let mut all_ok = true;
            for cyc in [197i64, 168] {
                for extra in [0i64, 20, 40, 60, 80, 120] {
                    let mut c0 = design("sweep, no rate rule", 3, None);
                    c0.bed_cycle = cyc;
                    c0.rs_extra = extra;
                    let o0 = run(&c0);
                    let mut c1 = c0.clone();
                    c1.max_picks_per_rel = 1;
                    let o1 = run(&c1);
                    all_ok &= ok(&o1);
                    pts.push(format!("{cyc} + {extra}: {} multi-pick RELs -> {} deferred, worst {} gt{}", o0.multi_pick.len(), o1.rate_deferred, o1.max_gap.0, if ok(&o1) { "" } else { " MISSES" }));
                }
            }
            let w = v.bar_n("ctl-pick: THE PICK-RATE RULE (one pick per REL) keeps the nap bars over the no-hold sweep (the cycle 197 / 168 gt, (B)'s travel + 0 / 20 / 40 / 60 / 80 / 120 gt)", 12, all_ok);
            lines.push(format!("  THE PICK-RATE RULE over the no-hold sweep: {}: {w}", pts.join("; ")));
        }
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
        let sc = v.scope("ctl-pick: RS_ready at the ball exit on the 333-gt cycle", "the exit-hold era's cycle, not the design's: the no-hold cycle below is judged");
        lines.push(format!("  RS_READY AT THE BALL EXIT (ironspec 06:28 (B)), each visit's cycle (333, the exit-hold era) grown by the R-string-to-ball-exit travel: {}: {sc}", costs.join("; ")));
        // (58f770cc 06:55 / ironspec 07:07 (2): THE NO-HOLD CYCLE: entry -> lie-down 27..37, the nap 56..76, wake -> RS_ready
        // 68..84: the bed's cycle 151..197 gt (median 168); the nap bar JUDGED at every point of (B)'s travel sweep 0..120
        // on the worst and the median cycle (G1 08:26 (9)); the travel itself measured by cc767b78)
        let (mut n_sw, mut bad_sw) = (0usize, Vec::new());
        for cyc in [197i64, 168] {
            let mut nc = Vec::new();
            for extra in [0i64, 20, 40, 60, 80, 120] {
                let mut c = design(&format!("the no-hold cycle {cyc}, + {extra} gt"), 3, None);
                c.bed_cycle = cyc;
                c.rs_extra = extra;
                let o = run(&c);
                n_sw += 1;
                if !ok(&o) {
                    bad_sw.push(format!("{cyc} + {extra}"));
                }
                nc.push(format!("+ {extra}: {} ({}){}", o.max_gap.0, 24_000 - o.max_gap.0, if ok(&o) { "" } else { " MISSES" }));
            }
            lines.push(format!("  THE NO-HOLD CYCLE {cyc} gt (58f770cc 06:55), the worst nap-to-nap interval (to 24,000) with (B)'s travel: {}", nc.join("; ")));
        }
        let w = v.bar_n("ctl-pick: THE NAP BAR ON THE NO-HOLD CYCLE at every point of (B)'s travel sweep (the cycle 197 / 168 gt, + 0 / 20 / 40 / 60 / 80 / 120 gt): 24 distinct every night, every interval < 24,000 - 1,000", n_sw, bad_sw.is_empty());
        lines.push(format!("  THE NAP BAR over the no-hold sweep ({n_sw} points){}: {w}", if bad_sw.is_empty() { String::new() } else { format!(" (MISSES at {})", bad_sw.join(", ")) }));
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
        // a single server; the way-in as bought: P1 OCCUPIED form (B), the token from sensed edges, the room's down-count
        // from the front emptying) passes every judged row; every control bites; the CLI exits 0)
        let mut v = Verdict::new("ctl-pick-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
        println!("ctl-pick fails: {:?}", v.fails());
        assert!(v.fails().is_empty(), "ctl-pick-bench: the pinned verdict changed: {:?}", v.fails());
        assert!(v.finish(false).is_ok());
    }
}
