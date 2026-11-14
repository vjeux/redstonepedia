//! ctl-shutter, LOGIC LEVEL: the stop's shutter / release block of record as a delayed-logic model at 1-gt
//! resolution, before its cells are laid on the stop's placement (the real-cell bench follows the layout and
//! replaces every DESIGNED delay below with its measured one).
//!
//! THE FORM OF RECORD (ironspec 16:00 platform, G1 15:58's tick-order conditions adopted; 16:06 OPEN's reset):
//! - THE RELEASE KEYS ON SUSTAINED PRESENCE, anchored at the bed-head pulse: RLS fires once P_STRING has been held
//!   CONTINUOUSLY from the latest observer pulse through pulse + D (D 12: the string's 10-gt release lag + the
//!   pulse's 2-gt latency), lands no later than pulse + W, and needs PARKED.
//!   - SINCE: S = a pulse AND P_STRING at its head change (a subtract comparator: the pulse in its rear, P_STRING's
//!     torch on its side, then a delay-2 repeater so a new pulse's restart closes WIN first); R = NOT P_STRING.
//!   - WIN: S = the pulse through the D line; R = the pulse itself (a new pulse restarts the window) OR the pulse
//!     through the W line. W + D + PARK 8 + the run-up's worst 19 + 10 < 100 (SleepInBed's LAST_WOKEN cooldown).
//!   - PARKED: S = the rising edge of ARM through the line's 52-gt tap (the worst rest 46 + the world-open term 2
//!     + 4); R = RLS OR ARM's edge. A70: the line's 72-gt end AND PARKED (a release before A70 leaves it unset),
//!     the same resets; it feeds OPEN only.
//!   - RLS = P_STRING AND SINCE AND WIN AND PARKED; one pulse a visit (PARKED's reset ends it).
//!     PARK = RLS through a delay-4 repeater, the park's powered rail.
//! - SO (slept once): S = a pulse, R = RLS; OPEN only.
//! - OPEN: S = A70 AND SO AND NOT P_STRING; R = RLS OR P_STRING (the shutter closes at the wake's press).
//!   SHUT = NOT OPEN (the latch's own torch).
//! A napper that lay down has left P, so its string drops inside D: no release (a stale press, a lie-down from P, a
//! step onto P with an immediate lie-down). A woken napper stands up on P (the only stand-up cell: 58f770cc 15:49,
//! tick-exact 15:54) in the wake tick, before the pulse, and stays (tick-exact 16:14: REST's random walk needs HOME
//! absent): it releases. A wanderer re-crossing P is not continuous from a pulse: no release.
//!
//! Every gate carries its DESIGNED delay (the gate list's PROPAGATION DELAYS, rev 15:42): a repeater 2 gt per
//! redstone tick, a torch 2, a comparator 2; ctl-junction's NOR latch: its set drive to Q 4 (the set repeater 2, the
//! output repeater 2), its reset drive to Q's fall 6 (the reset repeater 2, B's torch 2, the output repeater 2);
//! while both drives are on the set drive holds Q (it powers Q's block directly), and the later drive wins. The
//! napper and the cart are scripted per row from 58f770cc's measured bands (ARM -> lie-down 4..5, the window -> wake
//! 1..20, the cart at rest 33..46, the park -> P 13..19) with the dynamic rules that close the loop: a sleeper wakes
//! once the window has been open for its scan phase (Z1 seen), stands up on P, boards the cart that reaches P while
//! it stands there, and (calm, awake, past its cooldown, not off P in Z1's view) lies down again from where it
//! stands. P_STRING is a tripwire: pressed in the entity phase of the tick a box meets it, re-checked every 10 gt
//! from the press, released at the first re-check with no box. The observer is tick-exact's 26.3 model: a head
//! change schedules its pulse 2 gt later, 2 gt wide; a change while the pulse is pending or on is absorbed; a
//! lie-down and a wake in one tick are two changes (one pulse). Block ticks (the observer, the tripwire's re-check)
//! run before the entity phase; a napper lying down in the tick it steps onto P never meets the string (its sleep
//! moves it first). A world open freezes everything for its idle; ticks that fell due inside it fire at its end, in
//! due order (the convention placement: one chunk): a latch's drives and the edges are caught over the frozen span.

use crate::verdict::Verdict;

/// A torch's delay (gt).
pub const TORCH: i64 = 2;
/// A delay-1 repeater.
pub const REP: i64 = 2;
/// ctl-junction's NOR latch: the set drive to Q (its measured switch 4).
pub const SET: i64 = 4;
/// ctl-junction's NOR latch: the reset drive to Q's fall (its measured return 6).
pub const RESET: i64 = 6;
/// ARM to PARKED's edge comparator: the line's tap (6 x delay 4 + 1 x delay 2: the worst rest 46 + 2 + 4).
pub const PARKED_LINE: i64 = 52;
/// ARM to A70's edge comparator: the line's end (9 delay-4 repeaters).
pub const A70_LINE: i64 = 72;
/// An edge comparator's pulse width (its side path against its rear).
pub const EDGE_W: i64 = 4;
/// A comparator.
pub const CMP: i64 = 2;
/// An AND as NOT-OR-NOT: an input repeater into the OR block + the block's torch.
pub const AND: i64 = 4;
/// RLS to the park's powered rail: one delay-4 repeater.
pub const PARK: i64 = 8;
/// WIN's end: the pulse through 7 repeaters (6 x delay 4 + 1 x delay 1).
pub const W: i64 = 50;
/// WIN's start (the hold): the pulse through two delay-3 repeaters.
pub const D: i64 = 12;
/// SINCE's sample through one delay-2 repeater (a new pulse's restart closes WIN before SINCE can pass).
pub const SINCE_DELAY: i64 = 4;
/// The tripwire's re-check period.
pub const RECHECK: i64 = 10;
/// The observer: a head change to its pulse's rise, and the pulse's width (tick-exact, 26.3).
pub const OBS_RISE: i64 = 2;
pub const OBS_W: i64 = 2;
/// ARM, the detector rail: powered for the cart's pass + 20.
pub const ARM_W: i64 = 21;
/// The cart leaves P's cell this long after it reaches P (a boarded rider presses the string until then).
pub const LEAVE: i64 = 2;
/// SleepInBed's LAST_WOKEN cooldown and the margin bar on it (ironspec 15:56: wake + 100 - boarding >= 10).
pub const COOLDOWN: i64 = 100;
pub const COOLDOWN_BAR: i64 = 10;
/// The latest RLS rise a pulse's window allows, after the pulse + W: the latch's reset drive (4 to Q's block),
/// its NOT-Q output (4), the AND (4), less the first blocked tick.
pub const W_SLACK: i64 = 4 + 4 + AND - 1;

/// Planted defects, one per control.
#[derive(Clone, Copy, Default, Debug)]
pub struct Defects {
    pub no_parked_in_rls: bool,
    pub no_notp_in_open: bool,
    pub no_p_in_open_reset: bool,
    pub no_parked_reset: bool,
    pub no_so_reset: bool,
    pub no_parked_gate_a70: bool,
    pub no_since: bool,
    /// The hold removed: WIN opens just after the pulse's restart.
    pub d_zero: bool,
    /// The window's end removed.
    pub no_w: bool,
}

/// One scripted visit. Times are entity ticks (ARM at `arm`); a world open shifts every entity time after it.
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
    /// The napper stands on P before its first lie-down (the never-slept napper).
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
    /// Non-sleep head changes.
    pub head_only: Vec<i64>,
    /// The napper is moved off P for good this long after RLS's rise (a forced displacement: the step-off).
    pub step_off: Option<i64>,
    /// A calm awake napper lies down again this long after its wake (>= the cooldown), from where it stands,
    /// unless it is off P with the window open (Z1 in view).
    pub resleep: Option<i64>,
    /// A world open (game tick, idle).
    pub open: Option<(i64, i64)>,
    /// The next visit's ARM: the latches read just after its edge.
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
    pub rls_width: Vec<i64>,
    pub park: Vec<i64>,
    pub rls_asleep: Vec<i64>,
    pub park_before_rest: Vec<i64>,
    pub departed: Option<(i64, bool)>,
    pub wakes: Vec<i64>,
    pub lies: Vec<i64>,
    pub pulses: Vec<i64>,
    /// Per pulse: the press tick of the P_STRING press it found on (None: P low at the pulse).
    pub press_at_pulse: Vec<Option<i64>>,
    pub open_set: Vec<i64>,
    /// OPEN set while the napper was awake off P / standing on P.
    pub open_awake: Vec<i64>,
    pub open_on_p: Vec<i64>,
    /// Ticks an awake napper off P stood with the window open (Z1 in view).
    pub exposed: i64,
    pub window: Vec<(i64, i64)>,
    /// (SO, A70, PARKED, OPEN, SINCE, WIN) just after the next ARM's edge.
    pub latches_at_next: [bool; 6],
    /// P_STRING's falls between the boarding wake and the departure.
    pub p_falls_boarding: u32,
    /// The latest wake before the departure + the cooldown - the departure.
    pub cooldown_margin: Option<i64>,
    /// RLS's rise - the latest pulse before it.
    pub rls_after_pulse: Vec<i64>,
    /// The napper's cell per tick from the boarding wake to the departure: P on P, B aboard, x off P, b in bed.
    pub cells: String,
    /// A70 rose after RLS in the same visit.
    pub a70_after_rls: bool,
}

fn at(v: &[bool], t: i64, d: bool) -> bool {
    if t < 0 {
        d
    } else {
        v[t as usize]
    }
}

/// Any source tick in (lo, hi] for which `f` holds (a drive caught across a frozen span).
fn any_t(lo: i64, hi: i64, f: impl Fn(i64) -> bool) -> bool {
    let a = (lo + 1).max(0);
    a <= hi && (a..=hi).any(f)
}

/// One run of a row with the planted defects (none for the design).
pub fn run(row: &Row, def: Defects) -> Out {
    let idle_total = row.open.map(|o| o.1).unwrap_or(0);
    let t_max = row.horizon.max(row.next_arm + 30) + idle_total + 1;
    let n = t_max as usize;
    let mut o = Out::default();
    let mut arm = vec![false; n];
    let mut ps = vec![false; n];
    let mut obs = vec![false; n];
    let mut bed = vec![false; n];
    let mut praw = vec![false; n];
    let mut rls = vec![false; n];
    let mut park = vec![false; n];
    // the latches' internal state: Q's block (the one the set drive powers)
    let (mut q_so, mut q_pk, mut q_a70, mut q_open, mut q_since, mut q_win) = (vec![false; n], vec![false; n], vec![false; n], vec![false; n], vec![false; n], vec![false; n]);
    let mut ob: Option<(bool, i64)> = None;
    let (mut pressed, mut next_check, mut press_tick) = (false, 0i64, 0i64);
    let mut nap = if row.start_on_p { N::OnP } else { N::Riding };
    let mut cart = C::Arriving;
    let mut last_wake: Option<i64> = None;
    let mut win_open_for = 0i64;
    let mut frozen_n = 0i64;
    let mut boarding_wake: Option<i64> = None;
    let mut stepped = false;
    let mut resleep_done_for: Option<i64> = None;
    let mut pg: i64 = -1;
    let mut cells = String::new();
    for g in 0..t_max {
        let gi = g as usize;
        let frozen = row.open.map(|(t0, idle)| g >= t0 && g < t0 + idle).unwrap_or(false);
        if frozen {
            frozen_n += 1;
            if gi > 0 {
                for v in [&mut arm, &mut ps, &mut obs, &mut bed, &mut praw, &mut rls, &mut park, &mut q_so, &mut q_pk, &mut q_a70, &mut q_open, &mut q_since, &mut q_win] {
                    v[gi] = v[gi - 1];
                }
            }
            continue;
        }
        let e = g - frozen_n;
        let prev = |v: &Vec<bool>| gi > 0 && v[gi - 1];
        // ---- block ticks first (due in game time): the observer's scheduled tick, the tripwire's re-check
        let mut obs_now = prev(&obs);
        match ob {
            Some((true, due)) if due <= g => {
                obs_now = true;
                ob = Some((false, g + OBS_W));
            }
            Some((false, due)) if due <= g => {
                obs_now = false;
                ob = None;
            }
            _ => {}
        }
        if pressed && g >= next_check {
            if prev(&praw) {
                next_check = g + RECHECK;
            } else {
                pressed = false;
            }
        }
        // ---- entities (entity time e)
        let mut head_changes = 0;
        let park_prev = prev(&park);
        match cart {
            C::Arriving if e >= row.arm + row.rest => cart = C::Rest,
            C::Rest if park_prev => cart = C::ToP(e + row.board),
            C::ToP(t) if e >= t => {
                if nap == N::OnP {
                    nap = N::Boarded;
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
        if park_prev && cart == C::Arriving && !(gi >= 2 && park[gi - 2]) {
            o.park_before_rest.push(g);
        }
        // the window as the napper saw it last tick: the shutter is NOT OPEN (A's torch, 2 gt behind Q's block)
        let win = at(&q_open, g - 1 - TORCH, false);
        for &(a, b) in &row.off {
            if e == a && nap == N::OnP {
                nap = N::Off;
            }
            if e == b && nap == N::Off {
                nap = N::OnP;
            }
        }
        if let (Some(k), Some(&r0)) = (row.step_off, o.rls.first()) {
            if !stepped && g >= r0 + k && nap == N::OnP {
                nap = N::Off;
                stepped = true;
            }
        }
        let calm = matches!(nap, N::OnP | N::Off)
            && !(nap == N::Off && win)
            && match (row.resleep, last_wake) {
                (Some(r), Some(w)) => e - w >= r && resleep_done_for != Some(w),
                _ => false,
            };
        if (row.lie.contains(&e) && matches!(nap, N::Riding | N::OnP | N::Off)) || calm {
            if calm {
                resleep_done_for = last_wake;
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
                last_wake = Some(e);
                o.wakes.push(g);
                head_changes += 1;
            }
        } else {
            win_open_for = 0;
        }
        if nap == N::Off && win {
            o.exposed += 1;
        }
        bed[gi] = nap == N::Bed;
        praw[gi] = nap == N::OnP || (nap == N::Boarded && matches!(cart, C::AtP(_))) || row.planted.iter().any(|&(a, b)| e >= a && e < b);
        arm[gi] = (e >= row.arm && e < row.arm + ARM_W) || (e >= row.next_arm && e < row.next_arm + ARM_W);
        if row.head_only.contains(&e) {
            head_changes += 1;
        }
        if !pressed && praw[gi] {
            pressed = true;
            press_tick = g;
            next_check = g + RECHECK;
        }
        ps[gi] = pressed;
        if head_changes > 0 && ob.is_none() {
            ob = Some((true, g + OBS_RISE));
            o.pulses.push(g + OBS_RISE);
        }
        obs[gi] = obs_now;
        if obs_now && !prev(&obs) {
            o.press_at_pulse.push(if pressed { Some(press_tick) } else { None });
        }
        // ---- redstone
        // the readers: P's torch, a latch's Q (its output repeater) and NOT-Q (A's torch + its repeater)
        let p_at = |t: i64| at(&ps, t, false);
        let notp = |t: i64| !p_at(t - TORCH);
        let qv = |v: &Vec<bool>, t: i64| at(v, t - REP, false);
        let nqv = |v: &Vec<bool>, t: i64| !at(v, t - TORCH - REP, false);
        let obs_at = |t: i64| at(&obs, t, false);
        let rls_at = |t: i64| at(&rls, t, false);
        // a rising edge of ARM through a line of `l` gt and the edge comparator
        let edge = |t: i64, l: i64| at(&arm, t - CMP - l, false) && !at(&arm, t - CMP - l - EDGE_W, false);
        // ctl-junction's latch: the set drive (seen 2 gt later at Q's block) wins while on, else the reset drive
        // (seen 4 gt later), else hold; both caught over the frozen span
        let latch = |q: bool, s: &dyn Fn(i64) -> bool, r: &dyn Fn(i64) -> bool| -> bool {
            if any_t(pg - 2, g - 2, s) {
                true
            } else if any_t(pg - 4, g - 4, r) {
                false
            } else {
                q
            }
        };
        let rls_r = |t: i64| rls_at(t - REP);
        let arm_r = |t: i64| edge(t, 0);
        let v_so = latch(prev(&q_so), &|t| obs_at(t), &|t| !def.no_so_reset && rls_r(t));
        let v_pk = latch(prev(&q_pk), &|t| edge(t, PARKED_LINE), &|t| !def.no_parked_reset && (rls_r(t) || arm_r(t)));
        let v_a70 = latch(prev(&q_a70), &|t| edge(t, A70_LINE) && (def.no_parked_gate_a70 || qv(&q_pk, t - CMP)), &|t| rls_r(t) || arm_r(t));
        let since_s = |t: i64| {
            let u = t - if def.d_zero { 0 } else { SINCE_DELAY };
            obs_at(u - CMP) && !notp(u - CMP)
        };
        let v_since = !def.no_since && latch(prev(&q_since), &since_s, &|t| notp(t - REP));
        let d_eff = if def.d_zero { 4 } else { D };
        let v_win = latch(prev(&q_win), &|t| obs_at(t - d_eff), &|t| obs_at(t) || (!def.no_w && obs_at(t - W)));
        let open_s = |t: i64| !(nqv(&q_a70, t - AND) || nqv(&q_so, t - AND) || (!def.no_notp_in_open && p_at(t - AND)));
        let v_open = latch(prev(&q_open), &open_s, &|t| rls_r(t) || (!def.no_p_in_open_reset && p_at(t - REP)));
        q_so[gi] = v_so;
        q_pk[gi] = v_pk;
        q_a70[gi] = v_a70;
        q_since[gi] = v_since;
        q_win[gi] = v_win;
        q_open[gi] = v_open;
        let rls_block = notp(g - AND) || (!def.no_since && nqv(&q_since, g - AND)) || nqv(&q_win, g - AND) || (!def.no_parked_in_rls && nqv(&q_pk, g - AND));
        rls[gi] = !rls_block;
        park[gi] = at(&rls, g - PARK, false);
        let shut_now = !at(&q_open, g - TORCH, false);
        let shut_prev = !at(&q_open, g - 1 - TORCH, false);
        // ---- records
        let rise = |v: &Vec<bool>| v[gi] && (gi == 0 || !v[gi - 1]);
        if rise(&rls) {
            o.rls.push(g);
            if bed[gi] {
                o.rls_asleep.push(g);
            }
            if let Some(&pl) = o.pulses.iter().filter(|&&x| x <= g).last() {
                o.rls_after_pulse.push(g - pl);
            }
        }
        if gi > 0 && !rls[gi] && rls[gi - 1] {
            if let Some(&r0) = o.rls.last() {
                o.rls_width.push(g - r0);
            }
        }
        if rise(&park) {
            o.park.push(g);
        }
        if rise(&q_a70) && !o.rls.is_empty() && g < row.next_arm {
            o.a70_after_rls = true;
        }
        if rise(&q_open) {
            o.open_set.push(g);
            match nap {
                N::OnP => o.open_on_p.push(g),
                N::Bed => {}
                _ => o.open_awake.push(g),
            }
        }
        if !shut_now && shut_prev {
            o.window.push((g, i64::MAX));
        }
        if shut_now && !shut_prev {
            if let Some(w) = o.window.last_mut() {
                w.1 = g;
            }
        }
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
                if g > b && gi > 0 && ps[gi - 1] && !ps[gi] {
                    o.p_falls_boarding += 1;
                }
            }
        }
        if g == row.next_arm + idle_total + 12 {
            o.latches_at_next = [q_so[gi], q_a70[gi], q_pk[gi], q_open[gi], q_since[gi], q_win[gi]];
        }
        pg = g;
    }
    o.cells = cells;
    if let Some((d, true)) = o.departed {
        if let Some(&w) = o.wakes.iter().filter(|&&w| w <= d).last() {
            o.cooldown_margin = Some(w + COOLDOWN - d);
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
        Some((t, true)) => format!("BOARDED at {t}"),
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
        o.cooldown_margin.map(|m| format!(", the cooldown margin {m}")).unwrap_or_default(),
        if o.exposed > 0 { format!(", an awake napper off P in Z1's view for {} gt", o.exposed) } else { String::new() }
    )
}

/// The invariants every row keeps: no release to a sleeper, no park before the cart's rest, the cooldown margin
/// when the napper boards, RLS within W of its pulse, no empty departure (unless `allow_empty`).
fn invariants(o: &Out, allow_empty: bool) -> Vec<String> {
    let mut bad = Vec::new();
    if !o.rls_asleep.is_empty() {
        bad.push(format!("RLS while the napper sleeps at {}", fmt_ticks(&o.rls_asleep)));
    }
    if !o.park_before_rest.is_empty() {
        bad.push(format!("the park released before the cart's rest at {}", fmt_ticks(&o.park_before_rest)));
    }
    if let Some(m) = o.cooldown_margin {
        if m < COOLDOWN_BAR {
            bad.push(format!("the cooldown margin {m} < {COOLDOWN_BAR}"));
        }
    }
    if o.rls_after_pulse.iter().any(|&x| x > W + W_SLACK) {
        bad.push(format!("RLS {:?} gt after its pulse, past W {W} + {W_SLACK}", o.rls_after_pulse));
    }
    if !allow_empty && o.departed.map(|d| !d.1).unwrap_or(false) {
        bad.push("an EMPTY departure (a permanent halt)".into());
    }
    bad
}

/// A normal visit's own bars: one RLS, the napper aboard, on P every tick from its wake to boarding (the step-off
/// line), the window opened only on a sleeper, P continuous through the boarding, the latches clear after the next
/// ARM's edge.
fn normal_bars(o: &Out) -> Vec<String> {
    let mut bad = invariants(o, false);
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
    if o.p_falls_boarding > 0 {
        bad.push(format!("P_STRING fell {} times between the wake and the departure", o.p_falls_boarding));
    }
    if o.latches_at_next.iter().any(|&b| b) {
        bad.push(format!("latches after the next ARM's edge (SO, A70, PARKED, OPEN, SINCE, WIN) = {:?}", o.latches_at_next));
    }
    bad
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

/// The bench: the normal sweep, every forced row, a world open at every tick, the controls.
pub fn bench(v: &mut Verdict) -> Vec<String> {
    let mut lines = vec!["ctl-shutter (LOGIC LEVEL: designed gate delays, ctl-junction's measured latch; the real-cell bench on the stop's placement follows its layout): the form of record (ironspec 16:00: the release on sustained presence from the latest pulse, held D, within W, PARKED; OPEN set A70 AND SO AND NOT P, reset RLS OR P)".to_string()];
    lines.push(format!(
        "  delays (designed, gt): torch {TORCH}, repeater {REP} a tick, comparator {CMP}, latch set {SET} / reset {RESET} (ctl-junction), PARKED's tap {PARKED_LINE}, A70's line {A70_LINE}, the hold D {D}, the window W {W}, SINCE's delay {SINCE_DELAY}, the AND {AND}, PARK {PARK}, the string's re-check {RECHECK}, the observer +{OBS_RISE} x {OBS_W}; the bound W + D + PARK + 19 + {COOLDOWN_BAR} = {} < {COOLDOWN}",
        W + D + PARK + 19 + COOLDOWN_BAR
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
                for board in [13, 16, 19] {
                    let row = Row::normal("normal", lie, scan, rest, board);
                    let o = run(&row, Defects::default());
                    n += 1;
                    let b = normal_bars(&o);
                    if !b.is_empty() && bad.len() < 3 {
                        bad.push(format!("lie {lie} scan {scan} rest {rest} board {board}: {}", b.join("; ")));
                    }
                    if let (Some(&w), Some(&(_, c)), Some(&r), Some(&pk)) = (o.wakes.first(), o.window.first(), o.rls.first(), o.park.first()) {
                        put("close", c - w);
                        put("rls", r - w);
                        put("park", pk - w);
                        put("rls-close", r - c);
                        put("park-close", pk - c);
                        if let Some((d, _)) = o.departed {
                            put("board", d - w);
                        }
                    }
                    if let Some(m) = o.cooldown_margin {
                        put("cool", m);
                    }
                    if let (Some(&w), Some(Some(pt))) = (o.wakes.first(), o.press_at_pulse.get(1)) {
                        put("press-pulse", pt - (w + OBS_RISE));
                    }
                }
            }
        }
    }
    let r = |k: &str| mm.get(k).map(|x| format!("{}..{}", x.0, x.1)).unwrap_or("-".into());
    let w = v.bar_n("ctl-shutter: the normal visit (lie-down 4..5, scan 1..20, rest 33/40/46, park -> P 13/16/19): one RLS, the napper aboard and on P every tick from its wake to boarding, the window only on a sleeper, P continuous through the boarding, the latches clear after the next ARM's edge, no release to a sleeper, no park before rest, the cooldown margin >= 10, RLS within W of its pulse", n, bad.is_empty());
    lines.push(format!(
        "  the normal visit, {n} runs, from the wake: the press at the pulse {} (the woken napper's press precedes its pulse), the shutter closes + {}, RLS + {} (the close + {}), the park + {} (the close + {}: the hold delta, a minimum on this wiring), the departure + {}, the cooldown margin {} (bar >= {COOLDOWN_BAR}): {w}{}",
        r("press-pulse"), r("close"), r("rls"), r("rls-close"), r("park"), r("park-close"), r("board"), r("cool"),
        if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }
    ));
    let sample = run(&Row::normal("normal", 5, 10, 40, 16), Defects::default());
    lines.push(format!("    e.g. lie 5, scan 10, rest 40, park -> P 16: {}; the napper's cell per tick from its wake to boarding: {}", describe(&sample), compress(&sample.cells)));
    // (2) the forced rows
    let mut forced: Vec<(Row, &str)> = Vec::new();
    forced.push((Row::normal("a late lie-down (ARM + 218, A70 already set)", 218, 10, 40, 16).long(900), "normal"));
    for h in [7, 8, 9, 30, 60] {
        let mut r = Row::normal(&format!("an early wake from the bed (a hurt at ARM + {h})"), 5, 10, 40, 16).long(900);
        r.hurt = vec![h];
        forced.push((r, "normal"));
    }
    for (lie, d) in [(7, 0), (6, 1), (6, 2), (7, 3), (218, 0), (218, 2)] {
        let mut r = Row::normal(&format!("the merged path from the bed (lie-down at ARM + {lie}, a hurt at + {d})"), lie, 10, 40, 16).long(900);
        r.hurt = vec![lie + d];
        forced.push((r, "normal"));
    }
    {
        let mut r = Row::normal("the merged path at L + 0 FROM P (the napper standing on P lies down at ARM + 218, hurt in the same tick)", 218, 10, 40, 16).long(2000);
        r.start_on_p = true;
        r.hurt = vec![218];
        forced.push((r, "normal"));
        let mut r = Row::normal("a planted stale press over [lie - 10, lie - 1] before the lie-down at ARM + 218", 218, 10, 40, 16).long(2000);
        r.planted = vec![(208, 217)];
        forced.push((r, "normal"));
        let mut r = Row::normal("a planted press held through the lie-down at ARM + 218 (over [lie - 10, lie + 2])", 218, 10, 40, 16).long(2000);
        r.planted = vec![(208, 220)];
        forced.push((r, "normal"));
        let mut r = Row::normal("the never-slept napper standing on P lies down FROM P at ARM + 2000 (its own press on the string)", 2000, 10, 40, 16).long(3000);
        r.start_on_p = true;
        forced.push((r, "normal"));
        for k in [0, 1, 2] {
            let mut r = Row::normal(&format!("stepping onto P and lying down within {k} gt (an early wake at ARM + 8, off P at + 20, back onto P at + 150, lying down from P at + {})", 150 + k), 5, 10, 40, 16).long(2000);
            r.hurt = vec![8];
            r.off = vec![(20, 150)];
            r.lie = vec![5, 150 + k];
            r.resleep = None;
            forced.push((r, "recovers"));
        }
        let mut r = Row::normal("an early wake (ARM + 8) strolling off P at + 20 (before PARKED) and re-crossing P at + 140 (past W)", 5, 10, 40, 16).long(2000);
        r.hurt = vec![8];
        r.off = vec![(20, 140)];
        forced.push((r, "recovers"));
        let mut r = Row::normal("a calm napper back onto P after its cooldown, lying down FROM P (an early wake at ARM + 8, off P at + 20, back on P at + 150, lying down from P at + 160)", 5, 10, 40, 16).long(2000);
        r.hurt = vec![8];
        r.off = vec![(20, 150)];
        r.lie = vec![5, 160];
        r.resleep = None;
        forced.push((r, "recovers"));
        let mut r = Row::normal("the window on a wanderer, CROSSING P: G1's awake napper (merged from the bed at ARM + 6 + 1) off P at ARM + 40, back across P at ARM + 300, calm re-sleep past its cooldown", 6, 10, 40, 16).long(2000);
        r.hurt = vec![7];
        r.off = vec![(40, 300)];
        r.resleep = Some(400);
        forced.push((r, "recovers"));
        let mut r = Row::normal("the window on a wanderer, STAYING OFF P until dawn (ARM + 6000): G1's awake napper off P from ARM + 40, never calm in Z1's view (it recovers at its next lie-down, or by the HALT entry's re-home step)", 6, 10, 40, 16).long(9000);
        r.hurt = vec![7];
        r.off = vec![(40, 6000)];
        r.resleep = None;
        forced.push((r, "halt (self-ending at its next lie-down)"));
        let mut r = Row::normal("Z1 removed: no wake until dawn (ARM + 6000), the napper standing up on P", 5, 10, 40, 16).long(9000);
        r.z1 = false;
        r.resleep = None;
        r.dawn = Some(6000);
        forced.push((r, "normal"));
        forced.push((Row::normal("the never-slept night visit (no lie-down until ARM + 2000, standing on the bed foot)", 2000, 10, 40, 16).long(3000), "normal"));
        let mut r = Row::normal("the step-off (a forced displacement off P 5 gt after RLS, before the cart reaches it): the one permanent halt left, for the HALT entry", 5, 10, 40, 16);
        r.step_off = Some(5);
        forced.push((r, "expected: the permanent halt"));
        let mut r = Row::normal("a non-sleep head change at ARM + 5 (no sleep: the napper awake on the bed foot)", 5000, 10, 40, 16).long(900);
        r.lie = vec![];
        r.head_only = vec![5];
        r.resleep = None;
        forced.push((r, "fault (printed)"));
    }
    for (row, kind) in &forced {
        let o = run(row, Defects::default());
        let b = match *kind {
            "normal" => normal_bars(&o),
            k if k.starts_with("expected") => invariants(&o, true),
            _ => invariants(&o, false),
        };
        let mark = v.bar(&format!("ctl-shutter forced row ({kind}): {}", row.name), b.is_empty()).to_string();
        let to_board = match o.departed {
            Some((t, true)) => format!("; boarding {t} gt after ARM"),
            _ => String::new(),
        };
        let extra = format!(
            "{}{}{}; per pulse, the press it found {:?}{}{}",
            if o.open_awake.is_empty() { String::new() } else { format!("; the window opened on an awake napper OFF P at {}", fmt_ticks(&o.open_awake)) },
            if o.open_on_p.is_empty() { String::new() } else { format!("; the window opened on a napper ON P at {}", fmt_ticks(&o.open_on_p)) },
            if o.rls_after_pulse.is_empty() { String::new() } else { format!("; RLS {} gt after its pulse", fmt_ticks(&o.rls_after_pulse)) },
            o.press_at_pulse,
            if o.cells.is_empty() { String::new() } else { format!("; cells wake -> boarding {}", compress(&o.cells)) },
            to_board
        );
        lines.push(format!("  {}: {}{extra}: {mark}{}", row.name, describe(&o), if b.is_empty() { String::new() } else { format!(" [{}]", b.join("; ")) }));
    }
    // (3) a world open at every tick of the normal visit, idle 1 and 2
    let mut n = 0usize;
    let mut bad = Vec::new();
    for idle in [1, 2] {
        for t0 in 0..=300 {
            let mut r = Row::normal("normal + world open", 5, 10, 40, 16);
            r.open = Some((t0, idle));
            let o = run(&r, Defects::default());
            n += 1;
            let b = normal_bars(&o);
            if !b.is_empty() && bad.len() < 3 {
                bad.push(format!("open at {t0} idle {idle}: {}", b.join("; ")));
            }
        }
    }
    let w = v.bar_n("ctl-shutter: a world open (idle 1 and 2) at every tick 0..300 of the normal visit (the convention placement: one chunk): the normal visit's bars", n, bad.is_empty());
    lines.push(format!("  a world open at every tick 0..300, idle 1 and 2, {n} runs: {w}{}", if bad.is_empty() { String::new() } else { format!(" ({})", bad.join(" | ")) }));
    // (4) the controls: one planted defect each, on the row it must break
    let mut ctl = |name: &str, row: &Row, def: Defects, check: &dyn Fn(&Out) -> bool, lines: &mut Vec<String>| {
        let o = run(row, def);
        let c = v.control(&format!("ctl-shutter CONTROL: {name}"), 1, check(&o)).to_string();
        lines.push(format!("  CONTROL ({name}): {}: {c}", describe(&o)));
    };
    let mut early = Row::normal("early", 5, 10, 40, 16);
    early.hurt = vec![9];
    ctl("PARKED removed from RLS, the early wake at ARM + 9: the park released before the cart's rest", &early, Defects { no_parked_in_rls: true, ..Default::default() }, &|o| !o.park_before_rest.is_empty(), &mut lines);
    let mut lie2 = Row::normal("lie2", 218, 10, 40, 16).long(900);
    lie2.planted = vec![(215, 227)];
    ctl("the holds removed (WIN opens at the pulse, SINCE without its delay repeater), a box on the string over [lie - 3, lie + 9] around a lie-down at ARM + 218 (released at its re-check, pulse + 15): a release to the sleeper (the design needs P held to pulse + 16; the string's lag after the napper's own lie-down ends by pulse + 8)", &lie2, Defects { d_zero: true, ..Default::default() }, &|o| !o.rls_asleep.is_empty() || o.departed.map(|d| !d.1).unwrap_or(false), &mut lines);
    let mut never = Row::normal("never", 5000, 10, 40, 16).long(900);
    never.lie = vec![];
    never.start_on_p = true;
    never.head_only = vec![0];
    never.resleep = None;
    ctl("the window's end removed, a never-slept napper standing on P since a head change at ARM + 0: RLS past W of its pulse", &never, Defects { no_w: true, ..Default::default() }, &|o| o.rls_after_pulse.iter().any(|&x| x > W + W_SLACK), &mut lines);
    ctl("NOT P_STRING removed from OPEN's set, the same napper standing on P: the window opens on the napper on P", &never, Defects { no_notp_in_open: true, ..Default::default() }, &|o| !o.open_on_p.is_empty(), &mut lines);
    let mut awake = Row::normal("awake", 6, 10, 40, 16).long(2000);
    awake.hurt = vec![7];
    awake.off = vec![(40, 300)];
    awake.resleep = Some(400);
    ctl("P_STRING removed from OPEN's reset, G1's awake napper off P crossing P at ARM + 300: the window stays open after the cross", &awake, Defects { no_p_in_open_reset: true, ..Default::default() }, &|o| o.window.first().map(|&(_, b)| b == i64::MAX || b > 320).unwrap_or(false), &mut lines);
    let mut cross = Row::normal("cross", 5, 10, 40, 16).long(2000);
    cross.hurt = vec![8];
    cross.off = vec![(20, 150)];
    cross.lie = vec![5, 160];
    cross.planted = vec![(175, 260)];
    cross.resleep = None;
    ctl("SINCE removed, a lie-down from P at ARM + 160 and a box on the string from + 175 (inside the lie-down's window): a release to the sleeper", &cross, Defects { no_since: true, ..Default::default() }, &|o| !o.rls_asleep.is_empty(), &mut lines);
    let normal = Row::normal("normal", 5, 10, 40, 16);
    ctl("PARKED's resets removed: PARKED still set after the next ARM's edge (its early napper would release before its cart's rest)", &normal, Defects { no_parked_reset: true, ..Default::default() }, &|o| o.latches_at_next[2], &mut lines);
    ctl("SO's reset removed: SO still set after the next ARM's edge (its window would open before its napper sleeps)", &normal, Defects { no_so_reset: true, ..Default::default() }, &|o| o.latches_at_next[0], &mut lines);
    let mut early30 = Row::normal("early30", 5, 10, 40, 16);
    early30.hurt = vec![30];
    ctl("PARKED's gate removed from A70's set, an early wake at ARM + 30 released before A70: A70 rises after the release (its next visit's window could open before its cart parks)", &early30, Defects { no_parked_gate_a70: true, ..Default::default() }, &|o| o.a70_after_rls, &mut lines);
    let mut off = Row::normal("off", 5, 10, 40, 16);
    off.step_off = Some(5);
    ctl("the step-off line's positive control: a forced displacement off P 5 gt after RLS must show on the cell line and fail the normal visit's bars", &off, Defects::default(), &|o| o.cells.contains('x') && !normal_bars(o).is_empty(), &mut lines);
    lines.push("  OWED (not in this logic bench): the rest of the farm during each halt and the golem-rate cost of a mid-night-to-dawn halt (the counter / coverage model's halt row); the real-cell bench measures every delay above on the stop's placement; the real napper's cell per tick (58f770cc's stop bench on this wiring); tick-exact's game row for the press and pulse ticks and the string's measured release lag, from which D is set".into());
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
        assert!(normal_bars(&o).is_empty(), "{:?}", normal_bars(&o));
    }

    #[test]
    fn a_step_onto_p_and_lie_down_never_releases_to_the_sleeper() {
        for k in [0, 1, 2] {
            let mut r = Row::normal("t", 5, 10, 40, 16).long(2000);
            r.hurt = vec![8];
            r.off = vec![(20, 150)];
            r.lie = vec![5, 150 + k];
            r.resleep = None;
            let o = run(&r, Defects::default());
            assert!(o.rls_asleep.is_empty() && o.departed.map(|d| d.1).unwrap_or(true), "k {k}: {}", describe(&o));
        }
    }

    #[test]
    fn the_step_off_shows_on_the_cell_line() {
        let mut r = Row::normal("t", 5, 10, 40, 16);
        r.step_off = Some(5);
        let o = run(&r, Defects::default());
        assert!(o.cells.contains('x') && !normal_bars(&o).is_empty(), "{} / {}", describe(&o), o.cells);
    }

    #[test]
    fn the_bench_prints() {
        let mut v = Verdict::new("ctl-shutter-test");
        for l in bench(&mut v) {
            println!("{l}");
        }
    }
}
