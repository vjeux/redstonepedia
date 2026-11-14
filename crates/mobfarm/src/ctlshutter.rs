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
//!     line (50: RLS no later than pulse + 61).
//!   - PARKED = SENSED AND COOL (ironspec 17:35 (2) / 17:39 (3), the sensed park bought): SENSED, a latch set by the
//!     park-entry detector rail (0,-27,2) as the arriving cart passes it, through d = 28 (>= the measured pass -> rest
//!     16..26 + 2, 58f770cc 18:35), reset at RLS or ARM's edge (a press after RLS
//!     can't pre-set the next visit, G1 17:49 (a)); COOL, the boarding-cooldown bound, a latch set by ARM's edge through
//!     the line's 52 tap (C = the eject's latest tick ARM + 3 + the 60-gt boarding cooldown + the world-open term 2 - p
//!     - the fastest park -> P ride 13 = ARM + 48 <= 52), reset at RLS or ARM's edge.
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
/// ARM to COOL's edge comparator: the line's tap (6 x delay 4 + 1 x delay 2).
pub const COOL_LINE: i64 = 52;
/// An edge comparator's pulse width (its side path against its rear: a delay-2 repeater).
pub const EDGE_W: i64 = 4;
/// A comparator.
pub const CMP: i64 = 2;
/// An AND as NOT-OR-NOT: an input repeater into the OR block + the block's torch.
pub const AND: i64 = 4;
/// RLS to the park's powered rail, p (ironspec 17:35 (3) / tick-exact 17:52: one delay-1 repeater).
pub const PARK: i64 = 2;
/// WIN's end: the pulse through 7 repeaters (6 x delay 4 + 1 x delay 1).
pub const W_LINE: i64 = 50;
/// WIN's start, the D line: two delay-2 repeaters. RLS reads P_STRING at pulse + D_LINE + 4 = pulse + HOLD.
pub const D_LINE: i64 = 8;
/// The ruled hold (ironspec 16:00): P_STRING held continuously from the pulse through pulse + 12.
pub const HOLD: i64 = 12;
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
/// The cart leaves P's cell this long after it reaches P (a boarded rider presses the string until then).
pub const LEAVE: i64 = 2;
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
    /// The W line lengthened by one repeater tick.
    pub w_plus: bool,
}

/// What a row asserts about its outcome (G1 on 5f79952c2 B6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One RLS, the napper aboard, every normal-visit bar.
    Normal,
    /// No release to a sleeper, then the napper boards within the horizon.
    Recovers,
    /// An EMPTY departure (the permanent halt the fault note names).
    PermanentHalt,
    /// No departure until the scripted later lie-down, then boarding.
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
    let mut pending_press_close: Option<i64> = None;
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
                C::Rest if park_prev => cart = C::ToP(e + 1 + row.board),
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
                if !o.wakes.is_empty() && o.wakes.last() == Some(&g) {
                    pending_press_close = Some(g);
                }
            }
            if head_changes > 0 && ob.is_none() {
                ob = Some((true, kb + OBS_RISE));
                o.pulses.push(g + OBS_RISE);
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
        let w_eff = W_LINE + if def.w_plus { 2 } else { 0 };
        let v_win = latch(prev(&q_win), s_at(&|t| obs_at(t - d_eff)), r_at(&|t| obs_at(t) || obs_at(t - w_eff)));
        let parked_block = |t: i64| (!def.no_sensed && nqv(&q_sen, t)) || (!def.no_cool && nqv(&q_cool, t));
        let open_s = |t: i64| !(parked_block(t - AND) || nqv(&q_so, t - AND) || (!def.no_notp_in_open && p_at(t - AND)));
        let v_open = latch(prev(&q_open), s_at(&open_s), r_at(&|t| rls_r(t) || (!def.no_p_in_open_reset && p_at(t - REP))));
        q_since[k] = v_since;
        q_win[k] = v_win;
        q_open[k] = v_open;
        let rls_block = notp(kb - AND) || (!def.no_since && nqv(&q_since, kb - AND)) || nqv(&q_win, kb - AND) || (!def.no_parked_in_rls && parked_block(kb - AND));
        rls[k] = !rls_block;
        park[k] = at(&rls, kb - PARK, false);
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
            if let Some(&pl) = o.pulses.iter().filter(|&&x| x <= g).last() {
                o.rls_after_pulse.push(g - pl);
            }
        }
        if rise(&park) {
            o.park.push(g);
        }
        let pk_now = q_sen[k] && q_cool[k];
        let pk_prev = k > 0 && q_sen[k - 1] && q_cool[k - 1];
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
            if let Some(pr) = pending_press_close.take() {
                o.close_after_press.push(g - pr);
            }
        }
        if shut_now && pending_press_close.is_some() && o.window.last().map(|w| w.1 != i64::MAX).unwrap_or(true) {
            // (pressed while the window was already shut: nothing to close)
            pending_press_close = None;
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
    if o.close_after_press.iter().any(|&c| c > o.close_allow) {
        bad.push(format!("the shutter closed {:?} gt after the wake's press (bar <= {CLOSE_BAR} + a world open's idle = {})", o.close_after_press, o.close_allow));
    }
    if o.parked_after_rls {
        bad.push("PARKED (SENSED AND COOL) rose again after RLS within the visit".into());
    }
    if !o.open_after_rls.is_empty() {
        bad.push(format!("the window re-opened after RLS within the visit at {}", fmt_ticks(&o.open_after_rls)));
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
            if o.departed.map(|d| !d.1).unwrap_or(true) {
                b.push("it never boards after its later lie-down (a self-ending row)".into());
            }
            if o.lies.len() < 2 {
                b.push("no later lie-down in the script".into());
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
        f.push((r, Kind::Recovers, false));
    }
    for (lie, d) in [(4, 0), (7, 0), (6, 1), (6, 2), (7, 3), (218, 0), (218, 2)] {
        let mut r = Row::normal(&format!("the merged path from the bed (lie-down at ARM + {lie}, a hurt at + {d})"), lie, 10, 40, 16).long(900);
        r.hurt = vec![lie + d];
        f.push((r, Kind::Recovers, lie > 100));
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
pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec!["ctl-shutter (LOGIC LEVEL: designed gate delays, ctl-junction's measured latch; the real-cell bench on the stop's placement follows its layout): the form of record (ironspec 16:00 / 17:32 / 17:35: the release on sustained presence from the latest pulse with its onset, held to pulse + 12, within W, PARKED = SENSED AND COOL; OPEN set PARKED AND SO AND NOT P, reset RLS OR P; PARK = RLS + 2)".to_string()];
    lines.push(format!(
        "  delays (designed, gt): torch {TORCH}, repeater {REP} a tick, comparator {CMP}, latch set {SET} / reset {RESET} (ctl-junction), COOL's tap {COOL_LINE}, WIN's D line {D_LINE} (P read at pulse + {HOLD}), W line {W_LINE} (RLS by pulse + {}), SINCE's delay {SINCE_DELAY}, the onset {ONSET} gt, the AND {AND}, PARK {PARK}, the string's re-check {RECHECK}, the observer +{OBS_RISE} x {OBS_W}, the eject ARM + {EJECT} and its boarding cooldown {BOARD_COOLDOWN}; the cooldown at boarding: the latest wake + {COOLDOWN} - boarding >= {COOLDOWN_BAR}",
        W_LINE + W_SLACK
    ));
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
    // (2) every forced row across the bands (rest 33/40/46 x park -> P 13/16/19)
    for (row, kind, late) in forced_rows() {
        let mut nb = 0usize;
        let mut bad: Vec<String> = Vec::new();
        let mut boards: (i64, i64) = (i64::MAX, i64::MIN);
        let mut cool: (i64, i64) = (i64::MAX, i64::MIN);
        for rest in [33, 40, 46] {
            for board in [13, 16, 19] {
                let mut rr = row.clone();
                if row.rest <= 46 {
                    rr.rest = rest;
                }
                rr.board = board;
                let o = run(&rr, Defects::default());
                nb += 1;
                let b = kind_bars(&o, kind, late);
                if !b.is_empty() && bad.len() < 2 {
                    bad.push(format!("rest {} board {board}: {}", rr.rest, b.join("; ")));
                }
                if let Some(bt) = o.boarded_at {
                    boards = (boards.0.min(bt), boards.1.max(bt));
                }
                if let Some(m) = o.cooldown_margin {
                    cool = (cool.0.min(m), cool.1.max(m));
                }
            }
        }
        let o = run(&row, Defects::default());
        let mark = v.bar_n(&format!("ctl-shutter forced row ({kind:?}) across the bands: {}", row.name), nb, bad.is_empty());
        lines.push(format!(
            "  {} [{kind:?}, {nb} band runs: boarding {}..{} gt after ARM, the cooldown margin at boarding {}..{}]: e.g. {}{}: {mark}{}",
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
    // (4) the controls: one planted single change each, on the row it must break
    let mut ctl = |name: &str, row: &Row, def: Defects, check: &dyn Fn(&Out) -> bool, lines: &mut Vec<String>| {
        let o = run(row, def);
        let c = v.control(&format!("ctl-shutter CONTROL: {name}"), 1, check(&o)).to_string();
        lines.push(format!("  CONTROL ({name}): {}: {c}", describe(&o)));
    };
    let mut early = Row::normal("early", 5, 10, 46, 16);
    early.hurt = vec![9];
    ctl("PARKED (SENSED AND COOL) removed from RLS, an early wake at ARM + 9 with the slowest rest (46): the park released before the cart's rest", &early, Defects { no_parked_in_rls: true, ..Default::default() }, &|o| !o.park_before_rest.is_empty(), &mut lines);
    let mut early_late = Row::normal("later park", 5, 10, 70, 16);
    early_late.hurt = vec![30];
    ctl("SENSED removed (COOL alone), a later park (the cart rests at ARM + 70, past COOL's bound) with an early wake: the park pulses before the cart's rest (the permanent hold the sensed park buys out)", &early_late, Defects { no_sensed: true, ..Default::default() }, &|o| !o.park_before_rest.is_empty(), &mut lines);
    let mut cool_row = Row::normal("cool", 4, 10, 33, 13).long(900);
    cool_row.hurt = vec![4];
    ctl("COOL removed (SENSED alone), the merged path at ARM + 4 with the fastest park: the cart reaches P inside the napper's boarding cooldown", &cool_row, Defects { no_cool: true, ..Default::default() }, &|o| !invariants(o).is_empty(), &mut lines);
    let mut b2 = Row::normal("b2", 218, 10, 40, 16).long(2000);
    b2.planted = vec![(208, 240)];
    ctl("the onset removed, G1's B2 box (from lie - 10 through pulse + 20): a release to the sleeper", &b2, Defects { no_onset: true, ..Default::default() }, &|o| !o.rls_asleep.is_empty(), &mut lines);
    // (the D line: the hold covers the onset's allowance: a napper stepping onto P 1 gt before it lies down FROM P
    // passes the onset, and its string drops at the first re-check, press + 10 = pulse + 7; the design reads P at pulse
    // + 12 through WIN's D line, its only provider since SINCE's delay went (ironspec 18:12 (d)))
    let mut d_row = Row::normal("d-edge", 5, 10, 40, 16).long(2000);
    d_row.hurt = vec![8];
    d_row.off = vec![(20, 150)];
    d_row.lie = vec![5, 151];
    d_row.resleep = None;
    let scan = |back: bool| -> String {
        [0, 2, 4, 6]
            .iter()
            .map(|&cut| {
                let o = run(&d_row, Defects { d_cut: cut, since_delay_back: back, ..Default::default() });
                format!("D line {} (P read at pulse + {}) {}", D_LINE - cut, HOLD - cut, if o.rls_asleep.is_empty() { "holds" } else { "RELEASES to the sleeper" })
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push(format!("  the D line's margin on the onset-allowance row (stepping onto P 1 gt before lying down from P: its string drops at pulse + 7), the design (SINCE's delay removed): {}; for the record, with the old delay put back (the hold provided twice): {}", scan(false), scan(true)));
    ctl("the D line three steps shorter (2 gt: P read at pulse + 6), the onset-allowance row: a release to the sleeper", &d_row, Defects { d_cut: 6, ..Default::default() }, &|o| !o.rls_asleep.is_empty(), &mut lines);
    let mut never = Row::normal("never", 5000, 10, 40, 16).long(900);
    never.lie = vec![];
    never.start_on_p = true;
    never.head_only = vec![2];
    never.resleep = None;
    let mut wedge = Row::normal("w-edge", 5000, 10, 40, 16).long(900);
    wedge.lie = vec![];
    wedge.head_only = vec![0];
    wedge.planted = vec![(0, 200)];
    wedge.resleep = None;
    ctl("the W line one step longer, a press beginning at a head change at ARM + 0 and held past COOL's bound (the design's window closes 1 gt before COOL's; the longer line lets RLS land 62 gt after its pulse): RLS past W of its pulse", &wedge, Defects { w_plus: true, ..Default::default() }, &|o| o.rls_after_pulse.iter().any(|&x| x > W_LINE + W_SLACK), &mut lines);
    let wd = run(&wedge, Defects::default());
    lines.push(format!("    (the same row under the design: RLS {}: {})", fmt_ticks(&wd.rls), if wd.rls.is_empty() { "the window closes first: held" } else { "RELEASED inside W" }));
    let mut awake = Row::normal("awake", 6, 10, 40, 16).long(2000);
    awake.hurt = vec![7];
    awake.off = vec![(40, 300)];
    awake.resleep = Some(400);
    ctl("P_STRING removed from OPEN's reset, G1's awake napper off P crossing P at ARM + 300: the shutter doesn't close at its press (the close bar)", &awake, Defects { no_p_in_open_reset: true, ..Default::default() }, &|o| !invariants(o).is_empty() || o.window.iter().any(|&(a, b)| b == i64::MAX || b - a > 300), &mut lines);
    ctl("NOT P_STRING removed from OPEN's set, the false-SO napper standing on P: the window opens on the napper on P", &never, Defects { no_notp_in_open: true, ..Default::default() }, &|o| !o.open_on_p.is_empty(), &mut lines);
    let mut cross = Row::normal("cross", 5, 10, 40, 16).long(2000);
    cross.hurt = vec![8];
    cross.off = vec![(20, 150)];
    cross.lie = vec![5, 160];
    cross.planted = vec![(175, 260)];
    cross.resleep = None;
    ctl("SINCE removed, a lie-down from P at ARM + 160 and a box on the string from + 175: a release to the sleeper", &cross, Defects { no_since: true, ..Default::default() }, &|o| !o.rls_asleep.is_empty(), &mut lines);
    let normal = Row::normal("normal", 5, 10, 40, 16);
    ctl("COOL's resets removed: COOL still set after the next ARM's edge (its next early release would skip the boarding bound)", &normal, Defects { no_cool_reset: true, ..Default::default() }, &|o| o.latches_at_next[2], &mut lines);
    ctl("SO's reset removed: SO still set after the next ARM's edge (its window would open before its napper sleeps)", &normal, Defects { no_so_reset: true, ..Default::default() }, &|o| o.latches_at_next[0], &mut lines);
    let mut stray = Row::normal("stray", 5, 10, 40, 16);
    stray.park_stray = vec![200];
    ctl("SENSED's ARM-edge reset removed, a stray park-entry press after RLS: SENSED pre-set at the next visit (G1 17:49 (a))", &stray, Defects { no_sensed_arm_reset: true, ..Default::default() }, &|o| o.latches_at_next[1], &mut lines);
    let mut off = Row::normal("off", 5, 10, 40, 16);
    off.step_off = Some(5);
    ctl("the step-off line's positive control: a forced displacement off P 5 gt after RLS shows on the cell line and fails the normal visit's bars", &off, Defects::default(), &|o| o.cells.contains('x') && !normal_bars(o, false).is_empty(), &mut lines);
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
