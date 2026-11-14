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
/// RLS to the park's powered rail, p (ironspec 17:35 (3) / 17:53 (1): one delay-1 repeater, 2 gt).
pub const PARK: i64 = 2;
/// THE PARK's GAPLESS EXTENDER (G1's C5 on 5dd4204be, a design item): RLS can be as narrow as 3 gt (a previous pulse's W
/// line closes WIN just after PARKED releases, the early wake at ARM + 9), and a world open's entity lag (3) then
/// swallows the whole PARK pulse: the cart stays on the park with RLS spent (a permanent hold). A two-input OR (RLS and
/// RLS through an 8-gt line) leaves a GAP whenever RLS is shorter than 8 gt, and a cart that misses the first part
/// boards 8 gt later, at the cooldown edge (kept as a control). THE BUILT FORM: PARK = RLS OR P_HOLD, P_HOLD a latch SET
/// by RLS and RESET by RLS through an 8-gt repeater pair (two delay-4): the park powered without a gap from RLS's rise
/// until RLS's end + 8 at the earliest (P_HOLD's Q from RLS + SET_IN to RLS's fall + 8 + RESET_IN), ~4 blocks (the two
/// torches, the pair, the OR's dust).
pub const PARK_W: i64 = 8;
/// The cart reads the park's power in its next tick (the rail's state of the previous block tick: G1's C1, a step of
/// its own in the closed form beside the cart's first move).
pub const PARK_READ: i64 = 1;
/// WIN's end: the pulse through the W line (6 x delay 4 + 1 x delay 4 - 4 = 48: ironspec on G1's N1, day-1 19:25,
/// one repeater tick shorter than the ruled 50): the latest RLS lands at pulse + W_LINE + W_SLACK = pulse + 59, so
/// the cooldown edge keeps its >= 10 at boarding with every term (see `COOL_EDGE`).
pub const W_LINE: i64 = 48;
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
/// output (4), the AND (4), less the first blocked tick.
pub const W_SLACK: i64 = RESET_IN + TORCH + REP + AND - 1;
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
            let park_prev = prev(&park);
            match cart {
                C::Arriving if e >= row.arm + row.rest => cart = C::Rest,
                C::Rest if park_prev => cart = C::ToP(e + CART_RESPONSE + row.board),
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
        let v_so = latch(prev(&q_so), s_at(&|t| obs_at(t)), r_at(&|t| !def.no_so_reset && rls_r(t)));
        let v_sen = latch(prev(&q_sen), s_at(&|t| at(&pke, t - SENSE_D, false)), r_at(&|t| rls_r(t) || (!def.no_sensed_arm_reset && arm_r(t))));
        let v_cool = latch(prev(&q_cool), s_at(&|t| edge(&arm, t, COOL_LINE)), r_at(&|t| !def.no_cool_reset && (rls_r(t) || arm_r(t))));
        q_so[k] = v_so;
        q_sen[k] = v_sen;
        q_cool[k] = v_cool;
        let sd = CMP + if def.since_delay_back { OLD_SINCE_DELAY } else { SINCE_DELAY };
        let since_s = |t: i64| {
            let u = t - sd;
            // the pulse, P at its head change, and the onset (P rose within ONSET gt before the change)
            let onset = def.no_onset || !at(&ps, u - TORCH - ONSET - 1, false);
            obs_at(u) && !notp(u) && onset
        };
        let v_since = !def.no_since && latch(prev(&q_since), s_at(&since_s), r_at(&|t| notp(t - REP)));
        let d_eff = D_LINE - def.d_cut;
        let w_eff = W_LINE + def.w_delta;
        let v_win = latch(prev(&q_win), s_at(&|t| obs_at(t - d_eff)), r_at(&|t| obs_at(t) || obs_at(t - w_eff)));
        let parked_block = |t: i64| (!def.no_sensed && nqv(&q_sen, t)) || (!def.no_cool && nqv(&q_cool, t));
        let open_s = |t: i64| !(parked_block(t - AND) || nqv(&q_so, t - AND) || (!def.no_notp_in_open && p_at(t - AND)));
        let v_open = latch(prev(&q_open), s_at(&open_s), r_at(&|t| rls_r(t) || (!def.no_p_in_open_reset && p_at(t - REP))));
        q_since[k] = v_since;
        q_win[k] = v_win;
        q_open[k] = v_open;
        let rls_block = notp(kb - AND) || (!def.no_since && nqv(&q_since, kb - AND)) || nqv(&q_win, kb - AND) || (!def.no_parked_in_rls && parked_block(kb - AND));
        rls[k] = !rls_block;
        // (the park's drive: the gapless latch extender as built; the two-input OR and the bare RLS as controls)
        p_hold[k] = latch(k > 0 && p_hold[k - 1], at(&rls, kb - SET_IN, false), at(&rls, kb - PARK_W - RESET_IN, false));
        park[k] = if def.no_park_stretch {
            at(&rls, kb - PARK, false)
        } else if def.park_or2 {
            at(&rls, kb - PARK, false) || at(&rls, kb - PARK - PARK_W, false)
        } else {
            at(&rls, kb - PARK, false) || at(&p_hold, kb - PARK, false)
        };
        let shut_now = !at(&q_open, kb - TORCH, false);
        // ---- records (game ticks)
        let rise = |v: &Vec<bool>| v[k] && (k == 0 || !v[k - 1]);
        if rise(&q_since) {
            o.since_rises.push(g);
        }
        if rise(&rls) {
            o.rls.push(g);
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
        let open_add = row.open.map_or(0, |(t0, idle, kj)| if t0 >= r0 - 4 && t0 <= r0 + PARK + PARK_READ + CART_RESPONSE + row.board { idle + kj } else { 0 });
        o.first_window_by = Some(r0 + PARK + PARK_READ + CART_RESPONSE + row.board + open_add);
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

/// The bars a row's kind asserts.
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
            // (G1's C2: ON ITS FIRST WINDOW: no lie-down between its first wake and its boarding, and the boarding
            // within the first RLS's park + read + response + ride (+ a world open's idle and lag))
            if let (Some(&w0), Some(bt)) = (o.wakes.first(), o.boarded_at) {
                if o.lies.iter().any(|&l| l > w0 && l < bt) {
                    b.push(format!("boarded at {bt} after a re-sleep, not on its first window"));
                }
            }
            if let (Some(by), Some(bt)) = (o.first_window_by, o.boarded_at) {
                if bt > by {
                    b.push(format!("boarded at {bt}, past its first window's bound {by}"));
                }
            }
            b
        }
        Kind::Boards => {
            // (a row whose first window depends on the band: it boards on the first window where the window allows,
            // after its re-sleep otherwise; both safe, the split printed)
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
        f.push((r, if h <= 8 { Kind::Recovers } else if h < 60 { Kind::Boards } else { Kind::BoardsEarly }, false));
    }
    let mut r = Row::normal("an early wake (a hurt at ARM + 30) with a later park (the cart rests at ARM + 70, past COOL's bound): the sensed park holds RLS for the rest", 5, 10, 70, 16).long(900);
    r.hurt = vec![30];
    // (the later park's SENSED comes after the cut window: it recovers through its re-sleep in every band)
    f.push((r, Kind::Recovers, false));
    for (lie, d) in [(4, 0), (5, 0), (7, 0), (6, 1), (6, 2), (7, 3), (218, 0), (218, 2)] {
        let mut r = Row::normal(&format!("the merged path from the bed (lie-down at ARM + {lie}, a hurt at + {d})"), lie, 10, 40, 16).long(900);
        r.hurt = vec![lie + d];
        f.push((r, if lie > 100 { Kind::Recovers } else if d == 0 && lie >= 7 { Kind::BoardsEarly } else if d == 0 { Kind::Boards } else { Kind::Recovers }, lie > 100));
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

pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec!["ctl-shutter (LOGIC LEVEL: designed gate delays, ctl-junction's measured latch; the real-cell bench on the stop's placement follows its layout): the form of record (ironspec 16:00 / 17:32 / 17:35: the release on sustained presence from the latest pulse with its onset, held to pulse + 12, within W, PARKED = SENSED AND COOL; OPEN set PARKED AND SO AND NOT P, reset RLS OR P; PARK = RLS + 2)".to_string()];
    lines.push(format!(
        "  delays (designed, gt): torch {TORCH}, repeater {REP} a tick, comparator {CMP}, latch set {SET} / reset {RESET} (ctl-junction), COOL's tap {COOL_LINE}, WIN's D line {D_LINE} (P read at pulse + {HOLD}), W line {W_LINE} (RLS by pulse + {}), SINCE's delay {SINCE_DELAY}, the onset {ONSET} gt, the AND {AND}, PARK {PARK}, the string's re-check {RECHECK}, the observer +{OBS_RISE} x {OBS_W}, the eject ARM + {EJECT} and its boarding cooldown {BOARD_COOLDOWN}; the cooldown at boarding: the latest wake + {COOLDOWN} - boarding >= {COOLDOWN_BAR}",
        W_LINE + W_SLACK
    ));
    lines.push("  the napper's and the cart's bands: 58f770cc's stop bench (bed-tier-min, explore/bedtier-stop 2af606b9d, b48 NOTES.md: ARM -> lie-down 4..5, the window -> wake 1..20, the cart at rest 33..46, the park -> P 13..19; physical timings of the bench) and its real-cell pass -> rest 16..26 (58f770cc 18:35, the band at 4caaec86f; its real rest band 25..38 lies inside the swept 33..46 at the late end only, so the sweep is conservative)".to_string());
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
                    let o = run(&row, Defects::default());
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
    let w = v.bar_n("ctl-shutter: the normal visit (lie-down 4..5, scan 1..20, rest 33/40/46, park -> P 13/16/19, the park-entry pass -> rest 16/26): one RLS, the napper aboard and on P every tick from its wake to boarding, the window only on a sleeper and >= 6 gt after the lie-down, P continuous through the boarding, the latches clear after the next ARM's edge, no release to a sleeper, no park before rest, the cooldown margin at boarding >= 10, RLS within W of its pulse, the cart at P >= the boarding cooldown's end + 2, the shutter closed <= the press + 8", n, bad.is_empty());
    lines.push(format!(
        "  the normal visit, {n} runs, from the wake: the shutter closes + {}, RLS + {}, the park + {}, boarding + {}, the cooldown margin at boarding {} (>= {COOLDOWN_BAR}); the window opens {} gt after the lie-down (>= {WINDOW_AFTER_LIE}); the cart at P - the boarding cooldown's end {} (>= 2); the cart at P - (C + {HOSTILE_CLEAR}) {} (>= 1; C = the shutter's first closed tick; logic level: the hold delta on 58f770cc's cells, the first (-2,0,5) entry, is the figure): {w}{}",
        r("close"), r("rls"), r("park"), r("board"), r("cool"), r("win-lie"), r("board-cd"), r("hostile"),
        if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }
    ));
    let sample = run(&Row::normal("normal", 5, 10, 40, 16), Defects::default());
    lines.push(format!("    e.g. lie 5, scan 10, rest 40, park -> P 16: {}; the napper's cell per tick from its wake to boarding: {}", describe(&sample), compress(&sample.cells)));
    let margin = HOLD - STRING_DROP;
    lines.push(format!(
        "  the lie-down exclusion: a napper that lies down leaves P and its string drops by its pulse + {STRING_DROP} (26.3's TripWireBlock re-checks every {RECHECK} gt while pressed), against RLS's read of P at pulse + {HOLD}: margin {margin}, an entity-to-block timed pair: {} on the build (k + j 1), {} straddling (k + j 3)",
        margin - 1,
        margin - 3
    ));
    // (2) every forced row across its full band (band_variants)
    for (row, kind, late) in forced_rows() {
        let mut nb = 0usize;
        let mut bad: Vec<String> = Vec::new();
        let mut boards: (i64, i64) = (i64::MAX, i64::MIN);
        let mut cool: (i64, i64) = (i64::MAX, i64::MIN);
        let (mut first_w, mut later_w) = (0usize, 0usize);
        for rr in band_variants(&row) {
            let o = run(&rr, Defects::default());
            nb += 1;
            let b = kind_bars(&o, kind, late);
            if !b.is_empty() && bad.len() < 2 {
                bad.push(format!("rest {} board {} pass -> rest {} lie {:?}: {}", rr.rest, rr.board, rr.pass_to_rest, rr.lie.first(), b.join("; ")));
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
        let o = run(&row, Defects::default());
        let mark = v.bar_n(&format!("ctl-shutter forced row ({kind:?}) across the bands: {}", row.name), nb, bad.is_empty());
        lines.push(format!(
            "  {} [{kind:?}, {nb} band runs: boarding {}..{} gt after ARM ({first_w} on the first window, {later_w} after a re-sleep), the cooldown margin at boarding {}..{}]: e.g. {}{}: {mark}{}",
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
                let o = run(&r, Defects::default());
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
    let w = v.bar_n("ctl-shutter: a world open at every tick 0..700 (idle 1 / 2, the entity domain 1 / 3 ticks behind the block domain), on the normal visit at three band corners and on G1's cooldown edge (lie 4, merged L + 0, rest 33, board 19): each row's bars", n, bad.is_empty());
    lines.push(format!("  a world open at every tick 0..700, idle 1 / 2, k + j 1 / 3, {n} runs (G1's cooldown edge's least margin at boarding {edge_cool}; the woken napper's press - its head change {}..{} gt, against the onset's allowance -{ONSET}): {w}{}", offs.0, offs.1, if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }));
    // (G1 on a2186649e B1 / on 5dd4204be B1: the open sweep on EVERY forced row across its FULL band (rest x park -> P x
    // pass -> rest x the lie band, as the forced sweep), every tick over the visit 0..300, idle 1 / 2, the entity domain
    // 1 / 3 behind; one thread a row)
    let rows_all = forced_rows();
    let results: Vec<(usize, i64, Vec<String>)> = std::thread::scope(|s| {
        let hs: Vec<_> = rows_all
            .iter()
            .map(|(row, kind, late)| {
                s.spawn(move || {
                    let (mut n, mut least, mut bad) = (0usize, i64::MAX, Vec::new());
                    // (under a world open an early-woken napper may miss its first window and recover through its
                    // re-sleep instead: both safe, so the open sweep judges BoardsEarly rows as Boards)
                    let k = if *kind == Kind::BoardsEarly { Kind::Boards } else { *kind };
                    for rr in band_variants(row) {
                        for &(idle, kj) in &OPENS {
                            for t0 in 0..=300 {
                                let mut r = rr.clone();
                                r.open = Some((t0, idle, kj));
                                let o = run(&r, Defects::default());
                                n += 1;
                                if let Some(m) = o.cooldown_margin {
                                    least = least.min(m);
                                }
                                let b = kind_bars(&o, k, *late);
                                if !b.is_empty() && bad.len() < 2 {
                                    bad.push(format!("{} (rest {} board {} pass -> rest {} lie {:?}) open at {t0} idle {idle} kj {kj}: {}", row.name.chars().take(60).collect::<String>(), r.rest, r.board, r.pass_to_rest, r.lie.first(), b.join("; ")));
                                }
                            }
                        }
                    }
                    (n, least, bad)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n: usize = results.iter().map(|r| r.0).sum();
    let least_cool = results.iter().map(|r| r.1).min().unwrap_or(i64::MAX);
    let bad: Vec<String> = results.iter().flat_map(|r| r.2.iter().cloned()).take(4).collect();
    let w = v.bar_n("ctl-shutter: a world open at every tick 0..300 on EVERY forced row across its full band (rest x park -> P x pass -> rest x the lie band), idle 1 / 2, the entity domain 1 / 3 behind: each row's kind bars", n, bad.is_empty());
    lines.push(format!("  a world open at every tick 0..300 on every forced row across its full band, idle 1 / 2, k + j 1 / 3, {n} runs (the least cooldown margin at boarding {least_cool}): {w}{}", if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }));
    // (G1's N1 / C1: the cooldown edge's closed form, every term, against the bar, and EQUAL to the sweep's least margin)
    let edge_margin = COOLDOWN - COOL_EDGE;
    let w = v.bar("ctl-shutter: the cooldown edge's closed form (the wake's pulse + the latest RLS + PARK + the park's read + the cart's first move + the slowest ride + a world open's idle + the entity lag) keeps COOLDOWN - it >= 10", edge_margin >= COOLDOWN_BAR);
    lines.push(format!("  the cooldown edge, every term (G1's N1 / C1): the latest boarding = the wake + {OBS_RISE} (pulse) + {} (the latest RLS: W {W_LINE} + {W_SLACK}) + {PARK} (PARK) + {PARK_READ} (the park's read) + {CART_RESPONSE} (the cart's first move) + {BOARD_MAX} (the slowest ride) + {OPEN_IDLE} (an open's idle) + {OPEN_KJ} (the entity lag) = the wake + {COOL_EDGE}: margin {edge_margin} (>= {COOLDOWN_BAR}): {w}", W_LINE + W_SLACK));
    let eq = |cool_edge: i64| COOLDOWN - cool_edge == least_cool;
    let w = v.bar("ctl-shutter: the closed form is tight: the open sweep's least cooldown margin equals COOLDOWN - COOL_EDGE (a bound a run beats is not a bound)", eq(COOL_EDGE));
    lines.push(format!("    the sweep's least margin {least_cool} against the closed form's {edge_margin}: {w}"));
    let c = v.control("ctl-shutter CONTROL (G1's C1): the closed form without the park's read (one term dropped): the sweep's least margin no longer equals it", 1, !eq(COOL_EDGE - PARK_READ)).to_string();
    lines.push(format!("  CONTROL (the closed form without PARK_READ: margin {} against the sweep's {least_cool}): {c}", COOLDOWN - (COOL_EDGE - PARK_READ)));
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
    let (l1, l2) = (late_edge(1), late_edge(2));
    let set_in = |o: &Out| o.since_rises.iter().any(|&s| (100..=100 + HOLD).contains(&s));
    let w1 = v.bar("ctl-shutter: the onset's late edge, a press beginning 1 gt after the head change sets SINCE (inside the pulse's sample)", set_in(&l1));
    let w2 = v.bar("ctl-shutter: the onset's late edge, a press beginning 2 gt after the head change never sets SINCE", !set_in(&l2));
    lines.push(format!("  the onset's late edge (G1's N4): a press 1 gt after a head change at ARM + 100: SINCE {}: {w1}; 2 gt after: SINCE {}: {w2}", if set_in(&l1) { "set" } else { "NOT set" }, if set_in(&l2) { "SET" } else { "not set" }));
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
        let d = run(&r, Defects::default());
        let x = run(&r, def);
        let (db, xb) = (kind_bars(&d, kind, late), kind_bars(&x, kind, late));
        // (G1's C3 / C4: the defect must fail for the labelled reason: its named bar, not any)
        let named = need.is_empty() || xb.iter().any(|b| b.contains(need));
        let c = v.control(&format!("ctl-shutter CONTROL: {name}"), 1, db.is_empty() && !xb.is_empty() && named).to_string();
        lines.push(format!("  CONTROL ({name}; on the forced row \"{}\" [{kind:?}]{}): the design {}; the defect: {} [{}]: {c}", key, if need.is_empty() { String::new() } else { format!(", failing on \"{need}\"") }, if db.is_empty() { "passes".to_string() } else { format!("FAILS ({})", db.join("; ")) }, describe(&x), xb.join("; ")));
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
    twin("the D line three steps shorter (2 gt: P read at pulse + 6, before the string's drop at + 7): a release to the sleeper", "stepping onto P and lying down within 1 gt", None, None, Defects { d_cut: 6, ..Default::default() }, "RLS while the napper sleeps", &mut lines);
    lines.push("    (the latest drop, pulse + 7, rests on a modelling rule: a napper lying down in the tick it steps onto P never meets the string: tick-exact's pin, OWED)".into());
    // (W: G1's N1 cooldown edge, lie 5 merged at L + 0, the slowest rest with the shortest pass -> rest (SENSED binds RLS
    // at 68), the slowest ride, a world open of idle 2 with the entity domain 3 behind during the ride)
    let edge = Some((46, 19, 16));
    let edge_open = Some((80, OPEN_IDLE, OPEN_KJ));
    twin("the W line one step longer (W 50, the ruled figure before N1): on G1's cooldown edge the merged release lands at pulse + 61 and boards inside the cooldown bar", "lie-down at ARM + 5, a hurt at + 0", edge, edge_open, Defects { w_delta: 2, ..Default::default() }, "the cooldown margin at boarding", &mut lines);
    // (W from below, G1's B5 / C2: the early wake at + 9 needs W >= 46 to board on its first window)
    twin("P_STRING removed from OPEN's reset: G1's awake napper re-crossing P at ARM + 300 doesn't close the shutter (the close bar on every press)", "CROSSING P", None, None, Defects { no_p_in_open_reset: true, ..Default::default() }, "a press on P", &mut lines);
    twin("NOT P_STRING removed from OPEN's set: the window opens on the false-SO napper standing on P (Printed's negative)", "the false-SO path", None, None, Defects { no_notp_in_open: true, ..Default::default() }, "standing on P", &mut lines);
    twin("SINCE removed: the box on the string after the lie-down from P releases to the sleeper", "SINCE's twin row", None, None, Defects { no_since: true, ..Default::default() }, "RLS while the napper sleeps", &mut lines);
    twin("COOL's resets removed: COOL still set after the next ARM's edge", "a stray press of the park-entry detector after RLS", None, None, Defects { no_cool_reset: true, ..Default::default() }, "next ARM", &mut lines);
    twin("SO's reset removed: SO still set after the next ARM's edge", "a stray press of the park-entry detector after RLS", None, None, Defects { no_so_reset: true, ..Default::default() }, "next ARM", &mut lines);
    twin("SENSED's ARM-edge reset removed: the stray park-entry press pre-sets SENSED at the next visit (G1 17:49 (a))", "a stray press of the park-entry detector after RLS", None, None, Defects { no_sensed_arm_reset: true, ..Default::default() }, "next ARM", &mut lines);
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
        let (mut r2, _, _) = find("a hurt at ARM + 9)");
        r2.rest = 46;
        r2.board = 13;
        r2.pass_to_rest = 16;
        r2.lie = vec![4];
        let o2 = run(&r2, Defects::default());
        let b2 = kind_bars(&o2, Kind::BoardsEarly, false);
        let c = v.control("ctl-shutter CONTROL (BoardsEarly's negative, G1's C2): the + 9 wake judged as BoardsEarly at rest 46 / pass -> rest 16 (SENSED past the cut window): the first-window bar fails", 1, b2.iter().any(|b| b.contains("first window"))).to_string();
        lines.push(format!("  CONTROL (the + 9 wake judged BoardsEarly at rest 46 / pass -> rest 16): {} [{}]: {c}", describe(&o2), b2.join("; ")));
    }
    // (each kind's negative, G1 on a2186649e B6: the row's script with its defining event removed must fail its kind)
    let mut neg = |name: &str, key: &str, edit: &dyn Fn(&mut Row), lines: &mut Vec<String>| {
        let (mut r, kind, late) = find(key);
        edit(&mut r);
        let o = run(&r, Defects::default());
        let b = kind_bars(&o, kind, late);
        let c = v.control(&format!("ctl-shutter CONTROL (the kind's negative): {name}"), 1, !b.is_empty()).to_string();
        lines.push(format!("  CONTROL (the kind's negative: {name}): {} [{}]: {c}", describe(&o), b.join("; ")));
    };
    neg("PermanentHalt: the step-off row without its step-off has no empty departure", "the step-off (a forced displacement", &|r| r.step_off = None, &mut lines);
    neg("SelfEnding: the self-ending row without its later lie-down never boards after it", "self-ending at that lie-down", &|r| r.lie = vec![6], &mut lines);
    neg("Normal: the normal-kind self-heal row with a step-off fails the step-off line", "the self-healing homeless napper", &|r| r.step_off = Some(5), &mut lines);
    neg("Recovers: the hurt + 7 row without its re-sleep never boards", "a hurt at ARM + 7)", &|r| r.resleep = None, &mut lines);
    // (G1's N5 control: a planted box pressing P while the window is open (on the late lie-down's sleeper), with P
    // removed from OPEN's reset: the press never closes the window in time)
    let (mut nr, nk, nl) = find("a late lie-down (ARM + 218");
    nr.planted = vec![(240, 246)];
    let nd = run(&nr, Defects::default());
    let nx = run(&nr, Defects { no_p_in_open_reset: true, ..Default::default() });
    let (ndb, nxb) = (kind_bars(&nd, nk, nl), kind_bars(&nx, nk, nl));
    let c = v.control("ctl-shutter CONTROL (G1's N5): a planted box on P at ARM + 240..246 while the window is open on the late sleeper, P removed from OPEN's reset: the box's press doesn't close the window (every press judged)", 1, ndb.is_empty() && nxb.iter().any(|b| b.contains("a press on P"))).to_string();
    lines.push(format!("  CONTROL (the planted box on P while the window is open): the design {}; the defect {} [{}]: {c}", if ndb.is_empty() { "passes".to_string() } else { format!("FAILS ({})", ndb.join("; ")) }, describe(&nx), nxb.join("; ")));
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
                    let x = run(&r, Defects { park_or2: true, ..Default::default() });
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
    let px = run(&pr, Defects { no_park_stretch: true, ..Default::default() });
    let c = v.control("ctl-shutter CONTROL: PARK without its pulse extender: the early wake's 3-gt RLS, a world open holding the entities 3 gt behind: the cart misses the whole PARK pulse and stays on the park (a permanent hold); with the extender it boards", 1, pd.departed.map(|d| d.1).unwrap_or(false) && px.departed.is_none()).to_string();
    lines.push(format!("  CONTROL (PARK's pulse extender, the early wake at ARM + 9 with a world open at 67, idle 1, k + j 3): the design {}; without the extender {}: {c}", describe(&pd), describe(&px)));
    // (SINCE's old delay put back: every forced row must keep its outcome, the removal changes no row)
    let mut changed = Vec::new();
    for (row, kind, late) in forced_rows() {
        let a = kind_bars(&run(&row, Defects::default()), kind, late).is_empty();
        let b = kind_bars(&run(&row, Defects { since_delay_back: true, ..Default::default() }), kind, late).is_empty();
        if a != b {
            changed.push(row.name.chars().take(60).collect::<String>());
        }
    }
    let w = v.bar("ctl-shutter: SINCE's delay removed (ironspec 18:12 (d)): every forced row keeps its verdict with the old delay put back", changed.is_empty());
    lines.push(format!("  SINCE's delay removed: every forced row's verdict with the old delay put back: {}: {w}", if changed.is_empty() { "unchanged".to_string() } else { format!("CHANGED on {}", changed.join(" | ")) }));
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
        let mut v = Verdict::new("ctl-shutter-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
    }
}
