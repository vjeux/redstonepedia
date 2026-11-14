//! ctl-shutter, LOGIC LEVEL: the stop's shutter / release block of record as a delayed-logic model at 1-gt
//! resolution, before its cells are laid on the stop's placement (the real-cell bench follows the layout and
//! replaces every DESIGNED delay below with its measured one).
//!
//! THE FORM OF RECORD (ironspec 16:00 / 16:06 / 16:32 (4) / 17:32 / 17:35 / 17:39 / 17:46; G1 15:58 / 17:21 / 17:30 /
//! 17:33 / 17:40 / 17:49):
//! - THE RELEASE KEYS ON SUSTAINED PRESENCE, anchored at the bed-head pulse:
//!   - SINCE: S = a pulse AND P_STRING at its head change AND the ONSET (P_STRING rose within 1 gt of the change: its
//!     rising edge, a comparator with P on its rear and P through a delay-1 repeater on its side; G1 on 5f79952c2 B2);
//!     R = NOT P_STRING. A press standing from before the change never sets it. (Its old delay-2 repeater REMOVED,
//!     ironspec 18:12 (d): under the onset its twin failed no row, and it doubled the hold WIN's D line gives.)
//!   - WIN: S = the pulse through the D line (8; with WIN's set 2, its NOT-Q 4 and the AND 4, RLS reads P_STRING at
//!     pulse + 12, the ruled hold); R = the pulse itself (a new pulse restarts the window) OR the pulse through the W
//!     line (48, one tick under the ruled 50 for G1's N1: RLS no later than pulse + 59).
//!   - PARKED = SENSED AND COOL (ironspec 17:35 (2) / 17:39 (3), the sensed park bought): SENSED, a latch set by the
//!     park-entry detector rail (0,-27,2) as the arriving cart passes it, through d = 28 (>= the measured pass -> rest
//!     16..26 + 2, 58f770cc 18:35), reset at RLS or ARM's edge (a press after RLS
//!     can't pre-set the next visit, G1 17:49 (a)); COOL, the boarding-cooldown bound, a latch set by ARM's edge through
//!     the line's 52 tap (C = the eject's latest tick ARM + 3 + the 60-gt boarding cooldown + the world-open term 2 - p
//!     - the fastest park -> P ride 13 = ARM + 50 <= 52), reset at RLS or ARM's edge.
//!   - RLS = P_STRING AND SINCE AND WIN AND SENSED AND COOL; one pulse a visit (COOL's and SENSED's resets end it).
//!     PARK = RLS through a delay-1 repeater (p = 2: tick-exact 17:52 put NEAREST_HOSTILE's clear at the close + 19).
//! - SO (slept once): S = a pulse, R = RLS (one repeater: SO feeds OPEN only; the list's old "+ 8" had no job left).
//! - OPEN: S = PARKED AND SO AND NOT P_STRING (ironspec 17:32 (4) / 17:46 (4): PARKED in A70's place, A70 dropped;
//!   LAST_SLEPT is set at the lie-down, tick-exact 17:42); R = RLS OR P_STRING (the shutter closes at the wake's press).
//!   SHUT = NOT OPEN (the latch's torch).
//!
//! THE MODEL. Every gate carries its DESIGNED delay (the gate list's PROPAGATION DELAYS): a repeater 2 gt per redstone
//! tick, a torch 2, a comparator 2; ctl-junction's NOR latch, its set drive to Q 4 (SET: the set repeater 2, the output
//! repeater 2), its reset drive to Q's fall 6 (RESET: the reset repeater 2, B's torch 2, the output repeater 2); while
//! both drives are on the set drive holds Q (it powers Q's block directly) and the later drive wins. Two domains, as
//! 26.3 runs them: the BLOCK domain (every gate, the observer's scheduled tick, the tripwire's re-check) and the ENTITY
//! domain (the napper, the cart, the tripwire's press, the bed head's changes); block ticks run before entities in a
//! tick. A world open freezes both: the block domain resumes first (the overdue ticks fire then, in due order), the
//! entity domain `kj` ticks later (tick-exact's load order: k + j 1 on the build, 3 straddling); every delay counts
//! its own domain's ticks; game time runs on (SleepInBed's LAST_WOKEN cooldown counts game time; the 60-gt boarding
//! cooldown counts the villager's own entity ticks and a world open clears it, tick-exact 17:39).
//! The napper and the cart are scripted per row from 58f770cc's measured bands (bed-tier-min, explore/bedtier-stop
//! 2af606b9d (b48) NOTES.md: ARM -> lie-down 4..5, the window -> wake 1..20, the cart at rest 33..46, the park -> P
//! 13..19: physical timings of its stop bench) with the dynamic rules that close the loop: a sleeper wakes once the
//! window has been open its scan phase (Z1 seen), stands up on P (the only stand-up cell: 58f770cc 15:49, tick-exact
//! 15:54) in its wake tick, boards the cart that reaches P while it stands there and may board, and (calm, awake, past
//! its cooldown, not off P in Z1's view) lies down again where it stands. P_STRING is a tripwire (pressed in the
//! entity phase of the tick a box meets it, re-checked every 10 gt from the press, released at the first re-check with
//! no box); the observer is tick-exact's 26.3 model (a head change schedules its pulse 2 gt later, 2 gt wide; a change
//! while the pulse is pending or on is absorbed; a lie-down and a wake in one tick are two changes, one pulse); a
//! napper lying down in the tick it steps onto P never meets the string (its sleep moves it first).

use crate::verdict::Verdict;

/// A torch's delay (gt).
pub const TORCH: i64 = 2;
/// A delay-1 repeater.
pub const REP: i64 = 2;
/// ctl-junction's NOR latch: the set drive to Q (its measured switch 4): 2 to Q's block, 2 for the output repeater.
pub const SET: i64 = 4;
/// ctl-junction's NOR latch: the reset drive to Q's fall (its measured return 6): 4 to Q's block, 2 for the output.
pub const RESET: i64 = 6;
/// The latch's internal set (to Q's block) and reset (to Q's block's fall).
const SET_IN: i64 = SET - REP;
const RESET_IN: i64 = RESET - REP;
/// ARM to COOL's edge comparator: the line's tap (6 x delay 4 + 1 x delay 2). COOL's bound (ironspec 17:32 (3) /
/// 17:41): the eject's latest tick ARM + 3 + the 60-gt boarding cooldown + the world-open term 2 - p 2 - the fastest
/// park -> P ride 13 = ARM + 50: the tap at 52 keeps it with 2 gt to spare.
pub const COOL_LINE: i64 = 52;
/// An edge comparator's pulse width: its side path (a delay-2 repeater, 4 gt) against its rear.
pub const EDGE_W: i64 = 4;
/// A comparator's delay (2 gt).
pub const CMP: i64 = 2;
/// An AND as NOT-OR-NOT: an input repeater (2) into the OR block, the block's torch (2).
pub const AND: i64 = 4;
/// RLS to the park's powered rail: THE PLAIN DRIVE OF RECORD in the re-laid stop (ironspec 21:51: a delay-1 repeater at
/// (2,0,2) facing south into the slime (2,0,3), its input (2,0,1) PARK's OR dust: PARK = RLS OR P_HOLD at RLS + 2, p = 2).
pub const PARK: i64 = REP;
/// (g)'s latch drive, the FALLBACK if no face of the slime takes a repeater (ironspec 21:09 / 21:15 / 21:29): the latch's
/// two torches, Q-bar's (2) and Q's (2), the Q torch under the slime: PARK = Q = [RLS + 4, RLS + 12).
pub const PARK_TORCH: i64 = 2 * TORCH;
/// THE PARK's STRETCH: P_HOLD's reset line, RLS through a chain of delay-4 repeaters (setting 2 each), then the reset
/// repeater into the latch's base. THE PARK DRIVE MUST STAY ON >= 14 gt (58f770cc 22:05 on the real cells: the fastest
/// arrivals park low at x 1.45..1.49 and need up to 12 gt to leave the park cell, so a shorter drive lets the unpowered
/// brake re-catch them: W 8 and 10 strand 40 / 408, W 12 clean with margin 1..7, W 16 clean with margin 5..11). The
/// plain drive: PARK = B2 + 2 = [RLS + 2, RLS + PARK_W + 6), 16 gt with three repeaters (12). (The old drive's gate
/// model keeps its 8 for its controls: OLD_PARK_W.)
pub const PARK_W: i64 = 20;
/// (g)'s fallback: PARK = Q = [RLS + 4, RLS + PARK_W_TORCH + 4), 16 gt with four repeaters.
pub const PARK_W_TORCH: i64 = 16;
/// The old drive's stretch (its controls only).
pub const OLD_PARK_W: i64 = 8;
/// The cart's exit from the park cell after its first move, at the slowest (58f770cc 22:05: up to 12 gt for the fastest
/// arrivals, parked low): the last tick on the park, judged >= 2 gt before PARK's end.
pub const PARK_LEAVE: i64 = 12;
/// The cart reads the park's power in its next tick (the rail's state of the previous block tick: G1's C1, a step of
/// its own in the closed form beside the cart's first move).
pub const PARK_READ: i64 = 1;
/// WIN's end: the pulse through the W line, 48 for the plain drive of record (6 x delay 4 + 1 x delay 4 - 4: ironspec on
/// G1's N1, day-1 19:25; W back to 48 with the re-laid stop's plain drive, ironspec 21:51): the latest RLS lands at pulse
/// + W_LINE + W_SLACK = pulse + 59, so the cooldown edge keeps its >= 10 at boarding with every term (see `COOL_EDGE`).
pub const W_LINE: i64 = 48;
/// (g)'s W (the fallback): its 4-gt drive costs 2 gt over the plain drive's 2, and W pays them back (46: the closed form
/// stays wake + 89 / 11); then W IS PINNED WITH ZERO SLACK ON ONE SIDE (the + 9 wake's first window, G1 20:15 / 21:11).
pub const W_TORCH: i64 = 46;
/// WIN's start, the D line: two delay-2 repeaters (8 gt).
pub const D_LINE: i64 = 8;
/// The hold RLS reads (ironspec 16:00: P_STRING held from the pulse through pulse + 12). RLS's AND reads WIN through
/// WIN's NOT-Q output (REP + TORCH after Q's block) and P_STRING through P's own torch (TORCH); both then pass the AND.
/// WIN's Q block rises SET_IN after the D line, so at a given RLS tick the AND sees WIN's state of (that tick - AND -
/// REP - TORCH) and P's of (that tick - AND - TORCH): the two torches and the AND cancel, and P is read REP after
/// WIN's Q block: pulse + D_LINE + SET_IN + REP = 12 (one source: the D line).
pub const HOLD: i64 = D_LINE + SET_IN + REP;
/// SINCE's sample through one delay-2 repeater (a new pulse's restart closes WIN before SINCE can pass).
pub const SINCE_DELAY: i64 = 0;
/// SINCE's old delay-2 repeater (4 gt), kept only for the control that puts it back.
pub const OLD_SINCE_DELAY: i64 = 4;
/// The onset's allowance: a press beginning up to this many gt before the head change still counts as fresh (the
/// edge comparator's width 2 against the pulse's 2); tick-exact's press / pulse row sets it.
pub const ONSET: i64 = 1;
/// The tripwire's re-check period (26.3's TripWireBlock: scheduleTick(pos, this, 10) while pressed).
pub const RECHECK: i64 = 10;
/// The observer: a head change to its pulse's rise, and the pulse's width (tick-exact, 26.3).
pub const OBS_RISE: i64 = 2;
pub const OBS_W: i64 = 2;
/// ARM, the detector rail: powered for the cart's pass + 20 (DetectorRailBlock's 20-gt re-check).
pub const ARM_W: i64 = 21;
/// The park-entry detector (0,-27,2): the cart passes it this long before its rest (58f770cc 18:35 on the real cells:
/// pass -> rest 16..26; each row carries its own, default the worst 26); powered for its pass + 20.
pub const PARK_ENTRY_BEFORE: i64 = 26;
/// The detector to SENSED's set: a delay >= the press -> rest settle + 2 (ironspec 17:51; on 58f770cc's measured
/// pass -> rest 16..26: 28, seven delay-4 repeaters), so SENSED never rises before the cart rests.
pub const SENSE_D: i64 = 28;
/// The cart leaves P's cell this long after it reaches P (a boarded rider presses the string until then; 58f770cc's
/// stop bench: the boarded cart's exit from P's cell 2 gt after its entry).
pub const LEAVE: i64 = 2;
/// The park's powered rail -> the cart's first move (1 gt: the rail's power is read in the cart's next tick).
pub const CART_RESPONSE: i64 = 1;
/// The napper's eject from its riding cart (58f770cc: ARM + 2..3) and the boarding cooldown (Entity.removePassenger:
/// boardingCooldown = 60, decremented by the villager's own base tick; tick-exact 17:39).
pub const EJECT: i64 = 3;
pub const BOARD_COOLDOWN: i64 = 60;
/// SleepInBed's LAST_WOKEN cooldown (game time) and the margin bar on it at BOARDING (ironspec 15:56 / G1 on
/// 5f79952c2 B1: wake + 100 - boarding >= 10).
pub const COOLDOWN: i64 = 100;
pub const COOLDOWN_BAR: i64 = 10;
/// The latest RLS rise a pulse's window allows, after the pulse + W_LINE: WIN's reset (4 to Q's block), its NOT-Q
/// output (4), the AND (4), less the first blocked tick, LESS RLS's MINIMUM WIDTH - 1: RLS's output torch lights only for
/// an OR-block gap of >= 3 gt (the structural RLS >= 3), so the last rise sits 2 gt before the last unblocked tick.
pub const RLS_MIN_W: i64 = 3;
pub const W_SLACK: i64 = RESET_IN + TORCH + REP + AND - 1 - (RLS_MIN_W - 1);
/// The bands the sweeps run (58f770cc's stop bench and its real-cell pass -> rest), and the world opens: the closed
/// form takes its maxima from these arrays (G1's B7: no hand-typed copies).
pub const RESTS: [i64; 3] = [33, 40, 46];
pub const BOARDS: [i64; 3] = [13, 16, 19];
pub const PASS_TO_REST: [i64; 2] = [16, 26];
pub const LIES: [i64; 2] = [4, 5];
pub const OPENS: [(i64, i64); 4] = [(1, 1), (2, 1), (1, 3), (2, 3)];
pub const BOARD_MAX: i64 = BOARDS[2];
pub const OPEN_IDLE: i64 = OPENS[3].0;
pub const OPEN_KJ: i64 = OPENS[3].1;
/// G1's N1 / C1: the cooldown edge's closed form, every term: the latest boarding after the wake = the pulse's rise
/// (OBS_RISE) + the latest RLS (W_LINE + W_SLACK) + PARK + the cart's read of the park (PARK_READ) + its first move
/// (CART_RESPONSE) + the slowest ride + a world open's idle + the entity domain's lag; the margin COOLDOWN - that >=
/// COOLDOWN_BAR, and the sweep's least margin must EQUAL it (a bound a run beats is not a bound).
pub const COOL_EDGE: i64 = OBS_RISE + W_LINE + W_SLACK + PARK + PARK_READ + CART_RESPONSE + BOARD_MAX + OPEN_IDLE + OPEN_KJ;
/// The same closed form for (g)'s fallback drive (W 46, PARK 4).
pub const COOL_EDGE_TORCH: i64 = OBS_RISE + W_TORCH + W_SLACK + PARK_TORCH + PARK_READ + CART_RESPONSE + BOARD_MAX + OPEN_IDLE + OPEN_KJ;
/// The tripwire's release bound after the napper's box leaves: a lie-down's string drops by its pulse + 8.
pub const STRING_DROP: i64 = RECHECK - OBS_RISE;
/// NEAREST_HOSTILE's clear after the shutter's first closed tick C (tick-exact 17:52: C + 19): the cart's first entry
/// into the hold cell - (C + 19) >= 1 after k + j on the convention, >= 3 off it (ironspec 17:53, the name "close + 27"
/// retired); this logic model's cart reaches P (58f770cc's bench measures the real (-2,0,5) entry: 20 at p = 2).
pub const HOSTILE_CLEAR: i64 = 19;
/// G1 17:40 (1): the window's first open tick - the lie-down >= the observer's merge window 4 + a margin 2.
pub const WINDOW_AFTER_LIE: i64 = 6;
/// G1 on 5f79952c2 B3: the shutter closes within this many gt of the wake's press.
pub const CLOSE_BAR: i64 = 8;

/// THE PARK DRIVE OF RECORD, (g) (ironspec 21:09 / 21:15 / 21:26 / 21:29 / 21:31 / 21:32; day-1 21:12 / 21:18 / 21:23;
/// 58f770cc 21:38 / 21:42's cells): P_HOLD a two-torch NOR latch WITH FEEDBACK under the slime (2,0,4): B2 (2,-4,4) a
/// solid block carrying Q-bar's torch (2,-3,4), whose output strongly powers the base (2,-2,4) carrying Q's torch
/// (2,-1,4), which strongly powers the slime and so the park rail: PARK = Q. Everything entering B2 is DUST (weak power:
/// B2 powers no neighbour, so nothing backfeeds RLS's network): (i) RLS's set dust (west face); (ii) the BRANCH, a
/// delay-1 repeater on RLS's line downstream of RLS's output torch, through its own dust (north face): a repeater
/// stretches any shorter input to its 2 gt, so B2 is covered without a gap for every RLS that rises; (iii) Q's feedback
/// (east face), the memory. RESET: a delay-1 repeater into the base, its input an OR of RLS through the 8-gt pair (two
/// setting-2 repeaters) and ARM's edge through its own delay-1 diode (the power-up reset: a latch that comes up SET is
/// cleared at the first visit's ARM edge, long before the cart rests). PARK = [RLS + 4, RLS + 12) (the reset dominates
/// at the base). THE VISIT'S CONSUMING RESETS (SENSED's, COOL's, SO's) key on Q through a delay-1 diode, the TAP, not on
/// RLS (ironspec 21:29: consistent by construction: a latch that fails to set for ANY reason consumes nothing, and RLS
/// can rise again). Simulated at the torch / repeater level with 26.3's tick order: within a block tick the diodes'
/// scheduled ticks (HIGH .. EXTREMELY_HIGH) run before the torches' (NORMAL), a torch acts on its block's power AT its
/// scheduled tick (2 gt after the change that scheduled it), a repeater's scheduled turn-on always happens (it then
/// schedules its turn-off if its input has fallen: the 2-gt stretch), dust is instant.
#[derive(Clone, Copy, Debug)]
pub struct LatchOpts {
    /// The branch repeater into B2 (the design; off: its control).
    pub branch: bool,
    /// Q's feedback into B2 (the latch's memory; off: a plain two-torch buffer).
    pub feedback: bool,
    /// ARM's edge into the reset (the power-up reset; off: its control).
    pub arm_reset: bool,
    /// A non-width latch failure (G1 21:28 (2)): B2's set inputs (RLS's dust and the branch's) blocked.
    pub b2_blocked: bool,
    /// The latch's power-up state: P_HOLD SET (Q lit) or RESET.
    pub power_up_set: bool,
    /// The reset line's stretch (gt; delay-4 repeaters).
    pub pair_w: i64,
}

impl Default for LatchOpts {
    fn default() -> Self {
        LatchOpts { branch: true, feedback: true, arm_reset: true, b2_blocked: false, power_up_set: false, pair_w: PARK_W }
    }
}

/// A repeater (delay in gt) at 26.3's rules: its scheduled turn-on always happens, then it schedules its turn-off if
/// its input has fallen (so its output lasts at least its delay).
#[derive(Clone, Copy, Debug, Default)]
struct Rep {
    on: bool,
    due: Option<i64>,
    delay: i64,
}

impl Rep {
    fn new(delay: i64) -> Rep {
        Rep { on: false, due: None, delay }
    }
    /// Its scheduled tick, if due now (the diode phase).
    fn tick(&mut self, k: i64, input: bool) {
        if self.due == Some(k) {
            self.due = None;
            if !self.on {
                self.on = true;
                if !input {
                    self.due = Some(k + self.delay);
                }
            } else if !input {
                self.on = false;
            }
        }
    }
    /// A neighbour change: schedule when the output disagrees with the input.
    fn update(&mut self, k: i64, input: bool) {
        if self.due.is_none() && self.on != input {
            self.due = Some(k + self.delay);
        }
    }
}

/// A torch at 26.3's rules: lit when its block is unpowered; a mismatch schedules its tick 2 gt later, where it acts on
/// its block's power at that moment.
#[derive(Clone, Copy, Debug, Default)]
struct Torch {
    lit: bool,
    due: Option<(i64, u64)>,
}

impl Torch {
    fn update(&mut self, k: i64, block_powered: bool, seq: &mut u64) {
        if self.due.is_none() && self.lit == block_powered {
            *seq += 1;
            self.due = Some((k + TORCH, *seq));
        }
    }
}

/// The latch, the branch, the reset path and the consumers' tap, one block tick at a time.
#[derive(Clone, Debug)]
pub struct LatchSim {
    pub opts: LatchOpts,
    rls: bool,
    br: Rep,
    chain: Vec<Rep>,
    armd: Rep,
    rst: Rep,
    tap: Rep,
    qbar: Torch,
    q: Torch,
    seq: u64,
}

/// One block tick's latch state (after the tick).
#[derive(Clone, Copy, Debug, Default)]
pub struct LatchOut {
    pub b2: bool,
    pub q: bool,
    pub tap: bool,
    /// The branch repeater's output (for RLS_S's control).
    pub br: bool,
    /// B2 read unpowered at a Q-bar check while Q-bar was dark (the check that would flip the latch back).
    pub dip_at_check: bool,
}

impl LatchSim {
    pub fn new(opts: LatchOpts) -> LatchSim {
        let (qbar, q) = if opts.power_up_set { (false, true) } else { (true, false) };
        LatchSim {
            opts,
            rls: false,
            br: Rep::new(REP),
            chain: vec![Rep::new(2 * REP); (opts.pair_w / (2 * REP)).max(1) as usize],
            armd: Rep::new(REP),
            rst: Rep::new(REP),
            tap: Rep { on: q, due: None, delay: REP },
            qbar: Torch { lit: qbar, due: None },
            q: Torch { lit: q, due: None },
            seq: 0,
        }
    }
    fn b2(&self) -> bool {
        (!self.opts.b2_blocked && (self.rls || (self.opts.branch && self.br.on))) || (self.opts.feedback && self.q.lit)
    }
    fn base(&self) -> bool {
        self.qbar.lit || self.rst.on
    }
    fn rst_in(&self) -> bool {
        self.chain.last().is_some_and(|r| r.on) || (self.opts.arm_reset && self.armd.on)
    }
    /// One block tick: `rls` is RLS's line after this tick's torch phase (RLS's output torch is a NORMAL-priority torch:
    /// its change lands after the diodes, and before the latch's own torches, the conservative order for a falling RLS),
    /// `arm_edge` ARM's edge comparator's output.
    pub fn step(&mut self, k: i64, rls: bool, arm_edge: bool) -> LatchOut {
        // (the diode phase: every repeater due now, on the inputs as they stand before this tick's torches)
        let rls_prev = self.rls;
        self.br.tick(k, rls_prev);
        let mut input = rls_prev;
        for r in self.chain.iter_mut() {
            r.tick(k, input);
            input = r.on;
        }
        self.armd.tick(k, arm_edge);
        let ri = self.rst_in();
        self.rst.tick(k, ri);
        let ql = self.q.lit;
        self.tap.tick(k, ql);
        // (the torch phase: RLS's output torch first, then the latch's torches in the order they were scheduled)
        self.rls = rls;
        let mut dip = false;
        let mut due: Vec<(u64, u8)> = Vec::new();
        if let Some((t, s)) = self.qbar.due {
            if t == k {
                due.push((s, 0));
            }
        }
        if let Some((t, s)) = self.q.due {
            if t == k {
                due.push((s, 1));
            }
        }
        due.sort();
        for (_, which) in due {
            if which == 0 {
                self.qbar.due = None;
                let p = self.b2();
                if !self.qbar.lit && !p {
                    dip = true;
                }
                self.qbar.lit = !p;
            } else {
                self.q.due = None;
                let p = self.base();
                self.q.lit = !p;
            }
        }
        // (neighbour updates: schedule every part whose state disagrees with its input)
        let (b2, base) = (self.b2(), self.base());
        self.qbar.update(k, b2, &mut self.seq);
        self.q.update(k, base, &mut self.seq);
        self.br.update(k, self.rls);
        let mut input = self.rls;
        for r in self.chain.iter_mut() {
            r.update(k, input);
            input = r.on;
        }
        self.armd.update(k, arm_edge);
        let ri = self.rst_in();
        self.rst.update(k, ri);
        let ql = self.q.lit;
        self.tap.update(k, ql);
        LatchOut { b2: self.b2(), q: self.q.lit, tap: self.tap.on, br: self.br.on, dip_at_check: dip }
    }
}

/// Planted defects, one per control (each a single change).
#[derive(Clone, Copy, Default, Debug)]
pub struct Defects {
    pub no_parked_in_rls: bool,
    pub no_cool: bool,
    pub no_sensed: bool,
    pub no_notp_in_open: bool,
    pub no_p_in_open_reset: bool,
    pub no_cool_reset: bool,
    pub no_so_reset: bool,
    pub no_sensed_arm_reset: bool,
    pub no_since: bool,
    pub no_onset: bool,
    /// The D line shortened by this many gt (repeater steps of 2).
    pub d_cut: i64,
    /// SINCE's old delay-2 repeater put back (the design before ironspec 18:12 (d)).
    pub since_delay_back: bool,
    /// The W line lengthened (+) or shortened (-) by this many gt (a repeater tick = 2).
    pub w_delta: i64,
    /// PARK without its pulse extender (RLS's own width).
    pub no_park_stretch: bool,
    /// PARK through the two-input OR (RLS and RLS through 8 gt): the gap G1's C5 names.
    pub park_or2: bool,
    /// The closed form without the park's read (G1's C1 control: the sweep then beats the bound).
    pub edge_no_read: bool,
    /// The onset's old 2-gt sample ([-1, +1]) instead of the 4-gt one ([-1, +3]): its control.
    pub onset_narrow: bool,
    /// The cart's launch delayed this many gt past the first window's bound (BoardsEarly's negative).
    pub late_launch: i64,
    /// The drive before (g): PARK = RLS OR P_HOLD through the (3,0,4) repeater (p = 2), the consumers on RLS (W 48's
    /// design; its sub-controls `no_park_stretch` / `park_or2` apply only with it).
    pub old_drive: bool,
    /// The visit's consuming resets (SENSED, COOL, SO) on RLS (through one repeater) instead of the latch's tap.
    pub resets_on_rls: bool,
    /// The consuming resets on RLS_S = RLS OR the branch (ironspec 21:26's form, withdrawn at 21:29): a control.
    pub resets_on_rls_s: bool,
    /// (g) without the branch repeater into B2.
    pub no_branch: bool,
    /// (g) without ARM's edge in the reset (the power-up reset).
    pub no_arm_reset: bool,
    /// A non-width latch failure: B2's set inputs blocked (G1 21:28 (2)).
    pub b2_blocked: bool,
    /// (g)'s latch drive, the fallback (ironspec 21:51: only if no face of the slime takes a repeater): PARK = the latch's
    /// Q torch under the slime, [RLS + 4, RLS + 12) (run it with W 46: `w_delta: W_TORCH - W_LINE`).
    pub torch: bool,
    /// RLS WITHOUT its output torch's scheduled-read filter (a 1- or 2-gt OR-block gap lights it): G1's 23:xx (7) twin
    /// (tick-exact's AN rig, assert 5, shows a 2-gt pulse lighting Q: the filter is MODELLED until the AND rig on real
    /// cells says otherwise).
    pub rls_filter_off: bool,
    /// SINCE's reset back to the LEVEL reset by NOT P (before ironspec 22:52 (1) gated it by the sample): its control.
    pub since_reset_level: bool,
    /// The reset chain's width (P_HOLD's PARK_W) overridden (0: the design's): G1's leave-park control (PARK_W 8).
    pub park_w_override: i64,
}

/// What a row asserts about its outcome (G1 on 5f79952c2 B6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One RLS, the napper aboard, every normal-visit bar.
    Normal,
    /// An early wake that boards on its first window: one RLS, aboard, the invariants (G1 on a2186649e B6 (1c)).
    BoardsEarly,
    /// Boards on its first window where the band's window allows, after its re-sleep otherwise (G1's C2: the merged
    /// rows whose release lands inside or past W by the band); the split is printed.
    Boards,
    /// No release to a sleeper, then the napper boards within the horizon (after a re-sleep).
    Recovers,
    /// An EMPTY departure (the permanent halt the fault note names).
    PermanentHalt,
    /// No departure until the scripted later lie-down, then boarding after it.
    SelfEnding,
    /// The invariants only, the outcome printed (the false-SO path).
    Printed,
}

/// One scripted visit. Times are entity ticks (ARM at `arm`).
#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub arm: i64,
    /// The cart at rest on the park.
    pub rest: i64,
    /// The park's release (PARK's rise at rest) to the cart at P.
    pub board: i64,
    /// Scripted lie-downs (from wherever the napper stands: from P when it stands on P).
    pub lie: Vec<i64>,
    /// The napper stands on P before its first lie-down (the never-slept napper; no eject this visit).
    pub start_on_p: bool,
    pub hurt: Vec<i64>,
    pub dawn: Option<i64>,
    /// Z1 in place: a sleeper wakes once the window has been open `scan` gt.
    pub z1: bool,
    pub scan: i64,
    /// The awake napper off P over [a, b) (back on P at b: a P cross).
    pub off: Vec<(i64, i64)>,
    /// Planted boxes on the string over [a, b).
    pub planted: Vec<(i64, i64)>,
    /// Extra presses of the park-entry detector (a stray after RLS).
    pub park_stray: Vec<i64>,
    /// The cart's pass of the park-entry detector -> its rest (58f770cc 18:35: 16..26).
    pub pass_to_rest: i64,
    /// Non-sleep head changes.
    pub head_only: Vec<i64>,
    /// The napper is moved off P for good this long after RLS's rise (a forced displacement: the step-off).
    pub step_off: Option<i64>,
    /// A calm awake napper lies down again this long after its wake (>= the cooldown, from where it stands), unless off
    /// P with the window open (Z1 in view). Default 110: the composed nights' re-sleep after a wake (tick-exact's
    /// 110..133 after a damage wake; the cooldown 100 is its floor).
    pub resleep: Option<i64>,
    /// A world open (game tick, idle, the entity domain's extra lag kj).
    pub open: Option<(i64, i64, i64)>,
    /// The next visit's ARM: its edge resets the latches, read 12 gt after it.
    pub next_arm: i64,
    pub horizon: i64,
    /// (SelfEnding rows) the scripted later lie-down the boarding must follow (entity tick).
    pub self_end_lie: Option<i64>,
    /// The park latch's power-up state: P_HOLD SET (Q lit, the park powered) at the run's start.
    pub power_up_set: bool,
    /// The first RLS forced low this long after its rise, for 30 gt (an input dropping: RLS = 1 / 2 rows).
    pub rls_force_w: Option<i64>,
    /// The first RLS forced high for this long from its rise (the long-RLS row).
    pub rls_hold: Option<i64>,
}

impl Row {
    pub fn normal(name: &str, lie: i64, scan: i64, rest: i64, board: i64) -> Row {
        Row {
            name: name.to_string(),
            arm: 0,
            rest,
            board,
            lie: vec![lie],
            start_on_p: false,
            hurt: vec![],
            dawn: None,
            z1: true,
            scan,
            off: vec![],
            planted: vec![],
            park_stray: vec![],
            pass_to_rest: PARK_ENTRY_BEFORE,
            head_only: vec![],
            step_off: None,
            resleep: Some(110),
            open: None,
            next_arm: 600,
            horizon: 700,
            self_end_lie: None,
            power_up_set: false,
            rls_force_w: None,
            rls_hold: None,
        }
    }

    pub fn long(mut self, next: i64) -> Row {
        self.next_arm = next;
        self.horizon = next + 100;
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum N {
    Riding,
    Bed,
    OnP,
    Off,
    Boarded,
    Gone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum C {
    Arriving,
    Rest,
    ToP(i64),
    AtP(i64),
    Departed,
}

/// What one run printed and measured (game ticks).
#[derive(Clone, Debug, Default)]
pub struct Out {
    pub rls: Vec<i64>,
    pub park: Vec<i64>,
    pub rls_asleep: Vec<i64>,
    pub park_before_rest: Vec<i64>,
    pub departed: Option<(i64, bool)>,
    pub boarded_at: Option<i64>,
    pub wakes: Vec<i64>,
    pub lies: Vec<i64>,
    pub pulses: Vec<i64>,
    pub open_set: Vec<i64>,
    /// OPEN set after RLS within the visit (before the next ARM).
    pub open_after_rls: Vec<i64>,
    /// Per pulse with P pressed: the press's start - the head change (game ticks; 0 = the press began in the change's
    /// tick, the woken napper's case).
    pub press_minus_change: Vec<(i64, i64)>,
    /// SINCE's rises.
    pub since_rises: Vec<i64>,
    /// OPEN set while the napper was awake off P / standing on P.
    pub open_awake: Vec<i64>,
    pub open_on_p: Vec<i64>,
    /// Ticks an awake napper off P stood with the window open (Z1 in view).
    pub exposed: i64,
    /// The longest run of ticks the awake napper stood ON P with the window open (a hurt-woken press racing OPEN's set
    /// read flashes the window for at most OPEN's reset latency; longer is a design failure).
    pub exposed_on_p: i64,
    pub window: Vec<(i64, i64)>,
    /// (SO, SENSED, COOL, OPEN, SINCE, WIN) 12 gt after the next ARM's edge.
    pub latches_at_next: [bool; 6],
    /// P_STRING's falls between the boarding wake and the departure.
    pub p_falls_boarding: u32,
    /// The latest wake (game tick) before boarding + the cooldown - boarding (game ticks).
    pub cooldown_margin: Option<i64>,
    /// RLS's rise - the latest pulse before it.
    pub rls_after_pulse: Vec<i64>,
    /// The napper's cell per tick from the boarding wake to the departure: P on P, B aboard, x off P, b in bed.
    pub cells: String,
    /// PARKED (SENSED AND COOL) rose again after RLS within the visit (a second release possible).
    pub parked_after_rls: bool,
    /// The cart's arrival at P (its first entry into the hold cell).
    pub cart_at_p: Option<i64>,
    /// The game tick the napper may board from (its boarding cooldown's end).
    pub may_board_from: Option<i64>,
    /// Per wake press: the shutter's close - the press.
    pub close_after_press: Vec<i64>,
    /// The window's first open tick - the visit's first lie-down.
    pub window_after_lie: Option<i64>,
    /// The close bar for this run: CLOSE_BAR + a world open's idle (the block domain frozen between the press and the
    /// close delays the close by that much).
    pub close_allow: i64,
    /// Presses on P made while the window was open that the window never closed after (G1's N5).
    pub unclosed_presses: Vec<i64>,
    /// The first window's boarding bound (G1's C2): the first RLS + PARK + PARK_READ + CART_RESPONSE + the row's ride
    /// + a world open's idle and lag when one falls in between.
    pub first_window_by: Option<i64>,
    /// (SelfEnding rows) the game tick of the scripted later lie-down.
    pub self_end_lie: Option<i64>,
    /// The visit's consuming resets: each rise of the consumers' reset input (game ticks).
    pub consumed: Vec<i64>,
    /// The cart at rest on the park (game tick), and its first move off it.
    pub rest_g: Option<i64>,
    pub launched: Option<i64>,
    /// Each PARK pulse's end (game ticks).
    pub park_end: Vec<i64>,
    /// Each RLS pulse's width (block ticks).
    pub rls_widths: Vec<i64>,
    /// Ticks the latch's Q-bar check read B2 unpowered while Q-bar was dark (a dip that flips the latch back).
    pub latch_dips: Vec<i64>,
    /// The least gap from an ARM edge to P_HOLD's stretch (Q lit), game ticks, after the first park.
    pub arm_to_stretch: Option<i64>,
}

fn at(v: &[bool], t: i64, d: bool) -> bool {
    if t < 0 {
        d
    } else {
        v[t as usize]
    }
}

/// One run of a row with the planted defects (none for the design).
pub fn run(row: &Row, def: Defects) -> Out {
    let (t_open, idle, kj) = row.open.unwrap_or((i64::MAX, 0, 0));
    let t_max = row.horizon.max(row.next_arm + 30) + idle + kj + 1;
    let n = t_max as usize;
    let mut o = Out { close_allow: CLOSE_BAR + idle, ..Out::default() };
    // entity-domain inputs, per game tick
    let mut bed_g = vec![false; n];
    // block-domain histories, indexed by the block tick kb (a frozen block tick does not exist in this time)
    let mut arm = vec![false; n];
    let mut pke = vec![false; n];
    let mut ps = vec![false; n];
    let mut praw = vec![false; n];
    let mut obs = vec![false; n];
    let mut rls = vec![false; n];
    let mut park = vec![false; n];
    let mut p_hold = vec![false; n];
    // ((g)'s latch, or the plain drive's P_HOLD with the carried rulings; the old drive keeps its gate model)
    let mut lsim = LatchSim::new(LatchOpts { branch: !def.no_branch, feedback: true, arm_reset: !def.no_arm_reset, b2_blocked: def.b2_blocked, power_up_set: row.power_up_set, pair_w: if def.park_w_override > 0 { def.park_w_override } else if def.torch { PARK_W_TORCH } else { PARK_W } });
    let mut tap = vec![false; n];
    let mut brh = vec![false; n];
    let mut park_in = vec![false; n];
    let mut rls_first_kb: Option<i64> = None;
    let (mut rls_lit, mut rls_due): (bool, Option<i64>) = (false, None);
    let mut rls_rise_kb: i64 = 0;
    let mut cons_prev = false;
    let (mut q_so, mut q_sen, mut q_cool, mut q_open, mut q_since, mut q_win) = (vec![false; n], vec![false; n], vec![false; n], vec![false; n], vec![false; n], vec![false; n]);
    let mut kb: i64 = -1;
    let mut te: i64 = -1;
    let mut ob: Option<(bool, i64)> = None;
    let (mut pressed, mut next_check, mut press_tick_g) = (false, 0i64, 0i64);
    let mut nap = if row.start_on_p { N::OnP } else { N::Riding };
    let mut cart = C::Arriving;
    let mut last_wake: Option<(i64, i64)> = None;
    let mut win_open_for = 0i64;
    let mut boarding_wake: Option<i64> = None;
    let mut stepped = false;
    let mut resleep_done_for: Option<i64> = None;
    let mut board_cd: i64 = if row.start_on_p { 0 } else { -1 };
    let mut cells = String::new();
    let mut shut_prev = true;
    let mut pending_press_close: Vec<i64> = Vec::new();
    let mut on_p_run = 0i64;
    let mut pulses_kb: Vec<i64> = Vec::new();
    for g in 0..t_max {
        let gi = g as usize;
        let blk = !(g >= t_open && g < t_open + idle);
        let ent = !(g >= t_open && g < t_open + idle + kj);
        if g == t_open + idle {
            // (a world open clears the boarding cooldown: not stored in NBT, tick-exact 17:39)
            if board_cd > 0 {
                board_cd = 0;
            }
        }
        if !blk {
            continue;
        }
        kb += 1;
        let k = kb as usize;
        // carry the entity inputs into this block tick (updated below if the entities run)
        if k > 0 {
            arm[k] = arm[k - 1];
            praw[k] = praw[k - 1];
            pke[k] = pke[k - 1];
        }
        // ---- block ticks first (due in block time): the observer's scheduled tick, the tripwire's re-check
        let prev = |v: &Vec<bool>| k > 0 && v[k - 1];
        let mut obs_now = prev(&obs);
        match ob {
            Some((true, due)) if due <= kb => {
                obs_now = true;
                ob = Some((false, kb + OBS_W));
            }
            Some((false, due)) if due <= kb => {
                obs_now = false;
                ob = None;
            }
            _ => {}
        }
        if pressed && kb >= next_check {
            if prev(&praw) {
                next_check = kb + RECHECK;
            } else {
                pressed = false;
            }
        }
        // ---- entities (entity time te)
        let mut head_changes = 0;
        let win = at(&q_open, kb - 1 - TORCH, false);
        if ent {
            te += 1;
            let e = te;
            if board_cd > 0 {
                board_cd -= 1;
            }
            // (G1's C1 on 33e731660: `edge_no_read` removes the park's READ itself: the cart responds in the tick PARK rises
            // (its launch dated PARK_READ earlier, the ride unchanged); the entity phase runs before this tick's blocks, so
            // the rise is seen PARK_READ later and the launch is back-dated)
            let park_prev = at(&park, kb - PARK_READ, false);
            match cart {
                C::Arriving if e >= row.arm + row.rest => {
                    cart = C::Rest;
                    o.rest_g = Some(g);
                }
                C::Rest if park_prev => {
                    let back = if def.edge_no_read { PARK_READ } else { 0 };
                    cart = C::ToP(e - back + CART_RESPONSE + row.board + def.late_launch);
                    o.launched = Some(g - back);
                }
                C::ToP(t) if e >= t => {
                    o.cart_at_p = Some(g);
                    if nap == N::OnP && board_cd <= 0 {
                        nap = N::Boarded;
                        o.boarded_at = Some(g);
                        cart = C::AtP(e + LEAVE);
                    } else {
                        cart = C::Departed;
                        o.departed = Some((g, false));
                    }
                }
                C::AtP(t) if e >= t => {
                    cart = C::Departed;
                    o.departed = Some((g, true));
                    nap = N::Gone;
                }
                _ => {}
            }
            if park_prev && cart == C::Arriving && !(k >= 2 && park[k - 2]) {
                o.park_before_rest.push(g);
            }
            if e == row.arm + EJECT && nap == N::Riding {
                board_cd = BOARD_COOLDOWN;
                o.may_board_from = Some(g + BOARD_COOLDOWN);
            }
            for &(a, b) in &row.off {
                if e == a && nap == N::OnP {
                    nap = N::Off;
                }
                if e == b && nap == N::Off {
                    nap = N::OnP;
                }
            }
            if let (Some(kk), Some(&r0)) = (row.step_off, o.rls.first()) {
                if !stepped && g >= r0 + kk && nap == N::OnP {
                    nap = N::Off;
                    stepped = true;
                }
            }
            let calm = matches!(nap, N::OnP | N::Off)
                && !(nap == N::Off && win)
                && match (row.resleep, last_wake) {
                    (Some(r), Some((we, wg))) => e - we >= r && g - wg >= COOLDOWN && resleep_done_for != Some(we),
                    _ => false,
                };
            if (row.lie.contains(&e) && matches!(nap, N::Riding | N::OnP | N::Off)) || calm {
                if calm {
                    resleep_done_for = last_wake.map(|w| w.0);
                }
                nap = N::Bed;
                o.lies.push(g);
                if row.self_end_lie == Some(e) {
                    o.self_end_lie = Some(g);
                }
                head_changes += 1;
            }
            if nap == N::Bed {
                win_open_for = if win { win_open_for + 1 } else { 0 };
                let z1_wake = row.z1 && win && win_open_for >= row.scan;
                if row.hurt.contains(&e) || row.dawn == Some(e) || z1_wake {
                    nap = N::OnP;
                    last_wake = Some((e, g));
                    o.wakes.push(g);
                    head_changes += 1;
                }
            } else {
                win_open_for = 0;
            }
            if nap == N::OnP && win {
                on_p_run += 1;
                o.exposed_on_p = o.exposed_on_p.max(on_p_run);
            } else {
                on_p_run = 0;
            }
            if nap == N::Off && win {
                o.exposed += 1;
            }
            praw[k] = nap == N::OnP || (nap == N::Boarded && matches!(cart, C::AtP(_))) || row.planted.iter().any(|&(a, b)| e >= a && e < b);
            arm[k] = (e >= row.arm && e < row.arm + ARM_W) || (e >= row.next_arm && e < row.next_arm + ARM_W);
            let pe = row.arm + row.rest - row.pass_to_rest;
            pke[k] = (e >= pe && e < pe + ARM_W) || row.park_stray.iter().any(|&s| e >= s && e < s + ARM_W);
            if row.head_only.contains(&e) {
                head_changes += 1;
            }
            if !pressed && praw[k] {
                pressed = true;
                press_tick_g = g;
                next_check = kb + RECHECK;
                // (G1's N5: every press while the window is open, the wake's or any other, must close it)
                if win || (!o.wakes.is_empty() && o.wakes.last() == Some(&g)) {
                    pending_press_close.push(g);
                }
            }
            if head_changes > 0 && ob.is_none() {
                ob = Some((true, kb + OBS_RISE));
                o.pulses.push(g + OBS_RISE);
                pulses_kb.push(kb + OBS_RISE);
                if pressed {
                    o.press_minus_change.push((g, press_tick_g - g));
                }
            }
        }
        let _ = press_tick_g;
        bed_g[gi] = nap == N::Bed;
        ps[k] = pressed;
        obs[k] = obs_now;
        // ---- redstone (block time)
        let p_at = |t: i64| at(&ps, t, false);
        let notp = |t: i64| !p_at(t - TORCH);
        let nqv = |v: &Vec<bool>, t: i64| !at(v, t - TORCH - REP, false);
        let obs_at = |t: i64| at(&obs, t, false);
        let rls_at = |t: i64| at(&rls, t, false);
        let edge = |v: &Vec<bool>, t: i64, l: i64| at(v, t - CMP - l, false) && !at(v, t - CMP - l - EDGE_W, false);
        let latch = |q: bool, s: bool, r: bool| if s { true } else if r { false } else { q };
        let s_at = |f: &dyn Fn(i64) -> bool| f(kb - SET_IN);
        let r_at = |f: &dyn Fn(i64) -> bool| f(kb - RESET_IN);
        let rls_r = |t: i64| rls_at(t - REP);
        let arm_r = |t: i64| edge(&arm, t, 0);
        // (the visit's consuming resets: the latch's tap (Q through a delay-1 diode) by design; RLS through one repeater
        // in the old drive and its control; RLS_S (RLS OR the branch) in ironspec 21:26's withdrawn form)
        let cons_r = |t: i64| {
            if def.old_drive || def.resets_on_rls {
                rls_r(t)
            } else if def.resets_on_rls_s {
                rls_r(t) || at(&brh, t - REP, false)
            } else {
                at(&tap, t, false)
            }
        };
        // (SO's reset: RLS through one repeater OR the latch's tap: SO feeds OPEN only, so its early reset at RLS + 2
        // keeps a step-off from re-opening the window without consuming anything RLS reads (G1 21:28 (1)))
        let v_so = latch(prev(&q_so), s_at(&|t| obs_at(t)), r_at(&|t| !def.no_so_reset && (rls_r(t) || cons_r(t))));
        let v_sen = latch(prev(&q_sen), s_at(&|t| at(&pke, t - SENSE_D, false)), r_at(&|t| cons_r(t) || (!def.no_sensed_arm_reset && arm_r(t))));
        let v_cool = latch(prev(&q_cool), s_at(&|t| edge(&arm, t, COOL_LINE)), r_at(&|t| !def.no_cool_reset && (cons_r(t) || arm_r(t))));
        q_so[k] = v_so;
        q_sen[k] = v_sen;
        q_cool[k] = v_cool;
        let sd = CMP + if def.since_delay_back { OLD_SINCE_DELAY } else { SINCE_DELAY };
        // (THE ONSET WINDOW [-1, +3] (ironspec 22:15, day-1 22:12): SINCE's sample is the observer's pulse OR the pulse
        // through one delay-1 repeater (4 gt), so a press up to 3 gt after the head change (the merged lie-down + hurt
        // within 3 gt) still sets SINCE, while a press 2 gt before it (G1's B2 edge) never does; `onset_narrow` puts back
        // the 2-gt sample ([-1, +1]) as its control)
        let obs_s = |u: i64| obs_at(u) || (!def.onset_narrow && obs_at(u - REP));
        let since_s = |t: i64| {
            let u = t - sd;
            // the pulse, P at its head change, and the onset (P rose within ONSET gt before the change)
            let onset = def.no_onset || !at(&ps, u - TORCH - ONSET - 1, false);
            obs_s(u) && !notp(u) && onset
        };
        // (SINCE's reset, ironspec 22:52 (1): NOT P through one repeater, GATED BY THE SAMPLE: suppressed while a sample
        // stands within the set path's time + 2 (so a press arriving inside the onset window sets SINCE and keeps it);
        // `since_reset_level` puts back the plain level reset as its control)
        let since_r = |t: i64| notp(t - REP) && (def.since_reset_level || !(0..=sd + 2).any(|j| obs_s(t - j)));
        let v_since = !def.no_since && latch(prev(&q_since), s_at(&since_s), r_at(&since_r));
        let d_eff = D_LINE - def.d_cut;
        let w_eff = W_LINE + def.w_delta;
        let v_win = latch(prev(&q_win), s_at(&|t| obs_at(t - d_eff)), r_at(&|t| obs_at(t) || obs_at(t - w_eff)));
        let parked_block = |t: i64| (!def.no_sensed && nqv(&q_sen, t)) || (!def.no_cool && nqv(&q_cool, t));
        let open_s = |t: i64| !(parked_block(t - AND) || nqv(&q_so, t - AND) || (!def.no_notp_in_open && p_at(t - AND)));
        let v_open = latch(prev(&q_open), s_at(&open_s), r_at(&|t| rls_r(t) || (!def.no_p_in_open_reset && p_at(t - REP))));
        q_since[k] = v_since;
        q_win[k] = v_win;
        q_open[k] = v_open;
        let rls_block_at = |t: i64| notp(t) || (!def.no_since && nqv(&q_since, t)) || nqv(&q_win, t) || (!def.no_parked_in_rls && parked_block(t));
        // (RLS's output torch reads its OR block at its own scheduled tick (the block fed by the AND's input repeaters,
        // whose changes land before the torches): an OR-block gap of 1 or 2 gt never lights it, a gap of u >= 3 gives an
        // RLS of u, at the same ticks as the plain delay model: THE STRUCTURAL RLS >= 3)
        let or_now = rls_block_at(kb - AND + TORCH);
        if def.rls_filter_off {
            // (the twin: the plain delay model, no width filter: any OR-block gap lights RLS)
            rls_lit = !rls_block_at(kb - AND);
        } else {
            if rls_due == Some(kb) {
                rls_due = None;
                rls_lit = !or_now;
            }
            if rls_due.is_none() && rls_lit == or_now {
                rls_due = Some(kb + TORCH);
            }
        }
        rls[k] = rls_lit;
        // (planted RLS shapes: the first RLS cut to a width, or held high: the RLS = 1 / 2 and long-RLS rows)
        if rls[k] && rls_first_kb.is_none() && (k == 0 || !rls[k - 1]) {
            rls_first_kb = Some(kb);
        }
        if let Some(r0) = rls_first_kb {
            if let Some(w) = row.rls_force_w {
                if kb >= r0 + w && kb < r0 + w + 30 {
                    rls[k] = false;
                }
            }
            if let Some(h) = row.rls_hold {
                if kb >= r0 && kb < r0 + h {
                    rls[k] = true;
                }
            }
        }
        // (the park's drive: the plain drive of record (PARK = RLS OR P_HOLD through a delay-1 repeater, P_HOLD the
        // feedback latch with the carried rulings); (g)'s latch Q under the slime (the fallback); the old drive's gate
        // model (no carried rulings) with its two controls)
        let arm_edge_now = edge(&arm, kb, 0);
        if def.old_drive {
            p_hold[k] = latch(k > 0 && p_hold[k - 1], at(&rls, kb - SET_IN, false), at(&rls, kb - OLD_PARK_W - RESET_IN, false));
            park[k] = if def.no_park_stretch {
                at(&rls, kb - PARK, false)
            } else if def.park_or2 {
                at(&rls, kb - PARK, false) || at(&rls, kb - PARK - OLD_PARK_W, false)
            } else {
                at(&rls, kb - PARK, false) || at(&p_hold, kb - PARK, false)
            };
        } else {
            let lo = lsim.step(kb, rls[k], arm_edge_now);
            tap[k] = lo.tap;
            brh[k] = lo.br;
            if lo.dip_at_check {
                o.latch_dips.push(g);
            }
            if def.torch {
                park[k] = lo.q;
            } else {
                // (the plain drive: PARK's OR is B2 itself (RLS's dust, the branch's dust and Q's feedback dust all point
                // into it: a block ORs its inputs without joining their networks), read by the drive's delay-1 repeater
                // facing away from it: PARK = B2 + 2, gapless from RLS's rise to Q's reset)
                park_in[k] = lo.b2;
                park[k] = at(&park_in, kb - PARK, false);
            }
        }
        // (the consumers' reset input as it stands this tick; its rises after the first RLS are the visit's consumptions)
        let cons_now = if def.old_drive || def.resets_on_rls {
            at(&rls, kb - REP, false)
        } else if def.resets_on_rls_s {
            at(&rls, kb - REP, false) || at(&brh, kb - REP, false)
        } else {
            tap[k]
        };
        if cons_now && !cons_prev && !o.rls.is_empty() {
            o.consumed.push(g);
        }
        cons_prev = cons_now;
        let shut_now = !at(&q_open, kb - TORCH, false);
        // ---- records (game ticks)
        let rise = |v: &Vec<bool>| v[k] && (k == 0 || !v[k - 1]);
        if rise(&q_since) {
            o.since_rises.push(g);
        }
        if rise(&rls) {
            o.rls.push(g);
            rls_rise_kb = kb;
            if bed_g[gi] {
                o.rls_asleep.push(g);
            }
            // (in block time: W bounds the redstone's own window, a world open's frozen ticks excluded)
            if let Some(&pl) = pulses_kb.iter().filter(|&&x| x <= kb).last() {
                o.rls_after_pulse.push(kb - pl);
            }
        }
        if rise(&park) {
            o.park.push(g);
        }
        if k > 0 && rls[k - 1] && !rls[k] {
            o.rls_widths.push(kb - rls_rise_kb);
        }
        if k > 0 && park[k - 1] && !park[k] {
            o.park_end.push(g);
        }
        if arm_edge_now && !(k > 0 && edge(&arm, kb - 1, 0)) {
            if let Some(&pe) = o.park_end.last() {
                let gap = g - pe;
                o.arm_to_stretch = Some(o.arm_to_stretch.map_or(gap, |x: i64| x.min(gap)));
            }
        }
        // (PARKED as the logic sees it: a defect that removes SENSED or COOL from the gate removes it here too: G1's C3)
        let pk = |i: usize| (def.no_sensed || q_sen[i]) && (def.no_cool || q_cool[i]);
        let pk_now = pk(k);
        let pk_prev = k > 0 && pk(k - 1);
        if pk_now && !pk_prev && !o.rls.is_empty() && g < row.next_arm {
            o.parked_after_rls = true;
        }
        if rise(&q_open) {
            o.open_set.push(g);
            if !o.rls.is_empty() && g < row.next_arm {
                o.open_after_rls.push(g);
            }
            match nap {
                N::OnP => o.open_on_p.push(g),
                N::Bed => {}
                _ => o.open_awake.push(g),
            }
        }
        if !shut_now && shut_prev {
            o.window.push((g, i64::MAX));
            if o.window_after_lie.is_none() {
                if let Some(&l) = o.lies.first() {
                    o.window_after_lie = Some(g - l);
                }
            }
        }
        if shut_now && !shut_prev {
            if let Some(w) = o.window.last_mut() {
                w.1 = g;
            }
            for pr in pending_press_close.drain(..) {
                o.close_after_press.push(g - pr);
            }
        }
        if shut_now && !pending_press_close.is_empty() && o.window.last().map(|w| w.1 != i64::MAX).unwrap_or(true) {
            // (pressed while the window was already shut: nothing to close)
            pending_press_close.clear();
        }
        shut_prev = shut_now;
        if o.departed.is_none() && nap == N::OnP && !o.wakes.is_empty() && boarding_wake.map(|b| b < *o.wakes.last().unwrap()).unwrap_or(true) {
            boarding_wake = Some(g);
            o.p_falls_boarding = 0;
            cells.clear();
        }
        if let Some(b) = boarding_wake {
            if o.departed.is_none() && g >= b {
                cells.push(match nap {
                    N::OnP => 'P',
                    N::Boarded => 'B',
                    N::Bed => 'b',
                    _ => 'x',
                });
                if g > b && k > 0 && ps[k - 1] && !ps[k] {
                    o.p_falls_boarding += 1;
                }
            }
        }
        if g == row.next_arm + idle + kj + 12 {
            o.latches_at_next = [q_so[k], q_sen[k], q_cool[k], q_open[k], q_since[k], q_win[k]];
        }
    }
    // (G1's C2: the first window's boarding bound, in game time: an open after the first RLS adds its idle and lag)
    if let Some(&r0) = o.rls.first() {
        let pk = if def.torch { PARK_TORCH } else { PARK };
        let open_add = row.open.map_or(0, |(t0, idle, kj)| if t0 >= r0 - 4 && t0 <= r0 + pk + PARK_READ + CART_RESPONSE + row.board { idle + kj } else { 0 });
        o.first_window_by = Some(r0 + pk + PARK_READ + CART_RESPONSE + row.board + open_add);
    }
    // (G1's N5: a press on P while the window is open whose close never comes)
    o.unclosed_presses = pending_press_close.clone();
    o.cells = cells;
    if let Some(b) = o.boarded_at {
        if let Some(&w) = o.wakes.iter().filter(|&&w| w <= b).last() {
            o.cooldown_margin = Some(w + COOLDOWN - b);
        }
    }
    o
}

fn fmt_ticks(v: &[i64]) -> String {
    if v.is_empty() {
        "none".into()
    } else {
        v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
    }
}

/// One line on a run: its events in game ticks.
pub fn describe(o: &Out) -> String {
    let dep = match o.departed {
        Some((t, true)) => format!("BOARDED at {} (departed {t})", o.boarded_at.unwrap_or(t)),
        Some((t, false)) => format!("departed EMPTY at {t} (rider-keyed RS_ready never sets: a PERMANENT halt)"),
        None => "no departure in the horizon (the stop holds: napping halted, the next napper at R1)".into(),
    };
    format!(
        "lie-downs {}, wakes {}, pulses {}, OPEN set {}, RLS {}, PARK {}, window {}, {}{}{}",
        fmt_ticks(&o.lies),
        fmt_ticks(&o.wakes),
        fmt_ticks(&o.pulses),
        fmt_ticks(&o.open_set),
        fmt_ticks(&o.rls),
        fmt_ticks(&o.park),
        if o.window.is_empty() { "never open".to_string() } else { o.window.iter().map(|(a, b)| if *b == i64::MAX { format!("{a}..") } else { format!("{a}..{b}") }).collect::<Vec<_>>().join(",") },
        dep,
        o.cooldown_margin.map(|m| format!(", the cooldown margin at boarding {m}")).unwrap_or_default(),
        if o.exposed > 0 { format!(", an awake napper off P in Z1's view for {} gt", o.exposed) } else { String::new() }
    )
}

/// The invariants every row keeps: no release to a sleeper, no park before the cart's rest, the cooldown margin at
/// boarding, RLS within W of its pulse, the cart at P only after the napper may board, the shutter closing within 8 of
/// the wake's press, SENSED / COOL never rising after RLS within a visit.
fn invariants(o: &Out) -> Vec<String> {
    let mut bad = Vec::new();
    if !o.rls_asleep.is_empty() {
        bad.push(format!("RLS while the napper sleeps at {}", fmt_ticks(&o.rls_asleep)));
    }
    if !o.park_before_rest.is_empty() {
        bad.push(format!("the park released before the cart's rest at {}", fmt_ticks(&o.park_before_rest)));
    }
    if let Some(m) = o.cooldown_margin {
        if m < COOLDOWN_BAR {
            bad.push(format!("the cooldown margin at boarding {m} < {COOLDOWN_BAR}"));
        }
    }
    if o.rls_after_pulse.iter().any(|&x| x > W_LINE + W_SLACK) {
        bad.push(format!("RLS {:?} gt after its pulse, past W {W_LINE} + {W_SLACK}", o.rls_after_pulse));
    }
    if let (Some(ap), Some(mb)) = (o.cart_at_p, o.may_board_from) {
        if o.departed.map(|d| d.1).unwrap_or(false) && ap < mb + 2 {
            bad.push(format!("the cart at P {ap} before the napper may board + 2 ({mb})"));
        }
        if o.departed.map(|d| !d.1).unwrap_or(false) && ap < mb {
            bad.push(format!("the cart at P {ap} inside the napper's boarding cooldown (to {mb}): it passes empty"));
        }
    }
    if o.close_after_press.iter().any(|&c| c > o.close_allow) || !o.unclosed_presses.is_empty() {
        if !o.unclosed_presses.is_empty() {
            bad.push(format!("a press on P while the window was open never closed it (presses at {:?})", o.unclosed_presses));
        }
        bad.push(format!("the shutter closed {:?} gt after a press on P (bar <= {CLOSE_BAR} + a world open's idle = {})", o.close_after_press, o.close_allow));
    }
    if o.parked_after_rls {
        bad.push("PARKED (SENSED AND COOL) rose again after RLS within the visit".into());
    }
    if !o.open_after_rls.is_empty() {
        bad.push(format!("the window re-opened after RLS within the visit at {}", fmt_ticks(&o.open_after_rls)));
    }
    if o.exposed_on_p > CLOSE_BAR + 1 {
        bad.push(format!("the window stood open on the napper standing on P for {} gt (> the close bar {CLOSE_BAR} + the press's own tick: longer than OPEN's reset on P takes)", o.exposed_on_p));
    }
    // (THE CONSISTENCY BAR, both ways (ironspec 21:24 / 21:29 / 21:31): every consumption of the visit follows a PARK
    // that launched the cart, and every PARK consumes the visit)
    for &c in &o.consumed {
        let fired = o.park.iter().any(|&p| p <= c && c <= p + 8);
        let launched = o.launched.is_some_and(|l| o.park.iter().any(|&p| p <= c && c <= p + 8 && l >= p && l <= p + 12));
        if !fired || !launched {
            bad.push(format!("the visit consumed at {c} without a PARK that launched the cart (PARK {}, launched {:?})", fmt_ticks(&o.park), o.launched));
        }
    }
    if !o.rls.is_empty() {
        for &p in &o.park {
            if !o.consumed.iter().any(|&c| c >= p && c <= p + 8) {
                bad.push(format!("a PARK at {p} that consumed nothing (the consuming resets {})", fmt_ticks(&o.consumed)));
            }
        }
    }
    // (G1 21:20 (3), a judged bar: the cart at rest on the park by PARK's rise, every visit)
    if let (Some(&p), Some(rg)) = (o.park.first(), o.rest_g) {
        if !o.rls.is_empty() && rg > p {
            bad.push(format!("the cart came to rest at {rg}, after PARK's rise at {p}"));
        }
    }
    // (ironspec 21:20 (b): the cart's last tick on the park cell >= 2 gt before PARK's end; G1 on 33e731660 (9): a PARK
    // that rose and never launched the cart fails too)
    if let (Some(l), Some(&pe)) = (o.launched, o.park_end.first()) {
        if !o.rls.is_empty() && pe - (l + PARK_LEAVE) < 2 {
            bad.push(format!("the cart's last tick on the park ({}) within 2 gt of PARK's end ({pe})", l + PARK_LEAVE));
        }
    }
    if let (Some(&p), None) = (o.park.first(), o.launched) {
        if !o.rls.is_empty() && o.rest_g.is_some() {
            bad.push(format!("PARK rose at {p} and the resting cart never launched (it never left the park)"));
        }
    }
    bad
}

/// A normal visit's own bars: one RLS, the napper aboard and on P every tick from its wake to boarding (the step-off
/// line), the window opened only on a sleeper and >= 6 gt after the lie-down, P continuous through the boarding, the
/// latches clear after the next ARM's edge.
fn normal_bars(o: &Out, late_lie: bool) -> Vec<String> {
    let mut bad = invariants(o);
    if o.rls.len() != 1 {
        bad.push(format!("RLS rose {} times", o.rls.len()));
    }
    if o.departed.map(|d| !d.1).unwrap_or(true) {
        bad.push("the napper not aboard".into());
    }
    if o.cells.contains('x') {
        bad.push(format!("the step-off line: the napper off P between its wake and boarding ({})", o.cells));
    }
    if !o.open_awake.is_empty() || !o.open_on_p.is_empty() {
        bad.push(format!("the window opened on an awake napper at {}", fmt_ticks(&[o.open_awake.clone(), o.open_on_p.clone()].concat())));
    }
    if !late_lie && o.window_after_lie.map(|w| w < WINDOW_AFTER_LIE).unwrap_or(false) {
        bad.push(format!("the window opened {} gt after the lie-down (< {WINDOW_AFTER_LIE})", o.window_after_lie.unwrap()));
    }
    if o.p_falls_boarding > 0 {
        bad.push(format!("P_STRING fell {} times between the wake and the departure", o.p_falls_boarding));
    }
    if let (Some(&(_, c)), Some(ap)) = (o.window.first(), o.cart_at_p) {
        if c != i64::MAX && ap - (c + HOSTILE_CLEAR) < 1 {
            bad.push(format!("the cart at P - (C + {HOSTILE_CLEAR}) = {} (< 1)", ap - (c + HOSTILE_CLEAR)));
        }
    }
    if o.latches_at_next.iter().any(|&b| b) {
        bad.push(format!("latches after the next ARM's edge (SO, SENSED, COOL, OPEN, SINCE, WIN) = {:?}", o.latches_at_next));
    }
    bad
}

/// The bars a row's kind asserts, EXCEPT the first-window bar (G1 on 33e731660 (2): C2 is pinned per row x band on its
/// own, every other bar judged at 0 misses on the same runs).
fn kind_bars(o: &Out, kind: Kind, late_lie: bool) -> Vec<String> {
    match kind {
        Kind::Normal => normal_bars(o, late_lie),
        Kind::BoardsEarly => {
            let mut b = invariants(o);
            if o.rls.len() != 1 {
                b.push(format!("RLS rose {} times", o.rls.len()));
            }
            if o.departed.map(|d| !d.1).unwrap_or(true) {
                b.push("the early-woken napper not aboard".into());
            }
            b
        }
        Kind::Boards => {
            let mut b = invariants(o);
            if o.departed.map(|d| !d.1).unwrap_or(true) {
                b.push("it never boards in the horizon".into());
            }
            b
        }
        Kind::Recovers => {
            let mut b = invariants(o);
            if o.departed.map(|d| !d.1).unwrap_or(true) {
                b.push("it never boards in the horizon (a recovery row)".into());
            }
            b
        }
        Kind::PermanentHalt => {
            let mut b: Vec<String> = invariants(o).into_iter().filter(|x| !x.contains("passes empty")).collect();
            if o.departed.map(|d| d.1).unwrap_or(true) {
                b.push("no empty departure (the permanent-halt row must show it)".into());
            }
            b
        }
        Kind::SelfEnding => {
            let mut b = invariants(o);
            match (o.boarded_at, o.self_end_lie) {
                (Some(bt), Some(l)) if bt > l => {}
                (bt, l) => b.push(format!("not boarded after its scripted later lie-down (boarded {bt:?}, the later lie-down {l:?})")),
            }
            b
        }
        Kind::Printed => invariants(o),
    }
}

/// G1's C2, the FIRST-WINDOW bar on its own (G1 on 33e731660 (2)): a BoardsEarly row boards on its first window (no
/// lie-down between its first wake and its boarding, and within the first RLS's park + read + response + ride (+ a world
/// open's idle and lag)); a Boards row whose first RLS came before any re-sleep boards by the first window's bound.
/// Ironspec 22:58 (2): a FORCED wake (a hurt) also boards by wake + 90.
fn first_window_miss(o: &Out, kind: Kind) -> Option<String> {
    match kind {
        Kind::BoardsEarly => {
            if let (Some(&w0), Some(bt)) = (o.wakes.first(), o.boarded_at) {
                if o.lies.iter().any(|&l| l > w0 && l < bt) {
                    return Some(format!("boarded at {bt} after a re-sleep, not on its first window"));
                }
            }
            if let (Some(by), Some(bt)) = (o.first_window_by, o.boarded_at) {
                if bt > by {
                    return Some(format!("boarded at {bt}, past its first window's bound {by}"));
                }
            }
            if let (Some(&w0), Some(bt)) = (o.wakes.first(), o.boarded_at) {
                if bt - w0 > FORCED_BOARD_BY {
                    return Some(format!("a forced wake at {w0} boarded at {bt}, past wake + {FORCED_BOARD_BY}"));
                }
            }
            None
        }
        Kind::Boards => {
            if let (Some(&r0), Some(by)) = (o.rls.first(), o.first_window_by) {
                let resleep = o.wakes.first().and_then(|&w0| o.lies.iter().find(|&&l| l > w0).copied());
                if resleep.is_none_or(|l| r0 < l) && o.boarded_at.is_none_or(|bt| bt > by) {
                    return Some(format!("its first RLS at {r0} came before any re-sleep, yet it boarded at {:?}, past the first window's bound {by}", o.boarded_at));
                }
            }
            None
        }
        _ => None,
    }
}

/// Ironspec 22:58 (2): every pre-COOL wake is FORCED and boards on its first window AND by wake + 90.
pub const FORCED_BOARD_BY: i64 = 90;

/// Ironspec 23:55 (tick-exact's pin: every dismount sets the boarding cooldown 60, counted on the napper's own ticks, not
/// saved): the pickup must come >= 5 gt after the cooldown's end on every visit (pickup - eject >= 65).
pub const BOARD_CD_MARGIN: i64 = 5;

fn compress(cells: &str) -> String {
    let mut out = String::new();
    let mut it = cells.chars().peekable();
    while let Some(c) = it.next() {
        let mut k = 1;
        while it.peek() == Some(&c) {
            it.next();
            k += 1;
        }
        out.push_str(&format!("{c}x{k} "));
    }
    out.trim_end().to_string()
}

/// The forced rows (ironspec 15:58 / 16:00 / 17:35, G1 on 5f79952c2), each with its kind; rows that script their own lie
/// keep it, the bands (rest, park -> P) sweep over every one.
fn forced_rows() -> Vec<(Row, Kind, bool)> {
    let mut f: Vec<(Row, Kind, bool)> = Vec::new();
    f.push((Row::normal("a late lie-down (ARM + 218: the window opens at once; its window-after-lie bar exempt)", 218, 10, 40, 16).long(900), Kind::Normal, true));
    for h in [7, 8, 9, 30, 60] {
        let mut r = Row::normal(&format!("an early wake from the bed (a hurt at ARM + {h})"), 5, 10, 40, 16).long(900);
        r.hurt = vec![h];
        // (a wake at + 7 / + 8 falls inside the lie-down's own pulse and is absorbed: no fresh pulse, so it recovers
        // through its re-sleep; from + 9 the wake has its own pulse and boards on its first window: G1 B6 (1c))
        // (a wake at + 7 / + 8 falls inside the lie-down's own pulse and is absorbed: no fresh pulse, so it recovers
        // through its re-sleep; at + 9 / + 30 the wake's restarted window is cut by the lie-down pulse's W line (lie
        // pulse + 59), so it boards on its first window where COOL / SENSED release in time and recovers through its
        // re-sleep otherwise (Boards, the split printed: G1's C2); at + 60 the window is the wake's own: BoardsEarly)
        // (with the onset window [-1, + 3] the + 7 / + 8 wakes, merged into the lie-down's pulse 2 / 3 gt after it, pass
        // the onset and own a first window too: every hurt row is judged on its first window, ironspec 22:06 / 22:15)
        f.push((r, Kind::BoardsEarly, false));
    }
    let mut r = Row::normal("an early wake (a hurt at ARM + 30) with a later park (the cart rests at ARM + 70, past COOL's bound): the sensed park holds RLS for the rest", 5, 10, 70, 16).long(900);
    r.hurt = vec![30];
    // (the later park's SENSED comes after the cut window: it recovers through its re-sleep in every band)
    f.push((r, Kind::Recovers, false));
    for (lie, d) in [(4, 0), (5, 0), (7, 0), (6, 1), (6, 2), (7, 3), (218, 0), (218, 2)] {
        let mut r = Row::normal(&format!("the merged path from the bed (lie-down at ARM + {lie}, a hurt at + {d})"), lie, 10, 40, 16).long(900);
        r.hurt = vec![lie + d];
        // (ironspec 22:06: first-window boarding is JUDGED for every row with a first window (the merged paths and the
        // hurt wakes from + 9); with the onset window [-1, + 3] (ironspec 22:15) a hurt within 3 gt of the lie-down is a
        // merged first window too (merge2 included); the late lie-down rows stay Recovers)
        f.push((r, if lie > 100 { Kind::Recovers } else if d <= 3 { Kind::BoardsEarly } else { Kind::Recovers }, lie > 100));
    }
    let mut r = Row::normal("the self-healing homeless napper (re-homed while seated, it lies down at the set-down tick ARM + 3: tick-exact 17:42's game row on 58f770cc's bench)", 3, 10, 40, 16).long(900);
    r.lie = vec![3];
    f.push((r, Kind::Normal, false));
    let mut r = Row::normal("the merged path at L + 0 FROM P (the napper standing on P lies down at ARM + 218, hurt in the same tick: the onset fails, it recovers through its re-sleep)", 218, 10, 40, 16).long(2000);
    r.start_on_p = true;
    r.hurt = vec![218];
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("a planted stale press over [lie - 10, lie - 1] before the lie-down at ARM + 218", 218, 10, 40, 16).long(2000);
    r.planted = vec![(208, 217)];
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("G1's B2 box: a box on the string from lie - 10 through pulse + 20 around the lie-down at ARM + 218 (the onset must refuse it)", 218, 10, 40, 16).long(2000);
    r.planted = vec![(208, 240)];
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("the never-slept napper standing on P lies down FROM P at ARM + 2000 (its own press on the string)", 2000, 10, 40, 16).long(3000);
    r.start_on_p = true;
    f.push((r, Kind::Recovers, true));
    for kk in [0, 1, 2] {
        let mut r = Row::normal(&format!("stepping onto P and lying down within {kk} gt (an early wake at ARM + 8, off P at + 20, back onto P at + 150, lying down from P at + {})", 150 + kk), 5, 10, 40, 16).long(2000);
        r.hurt = vec![8];
        r.off = vec![(20, 150)];
        r.lie = vec![5, 150 + kk];
        r.resleep = None;
        f.push((r, Kind::Recovers, true));
    }
    let mut r = Row::normal("an early wake (ARM + 8) strolling off P at + 20 (before PARKED) and re-crossing P at + 140 (past W)", 5, 10, 40, 16).long(2000);
    r.hurt = vec![8];
    r.off = vec![(20, 140)];
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("a calm napper back onto P after its cooldown, lying down FROM P 10 gt later (an early wake at ARM + 8, off P at + 20, back on P at + 150, lying down from P at + 160: the string's release at its latest, pulse + 8)", 5, 10, 40, 16).long(2000);
    r.hurt = vec![8];
    r.off = vec![(20, 150)];
    r.lie = vec![5, 160];
    r.resleep = None;
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("a lie-down from P at ARM + 160 (back on P at + 150), then a box on the string from + 175 to + 260 (SINCE's twin row)", 5, 10, 40, 16).long(2000);
    r.hurt = vec![8];
    r.off = vec![(20, 150)];
    r.lie = vec![5, 160];
    r.planted = vec![(175, 260)];
    r.resleep = None;
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("the window on a wanderer, CROSSING P: G1's awake napper (merged from the bed at ARM + 6 + 1) off P at ARM + 40, back across P at ARM + 300, calm re-sleep past its cooldown", 6, 10, 40, 16).long(2000);
    r.hurt = vec![7];
    r.off = vec![(40, 300)];
    r.resleep = Some(400);
    f.push((r, Kind::Recovers, true));
    let mut r = Row::normal("the window on a wanderer, STAYING OFF P until ARM + 3000, then its next lie-down (calm, away from Z1's view) at ARM + 3000: self-ending at that lie-down", 6, 10, 40, 16).long(4000);
    r.hurt = vec![7];
    r.off = vec![(40, 2999)];
    r.lie = vec![6, 3000];
    r.resleep = None;
    r.self_end_lie = Some(3000);
    f.push((r, Kind::SelfEnding, true));
    let mut r = Row::normal("Z1 removed: no wake until dawn (ARM + 6000), the napper standing up on P", 5, 10, 40, 16).long(9000);
    r.z1 = false;
    r.resleep = None;
    r.dawn = Some(6000);
    f.push((r, Kind::Normal, false));
    f.push((Row::normal("the never-slept night visit (no lie-down until ARM + 2000, standing on the bed foot)", 2000, 10, 40, 16).long(3000), Kind::Normal, true));
    let mut r = Row::normal("the late park: the cart rests at ARM + 56, 10 gt past its worst band (ironspec 17:35 (2): released normally)", 5, 10, 56, 16);
    r.rest = 56;
    f.push((r, Kind::Normal, false));
    let r = Row::normal("a later park: the cart rests at ARM + 70, past COOL's bound (the sensed park holds RLS for it)", 5, 10, 70, 16);
    f.push((r, Kind::Normal, false));
    let mut r = Row::normal("a stray press of the park-entry detector after RLS (G1 17:49 (a): it must not pre-set the next visit's PARKED)", 5, 10, 40, 16);
    r.park_stray = vec![200];
    f.push((r, Kind::Normal, false));
    let mut r = Row::normal("P_STRING dropping right after RLS (the napper steps off P 1 gt after RLS): SO's reset at RLS + 2 must not let the window re-open (ironspec 18:12 (c)); the cart departs empty", 5, 10, 40, 16);
    r.step_off = Some(1);
    f.push((r, Kind::PermanentHalt, false));
    let mut r = Row::normal("the step-off (a forced displacement off P 5 gt after RLS, before the cart reaches it): the one permanent halt left, for the HALT entry", 5, 10, 40, 16);
    r.step_off = Some(5);
    f.push((r, Kind::PermanentHalt, false));
    let mut r = Row::normal("the false-SO path (G1's B9): a never-slept napper standing on P, a non-sleep head change at ARM + 3: the onset refuses its standing press (no release, the stop holds: one nap missed and the next napper held at R1), printed", 5000, 10, 40, 16).long(900);
    r.lie = vec![];
    r.start_on_p = true;
    r.head_only = vec![3];
    r.resleep = None;
    f.push((r, Kind::Printed, true));
    let mut r = Row::normal("a non-sleep head change at ARM + 5 (the napper awake on the bed foot): the window opens on it, no release, printed", 5000, 10, 40, 16).long(900);
    r.lie = vec![];
    r.head_only = vec![5];
    r.resleep = None;
    f.push((r, Kind::Printed, true));
    f
}

/// The bench: the normal sweep, every forced row across the bands, a world open at every tick, the controls.
/// Every band variant of a forced row (G1 on a2186649e / 5dd4204be B1): rest (when the row's own rest is in the band)
/// x park -> P x pass -> rest x the lie band (when the row's first lie-down is the band's; a hurt tied to it, within 3
/// gt, moves with it).
fn band_variants(row: &Row) -> Vec<Row> {
    let lies: Vec<i64> = if row.lie.first().is_some_and(|l| LIES.contains(l)) { LIES.to_vec() } else { vec![row.lie.first().copied().unwrap_or(0)] };
    let mut out = Vec::new();
    for &rest in &RESTS {
        if row.rest > RESTS[2] && rest != RESTS[0] {
            continue;
        }
        for &board in &BOARDS {
            for &ptr in &PASS_TO_REST {
                for &lie in &lies {
                    let mut rr = row.clone();
                    if row.rest <= RESTS[2] {
                        rr.rest = rest;
                    }
                    rr.board = board;
                    rr.pass_to_rest = ptr;
                    if lies.len() > 1 {
                        let d = lie - row.lie[0];
                        for h in rr.hurt.iter_mut() {
                            if (0..=3).contains(&(*h - row.lie[0])) {
                                *h += d;
                            }
                        }
                        rr.lie[0] = lie;
                    }
                    out.push(rr);
                }
            }
        }
    }
    out
}

/// The latch alone at the torch / repeater level (the micro-bench of G1 21:25 / ironspec 21:28 / 21:32): RLS's AND
/// output torch fed by its OR block (powered except over `gap` ticks from tick 10: the AND's inputs reach the block through
/// their input repeaters, so its changes land in the diode phase), or RLS's line forced high for `forced` ticks from tick
/// 12 (an input reaching the block by a NORMAL-priority path, as a control); ARM's edge at `arm_at`. Returns RLS's width,
/// PARK's window [start, end) (Q lit), the tap's window, and the ticks a Q-bar check read B2 unpowered.
pub fn latch_micro(gap: i64, forced: Option<i64>, opts: LatchOpts, arm_at: Option<i64>) -> (i64, Option<(i64, i64)>, Option<(i64, i64)>, Vec<i64>) {
    let mut sim = LatchSim::new(opts);
    let mut rls_t = Torch { lit: false, due: None };
    let mut seq = 0u64;
    let (mut w, mut park, mut tap, mut dips) = (0i64, (None, None), (None, None), Vec::new());
    let (mut q_prev, mut tap_prev) = (opts.power_up_set, opts.power_up_set);
    if opts.power_up_set {
        park.0 = Some(0);
        tap.0 = Some(0);
    }
    for k in 0..80i64 {
        let or_powered = !(k >= 10 && k < 10 + gap);
        let rls = match forced {
            Some(f) => k >= 12 && k < 12 + f,
            None => {
                if rls_t.due.map(|d| d.0) == Some(k) {
                    rls_t.due = None;
                    rls_t.lit = !or_powered;
                }
                rls_t.lit
            }
        };
        let arm_edge = arm_at.is_some_and(|a| k >= a && k < a + EDGE_W);
        let lo = sim.step(k, rls, arm_edge);
        if forced.is_none() {
            rls_t.update(k, or_powered, &mut seq);
        }
        if rls {
            w += 1;
        }
        if lo.dip_at_check {
            dips.push(k);
        }
        if lo.q && !q_prev {
            park.0 = park.0.or(Some(k));
        }
        if !lo.q && q_prev {
            park.1 = park.1.or(Some(k));
        }
        if lo.tap && !tap_prev {
            tap.0 = tap.0.or(Some(k));
        }
        if !lo.tap && tap_prev {
            tap.1 = tap.1.or(Some(k));
        }
        q_prev = lo.q;
        tap_prev = lo.tap;
    }
    let win = |p: (Option<i64>, Option<i64>)| p.0.map(|a| (a, p.1.unwrap_or(i64::MAX)));
    (w, win(park), win(tap), dips)
}

pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec![format!("ctl-shutter (LOGIC LEVEL: designed gate delays, the latch at the torch / repeater level in 26.3's tick order; the real-cell bench on the stop's placement follows its layout). TODAY's BAND IS SUPERSEDED GEOMETRY (ironspec 22:22 (3) / 22:24 / 22:25, G1 22:23): the bars judge it and this run exits non-zero while its C2 rows fail; C2 must pass on the re-laid stop's band. The form of record: the release on sustained presence from the latest pulse with its onset window [-1, +3], held to pulse + 12, within W {W_LINE}, PARKED = SENSED AND COOL; OPEN set PARKED AND SO AND NOT P, reset RLS OR P; PARK = RLS OR P_HOLD at RLS + {PARK} (P_HOLD the feedback latch: the branch, the ARM-edge reset, the consuming resets on its tap), PARK on {} gt", PARK_W + 4)];
    lines.push(format!(
        "  delays (designed, gt): torch {TORCH}, repeater {REP} a tick, comparator {CMP}, latch set {SET} / reset {RESET} (ctl-junction), COOL's tap {COOL_LINE}, WIN's D line {D_LINE} (P read at pulse + {HOLD}), W line {W_LINE} (RLS by pulse + {}), SINCE's delay {SINCE_DELAY}, the onset {ONSET} gt, the AND {AND}, PARK {PARK}, the string's re-check {RECHECK}, the observer +{OBS_RISE} x {OBS_W}, the eject ARM + {EJECT} and its boarding cooldown {BOARD_COOLDOWN}; the cooldown at boarding: the latest wake + {COOLDOWN} - boarding >= {COOLDOWN_BAR}",
        W_LINE + W_SLACK
    ));
    lines.push("  the napper's and the cart's bands: 58f770cc's stop bench (bed-tier-min, explore/bedtier-stop 2af606b9d, b48 NOTES.md: ARM -> lie-down 4..5, the window -> wake 1..20, the cart at rest 33..46, the park -> P 13..19; physical timings of the bench) and its real-cell pass -> rest 16..26 (58f770cc 18:35, the band at 4caaec86f; its real rest band 25..38 lies inside the swept 33..46 at the late end only, so the sweep is conservative)".to_string());
    // (THE TWO DRIVES, each through the whole sweep: the plain drive of record in the re-laid stop with (g)'s logic
    // rulings carried (W 48: ironspec 21:51), and (g)'s latch drive, its fallback (W 46))
    let forms = [
        ("the plain drive of record (PARK = RLS OR P_HOLD at RLS + 2, P_HOLD the feedback latch with the branch, the ARM-edge reset and the consumers on its tap), W 48", Defects::default(), COOL_EDGE, W_LINE, PARK),
        ("(g)'s latch drive, the fallback (PARK = the latch's Q, [RLS + 4, RLS + 12)), W 46", Defects { torch: true, w_delta: W_TORCH - W_LINE, ..Default::default() }, COOL_EDGE_TORCH, W_TORCH, PARK_TORCH),
    ];
    for (fi, (form, fdef, fedge, fw, fpk)) in forms.into_iter().enumerate() {
    // ((g)'s fallback runs every bar into a verdict of its own, PRINTED and not judged: the drive of record is the plain
    // one, ironspec 21:51; (g) would need its own pass of these same bars before it could be built)
    let mut vg = Verdict::new("ctl-shutter (g) fallback (printed)");
    let vv: &mut Verdict = if fi == 0 { &mut *v } else { &mut vg };
    lines.push(format!("  ==== {form} ===="));
    // (1) the normal sweep
    let mut n = 0usize;
    let mut bad: Vec<String> = Vec::new();
    let mut mm = std::collections::BTreeMap::<&str, (i64, i64)>::new();
    let mut put = |k: &'static str, x: i64| {
        let e = mm.entry(k).or_insert((i64::MAX, i64::MIN));
        e.0 = e.0.min(x);
        e.1 = e.1.max(x);
    };
    for lie in [4, 5] {
        for scan in 1..=20 {
            for rest in [33, 40, 46] {
                for (board, ptr) in [(13, 16), (16, 26), (19, 16), (13, 26), (19, 26)] {
                    let mut row = Row::normal("normal", lie, scan, rest, board);
                    row.pass_to_rest = ptr;
                    let o = run(&row, fdef);
                    n += 1;
                    let b = normal_bars(&o, false);
                    if !b.is_empty() && bad.len() < 3 {
                        bad.push(format!("lie {lie} scan {scan} rest {rest} board {board} pass -> rest {ptr}: {}", b.join("; ")));
                    }
                    if let (Some(&w), Some(&(_, c)), Some(&r), Some(&pk)) = (o.wakes.first(), o.window.first(), o.rls.first(), o.park.first()) {
                        put("close", c - w);
                        put("rls", r - w);
                        put("park", pk - w);
                        if let Some(b) = o.boarded_at {
                            put("board", b - w);
                        }
                        if let Some(ap) = o.cart_at_p {
                            put("hostile", ap - (c + HOSTILE_CLEAR));
                            if let Some(mb) = o.may_board_from {
                                put("board-cd", ap - mb);
                            }
                        }
                    }
                    if let Some(m) = o.cooldown_margin {
                        put("cool", m);
                    }
                    if let Some(wl) = o.window_after_lie {
                        put("win-lie", wl);
                    }
                }
            }
        }
    }
    let r = |k: &str| mm.get(k).map(|x| format!("{}..{}", x.0, x.1)).unwrap_or("-".into());
    let w = vv.bar_n(&format!("ctl-shutter [{form}]: the normal visit (lie-down 4..5, scan 1..20, rest 33/40/46, park -> P 13/16/19, the park-entry pass -> rest 16/26): one RLS, the napper aboard and on P every tick from its wake to boarding, the window only on a sleeper and >= 6 gt after the lie-down, P continuous through the boarding, the latches clear after the next ARM's edge, no release to a sleeper, no park before rest, the cooldown margin at boarding >= 10, RLS within W of its pulse, the cart at P >= the boarding cooldown's end + 2, the shutter closed <= the press + 8, the consistency bar both ways, the cart at rest by PARK's rise, its last tick on the park >= 2 gt before PARK's end"), n, bad.is_empty());
    lines.push(format!(
        "  the normal visit, {n} runs, from the wake: the shutter closes + {}, RLS + {}, the park + {}, boarding + {}, the cooldown margin at boarding {} (>= {COOLDOWN_BAR}); the window opens {} gt after the lie-down (>= {WINDOW_AFTER_LIE}); the cart at P - the boarding cooldown's end {} (>= 2); the cart at P - (C + {HOSTILE_CLEAR}) {} (>= 1; C = the shutter's first closed tick; logic level: the hold delta on 58f770cc's cells, the first (-2,0,5) entry, is the figure): {w}{}",
        r("close"), r("rls"), r("park"), r("board"), r("cool"), r("win-lie"), r("board-cd"), r("hostile"),
        if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }
    ));
    let sample = run(&Row::normal("normal", 5, 10, 40, 16), fdef);
    lines.push(format!("    e.g. lie 5, scan 10, rest 40, park -> P 16: {}; the napper's cell per tick from its wake to boarding: {}", describe(&sample), compress(&sample.cells)));
    let margin = HOLD - STRING_DROP;
    lines.push(format!(
        "  the lie-down exclusion: a napper that lies down leaves P and its string drops by its pulse + {STRING_DROP} (26.3's TripWireBlock re-checks every {RECHECK} gt while pressed), against RLS's read of P at pulse + {HOLD}: margin {margin}, an entity-to-block timed pair: {} on the build (k + j 1), {} straddling (k + j 3)",
        margin - 1,
        margin - 3
    ));
    // (2) every forced row across its full band (band_variants); G1 on 33e731660 (2): the first-window bar (C2) is its
    // own bar per row, its misses counted per band; every other bar of the row's kind judged at 0 misses on the same runs
    for (row, kind, late) in forced_rows() {
        let mut nb = 0usize;
        let mut bad: Vec<String> = Vec::new();
        let (mut c2n, mut c2ex) = (0usize, Vec::<String>::new());
        let mut boards: (i64, i64) = (i64::MAX, i64::MIN);
        let mut cool: (i64, i64) = (i64::MAX, i64::MIN);
        let (mut first_w, mut later_w) = (0usize, 0usize);
        for rr in band_variants(&row) {
            let o = run(&rr, fdef);
            nb += 1;
            let b = kind_bars(&o, kind, late);
            if !b.is_empty() && bad.len() < 2 {
                bad.push(format!("rest {} board {} pass -> rest {} lie {:?}: {}", rr.rest, rr.board, rr.pass_to_rest, rr.lie.first(), b.join("; ")));
            }
            if let Some(m) = first_window_miss(&o, kind) {
                c2n += 1;
                if c2ex.len() < 2 {
                    c2ex.push(format!("rest {} board {} pass -> rest {}: {m}", rr.rest, rr.board, rr.pass_to_rest));
                }
            }
            if let Some(bt) = o.boarded_at {
                boards = (boards.0.min(bt), boards.1.max(bt));
                // (the first window or a later one: a lie-down between the first wake and the boarding)
                if o.wakes.first().is_some_and(|&w0| o.lies.iter().any(|&l| l > w0 && l < bt)) {
                    later_w += 1;
                } else {
                    first_w += 1;
                }
            }
            if let Some(m) = o.cooldown_margin {
                cool = (cool.0.min(m), cool.1.max(m));
            }
        }
        let o = run(&row, fdef);
        let mark = vv.bar_n(&format!("ctl-shutter [{form}] forced row ({kind:?}) across the bands, every bar but the first window: {}", row.name), nb, bad.is_empty());
        let c2 = if matches!(kind, Kind::BoardsEarly | Kind::Boards) {
            let cls = if row.hurt.is_empty() { "NATURAL" } else { "FORCED" };
            let m = vv.bar_n(&format!("ctl-shutter [{form}] C2, THE FIRST WINDOW ({cls} wake{}): {}", if row.hurt.is_empty() { "" } else { ", by wake + 90" }, row.name), nb, c2n == 0);
            format!("; C2 (the first window): {c2n} of {nb} bands miss: {m}{}", if c2ex.is_empty() { String::new() } else { format!(" [{}]", c2ex.join(" | ")) })
        } else {
            String::new()
        };
        lines.push(format!(
            "  {} [{kind:?}, {nb} band runs: boarding {}..{} gt after ARM ({first_w} on the first window, {later_w} after a re-sleep), the cooldown margin at boarding {}..{}]: e.g. {}{}: {mark}{}{c2}",
            row.name,
            if boards.0 == i64::MAX { "-".into() } else { boards.0.to_string() },
            if boards.1 == i64::MIN { "-".into() } else { boards.1.to_string() },
            if cool.0 == i64::MAX { "-".into() } else { cool.0.to_string() },
            if cool.1 == i64::MIN { "-".into() } else { cool.1.to_string() },
            describe(&o),
            if o.cells.is_empty() { String::new() } else { format!("; cells wake -> boarding {}", compress(&o.cells)) },
            if bad.is_empty() { String::new() } else { format!(" [{}]", bad.join(" | ")) }
        ));
    }
    // (3) a world open at every tick to past the next ARM, idle 1 and 2, the entity domain 1 or 3 ticks behind; on the
    // normal visit across the bands and on G1's cooldown edge (lie 4, L + 0, board 19)
    let mut n = 0usize;
    let mut bad = Vec::new();
    let mut edge_cool = i64::MAX;
    let mut offs: (i64, i64) = (i64::MAX, i64::MIN);
    for (idle, kj) in [(1, 1), (2, 1), (1, 3), (2, 3)] {
        for t0 in (0..=700).step_by(1) {
            for (lie, rest, board, merged) in [(5, 40, 16, false), (4, 33, 19, false), (5, 46, 13, false), (4, 33, 19, true)] {
                let mut r = Row::normal("open", lie, 10, rest, board);
                if merged {
                    r.hurt = vec![lie];
                    r = r.long(900);
                }
                r.open = Some((t0, idle, kj));
                let o = run(&r, fdef);
                n += 1;
                if !merged {
                    for &(_, x) in &o.press_minus_change {
                        offs = (offs.0.min(x), offs.1.max(x));
                    }
                }
                let b = if merged { kind_bars(&o, Kind::Recovers, false) } else { normal_bars(&o, false) };
                if merged {
                    if let Some(m) = o.cooldown_margin {
                        edge_cool = edge_cool.min(m);
                    }
                }
                if !b.is_empty() && bad.len() < 3 {
                    bad.push(format!("open at {t0} idle {idle} kj {kj} (lie {lie} rest {rest} board {board}{}): {}", if merged { " merged" } else { "" }, b.join("; ")));
                }
            }
        }
    }
    let w = vv.bar_n(&format!("ctl-shutter [{form}]: a world open at every tick 0..700 (idle 1 / 2, the entity domain 1 / 3 ticks behind the block domain), on the normal visit at three band corners and on G1's cooldown edge (lie 4, merged L + 0, rest 33, board 19): each row's bars"), n, bad.is_empty());
    lines.push(format!("  a world open at every tick 0..700, idle 1 / 2, k + j 1 / 3, {n} runs (G1's cooldown edge's least margin at boarding {edge_cool}; the woken napper's press - its head change {}..{} gt, against the onset's allowance -{ONSET}): {w}{}", offs.0, offs.1, if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }));
    // (G1 on a2186649e B1 / on 5dd4204be B1: the open sweep on EVERY forced row across its FULL band (rest x park -> P x
    // pass -> rest x the lie band, as the forced sweep), every tick over the visit 0..300, idle 1 / 2, the entity domain
    // 1 / 3 behind; one thread a row)
    let rows_all = forced_rows();
    // (per row: runs, the least cooldown margin and the run that holds it (G1 on 33e731660 (1): the C1 twin runs on the
    // sweep's ARGMIN), failures of every bar but the first window, the first-window misses on their own (G1 (2)), and the
    // prints the latch's rulings ask for: RLS's least width, the least rest-before-PARK margin, the least
    // last-tick-on-the-park-before-PARK's-end margin, the longest visit (ARM -> boarding), and for the + 9 wake its
    // first-window / re-sleep split, its longest extra time and its runs that never board (G1 (8)))
    type OpenRes = (usize, i64, Vec<String>, [i64; 4], (usize, usize, i64, usize), (usize, Vec<String>), (Option<(i64, Row)>, Option<(i64, Row)>, usize, Option<(i64, Row)>, i64, usize));
    let results: Vec<OpenRes> = std::thread::scope(|s| {
        let hs: Vec<_> = rows_all
            .iter()
            .map(|(row, kind, late)| {
                s.spawn(move || {
                    let (mut n, mut least, mut bad) = (0usize, i64::MAX, Vec::new());
                    let (mut c1_at, mut w_at, mut argmin_n): (Option<(i64, Row)>, Option<(i64, Row)>, usize) = (None, None, 0);
                    let mut bcd_at: Option<(i64, Row)> = None;
                    // (ironspec 23:55: THE BOARDING COOLDOWN's margin: pickup - (eject + 60) >= 5 on every visit)
                    let (mut bcd_least, mut bcd_bad) = (i64::MAX, 0usize);
                    let (mut c2n, mut c2ex) = (0usize, Vec::<String>::new());
                    let mut st = [i64::MAX, i64::MAX, i64::MAX, i64::MIN];
                    let mut nine = (0usize, 0usize, i64::MIN, 0usize);
                    let is_nine = row.name.contains("a hurt at ARM + 9)");
                    let k = *kind;
                    for rr in band_variants(row) {
                        for &(idle, kj) in &OPENS {
                            for t0 in 0..=300 {
                                let mut r = rr.clone();
                                r.open = Some((t0, idle, kj));
                                let o = run(&r, fdef);
                                n += 1;
                                if let (Some(ap), Some(mb), Some(_)) = (o.cart_at_p, o.may_board_from, o.boarded_at) {
                                    bcd_least = bcd_least.min(ap - mb);
                                    if ap - mb < BOARD_CD_MARGIN {
                                        bcd_bad += 1;
                                    }
                                }
                                if let Some(m) = o.cooldown_margin {
                                    if m < least {
                                        least = m;
                                    }
                                    // (the runs G1's C1 / W controls run on: the least-margin run where removing the park's
                                    // read raises the margin by exactly one (the read is a term there), and the least-margin
                                    // run where W + 4 makes a NEW named miss (the W term binds there); only runs within 3
                                    // of the closed form's margin are tried)
                                    if m <= COOLDOWN - fedge + 3 {
                                        if c1_at.as_ref().is_none_or(|(bm, _)| m < *bm) && run(&r, Defects { edge_no_read: true, ..fdef }).cooldown_margin == Some(m + PARK_READ) {
                                            c1_at = Some((m, r.clone()));
                                        }
                                        if w_at.as_ref().is_none_or(|(bm, _)| m < *bm) && kind_bars(&run(&r, Defects { w_delta: fdef.w_delta + 4, ..fdef }), k, *late).iter().any(|b| b.contains("cooldown")) {
                                            w_at = Some((m, r.clone()));
                                        }
                                    }
                                    if m == COOLDOWN - fedge {
                                        argmin_n += 1;
                                        if bcd_at.is_none() {
                                            bcd_at = o.cart_at_p.zip(o.may_board_from).map(|(ap, mb)| (ap - mb, r.clone()));
                                        }
                                    }
                                }
                                if let Some(&w0) = o.rls_widths.first() {
                                    st[0] = st[0].min(w0);
                                }
                                if let (Some(&p), Some(rg)) = (o.park.first(), o.rest_g) {
                                    st[1] = st[1].min(p - rg);
                                }
                                if let (Some(l), Some(&pe)) = (o.launched, o.park_end.first()) {
                                    st[2] = st[2].min(pe - (l + PARK_LEAVE));
                                }
                                if is_nine && o.boarded_at.is_none() {
                                    nine.3 += 1;
                                }
                                if let Some(bt) = o.boarded_at {
                                    if is_nine {
                                        st[3] = st[3].max(bt - r.arm);
                                        if o.wakes.first().is_some_and(|&w0| o.lies.iter().any(|&l| l > w0 && l < bt)) {
                                            nine.1 += 1;
                                            if let Some(by) = o.first_window_by {
                                                nine.2 = nine.2.max(bt - by);
                                            }
                                        } else {
                                            nine.0 += 1;
                                        }
                                    }
                                }
                                let b = kind_bars(&o, k, *late);
                                if !b.is_empty() && bad.len() < 2 {
                                    bad.push(format!("{} (rest {} board {} pass -> rest {} lie {:?}) open at {t0} idle {idle} kj {kj}: {}", row.name.chars().take(60).collect::<String>(), r.rest, r.board, r.pass_to_rest, r.lie.first(), b.join("; ")));
                                }
                                if let Some(m) = first_window_miss(&o, k) {
                                    c2n += 1;
                                    if c2ex.len() < 1 {
                                        c2ex.push(format!("rest {} board {} pass -> rest {} open at {t0} idle {idle} kj {kj}: {m}", r.rest, r.board, r.pass_to_rest));
                                    }
                                }
                            }
                        }
                    }
                    (n, least, bad, st, nine, (c2n, c2ex), (c1_at, w_at, argmin_n, bcd_at, bcd_least, bcd_bad))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n: usize = results.iter().map(|r| r.0).sum();
    let least_cool = results.iter().map(|r| r.1).min().unwrap_or(i64::MAX);
    let c1_row: Option<(i64, Row)> = results.iter().filter_map(|r| r.6 .0.clone()).min_by_key(|(m, _)| *m);
    let w_row: Option<(i64, Row)> = results.iter().filter_map(|r| r.6 .1.clone()).min_by_key(|(m, _)| *m);
    let argmin_at: Option<(i64, Row)> = results.iter().find_map(|r| r.6 .3.clone());
    let argmin_bcd: Option<i64> = argmin_at.as_ref().map(|(x, _)| *x);
    let bcd_least: i64 = results.iter().map(|r| r.6 .4).min().unwrap_or(i64::MAX);
    let bcd_bad: usize = results.iter().map(|r| r.6 .5).sum();
    let wb = vv.bar_n(&format!("ctl-shutter [{form}]: THE BOARDING COOLDOWN's MARGIN over the open sweep (ironspec 23:55: every visit's pickup - (eject + {BOARD_COOLDOWN}) >= {BOARD_CD_MARGIN}; COOL and the release timing are load-bearing for it)"), n, bcd_bad == 0);
    lines.push(format!("    THE BOARDING COOLDOWN's MARGIN over it (pickup - the cooldown's end, >= {BOARD_CD_MARGIN}): least {bcd_least}, {bcd_bad} runs below; at the cooldown edge's argmin it is {}: {wb}", argmin_bcd.map_or("-".to_string(), |x| x.to_string())));
    let argmin_n: usize = results.iter().map(|r| r.6 .2).sum();
    let st_min = |i: usize| results.iter().map(|r| r.3[i]).min().unwrap_or(i64::MAX);
    let longest_visit = results.iter().map(|r| r.3[3]).max().unwrap_or(i64::MIN);
    let nine = results.iter().fold((0usize, 0usize, i64::MIN, 0usize), |a, r| (a.0 + r.4 .0, a.1 + r.4 .1, a.2.max(r.4 .2), a.3 + r.4 .3));
    let bad: Vec<String> = results.iter().flat_map(|r| r.2.iter().cloned()).take(4).collect();
    let w = vv.bar_n(&format!("ctl-shutter [{form}]: a world open at every tick 0..300 on EVERY forced row across its full band (rest x park -> P x pass -> rest x the lie band), idle 1 / 2, the entity domain 1 / 3 behind: every bar of each row's kind but the first window (0 misses required; G1 on 33e731660 (2))"), n, bad.is_empty());
    lines.push(format!("  a world open at every tick 0..300 on every forced row across its full band, idle 1 / 2, k + j 1 / 3, {n} runs (the least cooldown margin at boarding {least_cool}): {w}{}", if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }));
    // (C2 over the open sweep, per row: each row's first-window misses counted and pinned on their own)
    for ((row, kind, _), res) in rows_all.iter().zip(results.iter()) {
        if !matches!(kind, Kind::BoardsEarly | Kind::Boards) {
            continue;
        }
        let (c2n, ex) = &res.5;
        let m = vv.bar_n(&format!("ctl-shutter [{form}] C2 OVER THE OPEN SWEEP (the first window{}): {}", if row.hurt.is_empty() { "" } else { ", a FORCED wake by wake + 90" }, row.name), res.0, *c2n == 0);
        lines.push(format!("    C2 over the open sweep, {}: {c2n} of {} runs miss: {m}{}", row.name.chars().take(90).collect::<String>(), res.0, if ex.is_empty() { String::new() } else { format!(" [{}]", ex.join(" | ")) }));
    }
    // (the latch rulings' prints over that sweep: RLS's least width, the cart at rest before PARK, its last tick on the
    // park before PARK's end; the longest visit against the 1,200-gt HOME bound; the + 9 wake's split)
    let nine_runs = nine.0 + nine.1 + nine.3;
    let wv = vv.bar_n(&format!("ctl-shutter [{form}]: THE HURT + 9 WAKE over the open sweep: EVERY run boards (a run that never boards fails), within 1,200 gt of its ARM, on its first window or after its re-sleep (the stamp's HOME bound: no HOME loss, no permanent hold; G1 21:11 (2), G1 on 33e731660 (8))"), nine_runs, longest_visit < 1200 && nine.3 == 0);
    lines.push(format!(
        "    over it: RLS's least width {} gt (the logic model's AND passes a 1-gt overlap; the real AND's output torch turns a gap of 1 or 2 into no RLS at all: the latch-alone bars above); the cart at rest {} gt before PARK's rise at the least (the judged bar: >= 0); its last tick on the park {} gt before PARK's end at the least (the judged bar: >= 2); the + 9 wake's longest visit, ARM -> boarding, {longest_visit} gt (< 1,200), runs that never board {}: {wv}; THE HURT + 9 WAKE: {} runs board on their first window, {} after the re-sleep, the re-sleep's extra time at most {} gt past the first window's bound",
        st_min(0),
        st_min(1),
        st_min(2),
        nine.3,
        nine.0,
        nine.1,
        if nine.2 == i64::MIN { "-".to_string() } else { nine.2.to_string() }
    ));
    // (G1's N1 / C1: the cooldown edge's closed form, every term, against the bar, and EQUAL to the sweep's least margin)
    let edge_margin = COOLDOWN - fedge;
    let w = vv.bar(&format!("ctl-shutter [{form}]: the cooldown edge's closed form (the wake's pulse + the latest RLS + PARK + the park's read + the cart's first move + the slowest ride + a world open's idle + the entity lag) keeps COOLDOWN - it >= 10"), edge_margin >= COOLDOWN_BAR);
    lines.push(format!("  the cooldown edge, every term (G1's N1 / C1; the margin's purpose: the latest boarding against the napper's re-sleep at wake + {COOLDOWN}: a miss sends the released cart past P empty, an empty departure that halts napping until a player repairs it): the latest boarding = the wake + {OBS_RISE} (pulse) + {} (the latest RLS rise: W {fw} + WIN's reset to Q's block {RESET_IN} + its NOT-Q torch {TORCH} + the repeater {REP} + the AND {AND} - 1 (the first blocked tick) - {} (RLS's output torch reads its OR block at its scheduled tick, so an RLS needs a {RLS_MIN_W}-gt overlap: MODELLED, pending tick-exact's rig of the AND as built)) + {fpk} (PARK) + {PARK_READ} (the park's read) + {CART_RESPONSE} (the cart's first move) + {BOARD_MAX} (the slowest ride) + {OPEN_IDLE} (an open's idle) + {OPEN_KJ} (the entity lag) = the wake + {fedge}: margin {edge_margin} (>= {COOLDOWN_BAR}): {w}", fw + W_SLACK, RLS_MIN_W - 1));
    let eq = |cool_edge: i64| COOLDOWN - cool_edge == least_cool;
    let w = vv.bar(&format!("ctl-shutter [{form}]: the closed form is tight: the open sweep's least cooldown margin equals COOLDOWN - the closed form (a bound a run beats is not a bound)"), eq(fedge));
    lines.push(format!("    the sweep's least margin {least_cool} against the closed form's {edge_margin}: {w}"));
    // (G1 on 33e731660 (1): the C1 twin on the open sweep's ARGMIN run with the park's READ itself removed: the design's
    // margin there equals the closed form's, the twin's is one higher, so the tightness bar fails BY NAME on the twin;
    // G1 (2)'s control: W + 4 at the argmin makes a NEW named miss (the cooldown margin at boarding))
    {
        let desc = |r: &Row| format!("{}; rest {} board {} pass -> rest {} open {:?}", r.name.chars().take(70).collect::<String>(), r.rest, r.board, r.pass_to_rest, r.open);
        // (C1: a run AT the closed form's margin where the read is a term: the design's margin = the closed form's, the
        // twin's one higher, so the tightness bar fails by name on the twin)
        let (c, l1) = match &c1_row {
            Some((m1, ar)) => {
                let d = run(ar, fdef);
                let x = run(ar, Defects { edge_no_read: true, ..fdef });
                let bites = d.cooldown_margin == Some(*m1) && x.cooldown_margin == Some(m1 + PARK_READ);
                (vv.control(&format!("ctl-shutter CONTROL (G1's C1) [{form}]: the park's read removed (edge_no_read) at the least-margin run where the read is a term: the twin's margin one higher (the read is a real term of the latest boarding)"), 1, bites).to_string(), format!("{}: the design's margin {:?}, without the read {:?}", desc(ar), d.cooldown_margin, x.cooldown_margin))
            }
            None => (vv.control(&format!("ctl-shutter CONTROL (G1's C1) [{form}]: the park's read removed (edge_no_read) at the least-margin run where the read is a term: the twin's margin one higher (the read is a real term of the latest boarding)"), 1, false).to_string(), "no run within 3 of the closed form's margin where the read is a term".to_string()),
        };
        // (G1 (2)'s control: the least-margin run where W + 4 makes a NEW named miss; PRINTED when no run within 3 of the
        // closed form's margin is W-bound (W + 4 can't move a run W doesn't bind: the W-from-above sweep (W 52) below is
        // the control that bites))
        let (c2, l2) = match &w_row {
            Some((_, ar)) => {
                let (ak, al) = rows_all.iter().find(|(r, _, _)| r.name == ar.name).map(|(_, k, l)| (*k, *l)).unwrap_or((Kind::Recovers, false));
                let d = run(ar, fdef);
                let xw = run(ar, Defects { w_delta: fdef.w_delta + 4, ..fdef });
                let bw = kind_bars(&xw, ak, al);
                (vv.control(&format!("ctl-shutter CONTROL (G1 on 33e731660 (2)) [{form}]: the W line + 4 at the least-margin run where W binds: a NEW named miss (the cooldown margin at boarding), the design passing there"), 1, kind_bars(&d, ak, al).is_empty() && bw.iter().any(|b| b.contains("cooldown"))).to_string(), format!("{}: the design's margin {:?}, with W + 4 {:?} [{}]", desc(ar), d.cooldown_margin, xw.cooldown_margin, bw.join("; ")))
            }
            None => ("PRINTED (no W-bound run within 3 of the closed form's margin: the W-from-above sweep is the control)".to_string(), "no run within 3 of the closed form's margin where W binds".to_string()),
        };
        lines.push(format!("  CONTROLS (G1's C1 / (2)): C1 on {l1}: {c}; W + 4 on {l2}: {c2}"));
        // (THE FINDING the twins expose: the closed form's margin is MET (the sweep's least equals it), but at none of the
        // runs that meet it is the read or W a binding term; the argmin run and its read-removed twin are printed)
        let (am_d, am_x) = argmin_at.as_ref().map_or((String::new(), String::new()), |(_, ar)| (format!("{}: {}", desc(ar), describe(&run(ar, fdef))), describe(&run(ar, Defects { edge_no_read: true, ..fdef }))));
        lines.push(format!(
            "  FINDING (G1's C1, the closed form's terms): the sweep's least margin {least_cool} = the closed form's {edge_margin} at {argmin_n} runs; the park's read IS a term there (least margin where it is: {}), but W does NOT bind within 3 of it (least margin where W + 4 makes a miss: {}): at the argmin the latest RLS rises before the W bound and the closed form's W term is met by other terms' slack; the cart reaches P {} gt after the boarding cooldown's end there; the argmin run {am_d}; with the read removed: {am_x}",
            c1_row.as_ref().map_or("none within 3".to_string(), |(m, _)| m.to_string()),
            w_row.as_ref().map_or("none within 3".to_string(), |(m, _)| m.to_string()),
            argmin_bcd.map_or("-".to_string(), |x| x.to_string())
        ));
    }
    if fi == 1 {
        let f = vg.fails();
        lines.push(format!("  (g)'s FALLBACK, PRINTED, NOT JUDGED: {} of its bars miss{}", f.len(), if f.is_empty() { String::new() } else { format!(": {}", f.iter().take(6).cloned().collect::<Vec<_>>().join(" | ")) }));
    }
    }
    // (the onset's +-1-gt edge, G1 18:16 (2): a press 1 gt before the change sets SINCE, 2 gt before doesn't)
    let edge_row = |kk: i64| {
        let mut r = Row::normal("onset edge", 5, 10, 40, 16).long(2000);
        r.hurt = vec![8];
        r.off = vec![(20, 150)];
        r.lie = vec![5, 150 + kk];
        r.resleep = None;
        run(&r, Defects::default())
    };
    // (SINCE rising within the lie-down pulse's own span, not at the later wake's fresh press)
    let since_after = |o: &Out, from: i64| o.since_rises.iter().any(|&s| s >= from && s <= from + HOLD);
    let off_at = |o: &Out, c: i64| o.press_minus_change.iter().find(|p| p.0 == c).map(|p| p.1);
    let (e1, e2) = (edge_row(1), edge_row(2));
    let ok1 = since_after(&e1, 151) && e1.rls_asleep.is_empty();
    let ok2 = !since_after(&e2, 152) && e2.rls_asleep.is_empty();
    let w1 = v.bar("ctl-shutter: the onset's edge, a press beginning 1 gt before the head change (inside the allowance) sets SINCE, and the D line's hold refuses the release", ok1);
    let w2 = v.bar("ctl-shutter: the onset's edge, a press beginning 2 gt before the head change (outside the allowance) never sets SINCE", ok2);
    lines.push(format!("  the onset's edge: a napper stepping onto P 1 gt before lying down from P: SINCE {} at its lie-down's pulse (press - change {:?}), no release to the sleeper: {w1}; 2 gt before: SINCE {} (press - change {:?}): {w2}", if since_after(&e1, 151) { "set" } else { "NOT set" }, off_at(&e1, 151), if since_after(&e2, 152) { "SET" } else { "not set" }, off_at(&e2, 152)));
    // (G1's N4: the onset's late edge: a press beginning 1 gt after the head change is still at the pulse's sample and
    // sets SINCE; 2 gt after misses it)
    let late_edge = |kk: i64| {
        let mut r = Row::normal("late edge", 5000, 10, 40, 16).long(900);
        r.lie = vec![];
        r.resleep = None;
        r.head_only = vec![100];
        r.planted = vec![(100 + kk, 400)];
        run(&r, Defects::default())
    };
    let (l1, l2) = (late_edge(3), late_edge(4));
    let set_in = |o: &Out| o.since_rises.iter().any(|&s| (100..=100 + HOLD + REP).contains(&s));
    let w1 = v.bar("ctl-shutter: the onset's late edge (the window [-1, +3], ironspec 22:15), a press beginning 3 gt after the head change sets SINCE (inside the 4-gt sample)", set_in(&l1));
    let w2 = v.bar("ctl-shutter: the onset's late edge, a press beginning 4 gt after the head change never sets SINCE", !set_in(&l2));
    lines.push(format!("  the onset's late edge (G1's N4, the window [-1, +3]): a press 3 gt after a head change at ARM + 100: SINCE {}: {w1}; 4 gt after: SINCE {}: {w2}", if set_in(&l1) { "set" } else { "NOT set" }, if set_in(&l2) { "SET" } else { "not set" }));
    let ln = {
        let mut r = Row::normal("late edge", 5000, 10, 40, 16).long(900);
        r.lie = vec![];
        r.resleep = None;
        r.head_only = vec![100];
        r.planted = vec![(102, 400)];
        run(&r, Defects { onset_narrow: true, ..Default::default() })
    };
    let c = v.control("ctl-shutter CONTROL (the onset's old 2-gt sample, [-1, +1]): a press 2 gt after the head change no longer sets SINCE (merge2's case)", 1, !set_in(&ln)).to_string();
    lines.push(format!("  CONTROL (the old [-1, +1] onset, a press 2 gt after the change): SINCE {}: {c}", if set_in(&ln) { "SET" } else { "not set" }));
    // (4) the controls (G1 on a2186649e N3): each a planted single change run on a forced row's own script (a band
    // picked where the row needs one), judged only by that row's kind bars; a control bites when the design passes the
    // row and the defect fails it
    let rows = forced_rows();
    let find = |key: &str| -> (Row, Kind, bool) { rows.iter().find(|(r, _, _)| r.name.contains(key)).cloned().unwrap_or_else(|| panic!("ctl-shutter: no forced row named {key:?}")) };
    let mut twin = |name: &str, key: &str, band: Option<(i64, i64, i64)>, open: Option<(i64, i64, i64)>, def: Defects, need: &str, lines: &mut Vec<String>| {
        let (mut r, kind, late) = find(key);
        if let Some((rest, board, ptr)) = band {
            if r.rest <= RESTS[2] {
                r.rest = rest;
            }
            r.board = board;
            r.pass_to_rest = ptr;
        }
        r.open = open;
        // (when today's superseded band makes the design fail this row's kind at the named band (C2's finding), the twin
        // moves to the row's first band where the design passes AND the defect fails for its named reason, printed:
        // a control bites on its own defect, never on the C2 finding)
        let named_fail = |o: &Out| {
            let b = kind_bars(o, kind, late);
            !b.is_empty() && (need.is_empty() || b.iter().any(|x| x.contains(need)))
        };
        if !kind_bars(&run(&r, Defects::default()), kind, late).is_empty() || !named_fail(&run(&r, def)) {
            if let Some(rr) = band_variants(&r).into_iter().find(|rr| kind_bars(&run(rr, Defects::default()), kind, late).is_empty() && named_fail(&run(rr, def))) {
                r = rr;
            }
        }
        let d = run(&r, Defects::default());
        let x = run(&r, def);
        let (db, xb) = (kind_bars(&d, kind, late), kind_bars(&x, kind, late));
        // (G1's C3 / C4: the defect must fail for the labelled reason: its named bar, not any)
        let named = need.is_empty() || xb.iter().any(|b| b.contains(need));
        let c = v.control(&format!("ctl-shutter CONTROL: {name}"), 1, db.is_empty() && !xb.is_empty() && named).to_string();
        lines.push(format!("  CONTROL ({name}; on the forced row \"{}\" [{kind:?}] at rest {} board {} pass -> rest {}{}): the design {}; the defect: {} [{}]: {c}", key, r.rest, r.board, r.pass_to_rest, if need.is_empty() { String::new() } else { format!(", failing on \"{need}\"") }, if db.is_empty() { "passes".to_string() } else { format!("FAILS ({})", db.join("; ")) }, describe(&x), xb.join("; ")));
    };
    twin("PARKED (SENSED AND COOL) removed from RLS: the park released before the cart's rest", "a hurt at ARM + 9)", Some((46, 16, 26)), None, Defects { no_parked_in_rls: true, ..Default::default() }, "the park released before the cart's rest", &mut lines);
    twin("SENSED removed (COOL alone): the park pulses before the later park's rest (the permanent hold the sensed park buys out)", "an early wake (a hurt at ARM + 30) with a later park", None, None, Defects { no_sensed: true, ..Default::default() }, "the park released before the cart's rest", &mut lines);
    twin("COOL removed (SENSED alone), pass -> rest 26 (the SENSED-limited RLS early): the merged path's cart reaches P inside the napper's boarding cooldown", "lie-down at ARM + 4, a hurt at + 0", Some((33, 13, 26)), None, Defects { no_cool: true, ..Default::default() }, "boarding cooldown", &mut lines);
    twin("the onset removed: G1's B2 box releases to the sleeper", "G1's B2 box", None, None, Defects { no_onset: true, ..Default::default() }, "RLS while the napper sleeps", &mut lines);
    // (the D line: the hold covers the onset's allowance: a napper stepping onto P 1 gt before it lies down FROM P
    // passes the onset, and its string drops at the first re-check, press + 10 = pulse + 7, the latest an onset-passing
    // press can drop: a press beginning in the change's own tick would be a napper lying down in the tick it steps onto
    // P, which never meets the string; the design reads P at pulse + HOLD through WIN's D line, its only provider)
    let (d_row, d_kind, d_late) = find("stepping onto P and lying down within 1 gt");
    let scan = |back: bool| -> String {
        [0, 2, 4, 6]
            .iter()
            .map(|&cut| {
                let o = run(&d_row, Defects { d_cut: cut, since_delay_back: back, ..Default::default() });
                format!("D line {} (P read at pulse + {}) {}", D_LINE - cut, HOLD - cut, if kind_bars(&o, d_kind, d_late).is_empty() { "holds" } else { "FAILS (a release to the sleeper)" })
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push(format!("  the D line's margin on the onset-allowance row (stepping onto P 1 gt before lying down from P: its string drops at pulse + 7, the latest drop an onset-passing press can have), the design: {}; with the old SINCE delay put back (the hold provided twice): {}", scan(false), scan(true)));
    // (G1 on 33e731660 (7): THE rls_filter_off TWIN: the filter (an RLS needs a 3-gt overlap) is MODELLED (tick-exact's AN
    // rig, assert 5, shows a 2-gt pulse lighting Q); without it the closed form's margin is 11 and the open sweep's least
    // on the cooldown-edge rows must equal it; and the D-line twin is JUDGED under it: the D line cut by 6 releases to the
    // sleeper)
    let filt = {
        let (r, kind, late) = find("stepping onto P and lying down within 1 gt");
        let off = Defects { rls_filter_off: true, ..Default::default() };
        let d = run(&r, off);
        let x = run(&r, Defects { d_cut: 6, ..off });
        let (db, xb) = (kind_bars(&d, kind, late), kind_bars(&x, kind, late));
        let ctl_ok = db.is_empty() && xb.iter().any(|b| b.contains("RLS while the napper sleeps"));
        let ctl_line = format!("  CONTROL (rls_filter_off, the D line cut by 6, on the onset-allowance row): the design {}; the cut: {} [{}]", if db.is_empty() { "passes".to_string() } else { format!("FAILS ({})", db.join("; ")) }, describe(&x), xb.join("; "));
        let x8 = run(&r, Defects { d_cut: 8, ..Default::default() });
        lines.push(format!("  PRINTED (the filter modelled: the D line cut to nothing, P read at pulse + 4): {} [{}]", describe(&x8), kind_bars(&x8, kind, late).join("; ")));
        // (the closed form without the filter: an RLS of 1 gt counts, so the latest RLS rises 2 gt later)
        let edge_off = COOL_EDGE + (RLS_MIN_W - 1);
        let (mut least_off, mut n_off) = (i64::MAX, 0usize);
        for key in ["lie-down at ARM + 4, a hurt at + 0", "lie-down at ARM + 5, a hurt at + 0", "a hurt at ARM + 9)"] {
            let (row, _, _) = find(key);
            for rr in band_variants(&row) {
                for &(idle, kj) in &OPENS {
                    for t0 in 0..=300 {
                        let mut rr2 = rr.clone();
                        rr2.open = Some((t0, idle, kj));
                        n_off += 1;
                        if let Some(m) = run(&rr2, off).cooldown_margin {
                            least_off = least_off.min(m);
                        }
                    }
                }
            }
        }
        (ctl_ok, ctl_line, edge_off, least_off, n_off)
    };
    lines.push("    (the latest drop, pulse + 7, rests on a modelling rule: a napper lying down in the tick it steps onto P never meets the string: tick-exact's pin, OWED)".into());
    // (W: G1's N1 cooldown edge, lie 5 merged at L + 0, the slowest rest with the shortest pass -> rest (SENSED binds RLS
    // at 68), the slowest ride, a world open of idle 2 with the entity domain 3 behind during the ride)
    let edge = Some((46, 19, 16));
    let edge_open = Some((80, OPEN_IDLE, OPEN_KJ));
    // (the W control from above is a sweep, after the twins: see "W 52" below)
    // (W from below, G1's B5 / C2: the early wake at + 9 needs W >= 46 to board on its first window)
    twin("P_STRING removed from OPEN's reset: G1's awake napper re-crossing P at ARM + 300 doesn't close the shutter (the close bar on every press)", "CROSSING P", None, None, Defects { no_p_in_open_reset: true, ..Default::default() }, "a press on P", &mut lines);
    twin("NOT P_STRING removed from OPEN's set: the window opens on the false-SO napper standing on P (Printed's negative)", "the false-SO path", None, None, Defects { no_notp_in_open: true, ..Default::default() }, "standing on P", &mut lines);
    twin("SINCE removed: the box on the string after the lie-down from P releases to the sleeper", "SINCE's twin row", None, None, Defects { no_since: true, ..Default::default() }, "RLS while the napper sleeps", &mut lines);
    twin("COOL's resets removed: COOL still set after the next ARM's edge", "a stray press of the park-entry detector after RLS", None, None, Defects { no_cool_reset: true, ..Default::default() }, "next ARM", &mut lines);
    twin("SO's reset removed: SO still set after the next ARM's edge", "a stray press of the park-entry detector after RLS", None, None, Defects { no_so_reset: true, ..Default::default() }, "next ARM", &mut lines);
    twin("SENSED's ARM-edge reset removed: the stray park-entry press pre-sets SENSED at the next visit (G1 17:49 (a))", "a stray press of the park-entry detector after RLS", None, None, Defects { no_sensed_arm_reset: true, ..Default::default() }, "next ARM", &mut lines);
    {
        let (ctl_ok, ctl_line, edge_off, least_off, n_off) = filt;
        let c = v.control("ctl-shutter CONTROL (the D line under rls_filter_off, G1 on 33e731660 (7)): the D line cut by 6 releases to the sleeper (the onset-allowance row)", 1, ctl_ok).to_string();
        lines.push(format!("{ctl_line}: {c}"));
        let w = v.bar(&format!("ctl-shutter: THE rls_filter_off TWIN: the closed form without the 3-gt filter (margin COOLDOWN - {edge_off} = {}) equals the open sweep's least margin on the cooldown-edge rows without it (a bound both ways)", COOLDOWN - edge_off), least_off == COOLDOWN - edge_off);
        lines.push(format!("  THE rls_filter_off TWIN: the closed form's margin {} (>= {COOLDOWN_BAR}: {}), the open sweep's least without the filter {least_off} over {n_off} runs of the cooldown-edge rows: {w}", COOLDOWN - edge_off, if COOLDOWN - edge_off >= COOLDOWN_BAR { "passes" } else { "FAILS" }));
    }
    // (W from below and BoardsEarly's bar, G1's B5 / C2: the + 9 wake's window is cut at the lie pulse + W + 11, so at
    // the COOL-limited band (SENSED early: rest 33, pass -> rest 26) it boards on its first window only with W >= 47;
    // W 44 sends it through its re-sleep (liveness, not safety); and judged as BoardsEarly at rest 46 / pass -> rest
    // 16, where SENSED releases past the cut, the strict first-window bar must fail)
    {
        let (mut r, _, _) = find("a hurt at ARM + 9)");
        r.rest = 33;
        r.board = 16;
        r.pass_to_rest = 26;
        let first = |o: &Out| o.boarded_at.is_some_and(|bt| !o.wakes.first().is_some_and(|&w0| o.lies.iter().any(|&l| l > w0 && l < bt)));
        let d = run(&r, Defects::default());
        let x = run(&r, Defects { w_delta: -4, ..Default::default() });
        let c = v.control("ctl-shutter CONTROL: the W line two steps shorter (W 44): the early wake at ARM + 9 at the COOL-limited band (rest 33, pass -> rest 26) misses its first window (W's lower bound: 47 for this row)", 1, first(&d) && !first(&x) && x.boarded_at.is_some()).to_string();
        lines.push(format!("  CONTROL (W 44 on the + 9 wake, rest 33 / pass -> rest 26): the design {}; W 44 {}: {c}", describe(&d), describe(&x)));
        // (G1 on 33e731660 (4): the design-run "control" (the + 9 wake judged BoardsEarly at rest 46 / pass -> rest 16)
        // counted the design's own C2 miss as a control biting: DELETED; the late-launch negative below is the real one)
    }
    // (W FROM ABOVE, a sweep (the twin's single row fails C2 at the late-SENSED band): W 52 over the cooldown-edge rows'
    // full band and every open tick 40..120 must break the cooldown bar somewhere: the closed form's W bound is real)
    {
        let (mut w_n, mut w_bad, mut w_least) = (0usize, Vec::<String>::new(), i64::MAX);
        for key in ["lie-down at ARM + 4, a hurt at + 0", "lie-down at ARM + 5, a hurt at + 0", "a hurt at ARM + 9)"] {
            let (row, _, _) = find(key);
            for rr in band_variants(&row) {
                for &(idle, kj) in &OPENS {
                    for t0 in 40..=120 {
                        let mut r = rr.clone();
                        r.open = Some((t0, idle, kj));
                        w_n += 1;
                        let x = run(&r, Defects { w_delta: 4, ..Default::default() });
                        if let Some(m) = x.cooldown_margin {
                            w_least = w_least.min(m);
                            if m < COOLDOWN_BAR && w_bad.len() < 2 {
                                w_bad.push(format!("{key} (rest {} board {} pass -> rest {}) open at {t0} idle {idle} kj {kj}: margin {m}", r.rest, r.board, r.pass_to_rest));
                            }
                        }
                    }
                }
            }
        }
        let c = v.control(&format!("ctl-shutter CONTROL (W from above): W {} (two repeater ticks over the closed form's room) breaks the cooldown bar on the cooldown-edge rows' open sweep", W_LINE + 4), 1, !w_bad.is_empty()).to_string();
        lines.push(format!("  CONTROL (W {} over {w_n} open runs of the cooldown-edge rows: least margin {w_least}): {}: {c}", W_LINE + 4, if w_bad.is_empty() { "no run breaks the bar".to_string() } else { w_bad.join(" | ") }));
    }
    // ((g)'s FALLBACK at W 46, PRINTED (dead at W 46: ironspec 22:22 (5)): the first windows of the + 9 wake per band, at W
    // 46 and W 48)
    {
        let torch = |w: i64| Defects { torch: true, w_delta: w - W_LINE, ..Default::default() };
        let (row9, _, _) = find("a hurt at ARM + 9)");
        let first = |o: &Out| o.boarded_at.is_some_and(|bt| !o.wakes.first().is_some_and(|&w0| o.lies.iter().any(|&l| l > w0 && l < bt)));
        let (mut at46, mut at48, mut nb9) = (0usize, 0usize, 0usize);
        for rr in band_variants(&row9) {
            nb9 += 1;
            at46 += first(&run(&rr, torch(W_TORCH))) as usize;
            at48 += first(&run(&rr, torch(48))) as usize;
        }
        lines.push(format!("  (g)'s FALLBACK, PRINTED: the + 9 wake boards on its first window on {at46} of {nb9} bands at W 46 (G1's zero slack: (g) is dead at W 46) and on {at48} of {nb9} at W 48"));
    }
    // (THE LATCH ALONE, at the torch / repeater level with 26.3's tick order (G1 21:25, ironspec 21:28 / 21:32))
    let d0 = LatchOpts::default();
    let (w1, p1, _, _) = latch_micro(1, None, d0, None);
    let (w2, p2, _, _) = latch_micro(2, None, d0, None);
    let (w3, p3, t3, dp3) = latch_micro(3, None, d0, None);
    let (w20, p20, _, _) = latch_micro(20, None, d0, None);
    let win = |p: Option<(i64, i64)>| p.map_or("never".to_string(), |(a, b)| format!("[{a}, {b})"));
    let filtered = w1 == 0 && w2 == 0 && p1.is_none() && p2.is_none();
    let w = v.bar("ctl-shutter (MODELLED: LatchSim, pending the AND rig on real cells): THE STRUCTURAL RLS >= 3: every input reaches RLS's OR block through its input repeater (a diode tick, before the torches), so RLS's output torch, reading its block at its own scheduled tick, turns a 1- or 2-gt gap into NO RLS at all (no B2, no PARK, nothing consumed) and a gap of 3 into an RLS of 3", filtered && w3 == 3);
    lines.push(format!("  the latch alone: an OR-block gap of 1 gt: RLS {w1} gt, PARK {}; of 2: RLS {w2} gt, PARK {}; of 3: RLS {w3} gt, PARK {}, the tap {}, Q-bar checks reading B2 unpowered {:?}: {w}", win(p1), win(p2), win(p3), win(t3), dp3));
    let fixed = |p: Option<(i64, i64)>, r: i64| p == Some((r + PARK_TORCH, r + PARK_TORCH + PARK_W));
    let w = v.bar(&format!("ctl-shutter (MODELLED: LatchSim; form (b)'s game row agrees: Q [+3, +23) from the command for every width, tick-exact 23:47): the latch's Q (P_HOLD) is the fixed window [RLS + 4, RLS + 4 + {PARK_W}) for a 3-gt RLS and a 20-gt RLS alike (the reset dominates at the base)"), fixed(p3, 12) && fixed(p20, 12));
    lines.push(format!("    Q for a 3-gt RLS rising at 12: {}; for a 20-gt RLS: {} (design [{}, {})): {w}", win(p3), win(p20), 12 + PARK_TORCH, 12 + PARK_TORCH + PARK_W));
    let (f1, fp1, _, _) = latch_micro(0, Some(1), d0, None);
    let (f2, fp2, _, _) = latch_micro(0, Some(2), d0, None);
    let (_, fpn, _, _) = latch_micro(0, Some(2), LatchOpts { branch: false, ..d0 }, None);
    let w = v.bar(&format!("ctl-shutter (MODELLED: LatchSim; form (b)'s game row: the branch keeps the drive gapless): the branch repeater covers B2 without a gap for an RLS line forced to 1 or 2 gt (a NORMAL-priority path as a stress): Q sets, the window [RLS + 4, RLS + 4 + {PARK_W})"), fixed(fp1, 12) && fixed(fp2, 12));
    lines.push(format!("    an RLS line forced to {f1} gt: Q {}; to {f2} gt: Q {}: {w}", win(fp1), win(fp2)));
    let c = v.control("ctl-shutter CONTROL (the branch removed): an RLS line forced to 2 gt races Q-bar's check at RLS + 2 and the latch never sets (harmless under the tap-keyed consumers: nothing consumed, RLS retries)", 1, fpn.is_none()).to_string();
    lines.push(format!("  CONTROL (no branch, a forced 2-gt RLS line): Q {}: {c}", win(fpn)));
    let (_, pu_set, _, _) = latch_micro(0, None, LatchOpts { power_up_set: true, ..d0 }, Some(10));
    let (_, pu_noarm, _, _) = latch_micro(0, None, LatchOpts { power_up_set: true, arm_reset: false, ..d0 }, Some(10));
    let w = v.bar("ctl-shutter (MODELLED: LatchSim; tick-exact 23:47: ARM must hold >= 3 gt to clear Q): POWER-UP: a latch that comes up SET is cleared by ARM's edge through its diode (Q dark within 10 gt of the edge)", pu_set.is_some_and(|(_, b)| b <= 20));
    lines.push(format!("    power-up SET, ARM's edge at 10: Q lit until {}: {w}", pu_set.map_or("-".into(), |(_, b)| b.to_string())));
    let c = v.control("ctl-shutter CONTROL (the ARM-edge reset removed): a latch that came up SET stays SET (the park powered at rest)", 1, pu_noarm.is_some_and(|(_, b)| b == i64::MAX)).to_string();
    lines.push(format!("  CONTROL (power-up SET without the ARM-edge reset): Q lit until {}: {c}", pu_noarm.map_or("-".into(), |(_, b)| if b == i64::MAX { "never cleared".into() } else { b.to_string() })));
    // (G1 on 33e731660 (6): THE LATCH FINDING, stated: the as-listed latch (58f770cc 22:07) does NOT hold in 26.3 (tick-
    // exact's rig a/2239880473237753: its descending dust can't step down the glass staircase); FORM (b), the one-level
    // two-torch NOR latch (ironspec 22:55 (2), day-1's list a/1902620467569082), LATCHES in 26.3 (tick-exact 23:47,
    // a/1841767613704681: Q exactly [+3, +23) from the command for every RLS width 1..30; the drive [+1, +25) for widths
    // <= 20; ARM clears Q only when held >= 3 gt); the LatchSim bars above are MODELLED on form (b); PARK's 24 gt, the tap
    // consumers and the consistency / leave-park bars rest on them; the latch cites when its game row is on the cells AS
    // LAID: an OWED miss until then)
    lines.push("  THE LATCH FINDING: the as-listed latch does NOT hold in 26.3 (tick-exact a/2239880473237753); FORM (b), the one-level NOR latch, latches (tick-exact 23:47, a/1841767613704681: Q [+3, +23) for every RLS width 1..30, the drive [+1, +25) for widths <= 20, ARM clears only when held >= 3 gt); the LatchSim bars above are MODELLED on form (b)".into());
    let w = v.bar("ctl-shutter OWED: the P_HOLD latch's game row on the cells AS LAID (form (b) transplanted one level down into the re-laid stop, tick-exact's commit with its plants, ARM's 3-gt stretcher asserted): the LatchSim bars cite only then", false);
    lines.push(format!("  OWED (form (b)'s game row on the cells as laid): {w}"));
    // (THE VISIT WITH THE LATCH'S PLANTS, on the plain drive of record unless named; each row judged by its kind bars,
    // which carry the consistency bar both ways)
    let plant = |name: &str, key: &str, edit: &dyn Fn(&mut Row), def: Defects, kind: Kind, lines: &mut Vec<String>, v: &mut Verdict| {
        let (mut r, _, late) = find(key);
        edit(&mut r);
        let o = run(&r, def);
        let b = kind_bars(&o, kind, late);
        let m = v.bar(&format!("ctl-shutter planted row ({kind:?}): {name}"), b.is_empty());
        lines.push(format!("  PLANTED ({name}): RLS widths {:?}, PARK {} (ends {}), consumed {}, launched {:?}: {}{}: {m}", o.rls_widths, fmt_ticks(&o.park), fmt_ticks(&o.park_end), fmt_ticks(&o.consumed), o.launched, describe(&o), if b.is_empty() { String::new() } else { format!(" [{}]", b.join("; ")) }));
    };
    for so in [0, 1, 2] {
        plant(&format!("the napper stepping off P at RLS + {so} (a fault path: LAST_WOKEN keeps a re-sleep >= 41 gt from any RLS, so only a forced move reaches it)"), "the step-off (a forced displacement", &|r| r.step_off = Some(so), Defects::default(), Kind::PermanentHalt, &mut lines, v);
    }
    // (STRESS, PRINTED: RLS forced to 1 or 2 gt, unreachable through the AND's output torch, as a NORMAL-priority path
    // would give it: what each consumer does)
    for (wf, key, where_) in [(1, "a hurt at ARM + 9)", "at the window's end (the + 9 wake)"), (2, "a hurt at ARM + 9)", "at the window's end (the + 9 wake)"), (1, "the self-healing homeless napper", "mid-window"), (2, "the self-healing homeless napper", "mid-window")] {
        let (mut r, _, _) = find(key);
        r.rls_force_w = Some(wf);
        for (dn, dd) in [("the plain drive", Defects::default()), ("(g)", Defects { torch: true, w_delta: W_TORCH - W_LINE, ..Default::default() })] {
            let o = run(&r, dd);
            lines.push(format!("  STRESS, PRINTED ({dn}, RLS forced to {wf} gt {where_}): RLS widths {:?}, PARK {} (ends {}), consumed {}, launched {:?}: {} [{}]", o.rls_widths, fmt_ticks(&o.park), fmt_ticks(&o.park_end), fmt_ticks(&o.consumed), o.launched, describe(&o), invariants(&o).join("; ")));
        }
    }
    plant("RLS held high for 20 gt (PARK, the consumers and the cart: one visit)", "the self-healing homeless napper", &|r| r.rls_hold = Some(20), Defects::default(), Kind::Normal, &mut lines, v);
    plant("(g)'s fallback: RLS held high for 20 gt (PARK still [RLS + 4, RLS + 12): the reset dominates at the base)", "the self-healing homeless napper", &|r| r.rls_hold = Some(20), Defects { torch: true, w_delta: W_TORCH - W_LINE, ..Default::default() }, Kind::Normal, &mut lines, v);
    // (the plants keyed on the row's own RLS tick, read from its design run: entity ticks equal game ticks without an open)
    let r0_heal = {
        let (r, _, _) = find("the self-healing homeless napper");
        run(&r, Defects::default()).rls.first().copied().unwrap_or(0)
    };
    let r0_step = {
        let (mut r, _, _) = find("the step-off (a forced displacement");
        r.step_off = None;
        run(&r, Defects::default()).rls.first().copied().unwrap_or(0)
    };
    for d in [0, 2, 4, 6, 8] {
        plant(&format!("a stray press of the park-entry detector at RLS + {d} (it must not pre-set the next visit)"), "the self-healing homeless napper", &|r| r.park_stray = vec![r0_heal + d], Defects::default(), Kind::Normal, &mut lines, v);
    }
    for d in [1, 4, 8] {
        let (mut r, _, _) = find("the step-off (a forced displacement");
        r.step_off = None;
        r.lie = vec![5, r0_step + d];
        r.resleep = None;
        let o = run(&r, Defects::default());
        lines.push(format!("  FAULT PATH, PRINTED (a re-sleep pulse at RLS + {d}: unreachable, since LAST_WOKEN keeps a re-sleep >= 41 gt from any RLS; the napper lying down again before boarding): RLS widths {:?}, PARK {} (ends {}), consumed {}, launched {:?}: {} [{}]", o.rls_widths, fmt_ticks(&o.park), fmt_ticks(&o.park_end), fmt_ticks(&o.consumed), o.launched, describe(&o), invariants(&o).join("; ")));
    }
    // (power-up in the visit: SET with the ARM-edge reset (the first arrival braked) and without it (its control))
    {
        let (mut r, _, _) = find("the self-healing homeless napper");
        r.power_up_set = true;
        let o = run(&r, Defects::default());
        let x = run(&r, Defects { no_arm_reset: true, ..Default::default() });
        let entry = r.arm + r.rest - r.pass_to_rest;
        let w = v.bar("ctl-shutter: POWER-UP in the visit: a latch that came up SET is cleared before the first cart passes the park-entry detector (the park unpowered when the cart arrives: it brakes); what follows printed (ironspec 21:29 (3): an observer pulse inside the held reset sends the visit down the natural-wake path, liveness only, at power-up only)", o.park_end.first().is_some_and(|&pe| pe <= entry) && o.park.len() == 1 + o.rls.len());
        lines.push(format!("  POWER-UP SET (the visit): {}; SO set at the first lie-down's pulse: {}: {w}", describe(&o), if o.pulses.is_empty() { "no pulse".to_string() } else { "see OPEN / RLS above".to_string() }));
        let c = v.control("ctl-shutter CONTROL (power-up SET without the ARM-edge reset): the park is powered when the first cart arrives (it rides through unbraked)", 1, !x.park_before_rest.is_empty() || x.departed.map(|d| !d.1).unwrap_or(false)).to_string();
        lines.push(format!("  CONTROL (power-up SET, no ARM-edge reset): {}; park powered before the rest at {}: {c}", describe(&x), fmt_ticks(&x.park_before_rest)));
    }
    // (THE NON-WIDTH LATCH FAILURE (B2's set blocked), G1 21:28 (2) / ironspec 21:31 (2): under (g) nothing is consumed and
    // RLS retries (a visible stuck stop, never a consumed hold); the resets on RLS or on RLS_S consume the visit with no
    // PARK (the hold, failing by name); THE RESIDUAL of the plain drive (ironspec 21:51), printed: PARK still fires on
    // RLS alone, without the latch, and launches the cart without consuming the visit)
    {
        let (r, _, _) = find("the self-healing homeless napper");
        let g = run(&r, Defects { torch: true, w_delta: W_TORCH - W_LINE, b2_blocked: true, ..Default::default() });
        let gb = invariants(&g);
        let w = v.bar("ctl-shutter: (g)'s fallback with B2's set blocked: no PARK, nothing consumed, RLS free to rise again (the consistency bar holds)", g.park.is_empty() && g.consumed.is_empty() && gb.iter().all(|b| !b.contains("consumed")));
        lines.push(format!("  (g) with B2's set blocked: {}; RLS rises {}: {w}", describe(&g), fmt_ticks(&g.rls)));
        let xr = run(&r, Defects { torch: true, w_delta: W_TORCH - W_LINE, b2_blocked: true, resets_on_rls: true, ..Default::default() });
        let c = v.control("ctl-shutter CONTROL (g with B2 blocked, the consuming resets on RLS): the visit consumed with no PARK (the hold), failing the consistency bar by name", 1, invariants(&xr).iter().any(|b| b.contains("without a PARK"))).to_string();
        lines.push(format!("  CONTROL (resets on RLS): {} [{}]: {c}", describe(&xr), invariants(&xr).join("; ")));
        let xs = run(&r, Defects { torch: true, w_delta: W_TORCH - W_LINE, b2_blocked: true, resets_on_rls_s: true, ..Default::default() });
        let c = v.control("ctl-shutter CONTROL (g with B2 blocked, the consuming resets on RLS_S, ironspec 21:26's withdrawn form): the visit consumed with no PARK", 1, invariants(&xs).iter().any(|b| b.contains("without a PARK"))).to_string();
        lines.push(format!("  CONTROL (resets on RLS_S): {} [{}]: {c}", describe(&xs), invariants(&xs).join("; ")));
        let p = run(&r, Defects { b2_blocked: true, ..Default::default() });
        lines.push(format!("  RESIDUAL, PRINTED (the plain drive of record with B2's set inputs blocked: no B2, so no PARK at all; what the run shows, not a claim): {}; consumed {}; what the bars say: [{}]", describe(&p), fmt_ticks(&p.consumed), invariants(&p).join("; ")));
    }
    // (G1 on 33e731660 (9)'s control: the reset chain cut to PARK_W 8 on the plain drive, under a 3-gt RLS (the structural
    // minimum, planted): PARK ends before the resting cart leaves the park, and the leave-park bar fails by name)
    {
        let (mut r, kind, late) = find("the self-healing homeless napper");
        r.rls_force_w = Some(3);
        let d = run(&r, Defects::default());
        let x = run(&r, Defects { park_w_override: 8, ..Default::default() });
        let (db, xb) = (kind_bars(&d, kind, late), kind_bars(&x, kind, late));
        let c = v.control("ctl-shutter CONTROL (G1 on 33e731660 (9)): PARK_W 8 on the plain drive under a 3-gt RLS: the cart can't leave the park inside PARK (the leave-park bar fails by name; the design passes)", 1, db.is_empty() && xb.iter().any(|b| b.contains("PARK's end") || b.contains("never launched"))).to_string();
        lines.push(format!("  CONTROL (PARK_W 8, RLS planted 3 gt): the design {}; PARK_W 8: {} [{}]: {c}", if db.is_empty() { "passes".to_string() } else { format!("FAILS ({})", db.join("; ")) }, describe(&x), xb.join("; ")));
    }
    // (each kind's negative, G1 on a2186649e B6: the row's script with its defining event removed must fail its kind)
    // (G1's B6: each negative must fail for its NAMED reason, not any; every bar of the kind INCLUDING the first window)
    let mut neg = |name: &str, key: &str, edit: &dyn Fn(&mut Row), need: &str, lines: &mut Vec<String>| {
        let (mut r, kind, late) = find(key);
        edit(&mut r);
        let o = run(&r, Defects::default());
        let mut b = kind_bars(&o, kind, late);
        b.extend(first_window_miss(&o, kind));
        let c = v.control(&format!("ctl-shutter CONTROL (the kind's negative, failing on \"{need}\"): {name}"), 1, b.iter().any(|x| x.contains(need))).to_string();
        lines.push(format!("  CONTROL (the kind's negative: {name}; failing on \"{need}\"): {} [{}]: {c}", describe(&o), b.join("; ")));
    };
    neg("PermanentHalt: the step-off row without its step-off has no empty departure", "the step-off (a forced displacement", &|r| r.step_off = None, "no empty departure", &mut lines);
    neg("SelfEnding: the self-ending row without its later lie-down never boards after it", "self-ending at that lie-down", &|r| r.lie = vec![6], "not boarded after its scripted later lie-down", &mut lines);
    neg("Normal: the normal-kind self-heal row with a step-off fails the step-off line", "the self-healing homeless napper", &|r| r.step_off = Some(5), "the step-off line", &mut lines);
    neg("Recovers: the planted stale press row with neither Z1 nor a dawn nor a re-sleep never boards", "a planted stale press over [lie - 10, lie - 1]", &|r| { r.resleep = None; r.z1 = false; r.dawn = None; }, "never boards", &mut lines);
    // (BoardsEarly's negative through `run`: the cart's launch delayed 30 gt past the first window's bound)
    {
        let (mut r, kind, late) = find("a hurt at ARM + 9)");
        r.rest = 33;
        r.board = 13;
        r.pass_to_rest = 26;
        let o = run(&r, Defects { late_launch: 30, ..Default::default() });
        let mut b = kind_bars(&o, kind, late);
        b.extend(first_window_miss(&o, kind));
        let d = run(&r, Defects::default());
        let d_ok = kind_bars(&d, kind, late).is_empty() && first_window_miss(&d, kind).is_none();
        let c = v.control("ctl-shutter CONTROL (the kind's negative, failing on \"first window\"): BoardsEarly: the + 9 row's launch delayed 30 gt boards past its first window's bound (the design passes at this point)", 1, d_ok && b.iter().any(|x| x.contains("first window"))).to_string();
        lines.push(format!("  CONTROL (BoardsEarly's negative: the launch delayed 30 gt; the design at this point {}): {} [{}]: {c}", if d_ok { "passes" } else { "FAILS" }, describe(&o), b.join("; ")));
    }
    // (G1's N5 control: a planted box pressing P while the window is open (on the late lie-down's sleeper), with P
    // removed from OPEN's reset: the press never closes the window in time)
    let (mut nr, nk, nl) = find("a late lie-down (ARM + 218");
    nr.planted = vec![(240, 246)];
    let nd = run(&nr, Defects::default());
    let nx = run(&nr, Defects { no_p_in_open_reset: true, ..Default::default() });
    let (ndb, nxb) = (kind_bars(&nd, nk, nl), kind_bars(&nx, nk, nl));
    let c = v.control("ctl-shutter CONTROL (G1's N5): a planted box on P at ARM + 240..246 while the window is open on the late sleeper, P removed from OPEN's reset: the box's press doesn't close the window (every press judged)", 1, ndb.is_empty() && nxb.iter().any(|b| b.contains("a press on P"))).to_string();
    lines.push(format!("  CONTROL (the planted box on P while the window is open): the design {}; the defect {} [{}]: {c}", if ndb.is_empty() { "passes".to_string() } else { format!("FAILS ({})", ndb.join("; ")) }, describe(&nx), nxb.join("; ")));
    // (G1's N5 on 2260db896: the "never closed it" branch driven by a run: the same plant with the run ending before any
    // later close (the horizon cut 15 gt after the press))
    let mut nr2 = nr.clone();
    nr2.horizon = 261;
    nr2.next_arm = 230;
    let nx2 = run(&nr2, Defects { no_p_in_open_reset: true, ..Default::default() });
    let c = v.control("ctl-shutter CONTROL (G1's N5, the unclosed branch): the planted press with P out of OPEN's reset and the run ending first: \"never closed it\" fires", 1, invariants(&nx2).iter().any(|b| b.contains("never closed it"))).to_string();
    lines.push(format!("  CONTROL (the press never closed: the run cut at 261): {} [{}]: {c}", describe(&nx2), invariants(&nx2).join("; ")));
    // (G1's C5 control: the extender as a two-input OR (RLS and RLS through 8 gt) leaves a gap under a narrow RLS; over
    // the open sweep of the cooldown-edge rows it fails where the gapless latch passes)
    let mut or_n = 0usize;
    let mut or_bad: Vec<String> = Vec::new();
    for key in ["lie-down at ARM + 4, a hurt at + 0", "lie-down at ARM + 5, a hurt at + 0", "a hurt at ARM + 9)"] {
        let (row, kind, late) = find(key);
        let k = if kind == Kind::BoardsEarly { Kind::Boards } else { kind };
        for rr in band_variants(&row) {
            for &(idle, kj) in &OPENS {
                for t0 in 40..=120 {
                    let mut r = rr.clone();
                    r.open = Some((t0, idle, kj));
                    or_n += 1;
                    let x = run(&r, Defects { old_drive: true, park_or2: true, ..Default::default() });
                    let xb = kind_bars(&x, k, late);
                    if !xb.is_empty() && or_bad.len() < 2 {
                        or_bad.push(format!("{key} (rest {} board {} pass -> rest {}) open at {t0} idle {idle} kj {kj}: {}", r.rest, r.board, r.pass_to_rest, xb.join("; ")));
                    }
                }
            }
        }
    }
    let c = v.control("ctl-shutter CONTROL (G1's C5): PARK through the two-input OR (RLS and RLS through 8 gt) instead of the gapless latch: over the open sweep of the cooldown-edge rows a swallowed first pulse moves the cart 8 gt later and a row fails", 1, !or_bad.is_empty()).to_string();
    lines.push(format!("  CONTROL (the two-input OR extender over {or_n} open runs of the cooldown-edge rows): {}: {c}", if or_bad.is_empty() { "no row fails".to_string() } else { or_bad.join(" | ") }));
    // (the park's pulse extender: the early wake at ARM + 9, whose RLS is 3 gt wide (the lie-down pulse's W line closes
    // WIN just after PARKED), with a world open at 67 holding the entities 3 gt behind the blocks)
    let (mut pr, _, _) = find("a hurt at ARM + 9)");
    pr.open = Some((67, 1, 3));
    let pd = run(&pr, Defects::default());
    let px = run(&pr, Defects { old_drive: true, no_park_stretch: true, ..Default::default() });
    let c = v.control("ctl-shutter CONTROL: PARK without its pulse extender: the early wake's 3-gt RLS, a world open holding the entities 3 gt behind: the cart misses the whole PARK pulse and stays on the park (a permanent hold); with the extender it boards", 1, pd.departed.map(|d| d.1).unwrap_or(false) && px.departed.is_none()).to_string();
    lines.push(format!("  CONTROL (PARK's pulse extender, the early wake at ARM + 9 with a world open at 67, idle 1, k + j 3): the design {}; without the extender {}: {c}", describe(&pd), describe(&px)));
    // (SINCE's old delay put back, JUDGED (G1 on 33e731660 (3)): ironspec 18:12 (d) removed it when no row changed; with C2
    // judged and the onset window [-1, +3] it changes rows: a judged miss naming each changed row and its direction, every
    // bar of the row's kind INCLUDING the first window)
    let all_bars = |o: &Out, kind: Kind, late: bool| {
        let mut b = kind_bars(o, kind, late);
        b.extend(first_window_miss(o, kind));
        b
    };
    let mut changed = Vec::new();
    for (row, kind, late) in forced_rows() {
        let a = all_bars(&run(&row, Defects::default()), kind, late).is_empty();
        let b = all_bars(&run(&row, Defects { since_delay_back: true, ..Default::default() }), kind, late).is_empty();
        if a != b {
            changed.push(format!("{} ({})", row.name.chars().take(60).collect::<String>(), if b { "passes WITH the old delay" } else { "fails with the old delay" }));
        }
    }
    let w = v.bar("ctl-shutter: SINCE's old delay put back (4 gt more on SINCE's set path) changes no forced row's verdict (G1 on 33e731660 (3): judged; the rows it changes named)", changed.is_empty());
    lines.push(format!("  SINCE's old delay put back: {}: {w}", if changed.is_empty() { "no forced row's verdict changes".to_string() } else { format!("CHANGED on {}", changed.join(" | ")) }));
    lines.push("  OWED (not in this logic bench): the golem cost and the rest of the farm during a halt (the counter / coverage halt row); the real napper's per-tick cell and the hold delta's (-2,0,5) entry (58f770cc's stop bench on this wiring); tick-exact's press / pulse row, from which the onset's allowance is set".into());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_normal_visit_releases_once_with_the_napper_aboard() {
        let o = run(&Row::normal("t", 5, 10, 40, 16), Defects::default());
        assert_eq!(o.rls.len(), 1, "{}", describe(&o));
        assert_eq!(o.departed.map(|d| d.1), Some(true), "{}", describe(&o));
        assert!(normal_bars(&o, false).is_empty(), "{:?}", normal_bars(&o, false));
    }

    #[test]
    fn the_onset_refuses_a_box_standing_through_the_lie_down() {
        let mut r = Row::normal("t", 218, 10, 40, 16).long(2000);
        r.planted = vec![(208, 240)];
        let o = run(&r, Defects::default());
        assert!(o.rls_asleep.is_empty(), "{}", describe(&o));
        let o = run(&r, Defects { no_onset: true, ..Default::default() });
        assert!(!o.rls_asleep.is_empty(), "the control: {}", describe(&o));
    }

    #[test]
    fn the_step_off_shows_on_the_cell_line() {
        let mut r = Row::normal("t", 5, 10, 40, 16);
        r.step_off = Some(5);
        let o = run(&r, Defects::default());
        assert!(o.cells.contains('x') && !normal_bars(&o, false).is_empty(), "{} / {}", describe(&o), o.cells);
    }

    #[test]
    fn the_bench_prints() {
        // (G1 on 2260db896 / G1 22:21 / ironspec 22:22 (3), G1 on 33e731660: the test asserts the bench's EXACT verdict:
        // on today's superseded park geometry the C2 bar (first-window boarding) fails for exactly these rows, at the
        // COOL- and SENSED-limited bands, over the band and over the open sweep; every other bar of every row passes;
        // every control bites; the CLI exits non-zero on it; when the re-laid stop's band lands, this pin changes as a
        // visible diff)
        let mut v = Verdict::new("ctl-shutter-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
        let fails = v.fails();
        let rows = [
            "an early wake from the bed (a hurt at ARM + 7)",
            "an early wake from the bed (a hurt at ARM + 8)",
            "an early wake from the bed (a hurt at ARM + 9)",
            "an early wake from the bed (a hurt at ARM + 30)",
            "the merged path from the bed (lie-down at ARM + 4, a hurt at + 0)",
            "the merged path from the bed (lie-down at ARM + 5, a hurt at + 0)",
            "the merged path from the bed (lie-down at ARM + 7, a hurt at + 0)",
            "the merged path from the bed (lie-down at ARM + 6, a hurt at + 1)",
            "the merged path from the bed (lie-down at ARM + 6, a hurt at + 2)",
            "the merged path from the bed (lie-down at ARM + 7, a hurt at + 3)",
        ];
        // (G1 on 33e731660 (2): C2 pinned per row, over the band and over the open sweep, every other bar at 0 misses;
        // (3) the old-delay bar judged (it passes now: SINCE's sample-gated reset made both delays equal on every row);
        // (6) the latch's OWED game row; (7) the rls_filter_off twin's FINDING: the closed form without the filter (11) is
        // one below the sweep's least without it (12): conservative by one)
        let mut expected: Vec<(String, bool)> = Vec::new();
        for r in rows {
            expected.push((format!("C2, THE FIRST WINDOW (FORCED wake, by wake + 90): {r}"), true));
            expected.push((format!("C2 OVER THE OPEN SWEEP (the first window, a FORCED wake by wake + 90): {r}"), true));
        }
        expected.push(("THE rls_filter_off TWIN".to_string(), false));
        expected.push(("ctl-shutter OWED: the P_HOLD latch's game row on the cells AS LAID".to_string(), false));
        assert_eq!(fails.len(), expected.len(), "ctl-shutter-bench: the pinned verdict changed ({} misses, {} pinned): {:?}", fails.len(), expected.len(), fails);
        for (e, per_form) in &expected {
            assert_eq!(fails.iter().filter(|f| f.contains(e.as_str()) && (!per_form || f.contains("the plain drive of record"))).count(), 1, "ctl-shutter-bench: the pinned miss \"{e}\" is not exactly once among {:?}", fails);
        }
        // (G1 on 60ada7ea8 B1: the verdict fails the run: no flag turns it into exit 0)
        assert!(v.finish(false).is_err(), "ctl-shutter-bench: the pinned verdict must fail the run");
    }

    #[test]
    fn the_two_drives_release_alike_at_one_w() {
        // (the drive changes PARK, never RLS: at the same W both drives release on the same tick)
        let rows = forced_rows();
        let (mut r, _, _) = rows.iter().find(|(r, _, _)| r.name.contains("lie-down at ARM + 5, a hurt at + 0")).cloned().unwrap();
        r.rest = 46;
        r.board = 19;
        r.pass_to_rest = 16;
        r.open = Some((80, OPEN_IDLE, OPEN_KJ));
        let p = run(&r, Defects::default());
        let t = run(&r, Defects { torch: true, ..Default::default() });
        assert_eq!(p.rls, t.rls, "plain {} / torch {}", describe(&p), describe(&t));
    }
}
