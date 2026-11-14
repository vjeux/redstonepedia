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
    /// THE SENSED POINTER (the (i-b) item ring, ironspec 11:56a / the parent 11:55): the pick decision reads ONLY the
    /// lane's ring state (the tap's slot, the target token T's slot, the absent markers X, the spacing latch), never a
    /// cart's identity; the rejoin is the sensed rule (an entry carrying its pop's "the next slot was skipped" mark, AND
    /// the siding holding a napper: its front napper released behind it; that X extracted at its next tap arrival).
    pub sensed: bool,
    /// The forced test (the parent 11:49): the carts' ids PERMUTED inside each lane at the start (slot i carries cart
    /// l * N0 + (N0 - 1 - i)): a decision that reads a cart's id changes its result; the sensed form's must not.
    pub relabel: bool,
    /// The timing corner (ironspec 12:02p (1)): every circuit entry processed AFTER the same REL's pops instead of before
    /// them (an entry shifted past a pop by any amount, 2 gt included): the mark FIFO, the sensed rejoin and the extract
    /// flags must not depend on that order.
    pub late_entries: bool,
    /// G1 on 8e3d5f208 (2)'s control: the second hold's open condition WITHOUT its (9,22,3) term (the previous napper's
    /// exit detector unpowered): a napper waiting at the hold passes before the RESET level falls, its SET lands inside it.
    pub hold_no_exit: bool,
    /// Whole-farm FREEZES (a reload / unload, normal: ironspec 11:20a): (day, the farm REL it starts at, its length in
    /// RELs): game time runs on while nothing in the farm moves.
    pub freeze: Vec<(i64, i64, i64)>,
    /// THE ONE-PICK-PER-REL FORM (ironspec 12:12p / 12:14p): 0 = none (each REQ granted), 1 = the arbiter (the cap: a
    /// refused lane's pointer keeps its target), 2 = ROTATION (one lane eligible per REL: -1, 3, 7 in turn), 3 = STATIC
    /// PRIORITY (lane -1 over 3 over 7, stateless).
    pub grant_form: u8,
    /// The three rings' T tokens' starting slots (the build guide places them: the zero-cell cold-start fix).
    pub t_start: [usize; 3],
    /// THE POSITIONAL RELEASE (the simpler ring hardware): the rejoin read from the ring itself at the entry (an X two
    /// droppers downstream of the tap) instead of a mark FIFO, the X lifted out at the release (one X type, no flags).
    pub positional_release: bool,
    /// Every lane's leg (the pick -> the second hold) shifted by these gt (the +-2 gt corners on the pops / REL).
    pub leg_shift: [i64; 3],
    /// The napper's stop release -> its arrival at the row hold (9,0,8), gt (the record's per-tick shift time; swept until
    /// 58f770cc / b8949c96 measure it).
    pub arm3_trip: i64,
}

impl Cfg {
    pub fn new(name: &str, period: i64, window: i64, merge: Merge) -> Cfg {
        Cfg { name: name.to_string(), period, window, visit: 9, merge, repick: true, napped_first: false, rejoin_at: 2, pointer: false, day_jitter: vec![], held: 0, held_cap: 1, held_lane: None, ptr_step: 5, bed: false, halts: vec![], slip: vec![], late_picks: vec![], days: 8, missed: vec![], late: vec![], skipped: vec![], moved: vec![], ride_delay: vec![], skip_fix: true, lost_napper: vec![], resync: false, stop_stall: vec![], wayin_cap: 0, rs_extra: 0, bed_cycle: 333, missed_dep: vec![], extra_dep: vec![], gate_off: false, press_count: false, merge_picks: false, force_pick: vec![], max_picks_per_rel: 0, leg_override: None, sensed: false, relabel: false, late_entries: false, hold_no_exit: false, freeze: vec![], grant_form: 0, t_start: [0, 3, 6], positional_release: false, leg_shift: [0; 3], arm3_trip: 80 }
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
    /// THE SENSED POINTER's own checks (the judge reads the carts' ids here, the controller never does): pops whose cart
    /// is not the one the ring's tap predicted (sync lost); sensed rejoins that differ from the identity rule's; an X
    /// needing a type both of whose markers are in the ring (a third napper of a lane away); T's ignored first passes;
    /// the most X in one lane's ring at once.
    pub ring_mismatch: usize,
    pub rejoin_vs_ref: usize,
    pub x_conflict: usize,
    pub t_first_pass: usize,
    pub max_x: usize,
    /// Sensed rejoins whose X type (the toggle's, flipped per rejoin) differs from the mark's own type (the judge's check
    /// that the nappers rejoin in pick order).
    pub type_order_mismatch: usize,
    /// The least gap between two consecutive nappers' arrivals at the second hold (gt; i64::MAX with fewer than two).
    pub min_hold_gap: i64,
    /// The one-pick-per-REL forms: per lane, the longest wait (RELs) from a refused REQ to its grant.
    pub max_grant_wait: [i64; 3],
    /// The most nappers picked and not yet back at their siding at once (the sensed ring's record-depth bound).
    pub max_nappers_out: i64,
    /// The most nappers picked and not yet ARM3'd at once (the record's stage count: cc767b78's (a') registers).
    pub max_not_arm3: usize,
    /// The record's entries per tick: (written at, shifted at) gt; and the most alive at once (a shift at the same tick as
    /// a write counts first: the sort puts -1 before +1).
    pub record_spans: Vec<(i64, i64)>,
    pub max_open_measured: i64,
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
    // (the ring's slots: slot i of lane l carries this cart at the start (the forced test permutes the ids))
    let slot_cart: Vec<Vec<usize>> = (0..3).map(|l| (0..N0).map(|i| l * N0 + if cfg.relabel { N0 - 1 - i } else { i }).collect()).collect();
    let mut lanes: Vec<VecDeque<usize>> = (0..3).map(|l| slot_cart[l].iter().copied().collect()).collect();
    let mut napped: Vec<VecDeque<usize>> = vec![VecDeque::new(); 3];
    let nv = 3 * N0;
    let mut o = Out { naps: vec![Vec::new(); nv], nap_day: vec![Vec::new(); nv], min_depth: [usize::MAX; 3], min_hold_gap: i64::MAX, ..Out::default() };
    // (in transit: (back at REL, lane, cart, order-preserving predecessor if any))
    let mut transit: Vec<(i64, usize, usize, Option<usize>)> = Vec::new();
    // (waiting in R_l: (lane, cart, predecessor, arrival REL))
    let mut siding: Vec<(usize, usize, Option<usize>, i64)> = Vec::new();
    // (s's count: (lane, napper, predecessor, the predecessor's pops so far))
    let mut away: Vec<(usize, usize, usize, i64, i64)> = Vec::new();
    // (the ring's cyclic predecessor of each lane's first cart is its last one: the first napper rejoins in order too)
    let mut last_pop: [Option<usize>; 3] = [Some(slot_cart[0][N0 - 1]), Some(slot_cart[1][N0 - 1]), Some(slot_cart[2][N0 - 1])];
    // (THE SENSED POINTER's state per lane, the item ring's: the tap's slot (the next to pop), T's slot, the X markers
    // (slot, type a / b), the next X type, the "extract the next X of this type" flags (set by a sensed rejoin), the
    // spacing latch (set at a grant, cleared at T's first pass), REQ armed (T at the tap, the latch clear), and the
    // entry-mark FIFO (pushed per circuit pop: Some(type) when the slot after the popped one was a skipped X; popped per
    // circuit entry, in order: carts never overtake))
    let mut ring_next: [usize; 3] = [0; 3];
    let mut t_slot: [usize; 3] = cfg.t_start;
    let mut x_set: [Vec<(usize, usize)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut x_type_next: [usize; 3] = [0; 3];
    let mut x_extract: [[bool; 2]; 3] = [[false; 2]; 3];
    let mut spacing_latch: [bool; 3] = [false; 3];
    let mut req_armed: [bool; 3] = [cfg.t_start[0] == 0, cfg.t_start[1] == 0, cfg.t_start[2] == 0];
    // (the one-pick-per-REL forms' waits: per lane, the farm REL of its first refused REQ still pending)
    let mut refused_since: [Option<i64>; 3] = [None; 3];
    // (the widening's state: the REL of the last lane-7 grant)
    let mut last_g7: Option<i64> = None;
    let mut mark_fifo: [VecDeque<Option<usize>>; 3] = [VecDeque::new(), VecDeque::new(), VecDeque::new()];
    // (the hardware form: the X type a rejoin flags comes from a per-lane toggle flipped per rejoin (the nappers rejoin in
    // pick order, the X types alternate in pick order), not from reading the X: the mark's own type is the judge's check)
    let mut rejoin_type_next: [usize; 3] = [0; 3];
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
    let mut nappers_out: i64 = 0;
    // (the record's entries for the ARM3 stamp: (napper, its ARM3 REL once known))
    let mut open_nappers: Vec<(usize, Option<i64>)> = Vec::new();
    let mut prev_arr: i64 = i64::MIN / 4;
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
    let mut frozen_gt: i64 = 0;
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
        // (A WHOLE-FARM FREEZE (a reload / unload: the chunk and everything in it stop while game time runs on): at the
        // day's farm REL `start`, `len` RELs of game time pass with nothing in the farm moving: the day has `len` fewer
        // farm RELs, the night window loses the frozen part of it, and every later game-time stamp (the naps) is `len`
        // RELs later than the farm's clock)
        let fz = cfg.freeze.iter().find(|f| f.0 == day).copied();
        let day_len = DAY + cfg.day_jitter.get(day as usize).copied().unwrap_or(0) - fz.map_or(0, |f| f.2);
        let win = match fz {
            Some((_, s, len)) if s < cfg.window => cfg.window - len.min(cfg.window - s),
            _ => cfg.window,
        };
        for k in 0..day_len {
            if fz.is_some_and(|f| f.1 == k) {
                frozen_gt += fz.map_or(0, |f| f.2) * 100;
            }
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
                let (hold_open, exit_fall) = match prev_rel {
                    None => (i64::MIN / 4, i64::MIN / 4),
                    Some(pr) => {
                        let reset = pr + P1_TO_EXIT;
                        let u_det = det_off(prev_set, pr + P1_LEAVE_DET);
                        let u_exit = det_off(reset, reset + 1);
                        (if cfg.hold_no_exit { reset.max(u_det) } else { reset.max(u_det).max(u_exit) }, u_exit)
                    }
                };
                let pass = (leg + LEG_TO_HOLD).max(hold_open);
                // (ironspec 12:11p (1): the least gap between two consecutive nappers' arrivals at the second hold, against
                // b8949c96's misroute threshold, with and without the arbiter)
                let arr = leg + LEG_TO_HOLD;
                if prev_arr > i64::MIN / 4 {
                    o.min_hold_gap = o.min_hold_gap.min(arr - prev_arr);
                }
                prev_arr = arr;
                // (G1 on 8e3d5f208 (2): the lag from the FALL of the previous napper's RESET LEVEL ((9,22,3) unpowered at its
                // 20-gt recheck after the cart left it) to this napper's SET edge: > 0 means the SET never lands while the
                // reset-dominant RESET holds; its control (`hold_no_exit`) drops the hold's (9,22,3) term; and a napper
                // reaching the hold while the previous one is still at it (it waits BEHIND the hold, on the flat top
                // (9,25,8)): counted)
                if prev_rel.is_some() {
                    let lag = (pass + HOLD_TO_SET) - exit_fall;
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
                // (THE RECORD's entry, measured per tick (ironspec 12:47p (4)): written at the pick's REL + 2 (from G at the
                // previous REL + 92 / + 102), shifted at REL' + 61 where REL' is the first REL whose napper hold (REL' + 67)
                // comes at or after the napper's arrival there: its stop release (the bed free again) + the stop -> (9,0,8)
                // trip (Cfg::arm3_trip, swept until measured))
                {
                    let arr = bed_free + cfg.arm3_trip;
                    let relp = (arr - 67 + 99).div_euclid(100);
                    o.record_spans.push((tp * 100 + 2, relp * 100 + 61));
                }
                let wait = start - arrive;
                o.max_bed_wait = o.max_bed_wait.max(wait);
                if wait > 0 {
                    bed_waits.push((arrive, start));
                }
                v += (wait + 99) / 100;
                // (the nap is the villager's in the cart now; a mismatch's pair swaps villagers after its second nap)
                o.naps[who[c]].push(start + frozen_gt);
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
                // (THE ARM3 STAMP (cc767b78 12:21 / ironspec 12:30p (2)): the napper is taken at the station row's hold one REL
                // before its siding arrival at the latest (the hold at REL' + 67, the S-bend at REL' + 165..200): the
                // record's entry lives from the pick to here (the judge's bookkeeping by cart, not a controller read))
                for e in open_nappers.iter_mut().filter(|e| e.0 == c && e.1.is_none()) {
                    e.1 = Some(tp + v - 1);
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
            let mut sensed_arr: Vec<(usize, usize, Option<usize>)> = Vec::new();
            for &(at, l, c, pred) in &transit {
                if at < t || (at == t && !(cfg.late_entries && pred.is_none())) {
                    match (cfg.merge, pred) {
                        (Merge::Order, Some(p)) => {
                            siding.push((l, c, Some(p), t));
                            nappers_out -= 1;
                        }
                        (Merge::TwoTrack, Some(_)) => napped[l].push_back(c),
                        _ => {
                            lanes[l].push_back(c);
                            // (THE SENSED POINTER: a circuit entry pops its mark from the lane's FIFO, in order (carts never
                            // overtake); the mark rides with this arrival event to the rejoin rule below)
                            if cfg.sensed && pred.is_none() {
                                let mk = mark_fifo[l].pop_front().flatten();
                                sensed_arr.push((l, c, mk));
                            }
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
                // (THE SENSED REJOIN: this circuit entry's mark (its pop found the next slot's X skipped: this cart is an
                // away napper's predecessor) AND a napper of this lane waiting in the siding (its R hold's presence): the
                // siding's FRONT napper is released behind it, and the mark's X type is flagged for extraction at its
                // next tap arrival; the identity rule (`ready`) is kept as the reference, its disagreements counted)
                let rel: Option<(usize, usize)> = if cfg.sensed {
                    let circ = sensed_arr.iter().position(|e| e.0 == l && e.1 == c).map(|i| sensed_arr.remove(i));
                    let sx = if cfg.positional_release {
                        // (THE POSITIONAL RELEASE (the simpler hardware: no mark FIFO, no X types, no extract flags): at a
                        // circuit entry, an X in the dropper 2 steps DOWNSTREAM of the tap (the entering cart's successor
                        // slot: popped 2 RELs ago, its successor skipped) AND a napper in the siding: the siding's front
                        // napper released, that X lifted out at once by the extractor under that dropper)
                        let s2 = (ring_next[l] + N0 - 2) % N0;
                        match (circ, x_set[l].iter().position(|x| x.0 == s2), siding.iter().position(|s| s.0 == l)) {
                            (Some(_), Some(xi), Some(si)) => {
                                x_set[l].remove(xi);
                                Some((siding[si].1, 2))
                            }
                            _ => None,
                        }
                    } else {
                        circ.and_then(|e| e.2).and_then(|ty| siding.iter().position(|s| s.0 == l).map(|i| (siding[i].1, ty)))
                    };
                    if ready.map(|j| away[j].1) != sx.map(|x| x.0) {
                        o.rejoin_vs_ref += 1;
                    }
                    sx
                } else {
                    ready.map(|j| (away[j].1, usize::MAX))
                };
                if let Some((x, ty)) = rel {
                    if let Some(j) = away.iter().position(|a| a.0 == l && a.1 == x) {
                        let s = away[j].3;
                        if !o.s_seen[l].contains(&s) {
                            o.s_seen[l].push(s);
                        }
                        away.remove(j);
                    }
                    if ty < 2 {
                        if ty != rejoin_type_next[l] {
                            o.type_order_mismatch += 1;
                        }
                        x_extract[l][rejoin_type_next[l]] = true;
                        rejoin_type_next[l] ^= 1;
                    }
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
            let in_window = k < win;
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
            open_nappers.retain(|e| e.1.is_none_or(|a| a > t));
            o.max_not_arm3 = o.max_not_arm3.max(open_nappers.len());
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
                // (THE SENSED POINTER: the slot this pop takes is the ring tap's; the judge checks it against the cart
                // actually popped (sync), the controller never reads `c`)
                let popped_slot = ring_next[l];
                if cfg.sensed && slot_cart[l][popped_slot] != c {
                    o.ring_mismatch += 1;
                }
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
                    if cfg.sensed || !gone[tg] {
                        break;
                    }
                    target[l] = Some(l * N0 + (tg - l * N0 + delta) % N0);
                }
                let spaced = since[l] >= cfg.period;
                let by_target = if cfg.sensed { req_armed[l] } else { cfg.pointer && target[l] == Some(c) && spaced };
                let forced = in_window && cfg.force_pick.contains(&(gt, l));
                let sched = !cfg.sensed && (!cfg.pointer || target[l].is_none()) && k_rel == next_pick[l];
                let mut due = forced || (in_window && (sched || due_repick || by_target));
                in_flight.retain(|&e| e > t);
                let open = pending.len() + assigned.iter().filter(|a| a.1 > gt).count();
                o.max_open = o.max_open.max(open);
                if due && cfg.sensed && by_target && cfg.grant_form == 2 && k_rel.rem_euclid(3) != l as i64 {
                    // (ROTATION: one lane eligible per REL, -1 / 3 / 7 in turn: a REQ off its turn is refused; the ring's T
                    // comes round again)
                    o.rate_deferred += 1;
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
                if due && cfg.sensed && by_target && (cfg.grant_form == 1 || cfg.grant_form == 3 || cfg.grant_form == 4) && picks_today - picks_before >= 1 {
                    // (THE ARBITER / STATIC PRIORITY: one grant per REL, lane -1 over 3 over 7 (the lanes' order here))
                    o.rate_deferred += 1;
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
                if due && cfg.sensed && by_target && cfg.grant_form == 4 && l == 0 && last_g7 == Some(t - 1) {
                    // (THE WIDENING (ironspec 12:47p (c)): lane -1 refused in the REL right after a lane-7 grant (that pair
                    // alone reaches the second hold 100 + leg(-1) - leg(7) gt apart; every other consecutive pair is wider))
                    o.rate_deferred += 1;
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
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
                if cfg.sensed {
                    // (THE ITEM RING's update at this pop, from its own state and the GRANT only: a grant moves T 5 slots on
                    // (the sorter lifts T out of the tap and feeds it 5 droppers upstream), puts an X of the next type in
                    // the picked slot and sets the spacing latch; then the ring steps: an X at the tap is skipped (one more
                    // step), unless its type's extract flag is set (the napper is back: the X is lifted out, the slot
                    // taken); T reaching the tap with the latch set clears the latch (its first pass ignored), with the
                    // latch clear it arms REQ; the pop's mark = the slot after it was a skipped X (this cart is that away
                    // napper's predecessor), pushed for a circuit pop (a cart not picked))
                    req_armed[l] = false;
                    if due {
                        if let Some(s) = refused_since[l].take() {
                            o.max_grant_wait[l] = o.max_grant_wait[l].max(t - s);
                        }
                        if x_set[l].iter().any(|x| x.1 == x_type_next[l]) {
                            o.x_conflict += 1;
                        }
                        x_set[l].push((popped_slot, x_type_next[l]));
                        x_type_next[l] ^= 1;
                        // (the record's depth bound (cc767b78 12:21): nappers picked and not yet back at their siding, an
                        // upper bound on picked-but-not-ARM3'd (ARM3 comes before the siding))
                        nappers_out += 1;
                        o.max_nappers_out = o.max_nappers_out.max(nappers_out);
                        open_nappers.push((c, None));
                        if l == 2 {
                            last_g7 = Some(t);
                        }
                        o.max_x = o.max_x.max(x_set[l].len());
                        t_slot[l] = (popped_slot + delta) % N0;
                        spacing_latch[l] = true;
                    }
                    let mut nx = (popped_slot + 1) % N0;
                    let mut mark: Option<usize> = None;
                    let mut steps = 0;
                    loop {
                        let Some(i) = x_set[l].iter().position(|x| x.0 == nx) else { break };
                        let ty = x_set[l][i].1;
                        if x_extract[l][ty] {
                            x_set[l].remove(i);
                            x_extract[l][ty] = false;
                            break;
                        }
                        if steps == 0 {
                            mark = Some(ty);
                        }
                        if nx == t_slot[l] && spacing_latch[l] {
                            spacing_latch[l] = false;
                            o.t_first_pass += 1;
                        }
                        nx = (nx + 1) % N0;
                        steps += 1;
                        if steps > N0 {
                            o.x_conflict += 1;
                            break;
                        }
                    }
                    ring_next[l] = nx;
                    if nx == t_slot[l] {
                        if spacing_latch[l] {
                            spacing_latch[l] = false;
                            o.t_first_pass += 1;
                        } else {
                            req_armed[l] = true;
                        }
                    }
                    if !due {
                        mark_fifo[l].push_back(mark);
                    }
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
                        let mut leg = gt + leg_t[l] + dly.map_or(0, |r| r.3) + cfg.leg_shift[l];
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
                    o.naps[c].push(nap_at + frozen_gt);
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
    // (the record's measured depth: the most entries alive at once over the per-tick spans)
    {
        let mut ev: Vec<(i64, i64)> = Vec::new();
        for &(w, s) in &o.record_spans {
            ev.push((w, 1));
            ev.push((s, -1));
        }
        ev.sort();
        let mut cur = 0i64;
        for (_, d) in ev {
            cur += d;
            o.max_open_measured = o.max_open_measured.max(cur);
        }
    }
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
                // (G1 on 8e3d5f208 (1): the resync serves only lost-napper recovery (a player breaking a cart): OUT, dropped)
                let sc = v.scope(&format!("ctl-pick: {}: the record's desync without THE RESYNC", cfg.name), crate::ctlcredit::OUT_WHY);
                lines.push(format!("  OUT ({}): {} mismatches, distinct {:?}: {sc}", cfg.name, o.record_mismatch.len(), o.distinct.iter().max()));
            }
            if design && cfg.name.contains("THE RECORD's RESYNC") {
                let sc = v.scope(&format!("ctl-pick: {}: THE LANE RECORD after the loss", cfg.name), crate::ctlcredit::OUT_WHY);
                lines.push(format!("  OUT ({}): {} mismatches, {} resyncs, distinct up to {:?}: {sc}", cfg.name, o.record_mismatch.len(), o.resyncs, o.distinct.iter().max()));
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
        // (THE NORMAL-OPERATION PICKS FORM, priced for the scope reset (ironspec 11:20a: no cells for OUT cases): in normal
        // operation every lane has 8 carts and nothing defers a pick (the room term and HELD defer none on the normal
        // nights), so the pointer's next target (5 positions on) comes up exactly every 13 RELs: THE SAME PICKS as a
        // per-lane 13-REL timer (13 = 5 mod 8, coprime with 8), window-gated, staggered 0 / 4 / 8; no pointer, no HELD, no
        // room count; the pick-rate arbiter kept (ironspec 11:05a). Printed on the normal nights and over the no-hold
        // sweep: the distinct nappers per night and the worst interval)
        {
            let timer = |name: &str, period: i64, cyc: i64, extra: i64, merge: Merge| {
                let mut c = Cfg::new(name, period, 120, merge);
                c.pointer = false;
                c.days = 10;
                c.rejoin_at = 1;
                c.bed = true;
                c.max_picks_per_rel = 1;
                c.bed_cycle = cyc;
                c.rs_extra = extra;
                c
            };
            let mut pts = Vec::new();
            let mut all_ok = true;
            for merge in [Merge::Order, Merge::Back] {
                for period in [9i64, 11, 12, 13] {
                    for (cyc, extra) in [(333i64, 0i64), (197, 40)] {
                        let o = run(&timer("the timer form", period, cyc, extra, merge));
                        if period == 13 && merge == Merge::Order {
                            all_ok &= ok(&o);
                        }
                        pts.push(format!("{merge:?} period {period}, cycle {cyc} + {extra}: distinct per night {:?}, worst {} gt{}", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.max_gap.0, if ok(&o) { "" } else { " MISSES the nap bars" }));
                    }
                }
            }
            let sc = v.scope("ctl-pick: the normal-operation picks form (per-lane 13-REL timers)", &format!("priced for ironspec's ruling (the cells: three 13-REL counters + the arbiter + the record, vs three pointers + spacing + HELD + the room count); every point passes: {all_ok}"));
            lines.push(format!("  PRICED: THE NORMAL-OPERATION PICKS FORM (per-lane 13-REL timers, window-gated, staggered 0 / 4 / 8, the arbiter; no pointer / HELD / room count): {}: {sc}", pts.join("; ")));
        }
        // (THE SENSED POINTER, the (i-b) per-lane item ring (ironspec 11:56a / the parent 11:57): the decision reads only
        // the ring's state and the grant; the rejoin is the sensed rule; HELD and the room count are OUT (they defer no pick
        // on the normal nights), the pick-rate arbiter kept. Judged on the normal nights and the no-hold sweep (the bed
        // cycle x (B)'s travel): 24 distinct per night, every interval < 24,000 - margin, the ring in sync with the carts
        // actually popped (the judge's check), no X type conflict; the villager-gt past 24,000 per day printed (the golem
        // loss proxy: 0 = every villager summon-eligible all day). THE FORCED TEST: the carts' ids permuted inside each lane
        // (slot i carries id l * N0 + 7 - i): the sensed form's results are unchanged; its CONTROL: the identity pointer
        // (target = id + 5) under the same permutation changes its picks)
        {
            let sensed_cfg = |name: &str, relabel: bool, cyc: i64, extra: i64| {
                let mut c = design(name, 0, None);
                c.pointer = false;
                c.sensed = true;
                c.relabel = relabel;
                c.held = 0;
                // ((b') STATIC PRIORITY, the smallest passing one-pick-per-REL form (the forms priced below on the same rows))
                c.max_picks_per_rel = 0;
                c.grant_form = 3;
                c.bed_cycle = cyc;
                c.rs_extra = extra;
                c
            };
            let past = |o: &Out| -> i64 {
                o.naps.iter().map(|n| n.windows(2).map(|w| (w[1] - w[0] - 24_000).max(0)).sum::<i64>()).sum::<i64>() / 10
            };
            let mut pts = Vec::new();
            let mut all_ok = true;
            let mut forced_ok = true;
            for (cyc, extra) in [(333i64, 0i64), (197, 0), (197, 40), (197, 120), (168, 0), (168, 40), (168, 120)] {
                let o = run(&sensed_cfg("the sensed ring", false, cyc, extra));
                let p = run(&sensed_cfg("the sensed ring, ids permuted", true, cyc, extra));
                let good = ok(&o) && o.ring_mismatch == 0 && o.x_conflict == 0 && o.type_order_mismatch == 0;
                all_ok &= good;
                let same = o.distinct == p.distinct && o.max_gap.0 == p.max_gap.0 && p.ring_mismatch == 0;
                forced_ok &= same;
                pts.push(format!("cycle {cyc} + {extra}: distinct per night {:?} (night 0, the cold start: {}), worst {} gt, villager-gt past 24,000 per day {} (golems lost per day {}), ring out of sync {}, rejoins unlike the identity rule's {}, rejoins out of pick order {}, X conflicts {}, the most X in a ring {}, T's ignored first passes {}, rate-deferred {}; permuted ids: distinct {:?}, worst {} gt ({}){}", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.distinct.first().copied().unwrap_or(0), o.max_gap.0, past(&o), if past(&o) == 0 { "0".to_string() } else { "> 0".to_string() }, o.ring_mismatch, o.rejoin_vs_ref, o.type_order_mismatch, o.x_conflict, o.max_x, o.t_first_pass, o.rate_deferred, p.distinct.iter().skip(1).copied().collect::<Vec<_>>(), p.max_gap.0, if same { "unchanged" } else { "CHANGED" }, if good { "" } else { " MISSES" }));
            }
            let w = v.bar("ctl-pick: THE SENSED POINTER (the (i-b) item ring: the decision reads the ring's state and the grant only) meets the nap bars on the normal nights and the no-hold sweep (24 distinct, every interval < 24,000 - margin), its ring in sync with the carts popped, no X type conflict", all_ok);
            lines.push(format!("  THE SENSED POINTER (the (i-b) per-lane item ring; HELD and the room count OUT, the arbiter kept): {}: {w}", pts.join("; ")));
            // (THE ONE-PICK-PER-REL FORMS (ironspec 12:12p / 12:14p / 12:16p; G1 12:15), each on the SAME rows: the seven
            // sweep points (night 0 the cold start, the three T tokens at the build's staggered slots 0 / 3 / 6) and TRUE
            // WHOLE-FARM FREEZES (G1 12:18 / ironspec 12:18p: a reload or unload stops the whole chunk, entities included,
            // while game time runs on: the farm REL count stops, the night window loses the frozen part, the naps' game-time
            // stamps move on) of 1 / 2 / 5 RELs at every 3rd farm REL of night 3 and at three daytime phases: (a) NONE (each
            // REQ granted), (b) ROTATION (one lane eligible per REL, -1 / 3 / 7 in turn; a refused REQ's T comes round
            // again), (b') STATIC PRIORITY (lane -1 over 3 over 7). Printed per form: the RELs carrying two grants, the least
            // gap between two nappers' arrivals at the second hold against its 41-gt cycle (b8949c96's misroute basis: two
            // nappers inside one hold cycle, the cycle = D 16 + 25, the interface's), the longest wait per lane from a
            // refused REQ to its grant (RELs, gt), the worst interval, the golems lost per day, every row's nap bars and
            // the ring's sync. (a) also over every starting offset of lanes 3 / 7's T against lane -1's (64 rows))
            let mut freeze_rows: Vec<(i64, i64, i64)> = Vec::new();
            for len in [1i64, 2, 5] {
                for s in (0..120i64).step_by(3) {
                    freeze_rows.push((3, s, len));
                }
                for s in [140i64, 180, 220] {
                    freeze_rows.push((3, s, len));
                }
            }
            let hold_cycle = d_occ() + 25;
            let mut form_rows = Vec::new();
            let mut a_ok = false;
            let mut b_extra = String::new();
            let mut a_worst = 0i64;
            for (form, fname) in [(0u8, "(a) NONE"), (2, "(b) ROTATION"), (3, "(b') STATIC PRIORITY"), (4, "(b'+w) STATIC PRIORITY + THE WIDENING (lane -1 refused in the REL after a lane-7 grant)")] {
                let (mut multi, mut least_gap, mut wait, mut worst, mut rows, mut all, mut lost, mut out_max) = (0usize, i64::MAX, [0i64; 3], 0i64, 0usize, true, 0i64, 0i64);
                let (mut arm3_max, mut worst_at, mut corner_gap, mut corner_worst, mut vs_ref) = (0usize, String::new(), i64::MAX, 0i64, 0usize);
                let mut first_bad = String::new();
                let mut eval = |o: &Out, what: &str, corner: bool| {
                    rows += 1;
                    multi += o.multi_pick.len();
                    least_gap = least_gap.min(o.min_hold_gap);
                    for l in 0..3 {
                        wait[l] = wait[l].max(o.max_grant_wait[l]);
                    }
                    if o.max_gap.0 > worst {
                        worst = o.max_gap.0;
                        worst_at = what.to_string();
                    }
                    if corner {
                        corner_gap = corner_gap.min(o.min_hold_gap);
                        corner_worst = corner_worst.max(o.max_gap.0);
                    }
                    out_max = out_max.max(o.max_nappers_out);
                    arm3_max = arm3_max.max(o.max_not_arm3);
                    vs_ref += o.rejoin_vs_ref;
                    lost += past(o);
                    let good = ok(o) && o.ring_mismatch == 0 && o.x_conflict == 0 && past(o) == 0 && o.min_hold_gap > hold_cycle;
                    if !good && first_bad.is_empty() {
                        first_bad = format!("{what}: distinct {:?}, worst {} gt, sync {}, least hold gap {} gt", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.max_gap.0, o.ring_mismatch, o.min_hold_gap);
                    }
                    all &= good;
                };
                // ((b') also runs the POSITIONAL RELEASE on every row (the simpler ring hardware) and the +-2 gt corners on
                // EVERY row (every lane's pick -> hold leg shifted -2 / 0 / +2 gt: all 27 combinations))
                let variants: &[bool] = if form >= 3 { &[false, true] } else { &[false] };
                let corners: Vec<[i64; 3]> = if form >= 3 {
                    let mut v = Vec::new();
                    for s0 in [-2i64, 0, 2] {
                        for s1 in [-2i64, 0, 2] {
                            for s2 in [-2i64, 0, 2] {
                                v.push([s0, s1, s2]);
                            }
                        }
                    }
                    v
                } else {
                    vec![[0, 0, 0]]
                };
                for &pos in variants {
                    let tag = if pos { ", the positional release" } else { "" };
                    for &sh in &corners {
                        let corner = sh != [0, 0, 0];
                        let ctag = if corner { format!(", legs {:+} / {:+} / {:+} gt", sh[0], sh[1], sh[2]) } else { String::new() };
                        for (cyc, extra) in [(333i64, 0i64), (197, 0), (197, 40), (197, 120), (168, 0), (168, 40), (168, 120)] {
                            let mut c = sensed_cfg("a one-pick-per-REL form", false, cyc, extra);
                            c.grant_form = form;
                            c.positional_release = pos;
                            c.leg_shift = sh;
                            eval(&run(&c), &format!("cycle {cyc} + {extra}{ctag}{tag}"), corner);
                        }
                        for &fz in &freeze_rows {
                            let mut c = sensed_cfg("a one-pick-per-REL form, a true freeze", false, 333, 0);
                            c.grant_form = form;
                            c.positional_release = pos;
                            c.leg_shift = sh;
                            c.freeze = vec![fz];
                            eval(&run(&c), &format!("a {}-REL freeze at farm REL {} of night {}{ctag}{tag}", fz.2, fz.1, fz.0), corner);
                        }
                        for o3 in 0..N0 {
                            for o7 in 0..N0 {
                                let mut c = sensed_cfg("a one-pick-per-REL form, the T offsets swept", false, 197, 40);
                                c.grant_form = form;
                                c.positional_release = pos;
                                c.leg_shift = sh;
                                c.t_start = [0, o3, o7];
                                eval(&run(&c), &format!("T at 0 / {o3} / {o7}{ctag}{tag}"), corner);
                            }
                        }
                    }
                }
                if form == 3 {
                    a_ok = all;
                    a_worst = worst;
                    b_extra = format!("; (b') at the +-2 gt corners: the least H2 gap {corner_gap} gt (margin {} over the {hold_cycle}-gt cycle), the worst interval {corner_worst} gt (margin {} under 23,000); the worst interval's row: {worst_at}; the most nappers picked and not yet ARM3'd {arm3_max} (the record's stage count); the positional release's rejoins unlike the identity rule's (with the FIFO's): {vs_ref}", corner_gap - hold_cycle, 24_000 - MARGIN - corner_worst);
                }
                if form == 4 {
                    b_extra.push_str(&format!("; PRICED, (b'+w) THE WIDENING at the +-2 gt corners: the least H2 gap {corner_gap} gt (margin {} over the {hold_cycle}-gt cycle), the worst interval {corner_worst} gt (margin {} under 23,000), its row {worst_at}, every row passing: {all}; its cells: one repeater chain holding lane 7's grant one REL into lane -1's gate (a 100-gt delay line or a latch cleared at the next REL + 33)", corner_gap - hold_cycle, 24_000 - MARGIN - corner_worst));
                }
                form_rows.push(format!("{fname} ({rows} rows): RELs carrying two grants {multi}, the least arrival gap at the second hold {least_gap} gt (its cycle {hold_cycle} gt), the longest wait per lane from a refused REQ to its grant {:?} RELs ({:?} gt; lane 7's {} RELs: a refused T waits a ring revolution, 8 pops), the worst interval {worst} gt, villager-gt past 24,000 {lost} (golems lost per day {}), the most nappers picked and not back at their siding {out_max}, every row passing: {all}{}", wait, wait.map(|w| w * 100), wait[2], if lost == 0 { "0" } else { "> 0" }, if first_bad.is_empty() { String::new() } else { format!(" (first miss: {first_bad})") }));
            }
            let n_rows = 27 * 2 * (7 + freeze_rows.len() + (N0 * N0) as usize);
            let w = v.bar_n("ctl-pick: THE SENSED POINTER under (b') STATIC PRIORITY (lane -1 over 3 over 7, stateless: the smallest passing one-pick-per-REL form) over the sweep, true whole-farm freezes at every 3rd REL of a night and every T starting offset: the nap bars, the ring in sync, 0 golems lost, never two nappers inside one second-hold cycle", n_rows, a_ok);
            lines.push(format!("  THE ONE-PICK-PER-REL FORMS (the sensed ring; the same nap model; (b') with the positional release and the +-2 gt corners too, {n_rows} rows): {}: (b')'s worst interval {a_worst} gt{b_extra}: {w}", form_rows.join("; ")));
            let n_pts = freeze_rows.len();
            // (THE RECORD's MEASURED DEPTH (ironspec 12:47p (4)): the most entries alive at once per tick (written at the pick's
            // REL + 2, shifted at REL' + 61), under (b'+w), over the seven points and the true freezes, the stop -> (9,0,8)
            // trip swept 40 / 80 / 120 gt until measured)
            let mut depth = Vec::new();
            for trip in [40i64, 80, 120] {
                let mut m = 0i64;
                for (cyc, extra) in [(333i64, 0i64), (197, 0), (197, 40), (197, 120), (168, 0), (168, 40), (168, 120)] {
                    let mut c = sensed_cfg("the record's depth", false, cyc, extra);
                    c.grant_form = 4;
                    c.arm3_trip = trip;
                    m = m.max(run(&c).max_open_measured);
                }
                for &fz in &freeze_rows {
                    let mut c = sensed_cfg("the record's depth, a true freeze", false, 333, 0);
                    c.grant_form = 4;
                    c.arm3_trip = trip;
                    c.freeze = vec![fz];
                    m = m.max(run(&c).max_open_measured);
                }
                depth.push(format!("trip {trip} gt: {m}"));
            }
            let sc = v.scope("ctl-pick: the record's measured depth (picks written minus ARM3 shifts, per tick)", "cc767b78's stage count reads it; the stop -> (9,0,8) trip is swept until 58f770cc / b8949c96 measure it");
            lines.push(format!("  THE RECORD's MEASURED DEPTH under (b'+w) (the most entries alive at once, per tick; {} rows per trip): {}: {sc}", 7 + n_pts, depth.join(", ")));
            // (THE TIMING CORNER (ironspec 12:02p (1)): every circuit entry processed after the same REL's pops (any shift of
            // an entry past a pop, 2 gt included), at every point of the sweep: the results and the ring's sync)
            let mut corner = Vec::new();
            let mut corner_ok = true;
            for (cyc, extra) in [(333i64, 0i64), (197, 0), (197, 120), (168, 0), (168, 120)] {
                let mut c = sensed_cfg("the sensed ring, entries after the pops", false, cyc, extra);
                c.late_entries = true;
                let o = run(&c);
                let good = ok(&o) && o.ring_mismatch == 0 && o.x_conflict == 0 && o.type_order_mismatch == 0;
                corner_ok &= o.ring_mismatch == 0 && o.type_order_mismatch == 0 && o.x_conflict == 0;
                corner.push(format!("cycle {cyc} + {extra}: distinct {:?}, worst {} gt, sync {}, type order {}, stalls {}, the shallowest lane {:?}{}", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.max_gap.0, o.ring_mismatch, o.type_order_mismatch, o.stalls, o.min_depth, if good { "" } else { " MISSES" }));
                let mut r = design("the identity pointer, entries after the pops", 0, None);
                r.held = 0;
                r.max_picks_per_rel = 1;
                r.bed_cycle = cyc;
                r.rs_extra = extra;
                r.late_entries = true;
                let ro = run(&r);
                corner.push(format!("  (the identity pointer there: distinct {:?}, worst {} gt)", ro.distinct.iter().skip(1).copied().collect::<Vec<_>>(), ro.max_gap.0));
            }
            let sc = v.scope("ctl-pick: the sensed pointer with EVERY circuit entry after the same REL's pops (a stress row, not the +-2 gt corner)", &format!("the entries (the S-bend D presses: slot l's J(l) arrival REL + {} / {} / {} + the branch) precede the same lane's next pop (REL + 200 / 217 / 227) by >= ~37 gt, so a +-2 gt shift never reorders them and the REL model's order stands; this row delays EVERY entry past those pops (every rejoin chance resolved late): both pointer forms then reach a worst interval of 24,000+ gt: the nap bar's margin depends on the rejoin race (a returning napper vs its predecessor's entry), the composed battery's gt-level corner (step b) judges it; all rows in sync: {corner_ok}", crate::ctlcredit::SLOT_T[0][0], crate::ctlcredit::SLOT_T[1][1], crate::ctlcredit::SLOT_T[2][2]));
            lines.push(format!("  THE ENTRIES-AFTER-THE-POPS STRESS ROW (every rejoin chance resolved late; not the +-2 gt corner): {}: {sc}", corner.join("; ")));
            let w = v.bar("ctl-pick: THE FORCED IDENTITY-PEEK TEST: the carts' ids permuted inside each lane leave the sensed pointer's results unchanged at every point", forced_ok);
            lines.push(format!("  THE FORCED TEST (ids permuted, slot i carrying id l * 8 + 7 - i): every point unchanged: {w}"));
            let mut idc = design("the identity pointer (control)", 0, None);
            idc.held = 0;
            idc.max_picks_per_rel = 1;
            let a = run(&idc);
            idc.relabel = true;
            let b = run(&idc);
            let bites = a.distinct != b.distinct || a.max_gap.0 != b.max_gap.0 || a.naps.iter().map(|n| n.len()).collect::<Vec<_>>() != b.naps.iter().map(|n| n.len()).collect::<Vec<_>>();
            let c = v.control("ctl-pick CONTROL (the parent 11:49): THE IDENTITY POINTER (target = the picked cart's id + 5) under the same permutation CHANGES its picks by name", 1, bites).to_string();
            lines.push(format!("  CONTROL (the identity pointer, ids permuted): distinct {:?} -> {:?}, worst {} -> {} gt: {c}", a.distinct.iter().skip(1).copied().collect::<Vec<_>>(), b.distinct.iter().skip(1).copied().collect::<Vec<_>>(), a.max_gap.0, b.max_gap.0));
        }
        let w = v.bar("ctl-pick: THE WAY-IN ROOM TERM never defers a pick on the normal nights (the design unchanged: 24 distinct, every interval < 24,000 - margin) and the way-in holds at most 3", ok(&on) && on.wayin_deferred == 0 && on.max_wayin <= 3);
        lines.push(format!("  THE WAY-IN on the normal nights (the room term on): at most {} nappers in the way-in at once (margin {} vs its 3 places: the front + P1 + the second hold (9,24,7)), the most dispatched-not-departed {}, picks deferred by it {}: {w}", on.max_wayin, 3i64 - on.max_wayin as i64, on.max_open, on.wayin_deferred));
        // (the way-in's timings under (B), printed: the longest P1 wait past its earliest release (the token / front clear
        // / OCCUPIED), the longest front wait; the hardware-only check: every room-count change applied at or after its
        // sensed event (a negative lag = a read of a future tick, failing by name))
        let lag_bad = on.event_lag.iter().filter(|(at, ev)| at < ev).count();
        let sc = v.scope("ctl-pick: the room count's hardware-only check (every room-count change at or after its sensed event)", "the room count is SUPERSEDED (ironspec 11:48a / the parent 11:57): no cells, no bar (G1 on 568bfd522 (2) dissolves with it)");
        lines.push(format!("  THE WAY-IN under (B): the longest P1 wait past its earliest release {} gt, the longest front wait {} gt; (the superseded room count's changes {}, before their event: {lag_bad}): {sc}", on.max_hold_wait, on.max_front_wait, on.event_lag.len()));
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
        let sc = v.scope("ctl-pick: the stop stalled 2,000 / 6,000 gt (a long pause) and the way-in's 4th napper", crate::ctlcredit::OUT_WHY);
        lines.push(format!("  OUT (a long pause): THE WAY-IN with the stop stalled ({n} rows): at most {worst} nappers in the way-in ({worst_at}){}: {sc}", if miss.is_empty() { String::new() } else { format!(" ({})", miss.join(" | ")) }));
        // ((B)'s two prints, judged on NORMAL rows (G1 on 8e3d5f208 (2); the stall rows are a long pause, OUT): THE RESET ->
        // SET LAG from the FALL of the previous napper's RESET level to the next napper's SET, over the normal nights and the
        // no-hold sweep (the bed cycle 197 / 168 gt x (B)'s travel + 0 / 40 / 120): > 0 everywhere; A NAPPER WAITING BEHIND
        // THE HOLD (reaching it while the previous one is still there, resting on the flat top (9,25,8)): none. Its CONTROL:
        // the hold's (9,22,3) term dropped (`hold_no_exit`) on the same rows: a SET inside the RESET level by name)
        let _ = (least_lag, behind);
        let (mut nl, mut nb, mut nrows) = (on.reset_set_lag, on.behind_hold, 1usize);
        let mut cl: Option<i64> = {
            let mut cc = design("normal, the hold without its (9,22,3) term", 0, None);
            cc.hold_no_exit = true;
            run(&cc).reset_set_lag
        };
        for cyc in [197i64, 168] {
            for extra in [0i64, 40, 120] {
                let mut c = design("the no-hold sweep", 0, None);
                c.bed_cycle = cyc;
                c.rs_extra = extra;
                let o = run(&c);
                nrows += 1;
                if let Some(l) = o.reset_set_lag {
                    nl = Some(nl.map_or(l, |m| m.min(l)));
                }
                nb += o.behind_hold;
                c.hold_no_exit = true;
                if let Some(l) = run(&c).reset_set_lag {
                    cl = Some(cl.map_or(l, |m| m.min(l)));
                }
            }
        }
        // (the PLANTED row where the (9,22,3) term is load-bearing: three nappers picked in one REL on b8949c96's measured
        // 16-gt spacing (the legs 132 / 148 / 164), so the second and third reach the hold while the one before is still at
        // P1: the design holds each until the RESET level falls; the control lets it pass at the RESET press)
        let mut lags_planted = Vec::new();
        for g in [100i64, 500, 900] {
            let mut c = design("planted: three picks in one REL, the measured spacing", 0, None);
            c.force_pick = vec![(g, 0), (g, 1), (g, 2)];
            c.leg_override = Some([132, 148, 164]);
            let o = run(&c);
            nrows += 1;
            if let Some(l) = o.reset_set_lag {
                nl = Some(nl.map_or(l, |m| m.min(l)));
            }
            c.hold_no_exit = true;
            let oc = run(&c);
            if let Some(l) = oc.reset_set_lag {
                cl = Some(cl.map_or(l, |m| m.min(l)));
            }
            lags_planted.push(format!("at gt {g}: {:?} (control {:?})", o.reset_set_lag, oc.reset_set_lag));
        }
        let w = v.bar_n("ctl-pick: (B)'s RESET -> SET LAG (from the RESET level's FALL) over the normal nights, the no-hold sweep and the planted three-in-one-REL rows is > 0 (the SET never lands while the reset-dominant RESET holds) and NO NAPPER WAITS BEHIND THE HOLD on the normal rows", nrows, nl.is_some_and(|l| l > 0) && nb == 0);
        lines.push(format!("  (B): the least lag from the RESET level's fall to the next SET {nl:?} gt over {nrows} rows (the planted rows {}); nappers waiting behind the hold on the normal rows {nb}: {w}", lags_planted.join(", ")));
        let c = v.control("ctl-pick CONTROL (G1 on 8e3d5f208 (2)): the second hold WITHOUT its (9,22,3) term on the same rows: A SET INSIDE THE RESET LEVEL by name (a lag <= 0)", 1, cl.is_some_and(|l| l <= 0)).to_string();
        lines.push(format!("  CONTROL (the hold without its (9,22,3) term): the least lag {cl:?} gt: {c}"));
        let sc = v.scope("ctl-pick: the way-in room term's control (a 4th napper dispatched into a full way-in with the stop stalled)", "the room count is SUPERSEDED (ironspec 11:48a / the parent 11:57: it defers no pick on the normal nights); the stall is a long pause (OUT)");
        lines.push(format!("  OUT (the room count superseded): no room term, the stop stalled: at most {worst_off} nappers in the way-in ({off_at}): {sc}"));
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
            // (G1 on 10a359319 (3): this model FORGETS a planted pick the rule refuses (no deferral modelled), so these rows
            // can't carry a fairness claim: cc767b78's arbiter car owns the deferral, its fairness and the priced sweep where
            // the rule fires; printed here as design inputs)
            let sc = v.scope("ctl-pick: bunched picks under one pick per REL", "the deferral and its fairness are cc767b78's arbiter car's (G1 on 10a359319 (3)); this model forgets a refused planted pick");
            lines.push(format!("  PRINTED: BUNCHED PICKS under one pick per REL ({rows_b} rows, a refused planted pick forgotten): nappers behind the hold {behind_d}, at most {wayin_d} in the way-in: {sc}"));
            let sc = v.scope("ctl-pick: today's form (no pick-rate rule), three picks in one REL on the measured 16-gt spacing", "b8949c96's redsim rest on lane 7's R hold is the evidence for the rule (ironspec 11:05a); printed");
            lines.push(format!("  PRINTED (no pick-rate rule): {behind_c} nappers behind the hold: {sc}"));
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
            let sc = v.scope("ctl-pick: the pick-rate rule over the no-hold sweep", &format!("the rule never fires on these points (0 multi-pick RELs at each), so a bar here would be vacuous (G1 on 10a359319 (3)); cc767b78's arbiter car prices it where it fires; every point meets the nap bars: {all_ok}"));
            lines.push(format!("  PRINTED: THE PICK-RATE RULE over the no-hold sweep ({} points): {}: {sc}", pts.len(), pts.join("; ")));
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
