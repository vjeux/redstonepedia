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
/// ironspec 02:16 (3), ACCEPTED (day-1 02:15's cause fix): SEEN3_OLD samples SEEN3 at its OWN tap REL + 79 (the window
/// + 76..+ 82 removes every late and missing loss under a world open (idle 2, the carts 3 behind) at every tick of the
/// batch (+ 74..+ 82 with no open); re-centred from + 78 by the timing rule; X = + 72 sampled just before a faulted
/// batch's late D(3) set SEEN3).
pub const S3OLD_TAP: i64 = 79;
/// ironspec 02:16 (3): THE ONE-CREDIT-PER-BATCH GUARD (day-1's candidate, priced ~4 blocks: a 20-gt extender on the
/// completion pulse inhibiting D(7)'s credit line): a late slot 3's D(7) landing 1..7 gt after the D(-1) completion of
/// its own batch (inside SEEN3's reset delay) is not credited twice; the next batch's own D(7) comes ~80 gt later.
pub const DM1_BLOCK7: i64 = 20;
/// The resync net's credit: its dropper pulse lands this many gt after the minting wait's start; its source tag.
pub const NET_LAT: i64 = 4;
pub const NET_SRC: usize = usize::MAX - 1;
/// The lift's exit detector (-4,24,9) -> J(-1) (-1,25,9): the cart's ride (gt; ironfarm's split-row trace, ~3 cells).
pub const EXIT_TO_JM1: i64 = 10;
/// The D-press ring's resync: the row read empty this long before the batch-start state is forced (gt).
pub const RESYNC_QUIET: i64 = 30;
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
    /// G1 01:14 / ironspec 01:33 (3): THE D-PRESS RING ("armed until filled"): no REL-keyed ring reset; J(l)'s own D press
    /// disarms it and arms the next lane's J, chained -1 -> 3 -> 7 -> -1 (D(7) arms J(-1) and disarms J(7); J(7) is a
    /// latch, armed by D(3)); K(3)'s mask also covers D(3)'s ring outputs.
    pub ring: bool,
    /// The D-press ring's RESYNC on evidence (ironspec 01:41 (2)): the split row EMPTY (every cart that crossed the lift's
    /// exit detector (-4,24,9) has pressed a D or passed J(7): an up / down count at 0) for >= RESYNC_QUIET gt, and the
    /// ring not in the batch-start state -> J(-1) armed, J(3) and J(7) disarmed.
    pub ring_resync: bool,
    /// The D-press ring keeping the REL-keyed batch-start reset at X as its batch-boundary resync (J(-1) armed, J(3) and
    /// J(7) disarmed at REL + X): the hybrid.
    pub ring_reset_x: bool,
    /// SEEN3_OLD's sample at its OWN tap REL + n instead of the ring reset's X (ironspec 02:06's cause trace: the X
    /// sample misses a faulted batch's late D(3) by the latch's set delay).
    pub s3old_tap: Option<i64>,
    /// THE ONE-CREDIT-PER-BATCH GUARD (ironspec 02:16 (3)): a D(-1) completion blocks D(7)'s credit for this many gt (0:
    /// off).
    pub dm1_block7: i64,
    /// THE RESYNC NET's candidate (REL waiting with one batch in flight on two consecutive RELs mints a credit).
    pub resync_net: bool,
    /// The pick-dependent reset (ironspec 19:56 (4)): on a lane -1 pick the reset leaves J(-1) off and arms J(3)
    /// directly (no emulated D(-1)).
    pub pick_dep: bool,
    /// The fast credit path (the folded comparator and IN_FLIGHT; ironspec 20:26); false: the edge comparator + the
    /// AND into the dropper, readable only from the hopper's read (D(7) + 10).
    pub fast: bool,
    /// day-1's double-credit fix candidate (22:40): the D(-1) completion reads SEEN3_OLD, SEEN3 sampled at the ring
    /// reset tap (REL + X): a late slot 3 keeps SEEN3 set past X (the completion armed); a stray D(3) after X never sets it.
    pub dm1_old: bool,
    /// day-1's DONE form (22:53): the completion = D(-1) AND SEEN3 AND NOT DONE, DONE set by every credit and reset by
    /// REL ("this period's credit has come"); overrides `dm1_old`.
    pub dm1_done: bool,
    /// day-1's STORE form (22:59, ironspec 22:56's REL-keyed "owed" count): every credit (D(7)'s and the completion's)
    /// gated by the store being EMPTY (the hopper's read < 2 items AND NOT IN_FLIGHT); the completion = D(-1) AND SEEN3
    /// AND store-empty; overrides `dm1_done` / `dm1_old`.
    pub dm1_store: bool,
    /// day-1's OWED form (23:03): every credit gated by OWED >= 1 (at most one stored credit: the read <= the ballast + 1
    /// item, and NOT IN_FLIGHT); credits answer RELs in ORDER (FIFO), judged by count.
    pub dm1_owed: bool,
    /// A JAM (ironspec 23:00 (1)): (batch, slot, segment) the slot's cart stops on the split row in that segment (0:
    /// between J(-1) and J(3); 1: between J(3) and J(7)) and every later cart that would ride through the segment stops
    /// behind it.
    pub jam: Option<(usize, usize, usize)>,
    /// day-1's jam discrimination (23:06): the D(-1) completion also needs the split row EMPTY east of J(-1) (an
    /// entry / exit occupancy count: a lost cart never entered; a jammed or still-late one holds it above 0).
    pub occ_gate: bool,
    /// (batch, gt after its REL) F1's AND dipping for one tick (a slot-1 string held only by the raised check, released at
    /// a re-check) and re-rising: a second REL edge while the request latch's reset is still landing.
    pub f1_dip: Vec<(usize, i64)>,
    /// day-1's second-REL fix: the hopper's unlock keyed on the request latch (one unlock per request), not REL's edge.
    pub unlock_on_request: bool,
    /// (batch, extra gt) a returning napper's J arrivals jittered past its K window's centre (the window keeps the
    /// nominal timing: K is keyed on the napper's release, the jitter on its ride).
    pub napper_jitter: Vec<(usize, i64)>,
}

impl Row {
    pub fn normal(name: &str) -> Row {
        Row { name: name.to_string(), hopper0: HOPPER0, dropper0: DROPPER0, t_max: 3000, missing: vec![], napper: vec![], stray: vec![], holds_empty: vec![], late: vec![], open: None, pick: vec![], bypass: false, credit_dm1: true, ring_x: RING_X, ring: false, ring_resync: false, ring_reset_x: false, s3old_tap: Some(S3OLD_TAP), dm1_block7: DM1_BLOCK7, resync_net: true, pick_dep: false, fast: true, dm1_old: true, dm1_done: false, dm1_store: false, dm1_owed: false, jam: None, occ_gate: false, f1_dip: vec![], unlock_on_request: true, napper_jitter: vec![] }
    }
}

/// Planted defects (one per control).
#[derive(Clone, Copy, Default, Debug)]
pub struct Defects {
    pub no_k3_mask: bool,
    /// K(3)'s mask NOT covering D(3)'s J(3)-reset output (the twin of residual (ii)'s cause fix).
    pub k3_j3_unmasked: bool,
    /// K(7) masking NONE of D(7)'s outputs (the twin of ironspec 02:43 (1)'s K(7)).
    pub k7_unmasked: bool,
    /// The guard opened by the D(-1) completion only (the twin of its generalization to every credit).
    pub guard_completion_only: bool,
    /// K(-1)'s mask removed from all three of D(-1)'s outputs (J(-1) off, J(3) armed, the credit input).
    pub no_km1_mask: bool,
    /// K(-1)'s mask removed from ONE output each (G1 item 9: separate twins).
    pub no_km1_jm1: bool,
    pub no_km1_j3: bool,
    pub no_km1_credit: bool,
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
    /// Every credit dispensed twice (item 1's control: two credits for one batch).
    pub double_dispense: bool,
    /// G1 CHANGES 4 C1 / C2's controls: the credit of this batch DROPPED (the dropper never moves its item).
    pub drop_credit: Option<usize>,
    /// A second batch's credit dropped (the two-short state).
    pub drop_credit2: Option<usize>,
    /// K's half-width shortened (item 4's control: a jittered napper misses its own K and is misrouted).
    pub k_half: Option<i64>,
    /// The hopper's cooldown running through a world open (item 11's control: not saved).
    pub cooldown_unsaved: bool,
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
    /// Per credit: the hopper's items just before it lands (ironspec 02:16 (3): the store's level at both credits of a
    /// GAIN row; a cap refuses only at full).
    pub credit_level: Vec<i64>,
    /// D(7) presses whose credit THE ONE-CREDIT-PER-BATCH GUARD blocked (their ticks).
    pub d7_blocked: Vec<i64>,
    /// Every D(7) credit's gap after the last D(-1) completion (G1 02:34 (2): the guard's margin to legitimate credits).
    pub d7_gap_after_completion: Vec<i64>,
    /// Every napper press's margin inside its own K window (lane, margin; negative = outside), ironspec 02:43 (1).
    pub napper_k_margin: Vec<(i32, i64)>,
    /// At each REL wait's start (REL due, no credit): the tick and the batches in flight (the resync's premise).
    pub wait_inflight: Vec<(i64, usize)>,
    /// The resync net's mints (their ticks).
    pub net_fires: Vec<i64>,
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
    /// THE D-PRESS RING (`ring`): its resyncs, any resync with a cart on the row (the planted mid-row row), and per J the
    /// least gap from a cart diverting at that J to the next cart reaching it (gt; ironspec 01:33 (3) / G1 01:33b: the
    /// follower must reach the J after the leader's D press has disarmed it: D at J + J_TO_D, the T back at + RETURN).
    pub ring_resyncs: usize,
    pub resync_occupied: usize,
    pub j_gap_min: [Option<i64>; 3],
    /// ironspec 01:40 (c): every batch cart that rode past every J (into the way-in leg), by name.
    pub leg_entries: Vec<String>,
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
    /// Per credit: (the batch whose D(7) or D(-1) completed it, the batch whose D(3) set SEEN3; usize::MAX: a stray):
    /// the credit's SOURCE, judged PER BATCH beside the FIFO ledger (G1 CHANGES 4 C2: every released batch's evidence
    /// makes exactly one credit; a gain and a loss never cancel).
    pub credit_src: Vec<(usize, usize)>,
    /// The planted dropped credits (their batches).
    pub dropped: Vec<usize>,
    /// Credits still on their way into the hopper when the run ends (a dispense due past the last tick).
    pub in_flight_at_end: usize,
    /// The last REL's claim exempted from the ledger, with why (only when its batch's completion is still pending at
    /// the run's end: G1 CHANGES 4 C1; printed).
    pub ledger_exempt: Option<(usize, String)>,
    /// Per credit: the REL it ANSWERS in the FIFO ledger (each REL a claim, each credit answering the oldest open one;
    /// None: a GAIN, no claim open): G1 on 60ada7ea8 B2, ironspec 23:59c (cc767b78's r13).
    pub answers: Vec<Option<usize>>,
    /// The ledger's RELs never answered (every REL is judged: the pulses stop at the horizon and the run continues
    /// until every released batch completes).
    pub unanswered: Vec<usize>,
    /// Every stray press as it landed: (tick, lane, SEEN3 at the press) (item 7: the injections read back).
    pub strays_landed: Vec<(i64, i32, bool)>,
    /// Napper misroutes (routed by the J resolver's shapes, never by identity: item 4).
    pub napper_misroutes: Vec<String>,
    /// Per REL: the transfers its unlock windows made (item: exactly one transfer per REL), and the extra REL edges.
    pub transfers: Vec<usize>,
    pub ghost_rels: Vec<i64>,
    /// Per credit: the tick `armed` (RE_ARMED, the fast path's OR) first reads it after the press (item 8: measured).
    pub armed_latency: Vec<i64>,
    /// Every credit's press tick (D(7)'s or the completion's) and the ring reset taps (REL + X): PRINTED only (the
    /// judged attribution is the ledger's `answers`).
    pub credit_press: Vec<i64>,
    pub taps: Vec<i64>,
    /// The OWED form (the store read + IN_FLIGHT): its gate; the ledger judges every form alike.
    pub fifo: bool,
    /// Carts stopped behind a planted jam (batch, slot or napper) and the jam's tick.
    pub piled: Vec<(usize, Option<usize>)>,
}

struct Press {
    at: i64,
    lane: i32,
    batch: usize,
    slot: Option<usize>,
    napper: bool,
    /// G11's emulated press (redstone into the ring's outputs, never into the credit input: G1 on 2260db896 item 2).
    emulated: bool,
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
    // (a second REL edge's unlock: F1's AND re-rising while the request latch's reset is still landing)
    let mut ghost: Option<(i64, i64, usize)> = None;
    // (at, the batch that completed it, the batch whose D(3) set SEEN3)
    let mut dispense: Vec<(i64, usize, usize)> = Vec::new();
    let mut seen3_by = usize::MAX;
    let mut seen3_old = false;
    // (DONE, day-1's alternative to the tap: set by every credit, reset by REL: "this period's credit has come")
    let mut done = false;
    // (COMP, day-1 22:58: set by the completion, reset at the tap: D(7)'s credit reads NOT COMP, so a late slot 3's D(7)
    // landing just after its batch's completion (SEEN3's reset still landing) can't credit again, while an early
    // (misrouted) D(7) before the tap keeps crediting)
    let mut comp = false;
    let mut jam_seg: Option<usize> = None;
    let k_half = def.k_half.unwrap_or(K_HALF);
    // (the J's west of a returning napper's lane held off over its passage: OFF14_y)
    let mut off_windows: Vec<(i64, i64, usize)> = Vec::new();
    // (credits waiting for `armed` to read them: (the press tick))
    let mut wait_armed: Vec<i64> = Vec::new();
    // the latches' drives (block domain): set / reset intervals
    let (mut jm1_set, mut jm1_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let (mut j3_set, mut j3_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    // (THE D-PRESS RING's J(7) latch: armed by D(3), disarmed by D(7); without `ring` J(7) is on by default)
    let (mut j7_set, mut j7_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let mut j7 = false;
    let mut rel_waiting = false;
    let mut had_wait = false;
    let mut net_armed = false;
    let mut last_completion: Option<i64> = None;
    let mut quiet: i64 = 0;
    let mut last_div_at: [Option<i64>; 3] = [None; 3];
    let (mut s3_set, mut s3_rst): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let (mut hold3, mut hold7): (Vec<(i64, i64)>, Vec<(i64, i64)>) = (Vec::new(), Vec::new());
    let mut if_set: Vec<i64> = Vec::new();
    let (mut jm1, mut j3, mut seen3, mut in_flight) = (true, false, false, false);
    let mut resets: Vec<i64> = Vec::new();
    // (SEEN3_OLD's own sample ticks REL + tap (ironspec 02:16 (3)): a redstone delay line from REL, so a world open moves
    // the pending ones with the block domain, like X's taps)
    let mut s3old_at: Vec<i64> = Vec::new();
    let mut pending = false;
    let mut last_rel: Option<i64> = None;
    let mut pulse_at = 0i64;
    let mut presses: Vec<Press> = Vec::new();
    let mut k_windows: Vec<(i64, i64, usize)> = Vec::new();
    // (each K window's napper batch, and the batches already credited: K(7) masks by ORDER, day-1 02:51)
    let mut k_win_batch: Vec<usize> = Vec::new();
    let mut credited_b: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // (batch carts that left the lane route into the way-in leg)
    let mut left_route: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
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
            for d in [&mut jm1_set, &mut jm1_rst, &mut j3_set, &mut j3_rst, &mut j7_set, &mut j7_rst, &mut s3_set, &mut s3_rst, &mut hold3, &mut hold7] {
                sh(d);
            }
            for x in dispense.iter_mut().filter(|x| x.0 >= t_open) {
                x.0 += idle;
            }
            for x in if_set.iter_mut().filter(|x| **x >= t_open) {
                *x += idle;
            }
            // (X's taps and SEEN3_OLD's sample ticks are redstone delays from REL: they move with the block domain)
            for x in resets.iter_mut().chain(s3old_at.iter_mut()).filter(|x| **x >= t_open) {
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
            if let Some(u) = ghost.as_mut() {
                if u.0 + u.1 > t_open {
                    u.0 += idle;
                }
            }
            for x in off_windows.iter_mut() {
                if x.0 >= t_open {
                    x.0 += idle;
                }
                if x.1 >= t_open {
                    x.1 += idle;
                }
            }
            for x in wait_armed.iter_mut().filter(|x| **x >= t_open) {
                *x += idle;
            }
            if cool_until > t_open && !def.cooldown_unsaved {
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
        for &(at, b, src) in &dispense {
            if at == t {
                if (def.drop_credit == Some(b) || def.drop_credit2 == Some(b)) && !o.dropped.contains(&b) {
                    o.dropped.push(b);
                    continue;
                }
                if dropper > 0 {
                    dropper -= 1;
                    o.credit_level.push(items);
                    if items == 0 {
                        cool_until = t + INSERT_COOLDOWN;
                    }
                    items += 1;
                    if def.single_latch {
                        items = items.min(1);
                    }
                    o.credits.push((t, b));
                    o.credit_src.push((b, src));
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
                o.transfers[ri] += 1;
            }
            if t == a + w {
                o.at_rel[ri].1 = items;
                unlock = None;
                if def.single_latch {
                    items = 0;
                }
            }
        }
        // the second REL edge's unlock window (one push if the hopper's cooldown allows it)
        if let Some((a, w, ri)) = ghost {
            if t >= a && t < a + w && items > 0 && t >= cool_until && o.transfers[ri] < 2 {
                items -= 1;
                dropper += 1;
                cool_until = t + TRANSFER_COOLDOWN;
                o.transfers[ri] += 1;
                ghost = None;
            } else if t >= a + w {
                ghost = None;
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
        j7 = latch(j7, &j7_set, &j7_rst);
        // (THE D-PRESS RING's RESYNC on evidence: the row empty (no cart between the lift's exit detector and its J or
        // its lane: every cart that crossed (-4,24,9) has pressed a D or passed J(7)) for RESYNC_QUIET gt, and the ring
        // not in the batch-start state -> J(-1) armed, J(3) and J(7) disarmed)
        if row.ring && row.ring_resync {
            let on_row = carts.iter().any(|c| c.arr[0] - EXIT_TO_JM1 <= t) || !o.piled.is_empty();
            quiet = if on_row { 0 } else { quiet + 1 };
            if quiet >= RESYNC_QUIET && !(jm1 && !j3 && !j7) {
                o.ring_resyncs += 1;
                if on_row {
                    o.resync_occupied += 1;
                }
                jm1_set.push((t + SWITCH, t + SWITCH + PRESS_W));
                j3_rst.push((t + RETURN, t + RETURN + PRESS_W));
                j7_rst.push((t + RETURN, t + RETURN + PRESS_W));
                quiet = 0;
            }
        }
        seen3 = latch(seen3, &s3_set, &s3_rst);
        // (SEEN3_OLD, ironspec 22:52 (2) / 22:55 (2): SET at the ring reset tap (REL + 72) if SEEN3 is set, RESET by every
        // credit; the sample also reads NOT IN_FLIGHT, so a credit whose SEEN3 reset is still landing is never re-captured)
        if (if row.s3old_tap.is_some() { s3old_at.contains(&t) } else { resets.contains(&t) }) && seen3 && !in_flight {
            seen3_old = true;
        }
        // (DONE, day-1 22:56: RESET at the same tap (REL + 72), SET by every credit: at batch b+1's slot 1 D(-1)
        // (REL_(b+1) + 105) it says whether batch b's credit (normally at + 86) has come; unlike SEEN3_OLD it needs no
        // SEEN3 at the tap, so a legitimate late D(3) after + 72 (a missing cart's diverted slot 3) still completes)
        if resets.contains(&t) {
            done = false;
            comp = false;
        }
        let read = if t >= CMP { items_hist[(t - CMP) as usize] } else { items };
        if if_set.contains(&t) {
            in_flight = true;
        }
        if in_flight && read >= need {
            in_flight = false;
        }
        // F1 and REL (the golem pulses stop at the row's horizon; the run continues 600 gt so every batch released
        // completes and every credit it makes is counted: ONE HORIZON, G1 on 60ada7ea8 B2)
        if t % PERIOD == 0 && t < row.t_max {
            pending = true;
            pulse_at = t;
        }
        let b_ok = last_rel.is_none_or(|r| t >= r + PERIOD);
        let holds = !row.holds_empty.iter().any(|&(a, b)| t >= shift(a) && t < shift(b));
        let armed = read >= need || ((row.bypass || row.fast) && in_flight);
        // (item 8: the latency from a credit's press to `armed` reading it, measured on presses that found it false)
        wait_armed.retain(|&p| {
            if armed && t > p {
                o.armed_latency.push(t - p);
                false
            } else {
                true
            }
        });
        let armed_now = armed;
        // (ironspec 02:46 (1) / G1 02:44b (1): THE RESYNC's PREMISE: when REL is due and waits for a credit, how many
        // batches are in flight (released, a slot neither in a lane nor missing / picked): recorded at each wait's start)
        let due = pending && t >= pulse_at + REL_LAT && b_ok && holds;
        if due && !armed && !rel_waiting {
            rel_waiting = true;
            had_wait = true;
            let inflight = (0..o.presses.len()).filter(|&b| (0..3).any(|s| o.entered[b][s].is_none() && !left_route.contains(&(b, s)) && !row.missing.contains(&(b, s)) && row.pick.iter().find(|p| p.0 == b).map(|p| p.1) != Some(s))).count();
            o.wait_inflight.push((t, inflight));
            // (THE RESYNC NET's candidate (ironspec 02:40 (4) / 02:46 (1), G1 02:44b (1)): REL waiting for a credit (no halt:
            // `due` needs the holds) with ONE batch in flight on TWO CONSECUTIVE RELs mints one credit: a latch set by the
            // first such wait, cleared by any REL that fires on its clock without waiting, by a halt and by a world open;
            // no timer, no running count)
            if row.resync_net && inflight == 1 {
                if net_armed {
                    dispense.push((t + NET_LAT, NET_SRC, NET_SRC));
                    o.net_fires.push(t);
                    net_armed = false;
                } else {
                    net_armed = true;
                }
            }
        }
        if !holds {
            net_armed = false;
        }
        if !due || armed {
            rel_waiting = false;
        }
        if pending && t >= pulse_at + REL_LAT && b_ok && holds && armed {
            pending = false;
            if !had_wait {
                net_armed = false;
            }
            had_wait = false;
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
            o.transfers.push(0);
            // (a one-tick dip of F1's AND at REL + d re-rises at + d + 1: a second REL edge while the request latch's
            // reset (RESET) is still landing; its unlock window can move a second item unless the unlock is keyed on the
            // request latch (one unlock per request: `unlock_on_request`))
            let bi = o.rels.len() - 1;
            if let Some(&(_, d)) = row.f1_dip.iter().find(|x| x.0 == bi) {
                // (`unlock_on_request`, day-1's fix: every REL consumer keyed on the request latch's reset edge, one per
                // request, so a re-risen F1 AND makes no second REL at all)
                if d + 1 < RESET && !row.unlock_on_request {
                    o.ghost_rels.push(t + d + 1);
                    ghost = Some((t + d + 1 + UNLOCK, UNLOCK_W, o.at_rel.len() - 1));
                }
            }
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
                    // (K(lane) and the OFF windows of the J's west of it ride the napper's nominal timing; its ride may be
                    // jittered past them (item 4's control))
                    k_windows.push((nt[lane] - k_half, nt[lane] + k_half, lane));
                    k_win_batch.push(b);
                    for (ji, &ntj) in nt.iter().enumerate().take(lane) {
                        off_windows.push((ntj - k_half, ntj + k_half, ji));
                    }
                    let jit = row.napper_jitter.iter().filter(|x| x.0 == b).map(|x| x.1).sum::<i64>();
                    carts.push(Cart { batch: b, slot: None, ji: 0, arr: [nt[0] + jit, nt[1] + jit, nt[2] + jit], nap: Some(lane), picked: false });
                }
            }
            let x = t + row.ring_x;
            if let Some(tap) = row.s3old_tap {
                s3old_at.push(t + tap);
            }
            let dep = row.pick_dep && pick == Some(0);
            if !def.and_rearm {
                resets.push(x);
                if row.ring && !row.ring_reset_x {
                    // (the D-press ring: no REL-keyed reset of the J's; the tap X stays only for SEEN3_OLD's sample)
                } else if dep {
                    // (the pick-dependent reset: J(-1) left off, J(3) armed directly)
                    jm1_rst.push((x + RETURN, x + RETURN + RESET_W));
                    j3_set.push((x + SWITCH, x + SWITCH + RESET_W));
                } else {
                    jm1_set.push((x + SWITCH, x + SWITCH + RESET_W));
                    j3_rst.push((x + RETURN, x + RETURN + RESET_W));
                    if row.ring {
                        j7_rst.push((x + RETURN, x + RETURN + RESET_W));
                    }
                }
            }
            match pick {
                Some(0) => {
                    if !dep {
                        let e = if def.reset_order { x - 2 } else { x + RESET_W + 2 };
                        presses.push(Press { at: e, lane: -1, batch: b, slot: None, napper: false, emulated: true });
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
            // (G1 item 4 / ironspec 22:06 (2): EVERY cart is routed by the J resolver's shapes, never by identity:
            // J(y) = (ARMED_y AND NOT HOLD_y AND NOT OFF14_y) OR K(y); a napper diverts where K(its lane) holds the J at
            // branch and passes the J's west of it under OFF; a batch cart reaching J(y) inside K(y) diverts too)
            let k_on = k_windows.iter().any(|&(a, b2, kl)| kl == c.ji && t >= a && t <= b2);
            let off_on = off_windows.iter().any(|&(a, b2, kl)| kl == c.ji && t >= a && t <= b2);
            let armed_j = match c.ji {
                0 => jm1,
                1 => j3 && !active(&hold3, t),
                _ => (!row.ring || j7) && !active(&hold7, t),
            };
            let branch = (armed_j && !off_on) || k_on;
            // (the per-J follower gap: from the last diversion at this J to this cart reaching it)
            if let Some(td) = last_div_at[c.ji] {
                let gap = t - td;
                if gap > 0 {
                    o.j_gap_min[c.ji] = Some(o.j_gap_min[c.ji].map_or(gap, |m: i64| m.min(gap)));
                }
            }
            if branch {
                last_div_at[c.ji] = Some(t);
            }
            if let Some(lane) = c.nap {
                if branch && c.ji != lane {
                    o.napper_misroutes.push(format!("batch {}'s lane {} napper into lane {} at t {t}", c.batch, LANES[lane], LANES[c.ji]));
                } else if !branch && c.ji == 2 {
                    o.napper_misroutes.push(format!("batch {}'s lane {} napper passed every J at t {t}", c.batch, LANES[lane]));
                }
            }
            if let Some(s) = c.slot {
                if c.batch < o.pass.len() {
                    o.pass[c.batch][s][c.ji] = t;
                }
            }
            if branch {
                presses.push(Press { at: t + J_TO_D, lane: LANES[c.ji], batch: c.batch, slot: c.slot, napper: c.nap.is_some(), emulated: false });
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
                // (a planted JAM: the jamming cart stops inside the segment it enters; every later cart riding into the
                // same segment stops behind it)
                let seg = c.ji;
                let jammer = row.jam.is_some_and(|(jb, js, jseg)| jb == c.batch && Some(js) == c.slot && jseg == seg);
                if jammer {
                    jam_seg = Some(seg);
                    o.piled.push((c.batch, c.slot));
                } else if jam_seg == Some(seg) {
                    o.piled.push((c.batch, c.slot));
                } else {
                    c.ji += 1;
                    keep.push(c);
                }
            } else if let Some(s) = c.slot {
                if !c.picked {
                    o.misroutes.push(format!("batch {}'s lane {} cart passed every J at t {t}", c.batch, LANES[s]));
                    o.leg_entries.push(format!("A BATCH CART ENTERED THE WAY-IN LEG: batch {}'s lane {} cart at t {t}", c.batch, LANES[s]));
                    // (it has left the lane route: no longer in flight for the resync net's occupancy evidence)
                    left_route.insert((c.batch, s));
                    o.lane_delta[s] -= 1;
                }
            }
        }
        carts = keep;
        for &(s, lane) in &row.stray {
            if shift(s) == t {
                presses.push(Press { at: t, lane, batch: usize::MAX, slot: None, napper: false, emulated: false });
                o.strays_landed.push((t, lane, seen3));
            }
        }
        // presses landing now
        let in_k = |l: usize| k_windows.iter().any(|&(a, b2, kl)| kl == l && t >= a && t <= b2);
        // (THE SPLIT ROW's OCCUPANCY east of J(-1), day-1 23:06: carts that passed J(-1) on the through route and have
        // not yet left it (a D press's diversion or J(7)), plus any stopped behind a jam; the jam-vs-loss sensor: a lost
        // cart never entered (0), a jammed or still-late one is inside (>= 1))
        let occupancy = carts.iter().filter(|c| c.ji >= 1).count() + o.piled.len();
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
            // (ironspec 02:43 (1)'s condition: every napper press's margin inside its own K window (negative = outside))
            if p.napper && !p.emulated {
                let li = match p.lane { -1 => 0, 3 => 1, _ => 2 };
                let m = k_windows.iter().filter(|w| w.2 == li).map(|&(a, b2, _)| (t - a).min(b2 - t)).max();
                if let Some(m) = m {
                    o.napper_k_margin.push((p.lane, m));
                }
            }
            let pick = row.pick.iter().find(|q| q.0 == p.batch).map(|q| q.1);
            match p.lane {
                -1 => {
                    // (K(-1)'s mask covers all three of D(-1)'s outputs, ironspec 19:56 (3) / day-1 20:11: a lane -1
                    // napper's own D(-1) can land after the next batch's ring reset (R6 + 10 or later) and would turn
                    // the freshly armed J(-1) off under batch b+1's slot 1; G1 item 9: one twin per output)
                    let masked_jm1 = !(def.no_km1_mask || def.no_km1_jm1) && in_k(0);
                    let masked_j3 = !(def.no_km1_mask || def.no_km1_j3) && in_k(0);
                    let masked_credit = !(def.no_km1_mask || def.no_km1_credit) && in_k(0);
                    if !masked_jm1 {
                        if p.napper && jm1 {
                            o.napper_effects.push(format!("t {t}: the lane -1 napper's D(-1) turned J(-1) off"));
                        }
                        jm1_rst.push((t + RETURN, t + RETURN + PRESS_W));
                    }
                    if !masked_j3 {
                        if p.napper && !j3 {
                            o.napper_effects.push(format!("t {t}: the lane -1 napper's D(-1) armed J(3)"));
                        }
                        j3_set.push((t + SWITCH, t + SWITCH + PRESS_W));
                    }
                    if pick == Some(1) && p.slot == Some(0) {
                        emulated.push(Press { at: t + 2, lane: 3, batch: p.batch, slot: None, napper: false, emulated: true });
                    }
                    // (the D(-1) completion, ironspec 20:17 (A) / 22:06 (2): D(-1)'s edge while SEEN3 is set, under
                    // K(-1)'s mask, KEYED ON THE CIRCUIT'S INPUTS, never on who pressed (a stray or a napper outside K
                    // credits too; only G11's emulated presses, wired into the ring's outputs alone, are excluded); it
                    // completes the batch whose D(3) set SEEN3; `dm1_old` reads SEEN3_OLD instead (day-1 22:40))
                    let store_empty = !(read >= need || in_flight);
                    // (OWED >= 1 = at most one stored credit (items <= the ballast + 1) and none in flight)
                    let owed_ok = !(read > need || in_flight);
                    let s3_in = if row.dm1_owed { seen3 && owed_ok } else if row.dm1_store { seen3 && store_empty } else if row.dm1_done { seen3 && !done } else if row.dm1_old { seen3 && seen3_old } else { seen3 };
                    if row.credit_dm1 && !p.emulated && s3_in && !masked_credit && !(row.occ_gate && occupancy > 0) {
                        o.credit_press.push(t);
                        if p.napper {
                            o.napper_effects.push(format!("t {t}: the lane -1 napper's D(-1) made a credit"));
                        }
                        let lat = if row.fast { FAST } else { AND };
                        dispense.push((t + lat + DROPPER, seen3_by, seen3_by));
                        if def.double_dispense && seen3_by == 5 {
                            dispense.push((t + lat + DROPPER + 1, seen3_by, seen3_by));
                        }
                        if !armed_now {
                            wait_armed.push(t);
                        }
                        o.credit_latency.push(if row.fast { FAST } else { AND + DROPPER + CMP });
                        o.credit_ready.push((t + if row.fast { FAST } else { AND + DROPPER + CMP }, seen3_by));
                        o.dropper_edges.push((t + lat, t + lat + PRESS_W));
                        if row.fast {
                            if_set.push(t + FAST);
                        }
                        s3_rst.push((t + RESET, t + RESET + PRESS_W));
                        seen3_old = false;
                        done = true;
                        comp = true;
                        o.dm1_credits += 1;
                        last_completion = Some(t);
                        credited_b.insert(seen3_by);
                    }
                }
                3 => {
                    // (K(3)'s mask covers ALL of D(3)'s outputs, as K(-1)'s covers D(-1)'s (day-1 02:40, residual (ii)'s cause
                    // fix): a lane-3 napper's own D(3) landing after batch b+1's D(-1) armed J(3) would turn that J(3) off
                    // under b+1's slot 2 (a misroute into lane 7 and batch b+1's credit lost for good); `k3_j3_unmasked` is
                    // the twin without it)
                    if def.no_k3_mask || def.k3_j3_unmasked || !in_k(1) {
                        j3_rst.push((t + RETURN, t + RETURN + PRESS_W));
                    }
                    // (the D-press ring: D(3) arms J(7); K(3)'s mask covers it, as it covers SEEN3's set)
                    if row.ring && (def.no_k3_mask || !in_k(1)) {
                        j7_set.push((t + SWITCH, t + SWITCH + PRESS_W));
                    }
                    if def.no_k3_mask || !in_k(1) {
                        if p.napper && !seen3 {
                            o.napper_effects.push(format!("t {t}: the lane-3 napper's D(3) set SEEN3"));
                        }
                        s3_set.push((t + SET, t + SET + PRESS_W));
                        seen3_by = p.batch;
                    }
                    if pick == Some(2) && p.slot == Some(1) {
                        emulated.push(Press { at: t + DELTA7, lane: 7, batch: p.batch, slot: None, napper: false, emulated: true });
                    }
                }
                _ => {
                    // (ironspec 02:43 (1) / G1 02:44b (2): K(7) MASKS ALL OF D(7)'s OUTPUTS (the credit line, SEEN3's reset, the
                    // ring form's J(-1) arm and J(7) reset), as K(-1) / K(3) mask D(-1)'s / D(3)'s: a lane-7 napper's own D(7)
                    // landing in the next batch's SEEN3 would credit that batch early or clear its SEEN3 (the probe: a lane 7
                    // napper 50..90 gt late changed the store without it); `k7_unmasked` is the twin)
                    // (K(7) by ORDER (day-1 02:51): the lane-7 napper rides R6 = 10 behind its batch's lane-7 cart, the D(7)
                    // presser, so a plain +-K window around the napper masks the batch's OWN credit; K(7) masks only
                    // INSIDE the napper's window AND AFTER its batch's own D(7) credit (the latch `k7_after` set by that
                    // credit, cleared at the window's end): the napper's late D(7) can't credit or reset the next batch)
                    let masked7 = !def.k7_unmasked && k_windows.iter().zip(k_win_batch.iter()).any(|(&(a, b2, kl), &wb)| kl == 2 && t >= a && t <= b2 && credited_b.contains(&wb));
                    // (the D-press ring: D(7) arms J(-1) and disarms J(7): the wrap, safe by construction since D(7)
                    // fires after its cart has passed J(-1) and J(3))
                    if row.ring && !masked7 {
                        jm1_set.push((t + SWITCH, t + SWITCH + PRESS_W));
                        j7_rst.push((t + RETURN, t + RETURN + PRESS_W));
                    }
                    // (DONE gates EVERY credit in the DONE form: at most one credit per tap period by construction; in the
                    // STORE form every credit reads the store EMPTY (the hopper's read < 2 items and no credit in flight):
                    // the REL-keyed "owed" count IS the store, owed 2 = no stored credit)
                    let store_empty = !(read >= need || in_flight);
                    let owed_ok = !(read > need || in_flight);
                    // (ironspec 02:16 (3): THE ONE-CREDIT-PER-BATCH GUARD (`dm1_block7`): a D(-1) completion blocks D(7)'s credit
                    // for that many gt (a late slot 3 of the batch it just completed, its D(7) landing inside SEEN3's reset
                    // delay, is not credited twice; the next batch's own D(7) comes ~80 gt after the completion))
                    let blocked = row.dm1_block7 > 0 && last_completion.is_some_and(|lc| t - lc <= row.dm1_block7);
                    if blocked {
                        o.d7_blocked.push(t);
                    }
                    if !masked7 && seen3 && !blocked && !(row.dm1_done && comp) && !(row.dm1_store && !store_empty) && !(row.dm1_owed && !owed_ok) {
                        // (G1 02:34 (2): every D(7) credit's gap after the last D(-1) completion (the guard's window must end
                        // before every legitimate one: the least margin printed both ways))
                        if let Some(lc) = last_completion {
                            o.d7_gap_after_completion.push(t - lc);
                        }
                        // (THE GUARD GENERALIZED (day-1 03:06): ANY credit (D(7)'s or the completion's) opens the window, so
                        // a stalled lane-7 cart's D(7) landing inside SEEN3's reset delay after the NEXT batch's own D(7)
                        // credit is not credited twice either (the stalled-then-recovered cart, ironspec 02:10))
                        if !def.guard_completion_only {
                            last_completion = Some(t);
                        }
                        o.credit_press.push(t);
                        if p.napper {
                            o.napper_effects.push(format!("t {t}: the lane-7 napper's D(7) made a credit"));
                        }
                        let lat = if row.fast { FAST } else { AND };
                        dispense.push((t + lat + DROPPER, p.batch, seen3_by));
                        credited_b.insert(seen3_by);
                        // (the batch's own credit resets SEEN3_OLD (ironspec 22:52 (2): "reset by every credit") and sets DONE)
                        seen3_old = false;
                        done = true;
                        if def.double_dispense && p.batch == 5 {
                            dispense.push((t + lat + DROPPER + 1, p.batch, seen3_by));
                        }
                        if !armed_now {
                            wait_armed.push(t);
                        }
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
                    if !def.single_latch && !masked7 {
                        s3_rst.push((t + RESET, t + RESET + PRESS_W));
                    }
                }
            }
        }
        later.extend(emulated);
        presses = later;
    }
    o.taps = resets.clone();
    o.in_flight_at_end = dispense.iter().filter(|x| x.0 >= n as i64 - 1).count();
    o.fifo = row.dm1_owed;
    // (THE FIFO LEDGER, G1 on 60ada7ea8 B2: each REL opens a claim; each credit, in time order, answers the OLDEST open
    // claim; a credit with no claim open is a GAIN; a claim never answered is a REL that went UNANSWERED)
    {
        let mut ev: Vec<(i64, u8, usize)> = Vec::new();
        for (i, &r) in o.rels.iter().enumerate() {
            ev.push((r, 0, i));
        }
        for (i, &(t, _)) in o.credits.iter().enumerate() {
            ev.push((t, 1, i));
        }
        ev.sort();
        let mut open: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        let mut answered = vec![false; o.rels.len()];
        o.answers = vec![None; o.credits.len()];
        for (_, kind, i) in ev {
            if kind == 0 {
                open.push_back(i);
            } else if let Some(b) = open.pop_front() {
                answered[b] = true;
                o.answers[i] = Some(b);
            }
        }
        // (G1 CHANGES 4 C1: EVERY REL is judged; the last claim is exempt ONLY when its batch's completion is still pending
        // at the run's end (a slot of it neither in a lane nor missing / picked, and no credit sourced from it), printed)
        let pending = |b: usize| -> bool {
            let no_credit = !o.credit_src.iter().any(|c| c.0 == b);
            let unfinished = o.entered.get(b).is_some_and(|e| (0..3).any(|s| e[s].is_none() && !row.missing.contains(&(b, s)) && row.pick.iter().find(|p| p.0 == b).map(|p| p.1) != Some(s)));
            no_credit && unfinished
        };
        let nr = o.rels.len();
        let judged = if nr > 0 && pending(nr - 1) {
            o.ledger_exempt = Some((nr - 1, format!("REL {} (batch {}): its completion pending at the run's end (a slot not yet in a lane, no credit from it)", nr - 1, nr - 1)));
            nr - 1
        } else {
            nr
        };
        o.unanswered = (0..judged).filter(|&b| !answered[b]).collect();
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
/// constant, the dropper never dry; the credits tagged by source batch (G1 item 1): no batch credited twice, no credit
/// sourced by a stray's SEEN3.
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
    if o.transfers.iter().any(|&n| n > 1) {
        bad.push(format!("A REL MOVED TWO ITEMS (transfers per REL {:?})", o.transfers.iter().enumerate().filter(|x| *x.1 > 1).collect::<Vec<_>>()));
    }
    if !o.total_changes.is_empty() {
        bad.push(format!("the item total changed at {:?}", o.total_changes));
    }
    if !o.dry.is_empty() {
        bad.push(format!("credits fired dry at {:?}", o.dry));
    }
    bad
}

/// THE FIFO LEDGER's bars (G1 on 60ada7ea8 B2 / B3): every REL answered by exactly one credit (each credit answering the
/// oldest open claim), over ONE horizon (the pulses stop at the row's horizon, the run continues until every released
/// batch completes); a GAIN (a credit answering no claim) and an UNANSWERED REL are separate kinds, named.
/// THE STORE CORRECT (the FIFO ledger alone): every REL answered, no credit answering no claim, nothing in flight at the
/// end (single-fault rows, ironspec 02:54 (3)).
fn store_correct_bars(o: &Out) -> Vec<String> {
    let mut bad = Vec::new();
    let gains: Vec<i64> = o.answers.iter().zip(o.credits.iter()).filter(|(a, _)| a.is_none()).map(|(_, c)| c.0).collect();
    if !gains.is_empty() {
        bad.push(format!("A CREDIT GAINED (the store one over: credits at {gains:?} answer no open REL)"));
    }
    if !o.unanswered.is_empty() {
        bad.push(format!("A REL NEVER ANSWERED (the store one short: RELs {:?} of {})", o.unanswered, o.rels.len()));
    }
    if o.in_flight_at_end > 0 {
        bad.push(format!("A CREDIT STILL IN FLIGHT AT THE RUN's END ({} dispense(s) past the last tick)", o.in_flight_at_end));
    }
    bad
}

fn ledger_bars(o: &Out) -> Vec<String> {
    let mut bad = Vec::new();
    let gains: Vec<i64> = o.answers.iter().zip(o.credits.iter()).filter(|(a, _)| a.is_none()).map(|(_, c)| c.0).collect();
    if !gains.is_empty() {
        bad.push(format!("A CREDIT GAINED (the FIFO ledger: credits at {gains:?} answer no open REL)"));
    }
    if !o.unanswered.is_empty() {
        bad.push(format!("A REL NEVER ANSWERED (the FIFO ledger: RELs {:?} of {})", o.unanswered, o.rels.len()));
    }
    // (G1 CHANGES 4 C2: PER BATCH, beside the FIFO: every released batch's evidence makes EXACTLY ONE credit (the batch
    // whose D(7) or D(-1) completed it); a batch credited twice is a GAIN at its own source tick, a batch never credited a
    // LOSS; a gain and a loss never cancel; a credit from no batch (a stray) is a gain too)
    let nr = o.rels.len();
    let exempt = o.ledger_exempt.as_ref().map(|e| e.0);
    let mut twice: Vec<String> = Vec::new();
    let mut never: Vec<usize> = Vec::new();
    for b in 0..nr {
        let ticks: Vec<i64> = o.credit_src.iter().zip(o.credits.iter()).filter(|(s, _)| s.0 == b).map(|(_, c)| c.0).collect();
        if ticks.len() >= 2 {
            twice.push(format!("batch {b} at {ticks:?}"));
        } else if ticks.is_empty() && exempt != Some(b) {
            never.push(b);
        }
    }
    let strays: Vec<i64> = o.credit_src.iter().zip(o.credits.iter()).filter(|(s, _)| s.0 == usize::MAX).map(|(_, c)| c.0).collect();
    if !twice.is_empty() {
        bad.push(format!("A BATCH CREDITED TWICE (per batch: {})", twice.join("; ")));
    }
    if !strays.is_empty() {
        bad.push(format!("A CREDIT FROM NO BATCH (per batch: a stray's credit at {strays:?})"));
    }
    if !never.is_empty() {
        bad.push(format!("A BATCH NEVER CREDITED (per batch: batches {never:?} of {nr}: a credit lost)"));
    }
    if o.in_flight_at_end > 0 {
        bad.push(format!("A CREDIT STILL IN FLIGHT AT THE RUN's END ({} dispense(s) past the last tick)", o.in_flight_at_end));
    }
    bad
}

/// Consecutive values as ranges ("9..31, 49..60"; "none" when empty).
fn ranges_of(xs: &[i64]) -> String {
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
}

/// The credit's cause fix and its guard (ironspec 02:16 (3) / G1 02:17 (2) / 02:34 (2)): SEEN3_OLD's own tap judged under a
/// world open at every tick (the window's edges printed), the one-credit-per-batch guard's margin to every legitimate D(7)
/// credit, and the five gain rows without the guard (the store's level at both credits).
fn credit_fix_block(v: &mut Verdict, lines: &mut Vec<String>) {
    let mut rows: Vec<(String, Row)> = Vec::new();
    for (s, lates) in [(0usize, vec![9i64, 12, 15, 18, 21, 24, 27, 31, 49, 53, 57, 60]), (1, vec![29, 32, 35, 38, 40, 57, 60]), (2, vec![21, 23, 25, 27])] {
        for late in lates {
            let mut r = Row::normal("tap");
            r.late = vec![(5, s, late)];
            rows.push((format!("lane {} late {late}", LANES[s]), r));
        }
    }
    for s in 0..3 {
        let mut r = Row::normal("tap");
        r.missing = vec![(5, s)];
        rows.push((format!("lane {} missing", LANES[s]), r));
    }
    rows.push(("the normal run".to_string(), Row::normal("tap")));
    // (the timing rule: a world open (the block domain frozen 2 gt, the carts 3 behind) at every tick of REL_6 + 0..114)
    let sweep = |tap: i64, gaps: &mut Vec<i64>| -> (usize, usize, Vec<String>) {
        let (mut n, mut bad) = (0usize, 0usize);
        let mut ex = Vec::new();
        for (name, r0) in &rows {
            for t0 in 606..=720 {
                let mut r = r0.clone();
                r.s3old_tap = Some(tap);
                r.open = Some((t0, 2, 3));
                let o = run(&r, Defects::default());
                n += 1;
                gaps.extend(o.d7_gap_after_completion.iter().copied());
                let lb = ledger_bars(&o);
                if !lb.is_empty() {
                    bad += 1;
                    if ex.len() < 2 {
                        ex.push(format!("{name}, open at REL_6 + {}: {}", t0 - 606, lb.join("; ")));
                    }
                }
            }
        }
        (n, bad, ex)
    };
    let mut gaps: Vec<i64> = Vec::new();
    let (n, bad, ex) = sweep(S3OLD_TAP, &mut gaps);
    let mut edges = Vec::new();
    for tap in [S3OLD_TAP - 4, S3OLD_TAP - 3, S3OLD_TAP + 3, S3OLD_TAP + 4] {
        let (_, b, _) = sweep(tap, &mut Vec::new());
        edges.push(format!("+ {tap}: {b} runs changing the store"));
    }
    let w = v.bar_n(&format!("ctl-credit: SEEN3_OLD's OWN TAP at REL + {S3OLD_TAP} (the credit's cause fix, ironspec 02:16 (3)) keeps the store exact (per batch, the FIFO ledger) on every formerly-losing late row, the missing rows, the gain rows and the normal run under a world open (idle 2, the carts 3 behind) at every tick of REL_6 + 0..114 (the timing rule)"), n, bad == 0);
    lines.push(format!("  THE CREDIT's CAUSE FIX, SEEN3_OLD's tap REL + {S3OLD_TAP}, under a world open at every tick: {n} runs, {bad} changing the store{}; the window's edges: {}: {w}", if ex.is_empty() { String::new() } else { format!(" ({})", ex.join(" | ")) }, edges.join(", ")));
    // (G1 02:34 (2): THE GUARD's MARGIN both ways: every D(7) credit after a completion lands after the window's end;
    // the lateness sweep 0..60 on every slot, the napper batches and the world-open rows above)
    for s in 0..3 {
        for late in 0..=60 {
            let mut r = Row::normal("guard");
            r.late = vec![(5, s, late)];
            gaps.extend(run(&r, Defects::default()).d7_gap_after_completion.iter().copied());
        }
    }
    for lane in [0usize, 1, 2] {
        for late in 0..=30 {
            let mut r = Row::normal("guard napper");
            r.late = vec![(5, 2, late)];
            r.napper = vec![(5, lane, late)];
            gaps.extend(run(&r, Defects::default()).d7_gap_after_completion.iter().copied());
        }
    }
    let least = gaps.iter().copied().min();
    let w = v.bar(&format!("ctl-credit: THE ONE-CREDIT-PER-BATCH GUARD ({DM1_BLOCK7} gt) ends before every legitimate D(7) credit after a completion (G1 02:34 (2): a mask on a credit path; the late sweep 0..60 on every slot, the napper batches, the world-open rows)"), least.is_none_or(|g| g > DM1_BLOCK7));
    lines.push(format!("  THE GUARD's MARGIN: {} D(7) credits after a completion, the least gap {:?} gt (the window {DM1_BLOCK7}: margin {:?}; the duplicate it blocks lands 1..7 gt after): {w}", gaps.len(), least, least.map(|g| g - DM1_BLOCK7)));
    // (the five gain rows WITHOUT the guard: the store's level at both credits (a cap refuses only at full, 3 items);
    // the control: the guard removed, the rows gain by name)
    let mut levels = Vec::new();
    let mut gained = 0usize;
    for late in 21..=25 {
        let mut r = Row::normal("no guard");
        r.late = vec![(5, 2, late)];
        r.dm1_block7 = 0;
        let o = run(&r, Defects::default());
        if ledger_bars(&o).iter().any(|b| b.contains("A BATCH CREDITED TWICE")) {
            gained += 1;
        }
        for b in 0..o.rels.len() {
            let idx: Vec<usize> = o.credit_src.iter().enumerate().filter(|(_, s)| s.0 == b).map(|(i, _)| i).collect();
            if idx.len() >= 2 {
                levels.push(format!("late {late}: batch {b} credited at {:?} with the store at {:?} items", idx.iter().map(|&i| o.credits[i].0).collect::<Vec<_>>(), idx.iter().map(|&i| o.credit_level.get(i).copied().unwrap_or(-1)).collect::<Vec<_>>()));
            }
        }
    }
    let c = v.control("ctl-credit CONTROL: THE ONE-CREDIT-PER-BATCH GUARD removed: the lane 7 cart 21..25 gt late is credited twice (A BATCH CREDITED TWICE by name on all 5 rows), the store below full at both credits (no cap catches it)", 1, gained == 5).to_string();
    lines.push(format!("  CONTROL (the guard removed): {gained} of 5 rows credited twice: {}: {c}", levels.join(" | ")));
    // (THE RESYNC NET (ironspec 02:40 (4) / 02:46 (1), G1 02:44b (1)): THE PREMISE printed (the batches in flight at each REL
    // wait's start: the healthy pipeline waits with TWO, the one-short store with ONE); the trigger (REL waiting for a credit
    // with ONE batch in flight on TWO CONSECUTIVE RELs, no halt; a latch cleared by any REL on its clock, a halt or a world
    // open; no timer, no count) FIRES in the true-lost-credit rows and restores the store within a printed bound, and NEVER
    // fires in the false-fire rows (normal, every late row, the lost carts, the stalled-then-recovered cart, halts of
    // 500 / 2,000 / 6,000 gt, a world open at every tick); the control: the net off, the true-lost rows stay one short)
    {
        let lost_rows: Vec<(String, Row, Defects)> = vec![
            ("residual (ii)'s twin (K(3) without the J(3) reset)".to_string(), { let mut r = Row::normal("net"); r.late = vec![(5, 2, 25)]; r.napper = vec![(5, 1, 25)]; r }, Defects { k3_j3_unmasked: true, ..Default::default() }),
            ("a lane 7 napper released 50 gt late".to_string(), { let mut r = Row::normal("net"); r.napper = vec![(5, 2, 50)]; r }, Defects::default()),
            ("a lane 7 napper released 60 gt late".to_string(), { let mut r = Row::normal("net"); r.napper = vec![(5, 2, 60)]; r }, Defects::default()),
        ];
        let mut fire_lines = Vec::new();
        let mut fire_bad = Vec::new();
        let mut off_short = 0usize;
        for (name, r, d) in &lost_rows {
            let o = run(r, *d);
            let prem: Vec<String> = o.wait_inflight.iter().take(4).map(|(t, n)| format!("t{t}:{n}")).collect();
            fire_lines.push(format!("{name}: waits (tick: batches in flight) [{}..], the net fires at {:?}, the store [{}]", prem.join(" "), o.net_fires, store_correct_bars(&o).join("; ")));
            if o.net_fires.len() != 1 || !store_correct_bars(&o).is_empty() {
                fire_bad.push(name.clone());
            }
            let mut r0 = r.clone();
            r0.resync_net = false;
            if !store_correct_bars(&run(&r0, *d)).is_empty() {
                off_short += 1;
            }
        }
        let w = v.bar_n("ctl-credit: THE RESYNC NET FIRES ONCE in each true-lost-credit row and the store ends correct (the FIFO ledger)", lost_rows.len(), fire_bad.is_empty());
        lines.push(format!("  THE RESYNC NET, the true-lost rows: {}: {w}", fire_lines.join(" | ")));
        let c = v.control("ctl-credit CONTROL: the resync net off: every true-lost-credit row ends with the store one short (A REL NEVER ANSWERED)", 1, off_short == lost_rows.len()).to_string();
        lines.push(format!("  CONTROL (the net off): {off_short} of {} rows one short: {c}", lost_rows.len()));
        let (mut n, mut fires) = (0usize, Vec::new());
        let mut false_row = |name: String, r: Row| {
            let o = run(&r, Defects::default());
            n += 1;
            if !o.net_fires.is_empty() {
                fires.push(format!("{name} at {:?}", o.net_fires));
            }
        };
        false_row("the normal run".into(), Row::normal("net"));
        for s in 0..3 {
            for late in 0..=60 {
                let mut r = Row::normal("net");
                r.late = vec![(5, s, late)];
                false_row(format!("lane {} late {late}", LANES[s]), r);
            }
            let mut r = Row::normal("net");
            r.missing = vec![(5, s)];
            false_row(format!("lane {} lost", LANES[s]), r);
        }
        for late in [100i64, 200, 300, 600] {
            let mut r = Row::normal("net");
            r.late = vec![(5, 2, late)];
            false_row(format!("a stalled-then-recovered lane 7 cart ({late} late)"), r);
        }
        for d in [500i64, 2000, 6000] {
            let mut r = Row::normal("net");
            r.holds_empty = vec![(700, 700 + d)];
            r.t_max = 3000 + d;
            false_row(format!("a halt of {d} gt"), r);
        }
        for t0 in 500..720 {
            let mut r = Row::normal("net");
            r.open = Some((t0, 0, 3));
            false_row(format!("a world open at t {t0}"), r);
        }
        let w = v.bar_n("ctl-credit: THE RESYNC NET NEVER FIRES FALSELY (the normal run, every late row 0..60 on every slot, the lost carts, the stalled-then-recovered cart 100..600 late, halts of 500 / 2,000 / 6,000 gt, a world open at every tick of batch 5's span)", n, fires.is_empty());
        lines.push(format!("  THE RESYNC NET's FALSE-FIRE ROWS: {n} rows, fires {}: {w}", if fires.is_empty() { "none".to_string() } else { fires.join(" | ") }));
    }
    // (ironspec 03:04 (2) / G1 03:04 (5)(i): K(7) BY ORDER's premise and its holes: the order premise (the batch's lane-7
    // cart reaches D(7) before its lane-7 napper: SLOT_T[2] vs NAPPER_T[2], printed); A PLANTED REORDER (the batch's lane-7
    // cart 11..30 gt late, the napper on time: the napper reaches D(7) first) and the batch's lane-7 cart LOST: the mask
    // never arms and the napper's D(7) acts as real; printed per row: credits per batch, the lanes, the leg; judged on the
    // fault path's bars (the store correct, the resync net recovering)
    {
        lines.push(format!("  K(7) BY ORDER, THE PREMISE: the batch's lane-7 cart reaches J(7) at REL + {} and D(7) {J_TO_D} gt later; its lane-7 napper (riding behind it) at REL + {} (R6 = {} gt behind): the order holds unless the cart is delayed past the napper", SLOT_T[2][2], NAPPER_T[2], NAPPER_T[2] - SLOT_T[2][2]));
        let mut bad = Vec::new();
        let mut rows_out = Vec::new();
        for (name, r) in [
            ("the lane-7 cart 15 late, its napper on time (a reorder at D(7))".to_string(), { let mut r = Row::normal("k7o"); r.late = vec![(5, 2, 15)]; r.napper = vec![(5, 2, 0)]; r }),
            ("the lane-7 cart 30 late, its napper on time".to_string(), { let mut r = Row::normal("k7o"); r.late = vec![(5, 2, 30)]; r.napper = vec![(5, 2, 0)]; r }),
            ("the lane-7 cart LOST, its napper on time".to_string(), { let mut r = Row::normal("k7o"); r.missing = vec![(5, 2)]; r.napper = vec![(5, 2, 0)]; r }),
        ] {
            let o = run(&r, Defects::default());
            let mut rb = recovery_bars(&o, &r, 5);
            if !o.leg_entries.is_empty() {
                rb.push(format!("A BATCH CART ENTERED THE WAY-IN LEG: {}", o.leg_entries.join("; ")));
            }
            rows_out.push(format!("{name}: batch 5 entered {:?}, napper effects {:?}, per batch [{}], the net {:?}, the fault bars [{}]", o.entered.get(5), o.napper_effects, ledger_bars(&o).join("; "), o.net_fires, rb.join("; ")));
            if !rb.is_empty() {
                bad.push(name);
            }
        }
        let w = if bad.is_empty() {
            v.bar_n("ctl-credit: K(7) BY ORDER's HOLES (a reorder at D(7), the lane-7 cart lost): the fault path's adopted bars (the store correct with the net, no batch cart into the leg)", 3, true).to_string()
        } else {
            v.bar(&format!("ctl-credit: K(7) BY ORDER's HOLES: the fault path's adopted bars fail in: {}", bad.join("; ")), false).to_string()
        };
        lines.push(format!("  K(7) BY ORDER's HOLES: {}: {w}", rows_out.join(" | ")));
    }
    // (residual (ii)'s cause fix, day-1 02:40: K(3)'s mask covers ALL of D(3)'s outputs (the J(3) reset too); the control:
    // the twin without it (the late lane-3 napper's D(3) turns batch 6's freshly armed J(3) off: batch 6's lane 3 cart
    // into lane 7, batch 6's credit lost by name); the design's row judged clean)
    {
        let mut r = Row::normal("a lane 3 napper behind batch 5's slot 3, both 25 gt late");
        r.late = vec![(5, 2, 25)];
        r.napper = vec![(5, 1, 25)];
        let od = run(&r, Defects::default());
        let ot = run(&r, Defects { k3_j3_unmasked: true, ..Default::default() });
        let bd = ledger_bars(&od);
        let bt = ledger_bars(&ot);
        let w = v.bar("ctl-credit: K(3)'s mask covers ALL of D(3)'s outputs (day-1 02:40): a lane 3 napper behind batch 5's slot 3, both 25 gt late, leaves the store exact and no cart misrouted", bd.is_empty() && od.misroutes.is_empty() && od.lane_delta == [0; 3]);
        lines.push(format!("  K(3) OVER ALL OF D(3)'s OUTPUTS: the late lane 3 napper's row: misroutes {:?}, the lanes' counts {:?}, the ledger [{}]: {w}", od.misroutes, od.lane_delta, bd.join("; ")));
        let c = v.control("ctl-credit CONTROL: K(3)'s mask NOT covering D(3)'s J(3) reset: the late lane 3 napper's D(3) turns batch 6's armed J(3) off: batch 6's lane 3 cart into lane 7 and A BATCH NEVER CREDITED by name", 1, bt.iter().any(|b| b.contains("A BATCH NEVER CREDITED")) && !ot.misroutes.is_empty()).to_string();
        lines.push(format!("  CONTROL (K(3) without the J(3) reset): misroutes {:?}, the lanes' counts {:?}, the ledger [{}]: {c}", ot.misroutes, ot.lane_delta, bt.join("; ")));
    }
    // (ironspec 02:43 (1) / 02:46 (2), G1 02:44b (2): THE K MASKS AS A STATIC MATRIX: each D(l)'s outputs against K(l)'s
    // gated lines for all three lanes, as the press handlers implement them; every entry has a planted control leaving
    // that output unmasked (the per-output twins), so a missing gate fails by name)
    lines.push("  THE K MASKS' MATRIX (each K(l) masks ALL of D(l)'s outputs; the twin leaving one output unmasked, with its control):".to_string());
    for (l, out, gate, twin) in [
        (-1, "J(-1)'s reset", "K(-1)", "no_km1_jm1 (control: the napper 20 late turns batch 6's armed J(-1) off)"),
        (-1, "J(3)'s arm", "K(-1)", "no_km1_j3 (control: the napper's D(-1) arms J(3))"),
        (-1, "the D(-1) completion's credit input", "K(-1)", "no_km1_credit (control: the napper's D(-1) inside SEEN3 makes a credit)"),
        (3, "J(3)'s reset", "K(3) (day-1 02:40)", "k3_j3_unmasked (control: the late lane 3 napper turns batch 6's J(3) off, batch 6's credit lost)"),
        (3, "SEEN3's set", "K(3)", "no_k3_mask (control: the napper's D(3) sets SEEN3)"),
        (3, "J(7)'s arm (the ring form only)", "K(3)", "no_k3_mask (the ring rows)"),
        (7, "the D(7) credit line", "K(7) (ironspec 02:43 (1))", "k7_unmasked (control below: a late lane 7 napper credits the next batch early)"),
        (7, "SEEN3's reset", "K(7)", "k7_unmasked (control below)"),
        (7, "J(-1)'s arm and J(7)'s reset (the ring form only)", "K(7)", "k7_unmasked (the ring rows)"),
    ] {
        lines.push(format!("    D({l}) -> {out}: gated by {gate}; twin {twin}"));
    }
    {
        // (K(7)'s rows: a lane 7 napper alone (its batch on time), released 0..120 gt late. THE MATRIX's bar: its D(7) changes
        // no state at any lateness (no napper effect: no credit, no SEEN3 reset); the twin without K(7) credits the next
        // batch by name. Separately, a napper released 30..60 gt late meets batch 6 at the J's (its OFF windows and K(7)'s
        // branch force), a ROUTING fault judged on the fault path's bars (`recovery_bars`, the store per batch))
        let (mut eff_d, mut eff_t, mut route_bad) = (Vec::new(), Vec::new(), Vec::new());
        let mut route_ex = String::new();
        for extra in (0..=120).step_by(5) {
            let mut r = Row::normal("k7");
            r.napper = vec![(5, 2, extra)];
            let od = run(&r, Defects::default());
            if !od.napper_effects.is_empty() {
                eff_d.push(extra);
            }
            let rb = recovery_bars(&od, &r, 6);
            if !rb.is_empty() {
                route_bad.push(extra);
                if route_ex.is_empty() {
                    route_ex = format!("released {extra} late: misroutes {:?}; {}", od.misroutes, rb.join("; "));
                }
            }
            if !run(&r, Defects { k7_unmasked: true, ..Default::default() }).napper_effects.is_empty() {
                eff_t.push(extra);
            }
        }
        let w = v.bar_n("ctl-credit: K(7) MASKS ALL OF D(7)'s OUTPUTS: a lane 7 napper's D(7) at every release lateness 0..120 changes no state (no credit, no SEEN3 reset)", 25, eff_d.is_empty());
        lines.push(format!("  K(7)'s MATRIX ROW, a lane 7 napper released 0..120 gt late: its D(7) changes state at {}: {w}", ranges_of(&eff_d)));
        let c = v.control("ctl-credit CONTROL: K(7) masking none of D(7)'s outputs: a late lane 7 napper's D(7) credits the next batch by name (its D(7) inside that batch's SEEN3)", 1, !eff_t.is_empty()).to_string();
        lines.push(format!("  CONTROL (no K(7)): the napper's D(7) changes state at lateness {}: {c}", ranges_of(&eff_t)));
        let w = if route_bad.is_empty() {
            v.bar_n("ctl-credit: A LANE 7 NAPPER RELEASED 0..120 gt LATE: the fault path's adopted bars (recovery within 2 batches, the store per batch)", 25, true).to_string()
        } else {
            v.bar(&format!("ctl-credit: A LANE 7 NAPPER RELEASED 0..120 gt LATE: the fault path's adopted bars fail at release lateness {} (its J windows meet batch 6's carts)", ranges_of(&route_bad)), false).to_string()
        };
        lines.push(format!("  A LATE LANE 7 NAPPER's ROUTING (its OFF windows / K(7)'s branch force meeting batch 6): fails at {}{}: {w}", ranges_of(&route_bad), if route_ex.is_empty() { String::new() } else { format!(" (e.g. {route_ex})") }));
    }
    // (ironspec 02:43 (1)'s condition: the margin from each napper's press to its own K window's edges, worst over the
    // delayed-napper sweep (a lane l napper behind its batch's late slot 3, both 0..60 late, every lane) and a world open
    // (block domain running, the carts 3 behind) at every tick of the napper batch; a press OUTSIDE its window fails by
    // name. MODEL NOTE: a napper bunched behind a late cart is modelled as released with it (its K window riding its
    // release); a napper slowed AFTER its release by a cart ahead is not modelled (no cart collisions): owed to the game)
    {
        let mut ms: Vec<(i32, i64, String)> = Vec::new();
        for lane in [0usize, 1, 2] {
            for late in 0..=60 {
                let mut r = Row::normal("k margin");
                r.late = vec![(5, 2, late)];
                r.napper = vec![(5, lane, late)];
                for &(l, m) in &run(&r, Defects::default()).napper_k_margin {
                    ms.push((l, m, format!("lane {} napper behind slot 3, both {late} late", LANES[lane])));
                }
            }
            for t0 in 506..=720 {
                let mut r = Row::normal("k margin open");
                r.napper = vec![(5, lane, 0)];
                r.open = Some((t0, 0, 3));
                for &(l, m) in &run(&r, Defects::default()).napper_k_margin {
                    ms.push((l, m, format!("lane {} napper, a world open at t {t0}", LANES[lane])));
                }
            }
        }
        let least = ms.iter().min_by_key(|x| x.1).cloned();
        let outside: Vec<&(i32, i64, String)> = ms.iter().filter(|x| x.1 < 0).collect();
        let w = v.bar_n("ctl-credit: EVERY NAPPER PRESS INSIDE ITS OWN K WINDOW (ironspec 02:43 (1)'s condition: the delayed-napper sweep 0..60 on every lane and a world open at every tick of the napper batch)", ms.len(), outside.is_empty());
        lines.push(format!("  THE NAPPER PRESSES vs THEIR K WINDOWS (+- {K_HALF}): {} presses, the least margin {:?}{}: {w}", ms.len(), least.map(|x| format!("{} gt (lane {}, {})", x.1, x.0, x.2)), if outside.is_empty() { String::new() } else { format!("; OUTSIDE: {}", outside.iter().take(3).map(|x| x.2.clone()).collect::<Vec<_>>().join(" | ")) }));
    }
}

/// The design's bars on an unfaulted run: the store, P = 100, every batch in phase, no misroute, no picked cart
/// diverted, every napper press neutral.
fn design_bars(o: &Out, row: &Row, n_min: usize) -> Vec<String> {
    let mut bad = store_bars(o);
    bad.extend(ledger_bars(o));
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
    if !o.napper_misroutes.is_empty() {
        bad.push(format!("NAPPER MISROUTED: {}", o.napper_misroutes.join("; ")));
    }
    if o.lane_delta != [0; 3] {
        bad.push(format!("the lanes' cart counts changed: {:?}", o.lane_delta));
    }
    if !o.ghost_rels.is_empty() {
        bad.push(format!("A SECOND REL EDGE in a batch at {:?}", o.ghost_rels));
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
    // (ironspec 02:54 (3): on a single-fault row the REL slips print as COST, not a bar (the resync net's restoration
    // costs one or two long periods); the cadence is judged on the normal nights (`design_bars`))
    let _cadence_cost = o.rels.len() > from + 1 && o.rels[from..].windows(2).any(|w| w[1] - w[0] != PERIOD);
    let late_mis: Vec<&String> = o.misroutes.iter().filter(|m| m.split_whitespace().nth(1).and_then(|x| x.trim_end_matches("'s").parse::<usize>().ok()).is_some_and(|b| b >= from)).collect();
    if !late_mis.is_empty() {
        bad.push(format!("misroutes after the recovery: {late_mis:?}"));
    }
    if !o.napper_misroutes.is_empty() {
        bad.push(format!("NAPPER MISROUTED: {}", o.napper_misroutes.join("; ")));
    }
    if !o.ghost_rels.is_empty() {
        bad.push(format!("A SECOND REL EDGE in a batch at {:?}", o.ghost_rels));
    }
    // (ironspec 02:54 (3): a single-fault row is judged on THE STORE CORRECT within the printed bound (the FIFO ledger with
    // the resync net: every REL answered, no credit beyond the claims, nothing in flight at the end); the per-batch view is
    // printed as cost; the per-batch bars judge the normal nights (`design_bars`))
    bad.extend(store_correct_bars(o));
    if !o.piled.is_empty() {
        bad.push(format!("carts piled behind a jam: {}", o.piled.len()));
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

/// The failure KINDS a row's bars name (G1 on 60ada7ea8 B3: every multi-row bar pinned per (row, kind); gains apart).
fn kinds_of(bad: &[String]) -> String {
    let mut ks: Vec<&str> = bad
        .iter()
        .map(|b| {
            if b.contains("A REL NEVER ANSWERED") {
                "UNANSWERED"
            } else if b.contains("A BATCH NEVER CREDITED") {
                "LOST"
            } else if b.contains("A CREDIT GAINED") || b.contains("A BATCH CREDITED TWICE") || b.contains("A CREDIT FROM NO BATCH") {
                "GAIN"
            } else if b.contains("STILL IN FLIGHT") {
                "IN-FLIGHT"
            } else if b.contains("THE RING DOES NOT RETURN") || b.contains("out of phase") {
                "PHASE"
            } else if b.contains("cadence") || b.contains("periods") {
                "CADENCE"
            } else if b.contains("NAPPER MISROUTED") {
                "NAPPER-MISROUTE"
            } else if b.contains("misroute") {
                "MISROUTE"
            } else if b.contains("A SECOND REL EDGE") {
                "SECOND-REL"
            } else if b.contains("piled") {
                "PILED"
            } else if b.contains("A REL MOVED TWO ITEMS") {
                "TWO-ITEMS"
            } else if b.contains("hopper reached") || b.contains("fewer than 2 items") || b.contains("moved no item") || b.contains("item total") || b.contains("dry") {
                "STORE"
            } else if b.contains("only") {
                "SHORT"
            } else {
                "OTHER"
            }
        })
        .collect();
    ks.sort();
    ks.dedup();
    ks.join(", ")
}

fn line(v: &mut Verdict, name: &str, bad: &[String], o: &Out, row: &Row, lines: &mut Vec<String>) {
    let w = v.bar(&format!("ctl-credit: {name}{}", if bad.is_empty() { String::new() } else { format!(" [kinds: {}]", kinds_of(bad)) }), bad.is_empty());
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
    // (G1 on fff3c7045: K(-1)'s mask's least edge gap to a batch D(-1) / D(3), on the napper + 2 timing, judged >= 10
    // after k + j (the world open's entity lag, 3))
    let w = v.bar("ctl-credit: the K masks' least edge gap to a batch D(-1) / D(3) on the napper + 2 timing >= 10 + k + j (3)", o.mask_margin.is_some_and(|m| m >= 10 + 3));
    lines.push(format!("    the K masks' least edge gap {:?} (>= 13): {w}", o.mask_margin));
    // (G1 on 60ada7ea8: ONE batch's credit dispensed twice (batch 5): the ledger names the GAIN)
    let oc = run(&row, Defects { double_dispense: true, ..Default::default() });
    control(v, "batch 5's credit dispensed twice: A CREDIT GAINED fails by name in the FIFO ledger", ledger_bars(&oc).iter().any(|b| b.contains("A CREDIT GAINED")), &oc, &row, &mut lines);
    // (G1 CHANGES 4 C1's controls: batch 5's credit DROPPED must fail "A REL NEVER ANSWERED" by name; the LAST released
    // batch's credit dropped too (no last-REL exclusion hides it); the normal row answers every REL (judged above))
    let od5 = run(&row, Defects { drop_credit: Some(5), ..Default::default() });
    control(v, "batch 5's credit DROPPED: A REL NEVER ANSWERED and A BATCH NEVER CREDITED fail by name", ledger_bars(&od5).iter().any(|b| b.contains("A REL NEVER ANSWERED")) && ledger_bars(&od5).iter().any(|b| b.contains("A BATCH NEVER CREDITED")), &od5, &row, &mut lines);
    let last_b = o.rels.len().saturating_sub(1);
    let odl = run(&row, Defects { drop_credit: Some(last_b), ..Default::default() });
    control(v, &format!("the LAST released batch's credit (batch {last_b}) DROPPED: A BATCH NEVER CREDITED fails by name (no last-claim exclusion hides it)"), ledger_bars(&odl).iter().any(|b| b.contains("A BATCH NEVER CREDITED")), &odl, &row, &mut lines);
    // (G1 CHANGES 4 C2's control: ONE GAIN AND ONE LOSS in the same row (batch 5 credited twice, batch 9's credit
    // dropped): both fail by name, they never cancel)
    credit_fix_block(v, &mut lines);
    let ogl = run(&row, Defects { double_dispense: true, drop_credit: Some(9), ..Default::default() });
    let bgl = ledger_bars(&ogl);
    control(v, "one GAIN and one LOSS in one row (batch 5 credited twice, batch 9's credit dropped): A BATCH CREDITED TWICE and A BATCH NEVER CREDITED both fail by name (they never cancel)", bgl.iter().any(|b| b.contains("A BATCH CREDITED TWICE")) && bgl.iter().any(|b| b.contains("A BATCH NEVER CREDITED")), &ogl, &row, &mut lines);
    lines.push(format!("  THE LEDGER on the normal row: {} RELs, {} credits, the last claim {}", o.rels.len(), o.credits.len(), o.ledger_exempt.as_ref().map_or("judged".to_string(), |e| format!("EXEMPT: {}", e.1))));
    // (G1 on 60ada7ea8 B2's control: a stray D(7) inside SEEN3's reset window (batch 5's own D(7) press + 2): its second
    // credit answers no claim: A CREDIT GAINED by name. With THE GUARD GENERALIZED (every credit opens it, day-1 03:06) the
    // design blocks that stray's credit (judged); the control runs on the twin whose guard only a completion opens.
    // G1 CHANGES 6 non-blocking: no silent `if let` (a missing D(7) fails by name))
    {
        let d7 = o.presses.get(5).and_then(|ps| ps.iter().find(|p| p.1 == 7).map(|p| p.0));
        let w = v.bar("ctl-credit: batch 5's D(7) press found on the normal row (the stray-inside-SEEN3's-reset rows key on it)", d7.is_some());
        lines.push(format!("  batch 5's D(7) on the normal row: {d7:?}: {w}"));
        if let Some(t7) = d7 {
            let mut rs = row.clone();
            rs.stray = vec![(t7 + 2, 7)];
            let od = run(&rs, Defects::default());
            let w = v.bar(&format!("ctl-credit: a stray D(7) at t {} inside SEEN3's reset window (batch 5's D(7) + 2) makes no credit (THE GUARD GENERALIZED: every credit opens it)", t7 + 2), store_correct_bars(&od).is_empty() && od.d7_blocked.contains(&(t7 + 2)));
            lines.push(format!("  A STRAY D(7) INSIDE SEEN3's RESET WINDOW (t {}): blocked {:?}, the store [{}]: {w}", t7 + 2, od.d7_blocked, store_correct_bars(&od).join("; ")));
            let os = run(&rs, Defects { guard_completion_only: true, ..Default::default() });
            control(v, &format!("a stray D(7) at t {} inside SEEN3's reset window (batch 5's D(7) + 2), the guard opened by the completion alone: A CREDIT GAINED fails by name in the FIFO ledger", t7 + 2), ledger_bars(&os).iter().any(|b| b.contains("A CREDIT GAINED")), &os, &rs, &mut lines);
        }
    }
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
    // (G1 item 8: the latency MEASURED from `armed`, on a one-credit store (power-up 2 items: after the first REL every
    // D(7) press finds RE_ARMED false), the fast path and its slow twin through `run`)
    {
        let mut r2 = Row::normal("p2 measured");
        r2.hopper0 = 2;
        let of = run(&r2, Defects::default());
        let mut r2s = r2.clone();
        r2s.fast = false;
        let os2 = run(&r2s, Defects::default());
        let lf = of.armed_latency.iter().max().copied();
        let ls = os2.armed_latency.iter().max().copied();
        let w = v.bar("ctl-credit: the fast path's latency MEASURED from `armed` (a press finding RE_ARMED false) leaves >= 5 gt before the measured strings-ready", lf.is_some_and(|l| STRINGS_READY - (D7_MEASURED + l) >= 5));
        lines.push(format!("  the fast path's latency measured from `armed`: {} presses, latency {:?}..{:?} gt (margin {:?}): {w}", of.armed_latency.len(), of.armed_latency.iter().min(), lf, lf.map(|l| STRINGS_READY - (D7_MEASURED + l))));
        let c = v.control("ctl-credit CONTROL (G1 item 8): the slow path through `run` (the edge comparator + the AND into the dropper, read from the hopper): its measured latency leaves a negative margin", 1, ls.is_some_and(|l| STRINGS_READY - (D7_MEASURED + l) < 0)).to_string();
        lines.push(format!("  CONTROL (the slow path measured: latency {:?}..{ls:?}, margin {:?}): {c}", os2.armed_latency.iter().min(), ls.map(|l| STRINGS_READY - (D7_MEASURED + l))));
    }
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
        fault(&format!("batch 5's lane {} cart missing (TIER 2: lost after release; decided by the jam rows, ironspec 23:06)", LANES[s]), &r, &mut lines);
    }
    let mut readback = Vec::new();
    for lane in [-1, 3, 7] {
        // (G1 item 7: each injection read back as it landed, the stray D(3) moved into a SEEN3-clear span (697..750:
        // batch 5's SEEN3 reset at 697, batch 6's D(3) at 746), so it changes a latch)
        let at = if lane == 3 { 720 } else { 560 };
        let mut r = Row::normal("stray");
        r.stray = vec![(at, lane)];
        let o = fault(&format!("a stray D({lane}) press at t {at}"), &r, &mut lines);
        readback.push((lane, at, o.strays_landed.iter().find(|s| s.0 == at && s.1 == lane).copied()));
    }
    for (lane, at, landed) in readback {
        let w = v.bar(&format!("ctl-credit: the stray D({lane}) injection at t {at} landed (read back){}", if lane == 3 { ", in a SEEN3-clear span" } else { "" }), landed.is_some_and(|s| lane != 3 || !s.2));
        lines.push(format!("    the stray D({lane}) read back: {landed:?} (tick, lane, SEEN3 at the press): {w}"));
    }
    // (the tolerances, per slot, lateness 0..60 at 1 gt: where a cart moves between sidings (the compression misroute:
    // a cart reaching a J inside the switching time of the cart ahead, or slot 3 reaching J(-1) after the next ring
    // reset armed it), and where the ruled form (the credit on D(7) alone) loses a credit for good)
    let ranges = |xs: &[i64]| -> String { ranges_of(xs) };
    for s in 0..3 {
        let (mut moved, mut lost, mut lost_c, mut failing) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut by_kind: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
        let mut gain_levels: Vec<String> = Vec::new();
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
            let rb = recovery_bars(&o, &r, bf);
            if !rb.is_empty() {
                failing.push(late);
                for k in kinds_of(&rb).split(", ") {
                    by_kind.entry(k.to_string()).or_default().push(late);
                }
            }
            // (ironspec 02:16 (3): on every row with a batch credited twice, the store's level at both of its credits
            // (the cap refuses only at full: 3 items, the ballast + 2))
            for b in 0..o.rels.len() {
                let idx: Vec<usize> = o.credit_src.iter().enumerate().filter(|(_, s)| s.0 == b).map(|(i, _)| i).collect();
                if idx.len() >= 2 {
                    gain_levels.push(format!("late {late}: batch {b}'s credits at {:?} with the store at {:?} items", idx.iter().map(|&i| o.credits[i].0).collect::<Vec<_>>(), idx.iter().map(|&i| o.credit_level.get(i).copied().unwrap_or(-1)).collect::<Vec<_>>()));
                }
            }
        }
        if !gain_levels.is_empty() {
            lines.push(format!("  THE GAIN ROWS' STORE LEVELS (lane {}; ironspec 02:16 (3): a cap refuses only a credit landing on a FULL store, 3 items): {}", LANES[s], gain_levels.join(" | ")));
            gain_levels.clear();
        }
        // (G1 item 6 / on 60ada7ea8 B3: every one of the 61 lateness values JUDGED on the recovery bars, PINNED PER KIND:
        // one bar per failing kind naming its lateness values; the moved cart is the documented compression fault, printed)
        let mut marks = Vec::new();
        if by_kind.is_empty() {
            marks.push(v.bar_n(&format!("ctl-credit: batch 5's lane {} cart late by each of 0..60 gt: the recovery bars (every kind clear)", LANES[s]), 61, true).to_string());
        }
        for (k, lates) in &by_kind {
            marks.push(format!("{k} at {}: {}", ranges(lates), v.bar(&format!("ctl-credit: batch 5's lane {} cart late by each of 0..60 gt: kind {k} at lateness {}", LANES[s], ranges(lates)), false)));
        }
        let w = marks.join("; ");
        lines.push(format!("  TOLERANCE, batch 5's lane {} cart late 0..60 gt (X {RING_X}): a cart moves between sidings (the documented compression fault) at {}; the old form (the credit on D(7) alone) loses a credit for good at {}; the design (the D(-1) completion) at {}; the recovery bars fail at {}: {w}", LANES[s], ranges(&moved), ranges(&lost), ranges(&lost_c), ranges(&failing)));
    }
    // (ironspec 20:17 (D): the delayed returning nappers: a lane-3 or lane-7 napper behind a late slot 3 (0..30 gt),
    // each still routed to its own lane by K's precedence (J(y) = (ARMED_y AND NOT HOLD_y AND NOT OFF14_y) OR K(y)), and
    // batch b+1's slot 1 printed: J(-1) branched in time, or the damage inside the two-batch bound)
    for lane in [1usize, 2] {
        let mut out = Vec::new();
        let mut by_kind: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
        for late in (0..=30).step_by(5) {
            let mut r = Row::normal("delayed napper");
            r.late = vec![(bf, 2, late)];
            r.napper = vec![(bf, lane, late)];
            let o = run(&r, Defects::default());
            let mut bad = recovery_bars(&o, &r, bf);
            let s1 = o.entered.get(bf + 1).and_then(|e| e[0]);
            // (inside slot 3's tolerance (no misroute) the napper's presses must be neutral; past it, a napper press
            // completing the faulted next batch early is part of the accepted two-batch damage, printed, ONLY before
            // REL_(bf+2) (G1 item 5): a napper effect from REL_(bf+2) on is never excused)
            let cut = o.rels.get(bf + 2).copied().unwrap_or(i64::MAX);
            let late_effect = o.napper_effects.iter().any(|e| e.strip_prefix("t ").and_then(|x| x.split(':').next()).and_then(|x| x.trim().parse::<i64>().ok()).is_some_and(|t| t >= cut));
            if late_effect || (o.misroutes.is_empty() && !o.napper_effects.is_empty()) {
                bad.push(format!("napper presses not state-neutral (OTHER): {}", o.napper_effects.join("; ")));
            }
            for k in kinds_of(&bad).split(", ").filter(|k| !k.is_empty()) {
                by_kind.entry(k.to_string()).or_default().push(late);
            }
            out.push(format!("{late}: batch 6's slot 1 into lane {}{}{}", s1.map_or("-".into(), |l| l.to_string()), if bad.is_empty() { String::new() } else { format!(" ({})", bad.join("; ")) }, if o.napper_effects.is_empty() { String::new() } else { format!(" [napper effects: {}]", o.napper_effects.join("; ")) }));
        }
        // (G1 on 60ada7ea8 B3: pinned per kind with its lateness values)
        let mut marks = Vec::new();
        if by_kind.is_empty() {
            marks.push(v.bar(&format!("ctl-credit: a lane {} napper behind batch 5's late slot 3 (both 0..30 gt late): routed to its own lane (K's precedence), the ring and the cadence back within 2 batches, every napper press neutral (every kind clear)", LANES[lane]), true).to_string());
        }
        for (k, lates) in &by_kind {
            marks.push(format!("{k} at {}: {}", ranges(lates), v.bar(&format!("ctl-credit: a lane {} napper behind batch 5's late slot 3 (both 0..30 gt late): kind {k} at lateness {}", LANES[lane], ranges(lates)), false)));
        }
        let w = marks.join("; ");
        lines.push(format!("  the delayed lane {} napper (late 0..30 with its slot 3): {}: {w}", LANES[lane], out.join("; ")));
    }
    // (G1 00:35 / ironspec 00:38 (1): "NEVER SWITCH A T UNDER A CAR": a stray D(l) press at EVERY tick of REL_6 + 80..120,
    // each lane, judged on NO CART MISROUTED (batch carts and nappers; the lanes' counts unchanged); the failing ticks
    // pinned per lane; whether a source can reach each phase is the stray-source list's, printed beside; G1 01:01 (b):
    // the same sweep at a NAPPER batch's REL (batch 6 carrying a picked napper of each lane in turn: G11 emulates that
    // lane's D press, so a stray meets a different J state))
    // (G1 CHANGES 4 C3 / C4: each sweep row judged PER KIND (a batch cart into another lane; a batch cart into the
    // way-in leg; a napper misrouted; the lanes' counts changed), one bar per failing kind with its ticks; EVERY plant read
    // back with its role (a stray landed at REL_6 + d on its lane; a late cart reaching J(-1) exactly its lateness after
    // its normal tick), REL_6 read from the run)
    let rel6 = run(&Row::normal("rel6"), Defects::default()).rels.get(6).copied().unwrap_or(606);
    let kinds_route = |o: &Out| -> Vec<&'static str> {
        let mut k = Vec::new();
        if o.misroutes.iter().any(|m| !m.contains("passed every J")) {
            k.push("BATCH-MISROUTE");
        }
        if !o.leg_entries.is_empty() {
            k.push("LEG-ENTRY");
        }
        if !o.napper_misroutes.is_empty() {
            k.push("NAPPER-MISROUTE");
        }
        if o.lane_delta != [0; 3] {
            k.push("COUNTS");
        }
        k
    };
    let mut plant_bad: Vec<String> = Vec::new();
    let mut n_plants = 0usize;
    for pick in [None, Some(0usize), Some(1), Some(2)] {
        for lane in [-1i32, 3, 7] {
            let mut by_kind: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
            let mut documented: Vec<i64> = Vec::new();
            let mut ex = String::new();
            // (G1 02:17 (2)(b): the sweep brackets the new SEEN3_OLD tap (REL + 79): REL_6 + 66..120. ironspec 02:45 (3): judged
            // on the ADOPTED bars only: a stray is a fault (the stray guard PRICED: no single non-player root), so each
            // row is judged on the fault path's bars (`recovery_bars`: the ring and the cadence back within 2 batches, no
            // misroute after them, no napper misrouted, the store PER BATCH by the ledger) and on NO BATCH CART ENTERING
            // THE WAY-IN LEG (every row, ironspec 01:40 (c)); a cart moved between sidings inside the two batches is T's
            // documented compression fault (printed with its repair note, not a failing bar))
            for d in 66..=120 {
                let mut r = Row::normal("stray sweep");
                r.stray = vec![(rel6 + d, lane)];
                if let Some(pl) = pick {
                    r.pick = vec![(6, pl)];
                }
                let o = run(&r, Defects::default());
                n_plants += 1;
                if !o.strays_landed.iter().any(|s| s.0 == rel6 + d && s.1 == lane) && plant_bad.len() < 3 {
                    plant_bad.push(format!("the stray D({lane}) at REL_6 + {d} did not land ({:?})", o.strays_landed));
                }
                let mut bad = recovery_bars(&o, &r, 6);
                if !o.leg_entries.is_empty() {
                    bad.push(format!("A BATCH CART ENTERED THE WAY-IN LEG (LEG-ENTRY): {}", o.leg_entries.join("; ")));
                }
                for k in kinds_of(&bad).split(", ").filter(|k| !k.is_empty()) {
                    by_kind.entry(if k == "OTHER" && bad.iter().any(|b| b.contains("LEG-ENTRY")) { "LEG-ENTRY" } else { k }.to_string()).or_default().push(d);
                }
                if o.lane_delta != [0; 3] || !o.misroutes.is_empty() {
                    documented.push(d);
                }
                if ex.is_empty() && !bad.is_empty() {
                    ex = format!("at REL_6 + {d}: {}", bad.join("; "));
                }
            }
            let batch = match pick {
                None => "".to_string(),
                Some(pl) => format!(" with batch 6 carrying lane {}'s picked napper", LANES[pl]),
            };
            let mut marks = Vec::new();
            if by_kind.is_empty() {
                marks.push(v.bar_n(&format!("ctl-credit: A STRAY D({lane}) at every tick of REL_6 + 66..120{batch}: the fault path's adopted bars (recovery within 2 batches, the store per batch, no batch cart into the way-in leg; every kind clear)"), 55, true).to_string());
            }
            for (k, ds) in &by_kind {
                marks.push(format!("{k} at + {}: {}", ranges(ds), v.bar(&format!("ctl-credit: A STRAY D({lane}) at every tick of REL_6 + 66..120{batch}: the fault path's adopted bars: kind {k} at REL_6 + {}", ranges(ds)), false)));
            }
            lines.push(format!("  THE STRAY SWEEP, D({lane}) at REL_6 + 66..120{batch}: {}; T's documented compression fault (carts moved inside the two batches, repair \"rebalance\") at + {}{}", marks.join("; "), ranges(&documented), if ex.is_empty() { String::new() } else { format!(" (e.g. {ex})") }));
            documented.clear();
        }
    }
    // (C3's control: a NAPPER MISROUTE planted at ticks that already fail (K's half-width cut, so a lane-7 napper behind
    // a stray misses its own K): the per-kind aggregation must ADD a NAPPER-MISROUTE bar)
    {
        let mut nk = Vec::new();
        for d in 104..=106 {
            let mut r = Row::normal("stray + napper");
            r.stray = vec![(rel6 + d, 3)];
            // (item 4's plant: the lane -1 napper's ride 3 gt late with K cut to +-1: it misses K(-1) and is misrouted)
            r.napper = vec![(5, 0, 0)];
            r.napper_jitter = vec![(5, 3)];
            let o = run(&r, Defects { k_half: Some(1), ..Default::default() });
            if kinds_route(&o).contains(&"NAPPER-MISROUTE") {
                nk.push(d);
            }
        }
        control(v, "a NAPPER MISROUTE planted at ticks the stray sweep already fails (D(3) at REL_6 + 104..106; item 4's plant: the lane -1 napper 3 gt late with K cut to +-1): the per-kind aggregation adds the NAPPER-MISROUTE kind", nk.len() == 3, &run(&Row::normal("x"), Defects::default()), &Row::normal("x"), &mut lines);
    }
    // (G1 01:00 / 01:01: THE DELAYED-CART SWEEP: one slot's batch-5 cart late by each of 1..60 gt, each slot, judged PER
    // KIND (r14's chain: J routes by arrival order, so a reorder misroutes); λ_pass per slot = the first misrouting
    // lateness, the last passing beside it; each late cart read back (its J(-1) arrival = its normal one + the lateness))
    let o_norm = run(&Row::normal("norm"), Defects::default());
    // (ironspec 02:45 (3): the delayed-cart sweep judged on the ADOPTED bars: a misroute past λ_pass is T's compression
    // fault (a fault path: bounded, repair "rebalance"; its recovery bars are judged in the lateness sweep above), so the
    // adopted bars here are NO NORMAL EVENT REACHES λ_pass (a normal cart's lateness <= the world open's k + j 3 + the
    // jitter 2 = 5 gt) and NO BATCH CART INTO THE WAY-IN LEG at any lateness)
    const NORMAL_LATE: i64 = 5;
    for s in 0..3 {
        let mut leg: Vec<i64> = Vec::new();
        let mut first_bad: Option<i64> = None;
        let mut counts_at: Vec<String> = Vec::new();
        for late in 1..=60 {
            let mut r = Row::normal("delayed cart");
            r.late = vec![(bf, s, late)];
            let o = run(&r, Defects::default());
            n_plants += 1;
            let want = o_norm.pass.get(bf).map(|p| p[s][0] + late);
            let got = o.pass.get(bf).map(|p| p[s][0]);
            if (want != got || got == Some(0)) && plant_bad.len() < 3 {
                plant_bad.push(format!("batch {bf}'s lane {} cart late {late}: its J(-1) arrival {got:?}, expected {want:?}", LANES[s]));
            }
            let ks = kinds_route(&o);
            if !ks.is_empty() && first_bad.is_none() {
                first_bad = Some(late);
                counts_at.push(format!("{:?}", o.lane_delta));
            }
            if !o.leg_entries.is_empty() {
                leg.push(late);
            }
        }
        let lam = first_bad.map_or("none in 1..60".to_string(), |f| format!("{f} gt (the last passing {} gt; the lanes' counts there {})", f - 1, counts_at.join(" ")));
        let w = v.bar(&format!("ctl-credit: NO NORMAL EVENT REACHES λ_pass for batch 5's lane {} cart (λ_pass {} vs a normal cart's lateness <= {NORMAL_LATE} gt: the world open's k + j 3 + the jitter 2)", LANES[s], first_bad.map_or("> 60".to_string(), |f| f.to_string())), first_bad.is_none_or(|f| f > NORMAL_LATE));
        let wl = if leg.is_empty() {
            v.bar_n(&format!("ctl-credit: NO BATCH CART INTO THE WAY-IN LEG with batch 5's lane {} cart late by each of 1..60 gt", LANES[s]), 60, true).to_string()
        } else {
            v.bar(&format!("ctl-credit: NO BATCH CART INTO THE WAY-IN LEG with batch 5's lane {} cart late by each of 1..60 gt: LEG-ENTRY at lateness {}", LANES[s], ranges(&leg)), false).to_string()
        };
        lines.push(format!("  THE DELAYED-CART SWEEP, lane {} late 1..60: λ_pass (the first misrouting lateness) {lam}, margin over a normal cart's lateness {}: {w}; the way-in leg: {wl}", LANES[s], first_bad.map_or("> 55".to_string(), |f| (f - 1 - NORMAL_LATE).to_string())));
    }
    let w = v.bar_n("ctl-credit: EVERY PLANT READ BACK WITH ITS ROLE (ironspec 01:35's standing rule): each sweep stray landed at REL_6 + d on its lane (REL_6 read from the run), each late cart reached J(-1) exactly its lateness after its normal tick", n_plants, plant_bad.is_empty());
    lines.push(format!("  THE PLANTS' READ-BACK ({n_plants} plants, REL_6 = t {rel6}): {}: {w}", if plant_bad.is_empty() { "every plant landed in its role".to_string() } else { plant_bad.join(" | ") }));
    // (G1 01:14 / ironspec 01:33 (3) / 01:40: THE D-PRESS RING ("armed until filled"), the routing of record: J(l)'s own
    // D press disarms it and arms the next lane's J (-1 -> 3 -> 7 -> -1), no REL-keyed reset, the resync on evidence
    // (the row empty RESYNC_QUIET gt). Its DECIDING ROWS against today's time-keyed ring: the delayed-cart sweep (each
    // slot late 1..60, plain and napper batches), the stray sweep (REL_6 + 80..120, each lane, plain and napper
    // batches): judged on LANE BALANCE (the lanes' counts unchanged) and NO BATCH CART ENTERING THE WAY-IN LEG (every
    // row, ironspec 01:40 (c)); identity misroutes (villagers trading lanes) printed; per J the least follower gap)
    {
        // (three forms: T = today's time-keyed ring (the X reset each batch; J(7) on by default); R = the D-press ring with
        // the row-empty resync; H = the D-press ring keeping the X reset as its batch-boundary resync)
        let form = |mut r: Row, f: char| -> Out {
            match f {
                'R' => {
                    r.ring = true;
                    r.ring_resync = true;
                }
                'H' => {
                    r.ring = true;
                    r.ring_reset_x = true;
                }
                _ => {}
            }
            run(&r, Defects::default())
        };
        let mut j_gap: [Option<i64>; 3] = [None; 3];
        let mut fold_gap = |o: &Out| {
            for j in 0..3 {
                if let Some(g) = o.j_gap_min[j] {
                    j_gap[j] = Some(j_gap[j].map_or(g, |m: i64| m.min(g)));
                }
            }
        };
        // (the normal run under each ring form: every design bar, no misroute, no leg entry)
        for f in ['R', 'H'] {
            let rn = Row::normal("the D-press ring, normal");
            let on = form(rn.clone(), f);
            fold_gap(&on);
            let bn = design_bars(&on, &rn, 25);
            lines.push(format!("  THE D-PRESS RING ({}), normal, PRINTED: {}{}; resyncs {} -> {}", if f == 'R' { "R: the row-empty resync" } else { "H: the X reset kept" }, describe(&on, &rn), if bn.is_empty() { String::new() } else { format!(" [{}]", bn.join("; ")) }, on.ring_resyncs, if bn.is_empty() && on.misroutes.is_empty() && on.leg_entries.is_empty() { "every design bar, no misroute, no leg entry" } else { "MISSES" }));
        }
        // (the deciding rows, PRINTED per form (the routing awaits ironspec's re-ruling on this finding): the lanes' counts
        // changed, a batch cart in the leg, identity misroutes)
        let mut tally = [[0usize; 3]; 3];
        for (label, pick) in [("plain batch", None), ("batch 5 carrying lane -1's picked napper", Some(0usize)), ("batch 5 carrying lane 3's picked napper", Some(1)), ("batch 5 carrying lane 7's picked napper", Some(2))] {
            for s in 0..3 {
                if pick == Some(s) {
                    continue;
                }
                let mut sets: Vec<(Vec<i64>, Vec<i64>)> = vec![(Vec::new(), Vec::new()); 3];
                for late in 1..=60 {
                    let mut r = Row::normal("delayed cart");
                    r.late = vec![(bf, s, late)];
                    if let Some(pl) = pick {
                        r.pick = vec![(bf, pl)];
                    }
                    for (fi, f) in ['T', 'R', 'H'].into_iter().enumerate() {
                        let o = form(r.clone(), f);
                        if f != 'T' {
                            fold_gap(&o);
                        }
                        if o.lane_delta != [0; 3] {
                            sets[fi].0.push(late);
                            tally[fi][0] += 1;
                        }
                        if !o.leg_entries.is_empty() {
                            sets[fi].1.push(late);
                            tally[fi][1] += 1;
                        }
                    }
                }
                lines.push(format!("  ROUTING FORMS, {label}, lane {} late 1..60 (PRINTED): the lanes' counts change at T {} / R {} / H {}; a batch cart enters the way-in leg at T {} / R {} / H {}", LANES[s], ranges(&sets[0].0), ranges(&sets[1].0), ranges(&sets[2].0), ranges(&sets[0].1), ranges(&sets[1].1), ranges(&sets[2].1)));
            }
        }
        for (label, pick) in [("plain batch", None), ("batch 6 carrying lane -1's picked napper", Some(0usize)), ("batch 6 carrying lane 3's picked napper", Some(1)), ("batch 6 carrying lane 7's picked napper", Some(2))] {
            for lane in [-1i32, 3, 7] {
                let mut sets: Vec<(Vec<i64>, Vec<i64>)> = vec![(Vec::new(), Vec::new()); 3];
                for d in 80..=120 {
                    let mut r = Row::normal("stray, the ring");
                    r.stray = vec![(606 + d, lane)];
                    if let Some(pl) = pick {
                        r.pick = vec![(6, pl)];
                    }
                    for (fi, f) in ['T', 'R', 'H'].into_iter().enumerate() {
                        let o = form(r.clone(), f);
                        if o.lane_delta != [0; 3] {
                            sets[fi].0.push(d);
                            tally[fi][0] += 1;
                        }
                        if !o.leg_entries.is_empty() {
                            sets[fi].1.push(d);
                            tally[fi][1] += 1;
                        }
                    }
                }
                lines.push(format!("  ROUTING FORMS, {label}, a stray D({lane}) at REL_6 + 80..120 (PRINTED): the lanes' counts change at + T {} / R {} / H {}; a batch cart enters the leg at + T {} / R {} / H {}", ranges(&sets[0].0), ranges(&sets[1].0), ranges(&sets[2].0), ranges(&sets[0].1), ranges(&sets[1].1), ranges(&sets[2].1)));
            }
        }
        lines.push(format!("  ROUTING FORMS, THE TALLY over every deciding row (counts changed / a batch cart in the leg): T {} / {}, R {} / {}, H {} / {} (FINDING: under BUNCHING (two carts closer than a J's D-press disarm latency, J -> D {J_TO_D} + the T back {RETURN}) both turn at the same J and the D-press ring then lags the carts by one; without a batch-boundary resync the lag persists and carts reach the leg)", tally[0][0], tally[0][1], tally[1][0], tally[1][1], tally[2][0], tally[2][1]));
        // (the resync's evidence: a cart stopped MID-ROW (a planted jam in the J(-1) -> J(3) segment) holds the resync
        // off: no resync while any cart is on the row)
        let mut rj = Row::normal("the ring, a cart mid-row");
        rj.jam = Some((bf, 1, 0));
        let oj = form(rj.clone(), 'R');
        let w = v.bar("ctl-credit: THE D-PRESS RING's RESYNC (R) holds off while a cart is MID-ROW (a planted jam between J(-1) and J(3)): 0 resyncs with a cart on the row", oj.resync_occupied == 0);
        lines.push(format!("  THE RING's RESYNC with a cart mid-row (jam): resyncs {}, with a cart on the row {}: {w}", oj.ring_resyncs, oj.resync_occupied));
        let jl = |j: usize| j_gap[j].map_or("-".to_string(), |g| format!("{g} gt (margin {} after the D press's disarm: J -> D {J_TO_D} + the T back {RETURN})", g - (J_TO_D + RETURN)));
        lines.push(format!("  THE RING's PER-J LEAST FOLLOWER GAP (a cart diverting at the J -> the next cart reaching it, over the normal run and every deciding row): J(-1) {}, J(3) {}, J(7) {} (ironspec 01:33 (3): a follower inside the disarm latency turns into the leader's lane)", jl(0), jl(1), jl(2)));
    }
    // (the world open mid-batch: every tick of batch 5's span, idle 1 / 2, the carts 1 / 3 behind; G1 item 11: the
    // hopper's cooldown SAVED over the open: the cooldown left after the open EQUALS the cooldown left before it, judged
    // on every run whose open lands inside a cooldown (counted: at least one must))
    let mut n_open = 0usize;
    let mut bad_open = Vec::new();
    let (mut in_cool, mut cool_bad) = (0usize, Vec::new());
    for (idle, kj) in [(1, 1), (2, 1), (1, 3), (2, 3)] {
        for t0 in 500..720 {
            let mut r = Row::normal("open");
            r.open = Some((t0, idle, kj));
            r.napper = vec![(4, 2, 0), (5, 0, 0)];
            let o = run(&r, Defects::default());
            n_open += 1;
            if let Some((a, b)) = o.open_cooldown {
                if a > 0 {
                    in_cool += 1;
                    if b != a && cool_bad.len() < 2 {
                        cool_bad.push(format!("open at {t0} idle {idle}: {a} gt left before, {b} after"));
                    }
                }
            }
            if o.open_items.is_some_and(|(a, b)| a != b) && cool_bad.len() < 2 {
                cool_bad.push(format!("open at {t0}: the items changed over the open {:?}", o.open_items));
            }
            let bad = recovery_bars(&o, &r, bf);
            if !bad.is_empty() && bad_open.len() < 2 {
                bad_open.push(format!("open at {t0} idle {idle} kj {kj}: {}; {}", bad.join("; "), describe(&o, &r)));
            }
        }
    }
    let w = v.bar_n("ctl-credit: FAULT ROW a world open at every tick of batch 5's span (t 500..719; idle 1 / 2, the carts 1 / 3 behind the redstone; returning nappers in lanes 7 and -1): the ring in its same phase within 2 batches, P = 100, the store's bars, no napper misrouted, the hopper's items kept and its cooldown SAVED (left after = left before) on every open inside a cooldown", n_open, bad_open.is_empty() && cool_bad.is_empty() && in_cool > 0);
    lines.push(format!("  FAULT ROW a world open mid-batch, {n_open} runs ({in_cool} of them inside a hopper cooldown): {w}{}{}", if bad_open.is_empty() { String::new() } else { format!(" ({})", bad_open.join(" | ")) }, if cool_bad.is_empty() { String::new() } else { format!(" [the cooldown: {}]", cool_bad.join(" | ")) }));
    {
        let (mut nb, mut ex) = (0usize, String::new());
        for t0 in 500..720 {
            let mut r = Row::normal("open unsaved");
            r.open = Some((t0, 2, 1));
            let o = run(&r, Defects { cooldown_unsaved: true, ..Default::default() });
            if let Some((a, b)) = o.open_cooldown {
                if a > 0 && b != a {
                    nb += 1;
                    if ex.is_empty() {
                        ex = format!("open at {t0}: {a} left before, {b} after");
                    }
                }
            }
        }
        let c = v.control("ctl-credit CONTROL (G1 item 11): the hopper's cooldown running through the open (not saved): the saved-cooldown check fails on the opens inside a cooldown", 1, nb > 0).to_string();
        lines.push(format!("  CONTROL (the cooldown not saved over the open: {nb} opens caught, e.g. {ex}): {c}"));
    }
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
    // (G1 item 9: K(-1)'s mask on each of D(-1)'s three outputs, one twin each: the J(-1)-off twin on a lane -1 napper
    // 20 late (its D(-1) at REL_6 + 86, after batch 6's ring reset armed J(-1): unmasked, it turns J(-1) off under batch
    // 6's slot 1); the J(3) and credit twins on batch 5's lane -1 napper inside batch 5's SEEN3 (the credit twin with
    // the completion on SEEN3 alone, so only the mask protects it))
    for (name, d, need, extra, form_old) in [
        ("K(-1)'s mask removed from J(-1)'s off drive alone (the napper 20 late, its D(-1) after batch 6's ring reset): the napper's D(-1) turns the freshly armed J(-1) off", Defects { no_km1_jm1: true, ..Default::default() }, "turned J(-1) off", 20i64, true),
        ("K(-1)'s mask removed from J(3)'s arming alone: the napper's D(-1) arms J(3)", Defects { no_km1_j3: true, ..Default::default() }, "armed J(3)", 0, true),
        ("K(-1)'s mask removed from the credit input alone (the completion on SEEN3 alone): the napper's D(-1) inside batch 5's SEEN3 makes a credit", Defects { no_km1_credit: true, ..Default::default() }, "made a credit", 0, false),
    ] {
        let mut r = Row::normal("km1 twins");
        r.napper = vec![(5, 0, extra)];
        r.dm1_old = form_old;
        let od = run(&r, Defects::default());
        let oc = run(&r, d);
        let bites = od.napper_effects.is_empty() && oc.napper_effects.iter().any(|e| e.contains(need)) && oc.napper_effects.iter().all(|e| e.contains(need));
        control(v, name, bites, &oc, &r, &mut lines);
    }
    // (G1 item 4: every cart routed by the J resolver's shapes; the control: K shortened to +-1 and the lane -1 napper's
    // ride 3 gt late: it misses K(-1), passes J(-1) / J(3) and diverts at J(7): a NAPPER MISROUTE, failing by name)
    let mut r = Row::normal("napper jitter");
    r.napper = vec![(5, 0, 0)];
    r.napper_jitter = vec![(5, 3)];
    let od = run(&r, Defects::default());
    let oc = run(&r, Defects { k_half: Some(1), ..Default::default() });
    line(v, "batch 5's lane -1 napper 3 gt late on its ride (K(-1) +-10 holds it): routed to lane -1 by the shapes", &design_bars(&od, &r, 25), &od, &r, &mut lines);
    control(v, "K shortened to +-1 with the napper's ride 3 gt late: the napper misses K(-1) and is MISROUTED (the shapes, not its identity, route it)", design_bars(&oc, &r, 25).iter().any(|b| b.contains("NAPPER MISROUTED")), &oc, &r, &mut lines);
    // (ironspec 22:06 (2) / G1 item 2: the D(-1) completion keyed on the circuit's inputs: a STRAY D(-1) inside batch 5's
    // SEEN3 (t 680, SEEN3 set over 650..697) credits as the circuit does; printed with the store's verdict; with OWED
    // (day-1's candidate) it never completes)
    for dm1_old in [false, true] {
        let mut r = Row::normal("stray dm1");
        r.stray = vec![(680, -1)];
        r.dm1_old = dm1_old;
        let o = run(&r, Defects::default());
        let bad = recovery_bars(&o, &r, bf);
        line(v, &format!("a STRAY D(-1) at t 680 inside batch 5's SEEN3 ({}): the circuit's own answer, judged on the store's and the recovery bars (credits {:?}, D(-1) completions {}, the stray read back {:?})", if dm1_old { "the D(-1) completion on OWED, day-1's candidate" } else { "the D(-1) completion on SEEN3, as ruled" }, o.credit_src, o.dm1_credits, o.strays_landed), &bad, &o, &r, &mut lines);
    }
    // (ironspec 22:35 / G1 22:44: THE DOUBLE CREDIT: a stray D(3) at every tick of REL_6 + 80..112 (batch 5's D(7) at
    // REL_6 + 85, its SEEN3 reset at + 91, batch 6's slot 1 D(-1) at + 105); a complete batch with no stray is the normal
    // run's 0 D(-1) completions; as ruled (SEEN3) and with OWED (SEEN3 sampled at the ring reset tap REL + 72, cleared by
    // the batch's own D(7) credit and by the completion))
    let mut dbl = Vec::new();
    // (the three completion forms over the same rows: (0) SEEN3 alone (the 20:17 form), (1) SEEN3 AND SEEN3_OLD (ironspec
    // 22:52 / 22:55, the design here), (2) SEEN3 AND NOT DONE (day-1's alternative); rows: the stray D(3) at every tick of
    // REL_6 + 80..112, each slot of batch 5 late 0..60, each slot missing)
    let forms: [(&str, bool, bool, bool, bool); 5] = [("SEEN3 alone", false, false, false, false), ("SEEN3 AND SEEN3_OLD (ruled)", true, false, false, false), ("SEEN3 AND NOT DONE (+ COMP on D(7))", false, true, false, false), ("the STORE gate (owed = the store empty)", false, false, true, false), ("the OWED count (>= 1, FIFO by count)", false, false, false, true)];
    for (fi, &(fname, old, dn, st, ow)) in forms.iter().enumerate() {
        let mut rows: Vec<(String, Row)> = Vec::new();
        for d in 80..=112 {
            let mut r = Row::normal("double credit");
            r.stray = vec![(606 + d, 3)];
            rows.push((format!("a stray D(3) at REL_6 + {d}"), r));
        }
        for s in 0..3 {
            for late in 0..=60 {
                let mut r = Row::normal("late");
                r.late = vec![(bf, s, late)];
                rows.push((format!("batch 5's lane {} cart {late} late", LANES[s]), r));
            }
            let mut r = Row::normal("missing");
            r.missing = vec![(bf, s)];
            rows.push((format!("batch 5's lane {} cart missing", LANES[s]), r));
        }
        let (mut nbad, mut firsts, n) = (0usize, Vec::new(), rows.len());
        let (mut stray_bad, mut late_bad, mut miss_bad) = (0usize, 0usize, 0usize);
        // (G1 on 60ada7ea8 B3: the ruled form's failing rows pinned PER KIND, each kind naming its rows by class)
        let mut by_kind: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        for (name, mut r) in rows {
            r.dm1_old = old;
            r.dm1_done = dn;
            r.dm1_store = st;
            r.dm1_owed = ow;
            let o = run(&r, Defects::default());
            let bad = recovery_bars(&o, &r, bf);
            if !bad.is_empty() {
                nbad += 1;
                for k in kinds_of(&bad).split(", ") {
                    by_kind.entry(k.to_string()).or_default().push(name.clone());
                }
                if name.contains("stray") {
                    stray_bad += 1;
                } else if name.contains("missing") {
                    miss_bad += 1;
                } else {
                    late_bad += 1;
                }
                if firsts.len() < 3 {
                    firsts.push(format!("{name}: {}", bad.join("; ")));
                }
            }
        }
        let s = format!("{fname}: {nbad} of {n} rows fail (the stray sweep {stray_bad} / 33, late slots {late_bad} / 183, missing {miss_bad} / 3){}", if firsts.is_empty() { String::new() } else { format!(" [{}]", firsts.join(" | ")) });
        if fi == 1 {
            // (one judged bar per failing kind, its rows compressed by class: "stray +a..b", "lane L late a..b",
            // "lane L missing")
            let class_rows = |names: &[String]| -> String {
                let mut stray: Vec<i64> = Vec::new();
                let mut late: std::collections::BTreeMap<String, Vec<i64>> = std::collections::BTreeMap::new();
                let mut miss: Vec<String> = Vec::new();
                for nm in names {
                    if let Some(x) = nm.split("REL_6 + ").nth(1).and_then(|x| x.split_whitespace().next()).and_then(|x| x.trim_end_matches(':').parse::<i64>().ok()) {
                        stray.push(x);
                    } else if nm.contains("missing") {
                        miss.push(nm.split("lane ").nth(1).and_then(|x| x.split_whitespace().next()).unwrap_or("?").to_string());
                    } else if let (Some(l), Some(g)) = (nm.split("lane ").nth(1).and_then(|x| x.split_whitespace().next()), nm.split("cart ").nth(1).and_then(|x| x.split_whitespace().next()).and_then(|x| x.parse::<i64>().ok())) {
                        late.entry(l.to_string()).or_default().push(g);
                    }
                }
                let mut parts = Vec::new();
                if !stray.is_empty() {
                    parts.push(format!("stray D(3) at REL_6 + {}", ranges(&stray)));
                }
                for (l, gs) in &late {
                    parts.push(format!("lane {l} late {}", ranges(gs)));
                }
                if !miss.is_empty() {
                    parts.push(format!("lane {} missing", miss.join(" / ")));
                }
                parts.join("; ")
            };
            let mut marks = Vec::new();
            if by_kind.is_empty() {
                marks.push(v.bar_n("ctl-credit: THE DOUBLE CREDIT and the D(-1) completion, the ruled form (SEEN3 AND SEEN3_OLD): the stray D(3) at every tick of REL_6 + 80..112, each slot late 0..60, each slot missing (every kind clear)", n, true).to_string());
            }
            for (k, names) in &by_kind {
                let cls = class_rows(names);
                marks.push(format!("{k}: {}", v.bar(&format!("ctl-credit: THE DOUBLE CREDIT and the D(-1) completion, the ruled form (SEEN3 AND SEEN3_OLD): kind {k} on {cls}"), false)));
            }
            dbl.push(format!("{s}: {}", marks.join("; ")));
        } else {
            dbl.push(format!("{s} (printed)"));
        }
    }
    lines.push(format!("  THE D(-1) COMPLETION's FORMS over the stray, late and missing rows: {}", dbl.join("; ")));
    // (ironspec 23:00 (1): TODAY's FORM traced against a JAM (G1's (b)): batch 5's lane-3 cart stopped between J(-1) and
    // J(3), and its lane-7 cart stopped between J(3) and J(7); every later cart riding into the segment piles behind;
    // printed per form: the RELs after the jam, the carts piled, the credits after it (a fail-stop withholds REL once the
    // store drains: at most 2 RELs after the jammed batch's)
    let mut jams = Vec::new();
    for &(fname, old, dn, st, ow) in forms.iter().chain([("the OWED count + the split row's OCCUPANCY gate on the completion (day-1 23:06, needs entry / exit detectors: not bought)", false, false, false, true)].iter()) {
        for (jname, jam) in [("lane-3 cart between J(-1) and J(3)", (bf, 1usize, 0usize)), ("lane-7 cart between J(3) and J(7)", (bf, 2, 1))] {
            let mut r = Row::normal("jam");
            r.jam = Some(jam);
            r.dm1_old = old;
            r.dm1_done = dn;
            r.dm1_store = st;
            r.dm1_owed = ow;
            r.occ_gate = fname.contains("OCCUPANCY");
            let o = run(&r, Defects::default());
            let after = o.rels.iter().filter(|&&x| x > o.rels.get(bf).copied().unwrap_or(0)).count();
            let cr_after = o.credits.iter().filter(|c| c.0 > o.rels.get(bf).copied().unwrap_or(0) + 100).count();
            jams.push(format!("{fname}, {jname}: {after} RELs after batch 5's ({}), {} carts piled, {cr_after} credits after", if after <= 2 { "FAIL-STOP" } else { "RUNS INTO THE JAM" }, o.piled.len()));
        }
    }
    lines.push(format!("  JAM TRACES (ironspec 23:00 (1), printed): {}", jams.join(" | ")));
    {
        // (OWED's timeline on the normal run, G1 22:44's table: the sample at REL + 72 reads batch b's SEEN3 (set at REL_b
        // + 144 = REL_{b+1} + 44), batch b's D(7) credit at REL_{b+1} + 85 clears OWED, batch b+1's slot 1 D(-1) at + 105
        // finds it clear: 0 completions)
        let mut r = Row::normal("owed normal");
        r.dm1_old = true;
        r.napper = vec![(2, 0, 0), (4, 1, 0), (6, 2, 0)];
        let o = run(&r, Defects::default());
        let bad = design_bars(&o, &r, 25);
        let w = v.bar("ctl-credit: OWED on the normal run (a complete batch, no stray): 0 D(-1) completions, every design bar", bad.is_empty() && o.dm1_credits == 0);
        lines.push(format!("  OWED's timeline (REL + 72 sample of SEEN3, set at REL_b + 144 = REL_(b+1) + 44; batch b's D(7) credit at REL_(b+1) + 85 clears it; batch b+1's slot 1 D(-1) at + 105 finds it clear): {} D(-1) completions on the normal run{}: {w}", o.dm1_credits, if bad.is_empty() { String::new() } else { format!(" ({})", bad.join("; ")) }));
    }
    // (G1 22:32 / ironspec 22:30: WHAT STOPS A SECOND REL: F1's AND dipping one tick at REL_5 + d and re-rising at + d + 1
    // while the request latch's reset (RESET 6) is still landing: a second REL edge, whose unlock can move a second item;
    // judged "exactly one REL per batch" and "one transfer per REL", as built (the unlock on REL's edge) and with every
    // REL consumer keyed on the request latch's reset (day-1's fix))
    let mut srel = Vec::new();
    for fix in [false, true] {
        let (mut nbad, mut first, mut n) = (0usize, None, 0usize);
        for d in 1..=12 {
            let mut r = Row::normal("f1 dip");
            r.f1_dip = vec![(5, d)];
            r.unlock_on_request = fix;
            let o = run(&r, Defects::default());
            n += 1;
            let bad = design_bars(&o, &r, 25);
            if !bad.is_empty() {
                nbad += 1;
                if first.is_none() {
                    first = Some(format!("REL_5 + {d}: {}", bad.join("; ")));
                }
            }
        }
        let w = if fix {
            v.bar_n("ctl-credit: A SECOND REL (G1 22:32, ironspec 23:06 (4)): F1's AND dipping one tick at REL_5 + 1..12, REL one pulse per golem request (every consumer keyed on the request latch's reset): exactly one REL per batch and one transfer per REL", n, nbad == 0).to_string()
        } else {
            v.control("ctl-credit CONTROL (the second REL as built: the unlock and the other consumers on REL's edge): a dip inside the request latch's reset makes a second REL edge", 1, nbad > 0).to_string()
        };
        srel.push(format!("{}: {nbad} of {n} dips fail{}: {w}", if fix { "on the request latch" } else { "as built" }, first.map_or(String::new(), |f| format!(" (first: {f})"))));
    }
    lines.push(format!("  A SECOND REL, F1's AND dipping at REL_5 + 1..12: {}", srel.join("; ")));
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
    control(v, "no ballast (2 items, RE_ARMED = >= 1 item), batch 5's lane-7 cart 20 late: REL fires on the in-flight credit with the hopper EMPTY, the credit lands in an empty hopper (its insert cooldown 7) inside the unlock window, and that REL moves no item (the count gains one); with the ballast the same row keeps every store bar", od.at_rel.iter().all(|r| r.2.is_some()) && store_bars(&od).is_empty() && oc.at_rel.iter().any(|r| r.2.is_none()), &oc, &r, &mut lines);
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
    fn trace_the_lost_credit() {
        // (ironspec 02:06: WHY a late lane -1 cart loses a credit for good: the presses, the credits and their sources,
        // per batch, for lateness 0 / 10 / 20)
        for late in [0i64, 10, 20] {
            let mut r = Row::normal("trace");
            r.late = vec![(5, 0, late)];
            let o = run(&r, Defects::default());
            println!("TRACE late {late}: RELs {:?}", &o.rels[..o.rels.len().min(12)]);
            for b in 4..9.min(o.presses.len()) {
                println!("TRACE late {late}: batch {b} entered {:?} presses {:?}", o.entered[b], o.presses[b]);
            }
            println!("TRACE late {late}: credits (tick, batch) {:?}", &o.credits[..o.credits.len().min(12)]);
            println!("TRACE late {late}: credit_src (completing batch, SEEN3's batch) {:?}", &o.credit_src[..o.credit_src.len().min(12)]);
            println!("TRACE late {late}: credit presses {:?}; dm1 credits {}; misroutes {:?}; lane delta {:?}", &o.credit_press[..o.credit_press.len().min(12)], o.dm1_credits, o.misroutes, o.lane_delta);
        }
        // (the lane-7 GAIN rows: slot 3 late 23)
        {
            // (THE RESYNC's PREMISE: the batches in flight at each REL wait's start, healthy vs one-short)
            let show = |name: &str, r: &Row, d: Defects| {
                let o = run(r, d);
                let hist: Vec<String> = o.wait_inflight.iter().map(|(t, n)| format!("t{t}:{n}")).collect();
                println!("PREMISE {name}: {} waits [{}]; RELs {}, ledger [{}]", o.wait_inflight.len(), hist.iter().take(14).cloned().collect::<Vec<_>>().join(" "), o.rels.len(), ledger_bars(&o).join("; "));
            };
            show("normal", &Row::normal("p"), Defects::default());
            for (s, late) in [(2usize, 10i64), (2, 20), (2, 27), (0, 5), (0, 8), (1, 15)] {
                let mut r = Row::normal("p");
                r.late = vec![(5, s, late)];
                show(&format!("healthy: lane {} cart late {late}", LANES[s]), &r, Defects::default());
            }
            show("ONE-SHORT: batch 5's credit dropped", &Row::normal("p"), Defects { drop_credit: Some(5), ..Default::default() });
            for (s, late) in [(2usize, 10i64), (2, 20), (0, 8)] {
                let mut r = Row::normal("p");
                r.late = vec![(10, s, late)];
                show(&format!("ONE-SHORT (batch 5's credit dropped) + batch 10's lane {} cart late {late}", LANES[s]), &r, Defects { drop_credit: Some(5), ..Default::default() });
            }
            show("TWO-SHORT: batches 5 and 8's credits dropped (drop 5 then 8)", &Row::normal("p"), Defects { drop_credit: Some(5), drop_credit2: Some(8), ..Default::default() });
            {
                let mut r = Row::normal("p");
                r.late = vec![(5, 2, 25)];
                r.napper = vec![(5, 1, 25)];
                show("A TRUE LOST CREDIT (no press: residual (ii)'s twin, K(3) without the J(3) reset)", &r, Defects { k3_j3_unmasked: true, ..Default::default() });
                let mut r = Row::normal("p");
                r.napper = vec![(5, 2, 50)];
                show("A TRUE LOST CREDIT (a lane 7 napper released 50 late)", &r, Defects::default());
            }
            for late in [100i64, 300] {
                let mut r = Row::normal("p");
                r.late = vec![(5, 2, late)];
                show(&format!("a stalled-then-recovered lane 7 cart (late {late})"), &r, Defects::default());
            }
            let mut r = Row::normal("p");
            r.missing = vec![(5, 0)];
            show("lane -1 cart lost after release", &r, Defects::default());
            // (THE RESYNC NET on: the true-lost-credit rows must restore; every other row must never fire)
            let shown = |name: &str, r: &Row, d: Defects| {
                let mut r = r.clone();
                r.resync_net = true;
                let o = run(&r, d);
                println!("NET {name}: fires {:?}; RELs {}; unanswered {:?}; FIFO gains {}; periods after the fire {:?}", o.net_fires, o.rels.len(), o.unanswered, o.answers.iter().filter(|a| a.is_none()).count(), o.rels.windows(2).map(|w| w[1] - w[0]).filter(|&p| p != PERIOD).take(8).collect::<Vec<_>>());
            };
            shown("normal", &Row::normal("p"), Defects::default());
            {
                let mut r = Row::normal("p");
                r.late = vec![(5, 2, 25)];
                r.napper = vec![(5, 1, 25)];
                shown("TRUE LOST (K(3) twin)", &r, Defects { k3_j3_unmasked: true, ..Default::default() });
                let mut r = Row::normal("p");
                r.napper = vec![(5, 2, 50)];
                shown("TRUE LOST (lane 7 napper released 50 late)", &r, Defects::default());
                let mut r = Row::normal("p");
                r.napper = vec![(5, 2, 60)];
                shown("TRUE LOST (lane 7 napper released 60 late)", &r, Defects::default());
            }
            for (s, late) in [(2usize, 20i64), (2, 27), (0, 9), (0, 20), (1, 30), (2, 100), (2, 300)] {
                let mut r = Row::normal("p");
                r.late = vec![(5, s, late)];
                shown(&format!("lane {} cart late {late}", LANES[s]), &r, Defects::default());
            }
            for s in 0..3 {
                let mut r = Row::normal("p");
                r.missing = vec![(5, s)];
                shown(&format!("lane {} cart lost after release", LANES[s]), &r, Defects::default());
            }
            for d in [500i64, 2000, 6000] {
                let mut r = Row::normal("p");
                r.holds_empty = vec![(700, 700 + d)];
                r.t_max = 3000 + d;
                shown(&format!("a halt of {d} gt (holds empty)"), &r, Defects::default());
            }
            let mut nf = 0usize;
            for t0 in 500..720 {
                let mut r = Row::normal("p");
                r.open = Some((t0, 0, 3));
                r.resync_net = true;
                nf += run(&r, Defects::default()).net_fires.len();
            }
            println!("NET a world open (entities 3 behind) at every tick 500..719: {nf} fires");
        }
        {
            // (G1 02:44b (2): K(7) by the same matrix: a lane 7 napper alone (its batch on time) late 0..120: where does
            // its D(7) land, and does it change the store?)
            let mut fx = Vec::new();
            for extra in (0..=120).step_by(10) {
                let mut r = Row::normal("k7");
                r.napper = vec![(5, 2, extra)];
                let o = run(&r, Defects::default());
                let lb = ledger_bars(&o);
                fx.push(format!("{extra}: {} {:?} misroutes {:?}{}", if lb.is_empty() { "clean" } else { "CHANGED" }, o.napper_effects, o.misroutes, if lb.is_empty() { String::new() } else { format!(" [{}]", lb.join("; ")) }));
            }
            println!("K7PROBE a lane 7 napper alone, late: {}", fx.join(" | "));
        }
        {
            let mut r = Row::normal("trace3n");
            r.late = vec![(5, 2, 25)];
            r.napper = vec![(5, 1, 25)];
            let o = run(&r, Defects::default());
            for b in 4..8.min(o.presses.len()) {
                println!("TRACE3N: batch {b} entered {:?} presses {:?}", o.entered[b], o.presses[b]);
            }
            println!("TRACE3N: RELs {:?}", &o.rels[..o.rels.len().min(10)]);
            println!("TRACE3N: credits (tick, batch) {:?}; src {:?}; credit presses {:?}; dm1 credits {}; blocked {:?}; napper effects {:?}; ledger {:?}", &o.credits[..o.credits.len().min(10)], &o.credit_src[..o.credit_src.len().min(10)], &o.credit_press[..o.credit_press.len().min(10)], o.dm1_credits, o.d7_blocked, o.napper_effects, ledger_bars(&o));
            println!("TRACE3N: misroutes {:?}; napper misroutes {:?}; lane delta {:?}", o.misroutes, o.napper_misroutes, o.lane_delta);
            let mut r0 = r.clone();
            r0.napper = vec![];
            let o0 = run(&r0, Defects::default());
            println!("TRACE3N without the napper: credits {:?}; src {:?}; presses b5 {:?}; ledger {:?}", &o0.credits[..o0.credits.len().min(10)], &o0.credit_src[..o0.credit_src.len().min(10)], o0.presses.get(5), ledger_bars(&o0));
        }
        {
            let mut r = Row::normal("trace7");
            r.late = vec![(5, 2, 23)];
            let o = run(&r, Defects::default());
            for b in 4..8.min(o.presses.len()) {
                println!("TRACE7 late 23: batch {b} entered {:?} presses {:?}", o.entered[b], o.presses[b]);
            }
            println!("TRACE7 late 23: credits (tick, batch) {:?}; src {:?}; credit presses {:?}; dm1 credits {}; ledger {:?}", &o.credits[..o.credits.len().min(10)], &o.credit_src[..o.credit_src.len().min(10)], &o.credit_press[..o.credit_press.len().min(10)], o.dm1_credits, ledger_bars(&o));
        }
        // (the cause fix candidate: SEEN3_OLD sampled at its own tap REL + n; the late / missing sweeps' rows that still
        // LOSE or GAIN a credit, per tap)
        // (the one-credit-per-batch guard on top of the tap at REL + 78: the window swept; every late row (each slot
        // 0..60), the missing rows, the delayed nappers (lane 3 / 7 behind a late slot 3, 0..30) and the stray D(7) at
        // REL_6 + 66..120: rows losing / gaining a credit)
        for blk in [0i64, 10, 20, 30, 40, 60] {
            let (mut lost, mut gain) = (Vec::new(), Vec::new());
            let mut chk = |name: String, r: Row| {
                let mut r = r;
                r.dm1_block7 = blk;
                let o = run(&r, Defects::default());
                let lb = ledger_bars(&o);
                if lb.iter().any(|b| b.contains("NEVER")) {
                    lost.push(name.clone());
                }
                if lb.iter().any(|b| b.contains("GAIN") || b.contains("TWICE") || b.contains("NO BATCH")) {
                    gain.push(name);
                }
            };
            for s in 0..3 {
                for late in 0..=60 {
                    let mut r = Row::normal("g");
                    r.late = vec![(5, s, late)];
                    chk(format!("lane {} late {late}", LANES[s]), r);
                }
                let mut r = Row::normal("g");
                r.missing = vec![(5, s)];
                chk(format!("lane {} missing", LANES[s]), r);
            }
            for lane in [1usize, 2] {
                for late in (0..=30).step_by(5) {
                    let mut r = Row::normal("g");
                    r.late = vec![(5, 2, late)];
                    r.napper = vec![(5, lane, late)];
                    chk(format!("lane {} napper {late}", LANES[lane]), r);
                }
            }
            for d in 66..=120 {
                let mut r = Row::normal("g");
                r.stray = vec![(606 + d, 7)];
                chk(format!("stray D(7) + {d}"), r);
            }
            let mut rn = Row::normal("g");
            rn.dm1_block7 = blk;
            let on = run(&rn, Defects::default());
            println!("GUARDSWEEP tap 78, D(7) blocked {blk} gt after a completion: losing {} {:?}; gaining {} {:?}; normal {} RELs / {} credits {:?}", lost.len(), lost.iter().take(5).collect::<Vec<_>>(), gain.len(), gain.iter().take(6).collect::<Vec<_>>(), on.rels.len(), on.credits.len(), ledger_bars(&on));
        }
        // (G1 02:17 (2)(a) / the timing rule: the tap window under a world open (idle 2, the carts 3 behind) at every tick
        // of REL_6 + 0..114 (t 606..720) on the formerly-losing late rows, the missing rows and the gain rows)
        if std::env::var("CC_TAP_OPEN").is_ok() {
            let mut rows: Vec<(String, Row)> = Vec::new();
            for (s, lates) in [(0usize, vec![9i64, 12, 15, 18, 21, 24, 27, 31, 49, 53, 57, 60]), (1, vec![29, 32, 35, 38, 40, 57, 60]), (2, vec![21, 23, 25, 27])] {
                for late in lates {
                    let mut r = Row::normal("g");
                    r.late = vec![(5, s, late)];
                    rows.push((format!("lane {} late {late}", LANES[s]), r));
                }
            }
            for s in 0..3 {
                let mut r = Row::normal("g");
                r.missing = vec![(5, s)];
                rows.push((format!("lane {} missing", LANES[s]), r));
            }
            rows.push(("normal".to_string(), Row::normal("g")));
            for tap in [70i64, 72, 74, 76, 78, 80, 82, 84] {
                let (mut lost, mut gain, mut n) = (0usize, 0usize, 0usize);
                let mut ex: Vec<String> = Vec::new();
                for (name, r0) in &rows {
                    for t0 in 606..=720 {
                        let mut r = r0.clone();
                        r.s3old_tap = Some(tap);
                        r.open = Some((t0, 2, 3));
                        let o = run(&r, Defects::default());
                        n += 1;
                        let lb = ledger_bars(&o);
                        let l = lb.iter().any(|b| b.contains("NEVER"));
                        let g = lb.iter().any(|b| b.contains("A CREDIT GAINED") || b.contains("TWICE") || b.contains("NO BATCH"));
                        lost += l as usize;
                        gain += g as usize;
                        if (l || g) && ex.len() < 4 {
                            ex.push(format!("{name} open at REL_6 + {}", t0 - 606));
                        }
                    }
                }
                println!("TAPOPEN tap REL + {tap}: {n} runs, losing {lost}, gaining {gain} {ex:?}");
            }
        }
        for (fname, f) in [("SEEN3 AND SEEN3_OLD (ruled)", 0u8), ("+ DONE", 1), ("+ STORE", 2), ("+ OWED", 3)] {
        for tap in [None, Some(76), Some(78), Some(80)] {
            let (mut lost, mut gain, mut other) = (Vec::new(), Vec::new(), 0usize);
            for s in 0..3 {
                for late in 0..=60 {
                    let mut r = Row::normal("tap");
                    r.late = vec![(5, s, late)];
                    r.s3old_tap = tap;
                    r.dm1_done = f == 1;
                    r.dm1_store = f == 2;
                    r.dm1_owed = f == 3;
                    let o = run(&r, Defects::default());
                    let lb = ledger_bars(&o);
                    if lb.iter().any(|b| b.contains("NEVER")) {
                        lost.push((LANES[s], late));
                    }
                    if lb.iter().any(|b| b.contains("GAIN") || b.contains("TWICE") || b.contains("NO BATCH")) {
                        gain.push((LANES[s], late));
                    }
                }
                let mut r = Row::normal("tap missing");
                r.missing = vec![(5, s)];
                r.s3old_tap = tap;
                r.dm1_done = f == 1;
                r.dm1_store = f == 2;
                r.dm1_owed = f == 3;
                let o = run(&r, Defects::default());
                if ledger_bars(&o).iter().any(|b| b.contains("NEVER")) {
                    other += 1;
                }
            }
            let mut rn = Row::normal("tap normal");
            rn.s3old_tap = tap;
            rn.dm1_done = f == 1;
            rn.dm1_store = f == 2;
            rn.dm1_owed = f == 3;
            let on = run(&rn, Defects::default());
            println!("TAPSWEEP {fname}, SEEN3_OLD tap {tap:?}: rows losing a credit {} {:?}; gaining {} {:?}; missing rows losing {other}; normal: {} RELs / {} credits, ledger {:?}", lost.len(), lost.iter().take(6).collect::<Vec<_>>(), gain.len(), gain.iter().take(6).collect::<Vec<_>>(), on.rels.len(), on.credits.len(), ledger_bars(&on));
        }
        }
    }

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
        // (G1 / ironspec 23:06 (3) / 00:32, G1 on 60ada7ea8 B2 / B3, G1 CHANGES 4 on 76d5435d8, ironspec 01:57 (4) /
        // 02:16 (3) / 02:43 (1) / 02:45 (3) / 02:54 (3): the test asserts the bench's EXACT verdict PER (row, kind), on the
        // ADOPTED bars only (a superseded bar is removed in code): the normal nights per batch (0 LOST, 0 GAIN, 0 credit
        // from no batch, nothing in flight at the end) and NO CART MISROUTED; the single-fault rows on THE STORE CORRECT
        // (the FIFO ledger, the resync net restoring it), slips printed as cost. With SEEN3_OLD's own tap at REL + 79, THE
        // ONE-CREDIT-PER-BATCH GUARD (20 gt, opened by every credit), every K(l) masking ALL of D(l)'s outputs (K(3) over the
        // J(3) reset, K(7) by order) and THE RESYNC NET (two consecutive REL waits with one batch in flight), every late,
        // missing, stray and late-napper row keeps the store correct. THE OPEN FAILURES (adopted bars, by name): a batch cart
        // into the way-in leg under a stray D(-1) / D(3) with batch 6 carrying a picked napper (T's leg entries: the stray
        // guard priced); every other bar passes and every control bites; the CLI exits 1 by name)
        let mut v = Verdict::new("ctl-credit-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
        let fails = v.fails();
        let expected = [
            ("A STRAY D(-1) at every tick of REL_6 + 66..120 with batch 6 carrying lane 3's picked napper: the fault path's adopted bars: kind LEG-ENTRY at REL_6 + 71..73", ""),
            ("A STRAY D(-1) at every tick of REL_6 + 66..120 with batch 6 carrying lane 7's picked napper: the fault path's adopted bars: kind LEG-ENTRY at REL_6 + 71..93", ""),
            ("A STRAY D(3) at every tick of REL_6 + 66..120 with batch 6 carrying lane 7's picked napper: the fault path's adopted bars: kind LEG-ENTRY at REL_6 + 104..120", ""),
        ];
        assert_eq!(fails.len(), expected.len(), "ctl-credit-bench: the pinned verdict changed ({} misses, {} pinned): {:#?}", fails.len(), expected.len(), fails);
        for (e, k) in expected {
            assert_eq!(fails.iter().filter(|f| f.contains(e) && f.contains(k)).count(), 1, "ctl-credit-bench: the pinned miss \"{e}\" {k} is not exactly once among {:#?}", fails);
        }
        // (G1 on 60ada7ea8 B1: the verdict fails the run: no flag turns it into exit 0)
        assert!(v.finish(false).is_err(), "ctl-credit-bench: the pinned verdict must fail the run");
    }
}
