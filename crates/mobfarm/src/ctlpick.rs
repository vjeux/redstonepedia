//! ctl-pick: the outbound PICK RULE at ring level (ironspec 22:41 / 22:49 / 23:06 (3) / 23:21 / 23:34 / 23:45 / 23:48 /
//! 23:57 / 00:01, G1 22:40 / 23:18 / 23:22 / 23:37 / 23:45 / 23:58): each lane's carts circulate FIFO, one per REL; a
//! pick sends the lane's REL cart (its villager) on the nap detour INSTEAD of its batch cart; the bar is on VILLAGERS:
//! every villager naps every night (24 distinct) and every nap-to-nap interval < 24,000 gt - margin, robust to a late
//! visit, a skipped pick, a missed REL, two consecutive late picks and the day length +-10 RELs.
//!
//! THE DESIGN (judged; G1 on 239acc610, non-blocking: the header names the adopted form): THE SENSED POINTER, the (i-b)
//! per-lane item ring (ironspec 11:56a / 12:02p): each lane's pick decision reads ONLY its ring's state (the tap's slot, T's
//! slot, the X markers, the spacing latch) and the grant, never a cart's identity; THE POSITIONAL RELEASE (the rejoin read
//! from the ring itself: an X two droppers downstream of the tap at a circuit entry, the X lifted out at the release);
//! (b'+w) STATIC PRIORITY + THE WIDENING (grant_form 4: one grant per REL, lane -1 over 3 over 7, and lane -1 refused in
//! the REL after a lane-7 grant); the one bed a single server behind the way-in as bought (P1 OCCUPIED form (B): the token
//! from sensed edges, the room's down-count from the front emptying); THE RECORD 3-deep (written at the pick's REL + 2,
//! shifted at REL' + 61). HELD, the room count and the identity pointer are superseded: printed beside, never the design.
//!
//! The model: time in RELs (100 gt each), a day 240 RELs, the window the first 120 (REST, ~12,010 gt, tick-exact 23:38);
//! a batch circuit 2 RELs; a napper's absence 9 RELs (lane 7 + 1, a late visit + 1: cc767b78 23:20's tap-quantized
//! absences); the judge reads the carts' ids (sync, rejoin references, the forced test), the controller never does.

use crate::ctlcredit::{J_TO_D, PERIOD, SLOT_T, TRAIN15_OFFSETS};
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

/// The ±2 gt timing corners (ironspec 12:02p (1)): the pick -> second-hold legs and the entry / pop separation are judged
/// against this many gt either way.
pub const CORNER: i64 = 2;

/// THE SECOND HOLD's CYCLE (a napper's pass -> the hold open again), SUMMED from the model's own constants (G1's B5 on
/// 239acc610: no duplicated "+ 25"): the SET (HOLD_TO_SET) + OCCUPIED's D (read from the interface) + P1 -> its exit
/// (P1_TO_EXIT) + the exit detector's recheck (DET_RECHECK); the bench judges it equal to the interface's stated total.
pub fn hold_cycle() -> i64 {
    HOLD_TO_SET + d_occ() + P1_TO_EXIT + DET_RECHECK
}

/// The interface's stated total for the second hold's cycle ("... the exit detector's 20-gt recheck = N gt", read from the
/// file with its line breaks folded).
pub fn iface_hold_cycle() -> i64 {
    let f = include_str!("../../../designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-controller-interface.md");
    let flat = f.split_whitespace().collect::<Vec<_>>().join(" ");
    let key = "the exit detector's 20-gt recheck = ";
    assert_eq!(flat.matches(key).count(), 1, "the interface states the second hold's total once");
    let at = flat.find(key).expect("the interface states the second hold's cycle") + key.len();
    flat[at..].split(' ').next().and_then(|s| s.parse().ok()).expect("the second hold's cycle is a number of gt")
}

/// The batch's slot release offsets after REL (lanes -1 / 3 / 7), READ from the interface ("- **release offsets = a / b /
/// c gt**", stated once there).
pub fn release_offsets() -> [i64; 3] {
    let f = include_str!("../../../designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-controller-interface.md");
    let key = "- **release offsets = ";
    assert_eq!(f.matches(key).count(), 1, "the interface states the release offsets once");
    let at = f.find(key).expect("the interface states the release offsets") + key.len();
    let v: Vec<i64> = f[at..].split(" gt**").next().unwrap_or("").split(" / ").filter_map(|s| s.trim().parse().ok()).collect();
    assert_eq!(v.len(), 3, "three release offsets");
    [v[0], v[1], v[2]]
}

/// THE CONTROLLER's REL TAPS (exact: repeater-chain taps off REL), as the interface states them (`iface_rel_tap` reads
/// each back in the bench): G_l captured at REL + 92 and HELD UNTIL REL' + 85 (cc767b78 13:18: the clear moved from + 33,
/// so the ring gates its later taps with G directly), the record written from G at REL + 102 and SHIFTED at REL + 61,
/// HOME latched by the core at REL' + 57.
pub const G_CAPTURE: i64 = 92;
pub const G_CLEAR: i64 = 85;
pub const REC_WRITE: i64 = 102;
pub const REC_SHIFT: i64 = 61;
pub const HOME_LATCH: i64 = 57;
/// W7 takes G7 at REL + 84 (cc767b78, via day-1's addendum 3 on G1's CHANGES 6 (2); not yet in the interface at this base:
/// its statement is day-1's to add).
pub const W7_TAKE: i64 = 84;
/// The ring's skip: a tap EMPTY this long after a step takes one more step (ctlring8's header, the logic's).
pub const SKIP_RECHECK: i64 = 20;

/// One REL tap as the interface states it: the number after `key` (its line breaks folded), `key` required exactly once.
pub fn iface_rel_tap(key: &str) -> Result<i64, String> {
    let f = include_str!("../../../designs/farms/iron/carousel-assembly/wip/controller/f1q2-day1-controller-interface.md");
    let flat = f.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = flat.matches(key).count();
    if n != 1 {
        return Err(format!("\"{key}\" on {n} places (exactly one required)"));
    }
    let at = flat.find(key).map(|i| i + key.len()).unwrap_or(0);
    flat[at..].split(|ch: char| !ch.is_ascii_digit()).next().and_then(|s| s.parse().ok()).ok_or_else(|| format!("\"{key}\" is not followed by a number"))
}

/// The napper's stop release (its departure from the front, the bed start) -> its arrival at the bed (gt): NOT NAMED by the
/// controller interface nor the stop module (f1q2-module-stop9.txt), so 0 (the arrival taken at the departure: the earliest
/// possible, so the occupied-bed margin below is its least); its +-CORNER spread is swept per napper.
pub const STOP_TO_BED: i64 = 0;
/// 58f770cc's MEASURED wake band (13:37, 408 visits, bedtier-bench on the re-laid bed tier): the lie-down (E's eject onto
/// the bed head, taken at the napper's bed arrival) -> the wake, 56..76 gt (its middle 66); the bed is EMPTY from the wake
/// tick itself (26.3's stopSleeping clears OCCUPIED that tick: no exit lag).
pub const WAKE_BAND: [i64; 3] = [56, 66, 76];
/// The wake band's +-2 / reload spread: OWED (58f770cc's fix car, ~14:45); 0 until measured, added to the band's ends.
pub const WAKE_SPREAD: i64 = 0;
/// The swept lie-down -> wake values: the band's ends widened by WAKE_SPREAD, and its middle.
pub fn wake_sweep() -> [i64; 3] {
    [WAKE_BAND[0] - WAKE_SPREAD, WAKE_BAND[1], WAKE_BAND[2] + WAKE_SPREAD]
}

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
    /// G1's B1 control on 239acc610: the OLD freeze stamp (a nap stamped with only the freezes the REL loop has REACHED
    /// when its start is fixed, not every freeze before its start in farm time).
    pub stamp_reached_freezes: bool,
    /// G1's B3 control on 239acc610: ONE IDENTITY PEEK inside the sensed form: REQ armed at T's arrival names the cart id
    /// T's slot held at the build (l * N0 + T's slot) and fires at the pop of THAT CART ID (the same pop when the ids are
    /// unpermuted and the ring is in sync; another cart when they are permuted).
    pub peek_id: bool,
    /// The bed-occupied bar's control (day-1's addendum, ironspec 13:34p): the stop's bed-free wait dropped (a napper
    /// departs the front as soon as it is there: start = its front arrival).
    pub no_bed_wait: bool,
    /// Day-1's job 4 (does normal running NEED the ring's SPACING latch?): the latch dropped, the only change: REQ_l = T at
    /// the tap, so T's first pass after a grant (5 pops on) can request.
    pub no_spacing: bool,
}

impl Cfg {
    pub fn new(name: &str, period: i64, window: i64, merge: Merge) -> Cfg {
        Cfg { name: name.to_string(), period, window, visit: 9, merge, repick: true, napped_first: false, rejoin_at: 2, pointer: false, day_jitter: vec![], held: 0, held_cap: 1, held_lane: None, ptr_step: 5, bed: false, halts: vec![], slip: vec![], late_picks: vec![], days: 8, missed: vec![], late: vec![], skipped: vec![], moved: vec![], ride_delay: vec![], skip_fix: true, lost_napper: vec![], resync: false, stop_stall: vec![], wayin_cap: 0, rs_extra: 0, bed_cycle: 333, missed_dep: vec![], extra_dep: vec![], gate_off: false, press_count: false, merge_picks: false, force_pick: vec![], max_picks_per_rel: 0, leg_override: None, sensed: false, relabel: false, late_entries: false, hold_no_exit: false, freeze: vec![], grant_form: 0, t_start: [0, 3, 6], positional_release: false, leg_shift: [0; 3], arm3_trip: 80, stamp_reached_freezes: false, peek_id: false, no_bed_wait: false, no_spacing: false }
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
    /// Grants whose REQ came from T's FIRST pass after the lane's previous grant (the pass the spacing latch ignores: only
    /// with `no_spacing`).
    pub first_pass_grants: usize,
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
    /// Every nap as stamped (G1's B1 on 239acc610: the stamp = the farm start + every freeze before it in farm time).
    pub nap_log: Vec<NapRec>,
    /// THE BED NEVER OCCUPIED AT A NAPPER's ARRIVAL (day-1's addendum, ironspec 13:34p, 58f770cc's measured wake band):
    /// per swept lie-down -> wake value (`wake_sweep()`), the consecutive bed pairs whose second napper arrives before the
    /// first one's wake (its arrival at its start + STOP_TO_BED - CORNER, the first's lie-down at its start + STOP_TO_BED +
    /// CORNER), and the least margin (the arrival - the wake), with the pairs judged.
    pub bed_occupied_at_arrival: [usize; 3],
    pub bed_margin_min: [Option<i64>; 3],
    pub bed_pairs: usize,
    /// Per lane: the fewest pops between two of the lane's grants (the spacing latch's "the >= 11-pop spacing", measured).
    pub min_pick_pops: [Option<i64>; 3],
    /// G1's CHANGES 6 on 03f431632 (1): the pair THE WIDENING forbids, counted by the judge from the farm REL count: a lane
    /// -1 grant in the REL right after a lane-7 grant.
    pub w7_pairs: usize,
    /// G1's CHANGES 6 on 03f431632 (2): every refused REQ / due pick: (night, its farm REL index, the lane's index, why).
    pub refusals: Vec<(i64, i64, usize, &'static str)>,
    /// The nappers' (9,0,8) arrivals (presses), and those within +-CORNER of the HOME latch .. the shift (REL + 57 - 2 ..
    /// REL + 61 + 2).
    pub arm3_arrivals: usize,
    pub arm3_near_home_shift: usize,
    /// Point 6 of the ctl-pick report (day-1's job 3): the arrivals after the shift but at or before the hold's release
    /// ((REL + 61, REL + 67]): the record model shifts them at that REL' + 61, before they arrive.
    pub arm3_after_shift: usize,
}

/// One nap as stamped (the judge's bookkeeping): the villager, its lane, the night and the farm REL index in it of its pick,
/// the pick's farm gt, the bed start (farm gt) and the nap's game-time stamp.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NapRec {
    pub villager: usize,
    pub lane: usize,
    pub day: i64,
    pub k: i64,
    pub pick_gt: i64,
    pub start: i64,
    pub stamp: i64,
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

/// A day's FARM length in RELs: DAY + its jitter - its freeze's length (a freeze's RELs pass in game time only).
fn day_len_of(cfg: &Cfg, day: i64) -> i64 {
    DAY + cfg.day_jitter.get(day as usize).copied().unwrap_or(0) - cfg.freeze.iter().find(|f| f.0 == day).map_or(0, |f| f.2)
}

pub fn run(cfg: &Cfg) -> Out {
    let n_rec_gt = n_rec();
    let d_occ_gt = d_occ();
    // (G1's B1 on 239acc610: every freeze's FARM tick (its farm REL; the farm clock stops while game time runs on), so a nap
    // is stamped with EVERY freeze before its start in farm time, whichever REL fixed that start (a bed start is fixed the
    // REL after its pick, often > 100 gt ahead); `stamp_reached_freezes` (the control) keeps the old stamp, the freezes the
    // loop had reached)
    let (mut day_t0, mut freeze_at): (Vec<i64>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    {
        let mut acc = 0i64;
        for day in 0..cfg.days {
            day_t0.push(acc);
            if let Some(f) = cfg.freeze.iter().find(|f| f.0 == day) {
                freeze_at.push((100 * (acc + f.1 + 1), f.2 * 100));
            }
            acc += day_len_of(cfg, day);
        }
    }
    let stamp = |t_farm: i64, reached: i64| -> i64 {
        if cfg.stamp_reached_freezes {
            t_farm + reached
        } else {
            t_farm + freeze_at.iter().filter(|f| f.0 <= t_farm).map(|f| f.1).sum::<i64>()
        }
    };
    // (the previous bed user's start: the occupied-bed check)
    let mut prev_bed_start: Option<i64> = None;
    // (the identity peek's awaited cart per lane (`peek_id`, G1's B3 control), and each lane's first grant seen)
    let mut peek_req: [Option<usize>; 3] = std::array::from_fn(|l| (cfg.t_start[l] == 0).then_some(l * N0));
    let mut has_picked: [bool; 3] = [false; 3];
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
    // (job 4: T's first pass after a grant still to come, and a REQ armed by that first pass)
    let (mut first_pass_due, mut req_first): ([bool; 3], [bool; 3]) = ([false; 3], [false; 3]);
    let mut req_armed: [bool; 3] = [cfg.t_start[0] == 0, cfg.t_start[1] == 0, cfg.t_start[2] == 0];
    // (the one-pick-per-REL forms' waits: per lane, the farm REL of its first refused REQ still pending)
    let mut refused_since: [Option<i64>; 3] = [None; 3];
    // (THE WIDENING's W7 (G1's CHANGES 6 on 03f431632 (2)): the HARDWARE latch exactly: set by G7's capture (a lane-7
    // grant), read by the next capture's decision (cc767b78: W7 takes G7 at REL + 84 just before G's clear, the next decide
    // at + 92 reads it), then overwritten by that capture; it is a block state: a freeze (no REL runs) keeps it, and a
    // REL that runs no decision here (a stall) keeps it too (the conservative reading: lane -1 refused once more). The
    // judge's own view (`last_g7`, the farm REL of the last lane-7 grant) counts the pairs the latch must refuse)
    let mut w7 = false;
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
        let day_len = day_len_of(cfg, day);
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
            // sensed edges up to now: the record must be empty; any entry left is cleared. G1 on 239acc610, non-blocking (dead
            // code in the design's runs): the resync is OUT (lost-napper recovery, G1 on 8e3d5f208 (1)), so its levels are
            // kept and read only on the rows that plant it)
            if cfg.resync {
                sensed.retain(|s| s.5 + 2_000 > gt);
                let latch_on = sensed.iter().any(|s| s.0 <= gt && gt < s.1);
                let front_on = sensed.iter().any(|s| s.2 <= gt && gt < s.3);
                let stop_busy = sensed.iter().any(|s| s.4 <= gt && gt < s.5);
                if !latch_on && !front_on && !stop_busy && t - last_pick_t >= 3 && (!record.is_empty() || !rec_hold.is_empty()) {
                    o.resyncs += 1;
                    record.clear();
                    rec_hold.clear();
                }
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
                    if let Some((a, _tok, _u0, r0)) = prev_front {
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
                let mut start = if cfg.no_bed_wait { at_front } else { at_front.max(bed_free) };
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
                if cfg.resync {
                    sensed.push((set, release + P1_TO_EXIT, at_front, u_front, start, start + cfg.bed_cycle + cfg.rs_extra));
                }
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
                    // (G1 on 03f431632, non-blocking: the napper's (9,0,8) arrival is a PRESS (asynchronous to REL); count
                    // the arrivals within the press corner of the HOME latch (REL + 57) or the shift (REL + 61))
                    let ph = arr.rem_euclid(100);
                    o.arm3_arrivals += 1;
                    if (HOME_LATCH - CORNER..=REC_SHIFT + CORNER).contains(&ph) {
                        o.arm3_near_home_shift += 1;
                    }
                    if ph > REC_SHIFT && ph <= 67 {
                        o.arm3_after_shift += 1;
                    }
                }
                let wait = start - arrive;
                o.max_bed_wait = o.max_bed_wait.max(wait);
                if wait > 0 {
                    bed_waits.push((arrive, start));
                }
                v += (wait + 99) / 100;
                // (THE BED NEVER OCCUPIED AT A NAPPER's ARRIVAL (day-1's addendum, ironspec 13:34p; 58f770cc's measured wake
                // band): this napper arrives at the bed at its start + STOP_TO_BED (- CORNER, the early corner), the previous
                // one lay down at its start + STOP_TO_BED (+ CORNER, the late corner) and wakes w later; the bed is empty from
                // the wake tick itself: occupied when the arrival comes before the wake)
                if let Some(ps) = prev_bed_start {
                    o.bed_pairs += 1;
                    for (i, w) in wake_sweep().into_iter().enumerate() {
                        let m = (start + STOP_TO_BED - CORNER) - (ps + STOP_TO_BED + CORNER + w);
                        if m < 0 {
                            o.bed_occupied_at_arrival[i] += 1;
                        }
                        o.bed_margin_min[i] = Some(o.bed_margin_min[i].map_or(m, |x: i64| x.min(m)));
                    }
                }
                prev_bed_start = Some(start);
                // (the nap is the villager's in the cart now; a mismatch's pair swaps villagers after its second nap; its
                // game-time stamp counts every freeze before its start in farm time: G1's B1)
                let st = stamp(start, frozen_gt);
                o.naps[who[c]].push(st);
                o.nap_day[who[c]].push(pday);
                o.nap_log.push(NapRec { villager: who[c], lane: l, day: pday, k: tp - day_t0[pday as usize] - 1, pick_gt: tp * 100, start, stamp: st });
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
            // (each arrival: (lane, cart, its SENSED circuit-entry event's index in `sensed_arr`, None for a rejoin); the
            // controller's rules read the event (lane, tick), never the cart's id (G1 on 03f431632, non-blocking))
            let mut arrived: Vec<(usize, usize, Option<usize>)> = Vec::new();
            let mut sensed_arr: Vec<(usize, i64, Option<usize>)> = Vec::new();
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
                            // overtake); the mark rides with this arrival event (the lane's D(x) press at this tick) to the
                            // rejoin rule below)
                            let ev = if cfg.sensed && pred.is_none() {
                                let mk = mark_fifo[l].pop_front().flatten();
                                sensed_arr.push((l, at, mk));
                                Some(sensed_arr.len() - 1)
                            } else {
                                None
                            };
                            arrived.push((l, c, ev));
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
            while let Some((l, c, ev)) = arrived.pop() {
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
                    let circ = ev.map(|i| sensed_arr[i]);
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
                    arrived.push((l, x, None));
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
            // (this REL's capture: G7 granted)
            let mut g7_now = false;
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
                let due_repick = !cfg.sensed && cfg.repick && repick[l] == Some(c);
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
                // (the sensed form reads its ring's REQ; G1's B3 control `peek_id` fires at the pop of the cart id the REQ names)
                let by_target = if cfg.sensed {
                    if cfg.peek_id { peek_req[l] == Some(c) } else { req_armed[l] }
                } else {
                    cfg.pointer && target[l] == Some(c) && spaced
                };
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
                    o.refusals.push((day, k, l, "ROTATION (off its turn)"));
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
                if due && cfg.sensed && by_target && (cfg.grant_form == 1 || cfg.grant_form == 3 || cfg.grant_form == 4) && picks_today - picks_before >= 1 {
                    // (THE ARBITER / STATIC PRIORITY: one grant per REL, lane -1 over 3 over 7 (the lanes' order here))
                    o.rate_deferred += 1;
                    o.refusals.push((day, k, l, "PRIORITY (a higher lane granted this REL)"));
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
                if due && cfg.sensed && by_target && cfg.grant_form == 4 && l == 0 && w7 {
                    // (THE WIDENING (ironspec 12:47p (c)): lane -1 refused at the capture after a lane-7 grant, read from the
                    // W7 latch (that pair alone reaches the second hold 100 + leg(-1) - leg(7) gt apart; every other
                    // consecutive pair is wider))
                    o.rate_deferred += 1;
                    o.refusals.push((day, k, l, "W7 (the widening: lane 7 granted at the last capture)"));
                    refused_since[l].get_or_insert(t);
                    due = false;
                }
                if due && cfg.max_picks_per_rel > 0 && picks_today - picks_before >= cfg.max_picks_per_rel {
                    // (THE PICK-RATE RULE (priced for b8949c96 10:57 / G1 10:59, bunching at the second hold): at most this
                    // many picks per REL: a further due lane defers to the next REL (a pointer target comes round again; a
                    // scheduled pick moves one period))
                    o.rate_deferred += 1;
                    o.refusals.push((day, k, l, "RATE (max picks per REL)"));
                    if !by_target && !due_repick {
                        next_pick[l] += cfg.period;
                    }
                    due = false;
                }
                if due && cfg.wayin_cap > 0 && room >= cfg.wayin_cap as i64 {
                    // (THE WAY-IN ROOM TERM: the way-in's dry places are all spoken for: the pick is DEFERRED like HELD's)
                    o.wayin_deferred += 1;
                    o.deferred_at.push(gt);
                    o.refusals.push((day, k, l, "ROOM (the way-in full)"));
                    if !by_target && !due_repick {
                        next_pick[l] += cfg.period;
                    }
                    due = false;
                }
                if due && cfg.held > 0 && in_flight.len() >= cfg.held_cap {
                    // (HELD: the station's one napper is still in flight: the pick is DEFERRED; a pointer's target comes
                    // round again, a scheduled pick moves to the next period)
                    deferred_today += 1;
                    o.refusals.push((day, k, l, "HELD (a napper in flight)"));
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
                    let from_first = req_first[l];
                    req_first[l] = false;
                    req_armed[l] = false;
                    if due {
                        if from_first && by_target {
                            o.first_pass_grants += 1;
                        }
                        peek_req[l] = None;
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
                        // (the judge's pair: a lane -1 grant in the REL right after a lane-7 grant, by the farm REL count)
                        if l == 0 && last_g7 == Some(t - 1) {
                            o.w7_pairs += 1;
                        }
                        if l == 2 {
                            last_g7 = Some(t);
                            g7_now = true;
                        }
                        o.max_x = o.max_x.max(x_set[l].len());
                        t_slot[l] = (popped_slot + delta) % N0;
                        spacing_latch[l] = !cfg.no_spacing;
                        first_pass_due[l] = true;
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
                        if nx == t_slot[l] {
                            first_pass_due[l] = false;
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
                        let first = std::mem::take(&mut first_pass_due[l]);
                        if spacing_latch[l] {
                            spacing_latch[l] = false;
                            o.t_first_pass += 1;
                        } else {
                            req_armed[l] = true;
                            req_first[l] = first;
                            // (the peek names the cart id T's slot held at the build)
                            peek_req[l] = Some(l * N0 + t_slot[l]);
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
                    // (the spacing measured (G1 on 239acc610, non-blocking): the pops since the lane's previous grant)
                    if has_picked[l] {
                        o.min_pick_pops[l] = Some(o.min_pick_pops[l].map_or(since[l], |m: i64| m.min(since[l])));
                    }
                    has_picked[l] = true;
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
                    let st = stamp(nap_at, frozen_gt);
                    o.naps[c].push(st);
                    o.nap_day[c].push(day);
                    o.nap_log.push(NapRec { villager: c, lane: l, day, k: t - day_t0[day as usize] - 1, pick_gt: gt, start: nap_at, stamp: st });
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
            // (THIS REL's capture writes W7: G7 granted or not)
            w7 = g7_now;
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

/// The nap bars: 24 distinct villagers every night from night 2 on, every nap-to-nap interval < 24,000 - MARGIN.
fn nap_ok(o: &Out) -> bool {
    o.distinct.iter().skip(1).all(|&d| d == 3 * N0) && o.max_gap.0 < 24_000 - MARGIN
}

/// The villager-gt past 24,000 per day (the golem-loss proxy) over the run's OWN days (G1 on 239acc610, non-blocking: not
/// an assumed 10).
fn golem_gt_per_day(o: &Out) -> i64 {
    o.naps.iter().map(|n| n.windows(2).map(|w| (w[1] - w[0] - 24_000).max(0)).sum::<i64>()).sum::<i64>() / (o.distinct.len().max(1) as i64)
}

/// Every nap's game-time stamp, sorted (G1's B3 on 239acc610: the forced test's EXACT comparison: the same naps at the same
/// ticks, whichever villager took each).
fn sorted_stamps(o: &Out) -> Vec<i64> {
    let mut s: Vec<i64> = o.naps.iter().flatten().copied().collect();
    s.sort_unstable();
    s
}

/// The seven sweep points: (the bed cycle, (B)'s travel) gt.
const POINTS: [(i64, i64); 7] = [(333, 0), (197, 0), (197, 40), (197, 120), (168, 0), (168, 40), (168, 120)];
/// The MEASURED stop -> (9,0,8) trip band (58f770cc's pickup -> stop out 35..48 gt + ironfarm's stop R -> (9,0,8) seated
/// 78 gt on V's depth), each end swept +-CORNER for the record's depth.
const ARM3_TRIP_BAND: [i64; 2] = [113, 126];

/// The netlist's spacing latch (set at the grant, cleared by T's first pass): ">= 11-pop spacing" between a lane's grants.
const SPACING_MIN: i64 = 11;

/// A design row's terms (G1's B2 on 239acc610), each failing BY NAME: the nap bars (NAP), the ring in sync (SYNC), no X
/// type conflict (X-CONFLICT), the rejoins in pick order (TYPE-ORDER), no golem lost (GOLEMS), the sensed rejoins equal to
/// the identity rule's (REJOIN-VS-REF), never two nappers inside one second-hold cycle (H2-GAP: the least arrival gap at
/// the hold > its cycle) and NO NAPPER REACHING THE HOLD WHILE THE PREVIOUS ONE STILL WAITS THERE (BEHIND-HOLD), no REL
/// with two grants (TWO-GRANTS), no lane -1 grant in the REL right after a lane-7 grant (W7-PAIR: G1's CHANGES 6 on
/// 03f431632 (1), the pair the widening forbids), at most 3 nappers in the way-in (WAY-IN), (B)'s RESET -> SET lag > 0
/// (LAG), the record's measured depth <= 3 (DEPTH), and the netlist's spacing latch's ">= 11-pop spacing" between a lane's
/// grants, measured (SPACING).
fn row_terms(o: &Out, hc: i64) -> Vec<&'static str> {
    let mut t = Vec::new();
    for (bad, name) in [
        (!nap_ok(o), "NAP"),
        (o.ring_mismatch != 0, "SYNC"),
        (o.x_conflict != 0, "X-CONFLICT"),
        (o.type_order_mismatch != 0, "TYPE-ORDER"),
        (golem_gt_per_day(o) != 0, "GOLEMS"),
        (o.rejoin_vs_ref != 0, "REJOIN-VS-REF"),
        (o.min_hold_gap <= hc, "H2-GAP"),
        (o.behind_hold != 0, "BEHIND-HOLD"),
        (!o.multi_pick.is_empty(), "TWO-GRANTS"),
        (o.w7_pairs != 0, "W7-PAIR"),
        (o.max_wayin > 3, "WAY-IN"),
        (o.reset_set_lag.is_none_or(|l| l <= 0), "LAG"),
        (o.max_open_measured > 3, "DEPTH"),
        (o.min_pick_pops.iter().any(|m| m.is_some_and(|m| m < SPACING_MIN)), "SPACING"),
    ] {
        if bad {
            t.push(name);
        }
    }
    t
}

/// One one-pick-per-REL form over its rows (G1's B2 on 239acc610): each row judged on `row_terms`, the failing rows counted
/// per term; the rows' figures (the least H2 gap, the waits, the worst interval, the record's depth, the lag, the way-in,
/// the spacing, the bed's occupancy per swept wake value) kept for the print.
struct FormRec {
    hc: i64,
    rows: usize,
    multi: usize,
    w7_pairs: usize,
    arm3_n: usize,
    arm3_near: usize,
    arm3_after_shift: usize,
    first_pass_grants: usize,
    distinct_min: usize,
    least_gap: i64,
    wait: [i64; 3],
    worst: i64,
    worst_at: String,
    corner_gap: i64,
    corner_worst: i64,
    lost: i64,
    out_max: i64,
    arm3_max: usize,
    depth_max: i64,
    vs_ref: usize,
    behind: usize,
    wayin_max: usize,
    lag_min: Option<i64>,
    spacing: [Option<i64>; 3],
    bed_occ: [usize; 3],
    bed_margin: [Option<i64>; 3],
    bed_pairs: usize,
    term_fails: std::collections::BTreeMap<&'static str, usize>,
    first_bad: String,
}

impl FormRec {
    fn new(hc: i64) -> FormRec {
        FormRec { hc, rows: 0, multi: 0, w7_pairs: 0, arm3_n: 0, arm3_near: 0, arm3_after_shift: 0, first_pass_grants: 0, distinct_min: usize::MAX, least_gap: i64::MAX, wait: [0; 3], worst: 0, worst_at: String::new(), corner_gap: i64::MAX, corner_worst: 0, lost: 0, out_max: 0, arm3_max: 0, depth_max: 0, vs_ref: 0, behind: 0, wayin_max: 0, lag_min: None, spacing: [None; 3], bed_occ: [0; 3], bed_margin: [None; 3], bed_pairs: 0, term_fails: Default::default(), first_bad: String::new() }
    }

    fn take(&mut self, o: &Out, what: &str, corner: bool) {
        self.rows += 1;
        self.multi += o.multi_pick.len();
        self.w7_pairs += o.w7_pairs;
        self.arm3_n += o.arm3_arrivals;
        self.arm3_near += o.arm3_near_home_shift;
        self.arm3_after_shift += o.arm3_after_shift;
        self.first_pass_grants += o.first_pass_grants;
        self.distinct_min = self.distinct_min.min(o.distinct.iter().skip(1).copied().min().unwrap_or(0));
        self.least_gap = self.least_gap.min(o.min_hold_gap);
        for l in 0..3 {
            self.wait[l] = self.wait[l].max(o.max_grant_wait[l]);
            if let Some(s) = o.min_pick_pops[l] {
                self.spacing[l] = Some(self.spacing[l].map_or(s, |m: i64| m.min(s)));
            }
        }
        if o.max_gap.0 > self.worst {
            self.worst = o.max_gap.0;
            self.worst_at = what.to_string();
        }
        if corner {
            self.corner_gap = self.corner_gap.min(o.min_hold_gap);
            self.corner_worst = self.corner_worst.max(o.max_gap.0);
        }
        self.out_max = self.out_max.max(o.max_nappers_out);
        self.arm3_max = self.arm3_max.max(o.max_not_arm3);
        self.depth_max = self.depth_max.max(o.max_open_measured);
        self.vs_ref += o.rejoin_vs_ref;
        self.lost += golem_gt_per_day(o);
        self.behind += o.behind_hold;
        self.wayin_max = self.wayin_max.max(o.max_wayin);
        if let Some(l) = o.reset_set_lag {
            self.lag_min = Some(self.lag_min.map_or(l, |m: i64| m.min(l)));
        }
        self.bed_pairs += o.bed_pairs;
        for i in 0..3 {
            self.bed_occ[i] += o.bed_occupied_at_arrival[i];
            if let Some(m) = o.bed_margin_min[i] {
                self.bed_margin[i] = Some(self.bed_margin[i].map_or(m, |x: i64| x.min(m)));
            }
        }
        let terms = row_terms(o, self.hc);
        for &t in &terms {
            *self.term_fails.entry(t).or_default() += 1;
        }
        if !terms.is_empty() && self.first_bad.is_empty() {
            self.first_bad = format!("{what}: [{}]: distinct {:?}, worst {} gt, sync {}, least hold gap {} gt, behind the hold {}, two-grant RELs {}, the way-in {}, the lag {:?}, the depth {}", terms.join(", "), o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.max_gap.0, o.ring_mismatch, o.min_hold_gap, o.behind_hold, o.multi_pick.len(), o.max_wayin, o.reset_set_lag, o.max_open_measured);
        }
    }

    /// The failing terms with their rows ("none" when every row passes).
    fn fails(&self) -> String {
        if self.term_fails.is_empty() {
            "none".to_string()
        } else {
            self.term_fails.iter().map(|(t, n)| format!("{t} on {n} rows")).collect::<Vec<_>>().join(", ")
        }
    }
}

/// The one-pick-per-REL forms' rows (G1's B2 / non-blocking on 239acc610): for each release form (the FIFO's and, from (b')
/// on, the positional release) and each leg corner (from (b') on, every lane's pick -> hold leg at -CORNER / 0 / +CORNER:
/// 27 combinations): the seven sweep points, the true freezes (at EVERY sweep point on the zero corner, at 333 + 0 on the
/// others) and every T starting offset of lanes 3 / 7 against lane -1's; the record's trip at the measured band's late end
/// (+ CORNER). Each row: its config, its label, whether it sits at a leg corner.
fn form_rows(base: &dyn Fn(&str, i64, i64) -> Cfg, form: u8, freeze_rows: &[(i64, i64, i64)]) -> Vec<(Cfg, String, bool)> {
    let variants: &[bool] = if form >= 3 { &[false, true] } else { &[false] };
    let mut corners: Vec<[i64; 3]> = vec![[0, 0, 0]];
    if form >= 3 {
        corners.clear();
        for s0 in [-CORNER, 0, CORNER] {
            for s1 in [-CORNER, 0, CORNER] {
                for s2 in [-CORNER, 0, CORNER] {
                    corners.push([s0, s1, s2]);
                }
            }
        }
    }
    let mut rows = Vec::new();
    for &pos in variants {
        let tag = if pos { ", the positional release" } else { "" };
        for &sh in &corners {
            let corner = sh != [0, 0, 0];
            let ctag = if corner { format!(", legs {:+} / {:+} / {:+} gt", sh[0], sh[1], sh[2]) } else { String::new() };
            let mk = |mut c: Cfg| {
                c.grant_form = form;
                c.positional_release = pos;
                c.leg_shift = sh;
                c.arm3_trip = ARM3_TRIP_BAND[1] + CORNER;
                c
            };
            for (cyc, extra) in POINTS {
                rows.push((mk(base("a one-pick-per-REL form", cyc, extra)), format!("cycle {cyc} + {extra}{ctag}{tag}"), corner));
            }
            let fz_points: &[(i64, i64)] = if corner { &[(333, 0)] } else { &POINTS };
            for &(cyc, extra) in fz_points {
                for &fz in freeze_rows {
                    let mut c = mk(base("a one-pick-per-REL form, a true freeze", cyc, extra));
                    c.freeze = vec![fz];
                    rows.push((c, format!("a {}-REL freeze at farm REL {} of night {}, cycle {cyc} + {extra}{ctag}{tag}", fz.2, fz.1, fz.0), corner));
                }
            }
            for o3 in 0..N0 {
                for o7 in 0..N0 {
                    let mut c = mk(base("a one-pick-per-REL form, the T offsets swept", 197, 40));
                    c.t_start = [0, o3, o7];
                    rows.push((c, format!("T at 0 / {o3} / {o7}{ctag}{tag}"), corner));
                }
            }
        }
    }
    rows
}

pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec![format!("ctl-pick (RING LEVEL: each lane's carts FIFO, one per REL; a batch circuit {CIRCUIT} RELs; a visit 9 RELs (lane 7 + 1, late + 1: cc767b78's measured absences); the day {DAY} RELs; picks interleaved at offsets {OFFSET:?}; the bar 24 distinct per night from night 2 and every interval < 24,000 - {MARGIN})")];
    let ok = nap_ok;
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
        // (THE SENSED POINTER AS ADOPTED (the (i-b) per-lane item ring, ironspec 11:56a / the parent 11:57; the positional
        // release; (b'+w) STATIC PRIORITY + THE WIDENING): the decision reads only the ring's state and the grant; HELD and
        // the room count are OUT (they defer no pick on the normal nights). Judged on the seven points (the bed cycle x (B)'s
        // travel), each its own sample: 24 distinct per night, every interval < 24,000 - margin, the ring in sync with the
        // carts actually popped (the judge's check), no X type conflict, the rejoins in pick order; the villager-gt past
        // 24,000 per day printed (the golem loss proxy: 0 = every villager summon-eligible all day). THE FORCED TEST (G1's B3
        // on 239acc610: exact): the carts' ids permuted inside each lane (slot i carries id l * N0 + 7 - i): every point's
        // SORTED NAP STAMPS unchanged and the ring in sync; its CONTROL: ONE flag (`peek_id`: REQ fires at the pop of the cart
        // id T's slot held at the build) through the SAME `same`: identical to the design unpermuted, the forced test
        // failing by name permuted)
        {
            let sensed_cfg = |name: &str, relabel: bool, cyc: i64, extra: i64| {
                let mut c = design(name, 0, None);
                c.pointer = false;
                c.sensed = true;
                c.relabel = relabel;
                c.held = 0;
                // ((b'+w), the adopted one-pick-per-REL form, and the positional release (the forms priced below on the
                // same rows))
                c.max_picks_per_rel = 0;
                c.grant_form = 4;
                c.positional_release = true;
                c.bed_cycle = cyc;
                c.rs_extra = extra;
                c
            };
            let past = golem_gt_per_day;
            let same = |o: &Out, p: &Out| sorted_stamps(o) == sorted_stamps(p) && p.ring_mismatch == 0;
            let mut pts = Vec::new();
            let (mut n_good, mut n_same, mut n_peek_ident, mut n_peek_same) = (0usize, 0usize, 0usize, 0usize);
            for (cyc, extra) in POINTS {
                let o = run(&sensed_cfg("the sensed ring", false, cyc, extra));
                let p = run(&sensed_cfg("the sensed ring, ids permuted", true, cyc, extra));
                let good = ok(&o) && o.ring_mismatch == 0 && o.x_conflict == 0 && o.type_order_mismatch == 0;
                n_good += good as usize;
                let sm = same(&o, &p);
                n_same += sm as usize;
                // (the peek control's runs: the design + `peek_id`, unpermuted and permuted)
                let mut cq = sensed_cfg("the sensed ring + the identity peek", false, cyc, extra);
                cq.peek_id = true;
                let oq = run(&cq);
                cq.relabel = true;
                let pq = run(&cq);
                n_peek_ident += (sorted_stamps(&oq) == sorted_stamps(&o)) as usize;
                n_peek_same += same(&oq, &pq) as usize;
                pts.push(format!("cycle {cyc} + {extra}: distinct per night {:?} (night 0, the cold start: {}), worst {} gt, villager-gt past 24,000 per day {} (golems lost per day {}), ring out of sync {}, rejoins unlike the identity rule's {}, rejoins out of pick order {}, X conflicts {}, the most X in a ring {}, T's ignored first passes {}, rate-deferred {}; permuted ids: {} naps, worst {} gt ({}); the peek: unpermuted {}, permuted {} ({}){}", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.distinct.first().copied().unwrap_or(0), o.max_gap.0, past(&o), if past(&o) == 0 { "0".to_string() } else { "> 0".to_string() }, o.ring_mismatch, o.rejoin_vs_ref, o.type_order_mismatch, o.x_conflict, o.max_x, o.t_first_pass, o.rate_deferred, sorted_stamps(&p).len(), p.max_gap.0, if sm { "every stamp unchanged" } else { "CHANGED" }, if sorted_stamps(&oq) == sorted_stamps(&o) { "identical to the design" } else { "DIFFERS from the design" }, if same(&oq, &pq) { "unchanged" } else { "CHANGED" }, format!("distinct {:?}, worst {} gt, sync lost {}", pq.distinct.iter().skip(1).copied().collect::<Vec<_>>(), pq.max_gap.0, pq.ring_mismatch), if good { "" } else { " MISSES" }));
            }
            let w = v.bar_n("ctl-pick: THE SENSED POINTER AS ADOPTED ((b'+w), the positional release; the decision reads the ring's state and the grant only) meets the nap bars at each of the seven points (24 distinct, every interval < 24,000 - margin), its ring in sync with the carts popped, no X type conflict, the rejoins in pick order", POINTS.len(), n_good == POINTS.len());
            lines.push(format!("  THE SENSED POINTER AS ADOPTED (the (i-b) per-lane item ring, the positional release, (b'+w); HELD and the room count OUT): {}: {w}", pts.join("; ")));
            // (THE FORCED TEST and its single-change CONTROL through the same bar)
            let forced_name = "ctl-pick: THE FORCED IDENTITY-PEEK TEST (G1's B3 on 239acc610): the carts' ids permuted inside each lane leave EVERY nap stamp (sorted) unchanged and the ring in sync at each of the seven points";
            let w = v.bar_n(forced_name, POINTS.len(), n_same == POINTS.len());
            lines.push(format!("  THE FORCED TEST (ids permuted, slot i carrying id l * 8 + 7 - i; the sorted nap stamps compared exactly): {n_same} of {} points unchanged: {w}", POINTS.len()));
            let mut scratch = Verdict::new("ctl-pick: the forced test's control");
            scratch.bar_n(forced_name, POINTS.len(), n_peek_same == POINTS.len());
            let named = scratch.fails().iter().any(|f| f.contains("THE FORCED IDENTITY-PEEK TEST"));
            let c = v.control("ctl-pick CONTROL (G1's B3 on 239acc610): ONE IDENTITY PEEK inside the sensed form (`peek_id`: REQ fires at the pop of the cart id T's slot held at the build), the only change: identical to the design at every point unpermuted, and THE FORCED TEST FAILS BY NAME under the permutation", POINTS.len(), n_peek_ident == POINTS.len() && named).to_string();
            lines.push(format!("  CONTROL (the identity peek, the only change): identical to the design unpermuted at {n_peek_ident} of {} points; permuted, {n_peek_same} of {} points unchanged (the forced test {}): {c}", POINTS.len(), POINTS.len(), if named { "FAILS by name" } else { "passes" }));
            // (THE ONE-PICK-PER-REL FORMS (ironspec 12:12p / 12:14p / 12:16p; G1 12:15), each on the SAME rows (`form_rows`):
            // the seven sweep points (night 0 the cold start, the three T tokens at the build's staggered slots 0 / 3 / 6), TRUE
            // WHOLE-FARM FREEZES (G1 12:18 / ironspec 12:18p: a reload or unload stops the whole chunk, entities included, while
            // game time runs on: the farm REL count stops, the night window loses the frozen part, the naps' game-time stamps
            // move on) of 1 / 2 / 5 RELs at every 3rd farm REL of night 3 and at three daytime phases (on the zero corner at
            // every sweep point: G1 on 239acc610, non-blocking), and every starting offset of lanes 3 / 7's T against lane
            // -1's; from (b') on also the positional release and the 27 +-2 gt leg corners. EVERY ROW judged on `row_terms`
            // (G1's B2 on 239acc610), each failing term by name. (a) NONE (each REQ granted) and (b) ROTATION printed; (b')
            // STATIC PRIORITY and (b'+w) (THE ADOPTED FORM) judged; the controls run (b'+w)'s own rows through the same terms)
            let mut freeze_rows: Vec<(i64, i64, i64)> = Vec::new();
            for len in [1i64, 2, 5] {
                for s in (0..120i64).step_by(3) {
                    freeze_rows.push((3, s, len));
                }
                for s in [140i64, 180, 220] {
                    freeze_rows.push((3, s, len));
                }
            }
            // (G1's B5 on 239acc610: the hold's cycle SUMMED from the model's constants, judged equal to the interface's total)
            let hc = hold_cycle();
            let w = v.bar(&format!("ctl-pick: THE SECOND HOLD's CYCLE (G1's B5 on 239acc610) summed from the model's constants, the SET {HOLD_TO_SET} + D {} + P1 -> exit {P1_TO_EXIT} + the exit detector's recheck {DET_RECHECK} = {hc} gt, equals the interface's stated total", d_occ()), hc == iface_hold_cycle());
            lines.push(format!("  THE SECOND HOLD's CYCLE: the SET {HOLD_TO_SET} + D {} (read from the interface) + P1 -> exit {P1_TO_EXIT} + the exit detector's recheck {DET_RECHECK} = {hc} gt; the interface states {} gt: {w}", d_occ(), iface_hold_cycle()));
            let base = |name: &str, cyc: i64, extra: i64| sensed_cfg(name, false, cyc, extra);
            let form_bar = |fname: &str, rec: &FormRec| format!("ctl-pick: THE SENSED POINTER under {fname} over its rows (the seven points, the true freezes, every T starting offset; the FIFO's and the positional release; the 27 +-{CORNER} gt leg corners): every row clear on every term (the nap bars, the ring's sync, no X conflict, the rejoins in pick order and equal to the identity rule's, no golem lost, the least H2 arrival gap > the {}-gt cycle and no napper reaching H2 while the previous one still waits there, no REL with two grants, no lane -1 grant in the REL right after a lane-7 grant, the way-in <= 3, (B)'s RESET -> SET lag > 0, the record's measured depth <= 3, >= {SPACING_MIN} pops between a lane's grants){}", rec.hc, if rec.term_fails.is_empty() { String::new() } else { format!(" [failing: {}]", rec.fails()) });
            let mut form_lines = Vec::new();
            let (mut rows4, mut rec4, mut rec3): (Vec<(Cfg, String, bool)>, FormRec, FormRec) = (Vec::new(), FormRec::new(hc), FormRec::new(hc));
            for (form, fname) in [(0u8, "(a) NONE"), (2, "(b) ROTATION"), (3, "(b') STATIC PRIORITY (superseded by (b'+w))"), (4, "(b'+w) STATIC PRIORITY + THE WIDENING (lane -1 refused at the capture after a lane-7 grant, the W7 latch; THE ADOPTED FORM)")] {
                let rows = form_rows(&base, form, &freeze_rows);
                let mut rec = FormRec::new(hc);
                for (c, label, corner) in &rows {
                    rec.take(&run(c), label, *corner);
                }
                // (G1's CHANGES 6 on 03f431632 (1): the adopted form's results JUDGED (every term, the W7 pair included); the
                // superseded forms printed)
                let mark = if form == 4 { format!("JUDGED: {}", v.bar_n(&form_bar(fname, &rec), rec.rows, rec.term_fails.is_empty())) } else { "PRINTED".to_string() };
                let at_corners = |x: i64| if rec.corner_gap == i64::MAX { "no corner rows".to_string() } else { format!("{x} gt") };
                form_lines.push(format!("{fname} ({} rows, {mark}): failing terms {}; RELs carrying two grants {}, lane -1 grants in the REL right after a lane-7 grant {}, the least arrival gap at the second hold {} gt (its cycle {hc} gt; at the leg corners {}), nappers reaching the hold while the previous one waits there {}, the most in the way-in {}, (B)'s least RESET -> SET lag {:?} gt, the record's measured depth at most {} (the trip {} gt), the longest wait per lane from a refused REQ to its grant {:?} RELs ({:?} gt), the fewest pops between a lane's grants {:?} (the netlist's \">= 11-pop spacing\", measured), the worst interval {} gt ({}; at the leg corners {}), villager-gt past 24,000 {} (golems lost per day {}), the most nappers picked and not back at their siding {} (an upper bound, NOT the record's depth), not yet ARM3'd {}, the rejoins unlike the identity rule's {}{}", rec.rows, rec.fails(), rec.multi, rec.w7_pairs, rec.least_gap, at_corners(rec.corner_gap), rec.behind, rec.wayin_max, rec.lag_min, rec.depth_max, ARM3_TRIP_BAND[1] + CORNER, rec.wait, rec.wait.map(|w| w * 100), rec.spacing, rec.worst, rec.worst_at, at_corners(rec.corner_worst), rec.lost, if rec.lost == 0 { "0" } else { "> 0" }, rec.out_max, rec.arm3_max, rec.vs_ref, if rec.first_bad.is_empty() { String::new() } else { format!(" (first miss: {})", rec.first_bad) }));
                if form == 3 {
                    rec3 = rec;
                } else if form == 4 {
                    rows4 = rows;
                    rec4 = rec;
                }
            }
            lines.push(format!("  THE ONE-PICK-PER-REL FORMS (the sensed ring; the same nap model; each row judged on every term): {}", form_lines.join("; ")));
            // (THE CONTROLS on (b'+w)'s OWN ROWS through the same terms and bar, each the only change: (b') (grant_form 3,
            // the widening dropped: G1's CHANGES 6 on 03f431632 (1); `form_rows` builds (b')'s rows as (b'+w)'s with grant_form
            // 3, so the (b') row set above IS this control's) biting on the W7 pair by name; NONE (grant_form 0, G1's B2 on
            // 239acc610) biting on the two-grant RELs by name)
            {
                let mut scratch = Verdict::new("ctl-pick: the widening's control");
                scratch.bar_n(&form_bar("(b'+w), the widening dropped", &rec3), rec3.rows, rec3.term_fails.is_empty());
                let named = scratch.fails().iter().any(|f| f.contains("W7-PAIR on "));
                let c = v.control("ctl-pick CONTROL (G1's CHANGES 6 on 03f431632 (1)): (b') (grant_form 3: the widening dropped) on (b'+w)'s rows, the only change: the form's bar FAILS with W7-PAIR by name", rec3.rows, named).to_string();
                lines.push(format!("  CONTROL ((b'): the widening dropped, {} rows): lane -1 grants in the REL right after a lane-7 grant {}, failing terms {}: {c}", rec3.rows, rec3.w7_pairs, rec3.fails()));
            }
            {
                let mut rc = FormRec::new(hc);
                for (c, label, corner) in &rows4 {
                    let mut c = c.clone();
                    c.grant_form = 0;
                    rc.take(&run(&c), label, *corner);
                }
                let mut scratch = Verdict::new("ctl-pick: NONE's control");
                scratch.bar_n(&form_bar("(b'+w), every REQ granted", &rc), rc.rows, rc.term_fails.is_empty());
                let named = scratch.fails().iter().any(|f| f.contains("TWO-GRANTS on "));
                let c = v.control("ctl-pick CONTROL (G1's B2 on 239acc610): NONE (grant_form 0: each REQ granted) on (b'+w)'s own rows, the only change: the form's bar FAILS with TWO-GRANTS by name", rc.rows, named).to_string();
                lines.push(format!("  CONTROL (NONE: each REQ granted, {} rows): two-grant RELs {}, failing terms {}: {c}", rc.rows, rc.multi, rc.fails()));
            }
            // ((B)'s LAG on the adopted form (G1's B2 on 239acc610: hold_no_exit on the lag): on (b'+w)'s OWN rows the hold's
            // (9,22,3) term never carries the load (printed: the least lag with it dropped); its CONTROL runs where it does:
            // the adopted form's seven points with THREE PICKS FORCED in one REL on b8949c96's measured 16-gt spacing (legs
            // 132 / 148 / 164) at gt 100 / 500 / 900: the design holds each SET past the RESET level's fall (LAG > 0, judged);
            // with the term dropped, a SET inside the RESET level: the LAG term fails BY NAME)
            {
                let mut rn = FormRec::new(hc);
                for (c, label, corner) in &rows4 {
                    let mut c = c.clone();
                    c.hold_no_exit = true;
                    rn.take(&run(&c), label, *corner);
                }
                lines.push(format!("  THE (9,22,3) TERM on (b'+w)'s own rows ({} rows): dropped, the least RESET -> SET lag {:?} gt (with it {:?} gt), failing terms {}: never load-bearing on the design's rows (printed; its control below is planted where it is)", rn.rows, rn.lag_min, rec4.lag_min, rn.fails()));
                let (mut n_pl, mut pl_ok, mut pl_named, mut lag_d, mut lag_c) = (0usize, 0usize, 0usize, None::<i64>, None::<i64>);
                for (cyc, extra) in POINTS {
                    for g in [100i64, 500, 900] {
                        let mut c = sensed_cfg("planted: three picks in one REL, the measured spacing", false, cyc, extra);
                        c.force_pick = vec![(g, 0), (g, 1), (g, 2)];
                        c.leg_override = Some([132, 148, 164]);
                        let o = run(&c);
                        c.hold_no_exit = true;
                        let oc = run(&c);
                        n_pl += 1;
                        pl_ok += (!row_terms(&o, hc).contains(&"LAG")) as usize;
                        pl_named += row_terms(&oc, hc).contains(&"LAG") as usize;
                        if let Some(l) = o.reset_set_lag {
                            lag_d = Some(lag_d.map_or(l, |m| m.min(l)));
                        }
                        if let Some(l) = oc.reset_set_lag {
                            lag_c = Some(lag_c.map_or(l, |m| m.min(l)));
                        }
                    }
                }
                let w = v.bar_n("ctl-pick: (B)'s RESET -> SET LAG on the adopted form with three picks forced in one REL (the measured 16-gt spacing, the seven points): every SET held past the RESET level's fall (the LAG term clear)", n_pl, pl_ok == n_pl);
                lines.push(format!("  (B)'s LAG, the adopted form + three picks in one REL ({n_pl} rows): the least lag {lag_d:?} gt: {w}"));
                let c = v.control("ctl-pick CONTROL (G1's B2 on 239acc610): the second hold without its (9,22,3) term (hold_no_exit) on the same planted rows, the only change: the LAG term FAILS by name (a SET inside the RESET level)", n_pl, pl_named > 0).to_string();
                lines.push(format!("  CONTROL (hold_no_exit, the same planted rows): the LAG term failing on {pl_named} of {n_pl} rows, the least lag {lag_c:?} gt: {c}"));
            }
            // (day-1's job 4, A FINDING (printed, not a bar): DOES NORMAL RUNNING NEED THE RING's SPACING LATCH? `no_spacing`
            // (REQ_l = T at the tap, no latch: T's first pass after a grant can request), the only change, on (b'+w)'s own
            // rows (the seven points, the true freezes, every T offset, both releases, the 27 leg corners) through the same
            // terms; the grants on a T first pass counted. THE LATCH IS LOAD-BEARING exactly when a functional term fails
            // without it (SPACING, the netlist's ">= 11-pop" rule, is the latch's own definition: its measured least printed,
            // excluded from the call))
            {
                let (mut ra, mut rp) = (FormRec::new(hc), FormRec::new(hc));
                for (c, label, corner) in &rows4 {
                    let mut c = c.clone();
                    c.no_spacing = true;
                    let o = run(&c);
                    if c.positional_release {
                        rp.take(&o, label, *corner);
                    }
                    ra.take(&o, label, *corner);
                }
                let functional: Vec<String> = ra.term_fails.iter().filter(|(t, _)| **t != "SPACING").map(|(t, n)| format!("{t} on {n} rows")).collect();
                let bearing = !functional.is_empty();
                let rec_line = |r: &FormRec| format!("{} rows: grants on a T first pass {}, failing terms {}, the fewest pops between a lane's grants {:?}, the fewest distinct villagers in a night (from night 2) {}, the worst interval {} gt ({}), the least H2 gap {} gt (cycle {}), behind the hold {}, two-grant RELs {}, W7 pairs {}, the way-in at most {}, the least lag {:?} gt, the depth at most {}, the longest refused-REQ wait per lane {:?} RELs", r.rows, r.first_pass_grants, r.fails(), r.spacing, r.distinct_min, r.worst, r.worst_at, r.least_gap, r.hc, r.behind, r.multi, r.w7_pairs, r.wayin_max, r.lag_min, r.depth_max, r.wait);
                lines.push(format!("  FINDING (day-1's job 4: does normal running NEED the ring's SPACING latch?): `no_spacing` (REQ = T at the tap, no latch), the only change, on (b'+w)'s own rows: ALL {}; THE POSITIONAL RELEASE's {}; with the latch: the fewest pops between a lane's grants {:?}, the worst interval {} gt, grants on a T first pass {}; THE LATCH IS LOAD-BEARING: {} ({})", rec_line(&ra), rec_line(&rp), rec4.spacing, rec4.worst, rec4.first_pass_grants, if bearing { "YES" } else { "NO" }, if bearing { format!("without it: {}", functional.join(", ")) } else { "no functional term fails without it; SPACING, the latch's own rule, is the only term it changes".to_string() }));
            }
            // (day-1's addendum (G1 / ironspec 13:34p): NEVER DELIVER A NAPPER TO AN OCCUPIED BED, a normal-operation invariant
            // judged on (b'+w)'s rows on 58f770cc's MEASURED wake band (13:37, 408 visits): the previous napper's bed empty from
            // its lie-down + w (no exit lag), w swept over the band (its spread owed: WAKE_SPREAD), each napper's stop -> bed
            // travel at STOP_TO_BED +- CORNER (the worst pairing: this one early, the previous one late); the least margin (the
            // arrival - the previous wake) printed per w. Its CONTROL on the same rows: the stop's bed-free wait dropped)
            let ws = wake_sweep();
            let bed_name = format!("ctl-pick: THE BED NEVER OCCUPIED AT A NAPPER's ARRIVAL (day-1's addendum, ironspec 13:34p) on (b'+w)'s rows: every consecutive bed pair's second napper arrives (its start + STOP_TO_BED - {CORNER}) at or after the first one's wake (its lie-down at its start + STOP_TO_BED + {CORNER}, + w) for every w of 58f770cc's measured lie-down -> wake band {ws:?} gt");
            let wb = v.bar_n(&bed_name, rec4.bed_pairs, rec4.bed_occ.iter().all(|&n| n == 0));
            lines.push(format!("  THE BED AT EACH ARRIVAL ((b'+w)'s {} rows, {} bed pairs; STOP_TO_BED {STOP_TO_BED} gt: named by neither the interface nor the stop module (f1q2-module-stop9.txt), so 0; the travel +-{CORNER} per napper, the worst pairing; the band's spread WAKE_SPREAD {WAKE_SPREAD}, owed to 58f770cc's fix car): occupied arrivals per w {ws:?}: {:?}; THE LEAST MARGIN (the arrival - the previous wake) per w: {:?} gt, at the worst w {}: {:?} gt: {wb}", rec4.rows, rec4.bed_pairs, rec4.bed_occ, rec4.bed_margin, ws[2], rec4.bed_margin[2]));
            {
                let mut rc = FormRec::new(hc);
                for (c, label, corner) in &rows4 {
                    let mut c = c.clone();
                    c.no_bed_wait = true;
                    rc.take(&run(&c), label, *corner);
                }
                let mut scratch = Verdict::new("ctl-pick: the bed control's rows");
                scratch.bar_n(&bed_name, rc.bed_pairs, rc.bed_occ.iter().all(|&n| n == 0));
                let named = scratch.fails().iter().any(|f| f.contains("THE BED NEVER OCCUPIED AT A NAPPER's ARRIVAL"));
                let c = v.control("ctl-pick CONTROL (day-1's addendum): the stop's bed-free wait dropped (start = the front arrival), the only change, on (b'+w)'s rows: THE BED OCCUPIED AT A NAPPER's ARRIVAL by name", rc.rows, named).to_string();
                lines.push(format!("  CONTROL (no bed-free wait, {} rows): occupied arrivals per w {ws:?}: {:?}, the least margin per w {:?} gt: {c}", rc.rows, rc.bed_occ, rc.bed_margin));
            }
            let n_pts = freeze_rows.len();
            // (THE RECORD's MEASURED DEPTH (ironspec 12:47p (4); G1's B2 on 239acc610 and CHANGES 6 on 03f431632 (3): judged,
            // <= 3, the interface's stage count): the most entries alive at once per tick (written at the pick's REL + 2,
            // shifted at REL' + 61), under the adopted (b'+w) WITH THE POSITIONAL RELEASE (the design's form; the FIFO's release
            // is judged at the band's late end on every form row's DEPTH term), over the seven points and the true freezes,
            // each at the 27 +-2 gt leg corners, at the MEASURED stop -> (9,0,8) trip band (ARM3_TRIP_BAND), each end
            // +-CORNER. Its CONTROL: a trip far outside the band (250 gt), the only change, on one trip's rows: the same bar
            // FAILS by name)
            let depth_at = |trip: i64| -> (i64, usize) {
                let (mut m, mut n) = (0i64, 0usize);
                for s0 in [-CORNER, 0, CORNER] {
                    for s1 in [-CORNER, 0, CORNER] {
                        for s2 in [-CORNER, 0, CORNER] {
                            for (cyc, extra) in POINTS {
                                let mut c = sensed_cfg("the record's depth", false, cyc, extra);
                                c.arm3_trip = trip;
                                c.leg_shift = [s0, s1, s2];
                                m = m.max(run(&c).max_open_measured);
                                n += 1;
                            }
                            for &fz in &freeze_rows {
                                let mut c = sensed_cfg("the record's depth, a true freeze", false, 333, 0);
                                c.arm3_trip = trip;
                                c.leg_shift = [s0, s1, s2];
                                c.freeze = vec![fz];
                                m = m.max(run(&c).max_open_measured);
                                n += 1;
                            }
                        }
                    }
                }
                (m, n)
            };
            let mut depth = Vec::new();
            let (mut depth_max, mut depth_rows) = (0i64, 0usize);
            let trips = [ARM3_TRIP_BAND[0] - CORNER, ARM3_TRIP_BAND[0], ARM3_TRIP_BAND[0] + CORNER, ARM3_TRIP_BAND[1] - CORNER, ARM3_TRIP_BAND[1], ARM3_TRIP_BAND[1] + CORNER];
            for trip in trips {
                let (m, n) = depth_at(trip);
                depth_max = depth_max.max(m);
                depth_rows += n;
                depth.push(format!("trip {trip} gt: {m}"));
            }
            let depth_name = "ctl-pick: THE RECORD's MEASURED DEPTH (G1's B2 on 239acc610, CHANGES 6 on 03f431632 (3); picks written minus ARM3 shifts, per tick) under the adopted (b'+w) with the positional release is at most 3 (the interface's 3-deep record) over the measured trip band, each end +-2 gt";
            let w = v.bar_n(depth_name, depth_rows, depth_max <= 3);
            lines.push(format!("  THE RECORD's MEASURED DEPTH under the adopted (b'+w), the positional release (the most entries alive at once, per tick; {} rows per trip: the seven points and the true freezes, each at the 27 leg corners; the trip = the measured {}..{} gt, each end +-{CORNER}): {} (cc767b78's stage count reads it): {w}", 27 * (7 + n_pts), ARM3_TRIP_BAND[0], ARM3_TRIP_BAND[1], depth.join(", ")));
            {
                let far = 250i64;
                let (m, n) = depth_at(far);
                let mut scratch = Verdict::new("ctl-pick: the record depth's control");
                scratch.bar_n(depth_name, n, m <= 3);
                let named = scratch.fails().iter().any(|f| f.contains("THE RECORD's MEASURED DEPTH"));
                let c = v.control(&format!("ctl-pick CONTROL (G1's CHANGES 6 on 03f431632 (3)): a stop -> (9,0,8) trip of {far} gt (far outside the measured band), the only change, on one trip's rows: THE RECORD's MEASURED DEPTH bar FAILS by name (the depth past 3)"), n, named).to_string();
                lines.push(format!("  CONTROL (the trip {far} gt, {n} rows): the depth at most {m}: {c}"));
            }
            // (G1's B1 on 239acc610: THE FREEZE STAMP: a nap is stamped with every freeze before its start in farm time,
            // whichever REL fixed that start. THE PLANTED CASE: night 3's first lane-3 and lane-7 naps (each start fixed the REL
            // after its pick, > 200 gt after it), a 5-REL freeze planted at the farm REL two after each pick (REL k+1 for a pick
            // at REL k-1, already past the start's fixing): the frozen run's stamp minus the unfrozen one's = 500 exactly, the
            // start itself unchanged (the plant read back). Its CONTROL: the old stamp (the freezes the loop had reached when
            // the start was fixed), the only change: the same bar FAILS by name)
            {
                let fz_len = 5i64;
                let c0 = sensed_cfg("the freeze stamp", false, 333, 0);
                let o0 = run(&c0);
                let (mut cases, mut good, mut ctl_off) = (Vec::new(), 0usize, 0usize);
                for l in [1usize, 2] {
                    let Some(n0) = o0.nap_log.iter().find(|n| n.day == 3 && n.lane == l).copied() else {
                        cases.push(format!("lane {}: NO NAP on night 3", LANES[l]));
                        continue;
                    };
                    let fz = (3i64, n0.k + 2, fz_len);
                    let read = |reached: bool| -> (Option<i64>, Option<i64>) {
                        let mut c = c0.clone();
                        c.freeze = vec![fz];
                        c.stamp_reached_freezes = reached;
                        let n1 = run(&c).nap_log.iter().find(|n| n.villager == n0.villager && n.day == 3 && n.pick_gt == n0.pick_gt).copied();
                        (n1.map(|n| n.stamp - n0.stamp), n1.map(|n| n.start))
                    };
                    let (d, st) = read(false);
                    let (dc, _) = read(true);
                    let after = n0.start >= n0.pick_gt + 200;
                    if d == Some(fz_len * 100) && st == Some(n0.start) && after {
                        good += 1;
                    }
                    if dc != Some(fz_len * 100) {
                        ctl_off += 1;
                    }
                    cases.push(format!("lane {}: picked at farm REL {} of night 3 (gt {}), its bed start at gt {} (the freeze at REL {} = gt {}: the start after it {after}), the stamp moved by {d:?} gt (the start read back {st:?}), the old stamp's {dc:?}", LANES[l], n0.k, n0.pick_gt, n0.start, fz.1, n0.pick_gt + 200));
                }
                let name = "ctl-pick: THE FREEZE STAMP (G1's B1 on 239acc610): a nap starting after a freeze carries it, whichever REL fixed its start: night 3's first lane-3 and lane-7 naps with a 5-REL freeze planted at the farm REL two after each pick: the frozen stamp minus the unfrozen = 500 gt exactly, the start unchanged";
                let w = v.bar_n(name, 2, good == 2);
                lines.push(format!("  THE FREEZE STAMP (the stamp = the farm start + every freeze before it in farm time): {}: {w}", cases.join("; ")));
                let mut scratch = Verdict::new("ctl-pick: the freeze stamp's control");
                scratch.bar_n(name, 2, ctl_off == 0);
                let c = v.control("ctl-pick CONTROL (G1's B1 on 239acc610): the OLD stamp (+ the freezes the loop had reached when the start was fixed), the only change: THE FREEZE STAMP's bar FAILS by name (the stamp short of 500)", 2, ctl_off == 2 && scratch.fails().iter().any(|f| f.contains("THE FREEZE STAMP"))).to_string();
                lines.push(format!("  CONTROL (the old stamp): {ctl_off} of 2 planted naps short of 500: {c}"));
            }
            // (G1's CHANGES 6 on 03f431632 (2): THE W7 LATCH THROUGH A TRUE FREEZE: the first T-offset row of the adopted design
            // (cycle 197 + 40; lanes 3 / 7's T against lane -1's) with a W7 refusal after the cold start inside the frozen
            // window (lane -1 refused at farm REL k after a lane-7 grant at the last capture); a 5-REL freeze planted at farm
            // REL k (between that grant and the next REL: the latch is a block state): lane -1 still refused at farm REL k by
            // W7. And EVERY REFUSAL printed per REL (that row's night: farm REL, lane, why; every night's count per reason; the
            // default T offsets' count at 333 + 0))
            {
                let fz_len = 5i64;
                let w7_why = "W7 (the widening: lane 7 granted at the last capture)";
                let mut found: Option<(Cfg, Out, (i64, i64, usize, &str), String)> = None;
                'search: for o3 in 0..N0 {
                    for o7 in 0..N0 {
                        let mut c = sensed_cfg("the W7 latch", false, 197, 40);
                        c.t_start = [0, o3, o7];
                        let o = run(&c);
                        if let Some(r) = o.refusals.iter().find(|r| r.0 >= 1 && r.3 == w7_why && r.1 + fz_len < c.window).copied() {
                            found = Some((c, o, r, format!("T at 0 / {o3} / {o7}, cycle 197 + 40")));
                            break 'search;
                        }
                    }
                }
                let (held, case, refusals) = match found {
                    Some((c0, o0, (d, k, l, why), row)) => {
                        let mut c = c0.clone();
                        c.freeze = vec![(d, k, fz_len)];
                        let again = run(&c).refusals.iter().any(|r| r.0 == d && r.1 == k && r.2 == l && r.3 == why);
                        let mut per: std::collections::BTreeMap<&str, usize> = Default::default();
                        for r in &o0.refusals {
                            *per.entry(r.3).or_default() += 1;
                        }
                        let night: Vec<String> = o0.refusals.iter().filter(|r| r.0 == d).map(|r| format!("REL {}: lane {} {}", r.1, LANES[r.2], r.3)).collect();
                        (again, format!("{row}: night {d}, lane -1 refused by W7 at farm REL {k}; a {fz_len}-REL freeze planted at farm REL {k} (after the lane-7 grant's REL, before this one): refused again by W7 there: {again}"), format!("{row} ({} nights): per reason {per:?}; night {d}, per REL: {}", o0.distinct.len(), night.join("; ")))
                    }
                    None => (false, "NO T-offset row with a W7 refusal (nothing to plant)".to_string(), "none".to_string()),
                };
                let w = v.bar_n("ctl-pick: THE W7 LATCH THROUGH A TRUE FREEZE (G1's CHANGES 6 on 03f431632 (2)): a 5-REL freeze between a lane-7 grant and the next REL leaves lane -1 refused by W7 at that REL (the latch a block state)", 1, held);
                lines.push(format!("  THE W7 LATCH (set by G7's capture, read and overwritten by the next capture; W7 takes G7 at REL + {W7_TAKE}, the decide at REL + {G_CAPTURE}): {case}: {w}"));
                let o333 = run(&sensed_cfg("the refusals at the default T offsets", false, 333, 0));
                lines.push(format!("  EVERY REFUSAL (the adopted design): {refusals}; at the default T offsets 0 / 3 / 6, cycle 333 + 0: {} refusals", o333.refusals.len()));
            }
            // (G1's CHANGES 6 on 03f431632 (4): THE PRESS / TAP MARGINS by arithmetic: every press (each lane's POP, the station's
            // D (x,0,2) at its slot release, REL + the interface's offset (ctlring8's convention); its ENTRY D(x)) moves
            // +-CORNER gt, each independently; the REL taps are exact; the ring STEPS the interface's lag after the pop (`each
            // lane's ring steps N gt after the lane's SENSED pop`, the laid ring's STEP_LAG) and SKIPS SKIP_RECHECK after a
            // step. The decisions that compare them: G's capture (REL + 92) reads REQ, a level from the lane's last ring step
            // (the step or its skip) to its next pop: each > CORNER gt from the capture; the positional release at the ENTRY
            // reads the ring's slot 2 downstream of the tap, moved only by the lane's steps: each entry > 2 x CORNER gt from
            // every step (both move). The taps are read back from the interface; the table printed: which timings are REL
            // taps (exact) and which are presses (+-2), and the napper's (9,0,8) arrivals against the HOME latch / the shift
            // (4 gt apart) and against the shift .. the hold's release ((REL + 61, REL + 67]))
            let step_lag = iface_rel_tap("each lane's ring steps ");
            {
                let offs = release_offsets();
                let taps = [("G_l is captured at REL + ", G_CAPTURE), ("held until REL' + ", G_CLEAR), ("written from G at REL + ", REC_WRITE), ("SHIFTED at REL + ", REC_SHIFT), ("read (latched) by the core at REL' + ", HOME_LATCH)];
                let reads: Vec<(String, bool)> = taps.iter().map(|&(k, want)| match iface_rel_tap(k) {
                    Ok(n) => (format!("\"{}\" {n}", k.trim_end_matches(" + ")), n == want),
                    Err(e) => (e, false),
                }).collect();
                let all_read = reads.iter().all(|r| r.1);
                let w = v.bar_n("ctl-pick: THE REL TAPS read back from the interface (G_l captured / held until, the record written / shifted, HOME latched) equal the model's constants", taps.len(), all_read);
                lines.push(format!("  THE REL TAPS (the interface): {}: {w}", reads.iter().map(|r| r.0.clone()).collect::<Vec<_>>().join(", ")));
                let w = v.bar("ctl-pick: THE RING's STEP LAG read from the interface equals the laid ring's (ctlring8::STEP_LAG)", step_lag == Ok(crate::ctlring8::STEP_LAG));
                lines.push(format!("  THE RING's STEP LAG (the interface: \"each lane's ring steps N gt after the lane's SENSED pop\"): {step_lag:?}; the laid ring's STEP_LAG {}: {w}", crate::ctlring8::STEP_LAG));
                let lag = step_lag.clone().unwrap_or(crate::ctlring8::STEP_LAG);
                let near = |a: i64, b: i64| (a - b).rem_euclid(PERIOD).min((b - a).rem_euclid(PERIOD));
                let (mut least_cap, mut least_ent, mut rows) = (i64::MAX, i64::MAX, Vec::new());
                for l in 0..3 {
                    let pop = offs[l];
                    let step = pop + lag;
                    let skip = step + SKIP_RECHECK;
                    let entry = SLOT_T[l][l] + (offs[l] - TRAIN15_OFFSETS[l]) + J_TO_D;
                    let cap = near(pop, G_CAPTURE).min(near(step, G_CAPTURE)).min(near(skip, G_CAPTURE));
                    let ent = near(entry, step).min(near(entry, skip));
                    least_cap = least_cap.min(cap);
                    least_ent = least_ent.min(ent);
                    rows.push(format!("lane {}: the pop REL + {pop}, its step + {step}, a skip + {skip}, the entry + {entry} (= REL' + {}): the capture + {G_CAPTURE} {cap} gt from the nearest pop / step; the entry {ent} gt from the nearest step", LANES[l], entry.rem_euclid(PERIOD)));
                }
                let w = v.bar_n(&format!("ctl-pick: THE PRESS / TAP MARGINS (G1's CHANGES 6 on 03f431632 (4)): every lane's pop, step and skip more than {CORNER} gt from G's capture (REL + {G_CAPTURE}), and every entry more than {} gt from the lane's ring steps (each press +-{CORNER} independently)", 2 * CORNER), 6, least_cap > CORNER && least_ent > 2 * CORNER);
                lines.push(format!("  THE PRESS / TAP MARGINS (the ring's step {lag} gt after the pop): {}; the least {least_cap} gt to the capture (> {CORNER}), {least_ent} gt entry -> step (> {}): {w}", rows.join("; "), 2 * CORNER));
                lines.push(format!("  THE TIMINGS: REL TAPS (exact): G_l captured + {G_CAPTURE}, held until REL' + {G_CLEAR}, W7 takes G7 + {W7_TAKE} (cc767b78), the record written + {REC_WRITE}, shifted + {REC_SHIFT}, HOME latched + {HOME_LATCH} ({} gt before the shift: two taps, exact), the napper hold's lever + 61 / its rail + 67; PRESSES (+-{CORNER}): the pops + {:?} (the slot releases; the ring's steps {lag} gt after them, a skip {SKIP_RECHECK} after a step), the entries D(x), the napper's (9,0,8) arrival (its stop release + the trip, asynchronous to REL): over (b'+w)'s rows (the trip {} gt) {} of {} arrivals fall within +-{CORNER} gt of the HOME latch .. the shift (REL + {} .. {}), and {} after the shift but at or before the hold's release (REL + {} .. 67], shifted at that REL' + {REC_SHIFT} by the record model before they arrive) (PRINTED: cc767b78's record reads them)", REC_SHIFT - HOME_LATCH, offs, ARM3_TRIP_BAND[1] + CORNER, rec4.arm3_near, rec4.arm3_n, HOME_LATCH - CORNER, REC_SHIFT + CORNER, rec4.arm3_after_shift, REC_SHIFT + 1));
            }
            // (G1's B6 on 239acc610: THE ENTRY / STEP SEPARATION, computed (no literals): each lane's circuit entry (the S-bend
            // D press: its slot's J(l) arrival on train 16 + J -> D) against the same lane's RING STEPS one and two RELs after
            // its release (REL + 100 / 200 + its release offset, read from the interface, + the ring's step lag): the REL model
            // processes the entry after the first and before the second; judged > CORNER gt each side, so a +-2 gt shift never
            // reorders them)
            {
                let offs = release_offsets();
                let lag = step_lag.clone().unwrap_or(crate::ctlring8::STEP_LAG);
                let (mut least, mut seps) = (i64::MAX, Vec::new());
                for l in 0..3 {
                    let entry = SLOT_T[l][l] + (offs[l] - TRAIN15_OFFSETS[l]) + J_TO_D;
                    let (p1, p2) = (PERIOD + offs[l] + lag, 2 * PERIOD + offs[l] + lag);
                    least = least.min(entry - p1).min(p2 - entry);
                    seps.push(format!("lane {}: the entry at REL + {entry}: {} gt after the step at REL + {p1}, {} gt before the step at REL + {p2}", LANES[l], entry - p1, p2 - entry));
                }
                let w = v.bar_n(&format!("ctl-pick: THE ENTRY / STEP SEPARATION (G1's B6 on 239acc610): each lane's circuit entry (its slot's train-16 J(l) arrival + J -> D {J_TO_D}) lies between the same lane's ring steps one and two RELs after its release (REL + 100 / 200 + the interface's release offsets + the ring's step lag {lag}) by > {CORNER} gt each side"), 3, least > CORNER);
                lines.push(format!("  THE ENTRY / STEP SEPARATION (SLOT_T, the interface's release offsets {offs:?}, the ring's step lag {lag} gt): {}; the least {least} gt (> {CORNER}): {w}", seps.join("; ")));
            }
            // (THE TIMING CORNER (ironspec 12:02p (1)): every circuit entry processed after the same REL's pops (any shift of an
            // entry past a pop, 2 gt included), at five points: THE ORDER-INDEPENDENCE CHECK judged (G1's B6: the ring in sync,
            // the rejoins in pick order, no X conflict, each point its own sample) on the MARK-FIFO release, the form that
            // check was asked of ("the mark FIFO, the sensed rejoin and the extract flags must not depend on that order"). THE
            // POSITIONAL RELEASE (the adopted form) reads the ring's slot 2 downstream of the tap AT the entry, so it depends on
            // the entry following the lane's last step and preceding its next by construction: its stress row printed; that
            // order is what the entry / step separation and the press / tap margins above judge. The nap bar on the stress rows is scoped (every
            // rejoin chance resolved late))
            let mut corner = Vec::new();
            let (mut n_corner, mut corner_good) = (0usize, 0usize);
            for (cyc, extra) in [(333i64, 0i64), (197, 0), (197, 120), (168, 0), (168, 120)] {
                let mut c = sensed_cfg("the sensed ring (the mark FIFO), entries after the pops", false, cyc, extra);
                c.positional_release = false;
                c.late_entries = true;
                let o = run(&c);
                n_corner += 1;
                let good = ok(&o) && o.ring_mismatch == 0 && o.x_conflict == 0 && o.type_order_mismatch == 0;
                corner_good += (o.ring_mismatch == 0 && o.type_order_mismatch == 0 && o.x_conflict == 0) as usize;
                c.positional_release = true;
                let op = run(&c);
                corner.push(format!("cycle {cyc} + {extra}: the mark FIFO: distinct {:?}, worst {} gt, sync {}, type order {}, X conflicts {}, stalls {}, the shallowest lane {:?}{}; the positional release: sync lost {}, distinct {:?}, worst {} gt", o.distinct.iter().skip(1).copied().collect::<Vec<_>>(), o.max_gap.0, o.ring_mismatch, o.type_order_mismatch, o.x_conflict, o.stalls, o.min_depth, if good { "" } else { " MISSES the nap bar" }, op.ring_mismatch, op.distinct.iter().skip(1).copied().collect::<Vec<_>>(), op.max_gap.0));
                let mut r = design("the identity pointer, entries after the pops", 0, None);
                r.held = 0;
                r.max_picks_per_rel = 1;
                r.bed_cycle = cyc;
                r.rs_extra = extra;
                r.late_entries = true;
                let ro = run(&r);
                corner.push(format!("  (the identity pointer there: distinct {:?}, worst {} gt)", ro.distinct.iter().skip(1).copied().collect::<Vec<_>>(), ro.max_gap.0));
            }
            let w = v.bar_n("ctl-pick: THE ORDER-INDEPENDENCE CHECK (ironspec 12:02p; G1's B6 on 239acc610): with EVERY circuit entry processed after the same REL's pops, the sensed ring on the mark FIFO stays in sync with the carts popped, its rejoins in pick order, no X type conflict, at each point", n_corner, corner_good == n_corner);
            lines.push(format!("  THE ORDER-INDEPENDENCE CHECK (the mark FIFO; every entry after the same REL's pops): {corner_good} of {n_corner} points in sync: {w}"));
            let sc = v.scope("ctl-pick: the nap bar on the entries-after-the-pops stress rows (every rejoin chance resolved late)", "a stress row, not the +-2 gt corner: the entries' separation from the lane's ring steps is judged above (> 2 gt each side, so no +-2 gt shift reorders them); here EVERY entry is delayed past those pops, and both pointer forms reach 24,000+ gt: the nap bar's margin then depends on the rejoin race, the composed battery's gt-level corner (step b) judges it");
            lines.push(format!("  THE ENTRIES-AFTER-THE-POPS STRESS ROW: {}: {sc}", corner.join("; ")));
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
        let sc = v.scope("ctl-pick: the stop stalled 2,000 / 6,000 gt (a long pause) and the way-in's 4th napper", crate::ctlcredit::OUT_WHY);
        lines.push(format!("  OUT (a long pause): THE WAY-IN with the stop stalled ({n} rows): at most {worst} nappers in the way-in ({worst_at}){}: {sc}", if miss.is_empty() { String::new() } else { format!(" ({})", miss.join(" | ")) }));
        // ((B)'s two prints, judged on NORMAL rows (G1 on 8e3d5f208 (2); the stall rows are a long pause, OUT): THE RESET ->
        // SET LAG from the FALL of the previous napper's RESET level to the next napper's SET, over the normal nights and the
        // no-hold sweep (the bed cycle 197 / 168 gt x (B)'s travel + 0 / 40 / 120): > 0 everywhere; A NAPPER WAITING BEHIND
        // THE HOLD (reaching it while the previous one is still there, resting on the flat top (9,25,8)): none. Its CONTROL:
        // the hold's (9,22,3) term dropped (`hold_no_exit`) on the same rows: a SET inside the RESET level by name)
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
        // (the pinned verdict: the adopted design (the sensed ring, the positional release, (b'+w), the record 3-deep, the
        // bed a single server) passes every judged row and every control bites, EXCEPT the two timing misses pinned by name
        // below until the ring's laid timing is ruled on; the CLI then fails by name)
        let mut v = Verdict::new("ctl-pick-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
        let fails = v.fails();
        println!("ctl-pick fails: {:?}", fails);
        // (day-1's job 3: the merged interface's "each lane's ring steps 6 gt after the lane's SENSED pop" (the laid ring's
        // STEP_LAG) puts lane -1's circuit entry (D(-1) at REL' + 5) 1 gt BEFORE its ring's step (REL' + 6), against the REL
        // model's order (the entry after the lane's last step), and lane 3's entry (REL' + 43) on its ring's skip step (the
        // step + 20): both timing bars miss by name, PINNED)
        // (day-1 15:00: the interface now states the record's hardware shift, the napper's own ARM3 (the station detector's
        // press + 4 gt, cc767b78 14:51), where the model still shifts at REL + 61: the read-back misses by name, PINNED until
        // the model's shift is re-keyed to the arrival (job 5))
        let expected: [&str; 3] = ["THE PRESS / TAP MARGINS", "THE ENTRY / STEP SEPARATION", "THE REL TAPS read back from the interface"];
        assert_eq!(fails.len(), expected.len(), "ctl-pick-bench: the pinned verdict changed ({} misses, {} pinned): {:#?}", fails.len(), expected.len(), fails);
        for e in expected {
            assert_eq!(fails.iter().filter(|f| f.contains(e)).count(), 1, "ctl-pick-bench: the pinned miss \"{e}\" is not exactly once among {:#?}", fails);
        }
        assert_eq!(v.finish(false).is_ok(), expected.is_empty(), "ctl-pick-bench: the verdict must fail exactly on the pinned misses");
    }
}
