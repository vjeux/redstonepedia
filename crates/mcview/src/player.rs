//! The live viewer's state, independent of the platform: a session played at a speed (ticks per
//! second) with the pointer's inputs (orbit, zoom, pan, click to flip a lever), the frame's
//! triangles rebuilt only when something moved.

use crate::atlas::Atlas;
use crate::camera::Camera;
use crate::design::Pos;
use crate::mesh::{Mesh, Vertex};
use crate::scene::{self, Look};
use crate::session::{Mode, Options, RunState, Session};

pub struct Player {
    pub session: Session,
    pub look: Look,
    pub camera: Camera,
    /// The camera as framed at load (double click / reset view).
    pub home: Camera,
    pub playing: bool,
    /// Game ticks per second (20 = real time).
    pub tps: f32,
    acc: f32,
    /// Fraction of the next tick shown (moving blocks interpolated; 0 when paused).
    pub partial: f32,
    mesh: Option<Mesh>,
    mesh_key: (u32, u32),
    /// Something changed since the last frame.
    pub dirty: bool,
    /// Seconds spent at a flying machine's end before it starts again.
    hold: f32,
    /// A travelling design's last centre: the camera follows it.
    follow: Option<[f32; 3]>,
    /// Circuit inputs are pulses (a tap = a short pulse) rather than levels (a tap flips).
    pub pulse_inputs: bool,
    /// The checker's phases (`data-phases`): (from, to, label).
    pub phases: Vec<(u32, u32, String)>,
    /// The last [`HISTORY`] ticks of each output (on/off), oldest first.
    pub history: Vec<std::collections::VecDeque<bool>>,
    /// The tick the history last recorded.
    history_tick: Option<u32>,
    /// When each held input went down (tick), for momentary presses.
    pressed: Vec<Option<u32>>,
    /// The door demo's stage (None: not running).
    pub door_demo: Option<DoorDemo>,
    /// The tick the auto-demo is shown from ([`Player::skip_demo_wait`]; 0: from placement).
    pub demo_from: u32,
    /// A view that has not played yet ([`Player::new_lazy`]): its design text and the options of
    /// the session it plays with; its `session` is a still until then ([`Player::wake`]).
    lazy: Option<(String, Options)>,
    /// What waits for the session that plays: the auto-demo ([`Player::attach_demo`]), its
    /// skipped wait, a start tick ([`Player::run_on_wake`]).
    pending_demo: Option<crate::circuit::Schedule>,
    pending_skip: bool,
    pending_run: Option<u32>,
    /// Set when the view just woke up (taken by [`Player::take_woke`]: the page then picks its
    /// speed from the live session).
    woke: bool,
    /// The browser's clock (ms): a view that wakes then builds its session over frames
    /// ([`Player::wake`], `crate::staged`); None (the native tools): at once.
    clock: Option<std::rc::Rc<dyn Fn() -> f64>>,
    /// The session being built over frames ([`Player::wake`]), and its budget.
    waking: Option<(crate::staged::Task<Result<(Session, Option<u32>), String>>, crate::staged::Budget)>,
    /// The short cycle looked for as the view woke ([`Session::short_cycle`]; Some once looked).
    cycle: Option<Option<u32>>,
    /// Ticks played through unseen before the picture moves on (the auto-demo's skipped wait,
    /// `data-tick`), over frames: the tick to reach, and whether the history starts there.
    fast_forward: Option<(u32, bool)>,
    /// What the person did while the view was loading (pressed play, a button, a lever), done in
    /// order once it plays.
    pending: Vec<Box<dyn FnOnce(&mut Player)>>,
    /// The person's input actions this run, (tick, op): replayed from the run's start (or its
    /// nearest snapshot) to show an earlier tick (the timelapse). The demo's own edges are
    /// not logged: the session replays them from its schedule.
    input_log: Vec<(u32, InputOp)>,
    /// Snapshots of the run (tick, state), every [`SNAPSHOT_EVERY`] ticks, at most
    /// [`MAX_SNAPSHOTS`] (the oldest drops): the timelapse restores the nearest at or
    /// before its target and replays the log from there, so going back costs at most
    /// [`SNAPSHOT_EVERY`] ticks.
    snapshots: Vec<(u32, Snapshot)>,
    /// Rebuilding the run from a snapshot ([`Player::seek`]): the input log is not appended
    /// to, and deferred actions run at once.
    replaying: bool,
    /// The run's (start, end, active) stretches, contiguous (the scrubber's strip): a tick
    /// is active when the world was not idle after it, an input landed on it, or the door
    /// demo changed on it; the rest (waits, holds, a settled world) is inactive, dim on
    /// the strip. Rebuilt by replaying, truncated by rewinding.
    activity_runs: Vec<(u32, u32, bool)>,
    /// Bumped when [`Player::activity_runs`] changes (the strip's redraw signal).
    activity_version: u64,
    /// The furthest tick reached this run (the scrubber's end): only forward, never
    /// rewound.
    high_water: u32,
}

/// One of the person's input actions, for the timelapse: replayed from the run's start (or
/// its nearest snapshot) to show an earlier tick. Logged at execution time (a deferred
/// action logs when it runs, not when called): the tick it ran at.
#[derive(Clone, Debug)]
enum InputOp {
    /// The auto-demo ended (the person's first input clears it, before the input itself).
    StopDemo,
    /// A tap on circuit input `i` ([`Player::tap_input`]).
    TapInput(usize),
    /// A press on input `i`'s button ([`Player::input_down`]).
    InputDown(usize),
    /// The release of input `i`'s button ([`Player::input_up`]).
    InputUp(usize),
    /// Press-and-hold on circuit input `i` ([`Player::hold_input`]).
    HoldInput(usize, bool),
    /// A click on a block that is not a circuit input ([`Player::click`], [`Player::flip`]).
    Toggle(Pos),
    /// The door demo started ([`Player::start_door_demo`]).
    StartDoorDemo,
}

/// The player's state at a snapshot (the timelapse), beside the session's [`RunState`).
#[derive(Clone)]
struct Snapshot {
    run: RunState,
    history: Vec<std::collections::VecDeque<bool>>,
    history_tick: Option<u32>,
    door_demo: Option<DoorDemo>,
    pressed: Vec<Option<u32>>,
}

/// Ticks between snapshots (the timelapse), and how many are kept.
const SNAPSHOT_EVERY: u32 = 512;
const MAX_SNAPSHOTS: usize = 16;

/// A frame's time for building a waking view's session ([`Player::wake`]), ms.
pub const WAKE_SLICE_MS: f64 = 10.0;

/// Ticks of output history kept.
pub const HISTORY: usize = 40;

/// The most ticks a frame plays ([`Player::update`]): a fast speed drops what it cannot play
/// rather than block the page.
pub const MAX_STEPS_PER_FRAME: u32 = 64;

/// The door demo's stage ([`Player::start_door_demo`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorDemo {
    /// Closed at the start, the lever flipped at this tick.
    Wait(u32),
    /// The lever flipped on: the door opens.
    Opening,
    /// Open, the lever flipped back at this tick.
    Open(u32),
    /// The lever flipped off: the door closes.
    Closing,
    /// Closed again, the lever flipped at this tick.
    Closed(u32),
}

impl DoorDemo {
    /// The badge's words for the stage.
    pub fn words(self) -> &'static str {
        match self {
            DoorDemo::Wait(_) | DoorDemo::Closed(_) => "demo: the door, closed",
            DoorDemo::Opening => "demo: lever on, the door opens",
            DoorDemo::Open(_) => "demo: the door, open",
            DoorDemo::Closing => "demo: lever off, the door closes",
        }
    }

    /// Every badge text, for its size.
    pub const ALL_WORDS: [&'static str; 4] = ["demo: the door, closed", "demo: lever on, the door opens", "demo: the door, open", "demo: lever off, the door closes"];
}

/// The door demo's holds (`door video`'s: 1.5 s closed, 1.2 s open).
const DOOR_HOLD_CLOSED: u32 = 30;
const DOOR_HOLD_OPEN: u32 = 24;

/// A combination lock's recording when its page gives it no demo (its judge's sweep turns the
/// first dial through every code: tens of thousands of ticks; a lock's page stays paused): each
/// DIAL turned to its CODE frame's turn, one right-click every [`LOCK_STEP`] gt, the lamp lit and
/// held, then the last dial turned once more: shut again. With its phases (the badge names each
/// step). Like a door's recording, which shows its cycle. None: not a lock (or not simulated).
/// (Recording QA at phone size: a lock's video was 10 s of a lock standing still.)
pub fn lock_demo(s: &Session) -> Option<(crate::circuit::Schedule, Vec<(u32, u32, String)>)> {
    let lock = s.design.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("lock"));
    if !lock || !s.simulated() {
        return None;
    }
    let frame = |pre: char, k: u32| s.circuit_inputs.iter().position(|c| c.how == crate::circuit::InputHow::Frame && c.name.starts_with(pre) && c.name[1..].parse::<u32>().ok() == Some(k));
    let mut levels: Vec<(u32, usize, u8)> = Vec::new();
    let mut phases: Vec<(u32, u32, String)> = Vec::new();
    let mut t = s.tick + LOCK_STEP * 2;
    phases.push((s.tick, t, "shut: each DIAL turns to its CODE".to_string()));
    let mut last = None;
    for k in 0.. {
        let (Some(d), Some(c)) = (frame('d', k), frame('k', k)) else { break };
        let steps = (s.frame_turn(c)? + 8 - s.frame_turn(d)?) % 8;
        if steps == 0 {
            continue;
        }
        let from = t;
        for _ in 0..steps {
            levels.push((t, d, 0));
            t += LOCK_STEP;
        }
        // the phase ends with its last turn
        phases.push((from, t - LOCK_STEP + 1, format!("DIAL {} to CODE {}", k + 1, k + 1)));
        last = Some(d);
    }
    let d = last.or_else(|| frame('d', 0))?;
    // open from the last turn (the comparators and the lamp take a few ticks), held; then one
    // more turn: shut
    let open = phases.last().map(|p| p.1).unwrap_or(t);
    phases.push((open, open + LOCK_OPEN, "every DIAL at its CODE: open".to_string()));
    let shut = open + LOCK_OPEN;
    levels.push((shut, d, 0));
    phases.push((shut, shut + LOCK_OPEN, "one more turn: shut".to_string()));
    let ticks = shut + LOCK_OPEN;
    Some((crate::circuit::Schedule { edges: vec![Vec::new(); s.circuit_inputs.len()], ticks, events: Vec::new(), levels }, phases))
}

/// [`lock_demo`]'s pace: a right-click every this many ticks, and the open lock held this long.
pub const LOCK_STEP: u32 = 8;
pub const LOCK_OPEN: u32 = 50;

/// A door demo's whole cycle in ticks from the start (closed, opening, open, closing, and the
/// end of the closed hold after), run on its own copy of the design; None: not a door, or it never
/// settles within 4000 ticks.
pub fn door_demo_cycle(text: &str) -> Option<u32> {
    let mut p = Player::new(text, 1.0).ok()?;
    if !p.session.mode.is_door() || !p.session.simulated() {
        return None;
    }
    p.start_door_demo();
    let start = p.session.tick;
    let mut opened = false;
    for _ in 0..4000 {
        p.session.step();
        p.door_demo_tick();
        match p.door_demo {
            Some(DoorDemo::Open(_)) => opened = true,
            Some(DoorDemo::Closed(at)) if opened => return Some(at - start),
            _ => {}
        }
    }
    None
}

impl Player {
    pub fn new(text: &str, aspect: f32) -> Result<Player, String> {
        let mut session = Session::new(text, &Options::default())?;
        session.find_auto_output(20_000);
        Ok(Player::with_session(session, aspect))
    }

    /// The browser's view of a page (vjeux 2026-10-21, "Why is it simulating everything?"; the
    /// coordinator: a page's view simulates only on demand, incrementally, the judge's seeding
    /// kept): a still of the design as placed until it plays or is pressed ([`Player::wake`]
    /// builds then the session that plays, [`Options::live`]: no look-ahead). `still` is the
    /// page's `data-still` (its output cell and picture box, found at build time; no tick run).
    pub fn new_lazy(text: &str, aspect: f32, still: Option<&str>) -> Result<Player, String> {
        Player::new_lazy_with(text, aspect, still, None)
    }

    /// [`Player::new_lazy`] with a crop farm's judge's speed (`Options::farm_rts`: the page's
    /// `data-farm-rts`; a big farm is shown as built without it).
    pub fn new_lazy_with(text: &str, aspect: f32, still: Option<&str>, farm_rts: Option<u32>) -> Result<Player, String> {
        let mut session = Session::new(text, &Options { farm_rts, ..Options::still() })?;
        if let Some(a) = still {
            let _ = session.take_still_marks(a);
        }
        let mut p = Player::with_session(session, aspect);
        if p.session.simulated() {
            p.lazy = Some((text.to_string(), Options { farm_rts, ..Options::live() }));
        }
        Ok(p)
    }

    /// The browser's session that plays at once (a recording): the judge's seeding, no
    /// look-ahead ([`Options::live`]).
    pub fn new_web(text: &str, aspect: f32) -> Result<Player, String> {
        Player::new_web_with(text, aspect, None)
    }

    /// [`Player::new_web`] with a crop farm's judge's speed (`Options::farm_rts`).
    pub fn new_web_with(text: &str, aspect: f32, farm_rts: Option<u32>) -> Result<Player, String> {
        Ok(Player::with_session(Session::new(text, &Options { farm_rts, ..Options::live() })?, aspect))
    }

    fn with_session(session: Session, aspect: f32) -> Player {
        let mut look = scene::viewer_look(&session);
        look.lit_marker = true;
        let camera = scene::default_camera(&session, &look, aspect);
        let pulse_inputs = crate::circuit::pulse_kind(&session.design);
        let n_in = session.circuit_inputs.len();
        let n_out = session.outputs.len();
        let start = session.tick;
        let mut p = Player {
            session,
            look,
            home: camera,
            camera,
            playing: false,
            tps: 20.0,
            acc: 0.0,
            partial: 0.0,
            mesh: None,
            mesh_key: (u32::MAX, 0),
            dirty: true,
            hold: 0.0,
            follow: None,
            pulse_inputs,
            phases: Vec::new(),
            history: vec![Default::default(); n_out],
            history_tick: None,
            pressed: vec![None; n_in],
            door_demo: None,
            demo_from: 0,
            lazy: None,
            pending_demo: None,
            pending_skip: false,
            pending_run: None,
            woke: false,
            clock: None,
            waking: None,
            cycle: None,
            fast_forward: None,
            pending: Vec::new(),
            input_log: Vec::new(),
            snapshots: Vec::new(),
            replaying: false,
            activity_runs: Vec::new(),
            activity_version: 0,
            high_water: start,
        };
        p.follow_machine();
        p.record();
        p
    }

    /// Is the view still the picture it loaded with (it has not played or been pressed yet, or
    /// its session is still being built)?
    pub fn is_still(&self) -> bool {
        self.lazy.is_some() || self.waking.is_some()
    }

    /// Is its session being built over frames ([`Player::wake`])?
    pub fn is_waking(&self) -> bool {
        self.waking.is_some()
    }

    /// The browser's clock (ms): from now on a view that wakes builds its session over frames, a
    /// slice of [`WAKE_SLICE_MS`] a frame, and plays through skipped ticks within its frames'
    /// time (`crate::staged`; the coordinator 2026-10-22: pressing play ran one 300-450 ms task).
    pub fn set_clock(&mut self, clock: std::rc::Rc<dyn Fn() -> f64>) {
        self.clock = Some(clock);
    }

    /// The short cycle looked for as the view woke (Some once looked for: `Some(None)`, none).
    pub fn cycle_found(&self) -> Option<Option<u32>> {
        self.cycle
    }

    /// The view stays the still it loaded with and never plays (the page's `data-asbuilt`: the
    /// site's build found the browser cannot run the design): shown as built, `why` said.
    pub fn keep_still(&mut self, why: &str) {
        self.lazy = None;
        self.waking = None;
        self.pending.clear();
        if self.session.unsimulated.is_none() {
            self.session.unsimulated = Some(why.to_string());
        }
        self.session.circuit_inputs.clear();
        self.session.input_on.clear();
        self.pressed.clear();
        self.invalidate();
    }

    /// The view plays from now on: the session that plays replaces the still (the judge's
    /// seeding: its placement, settle and chests, as `Session::new` builds it), the camera kept;
    /// the auto-demo, its skipped wait and a start tick waiting for it applied. With the browser's
    /// clock ([`Player::set_clock`]) the session is built over frames ([`Player::update_with`]
    /// goes on with it), its short cycle looked for there too; else at once. False when it was
    /// already awake (or building).
    pub fn wake(&mut self) -> bool {
        let Some((text, o)) = self.lazy.take() else { return false };
        match self.clock.clone() {
            Some(clock) => {
                let b = crate::staged::Budget::with_clock(clock);
                let budget = b.clone();
                let task = crate::staged::Task::new(async move {
                    let s = Session::build(text, o, budget.clone()).await?;
                    let cycle = if s.wants_cycle_check() { s.short_cycle_staged(9, 120, &budget).await } else { None };
                    Ok((s, cycle))
                });
                self.waking = Some((task, b));
            }
            None => {
                let s = match Session::new(&text, &o) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                let cycle = if s.wants_cycle_check() { s.short_cycle(9, 120) } else { None };
                self.finish_wake(s, cycle);
            }
        }
        true
    }

    /// The session being built: `ms` more of it. True once it plays (or never will: it failed).
    fn poll_wake(&mut self, ms: f64) -> bool {
        let Some((task, b)) = self.waking.as_mut() else { return true };
        b.give(ms);
        match task.poll() {
            None => false,
            Some(Ok((s, cycle))) => {
                self.waking = None;
                self.finish_wake(s, cycle);
                true
            }
            Some(Err(e)) => {
                // it cannot play: the still stays, saying why
                self.waking = None;
                self.pending.clear();
                self.playing = false;
                self.session.unsimulated = Some(e);
                self.invalidate();
                true
            }
        }
    }

    /// The built session takes the still's place.
    fn finish_wake(&mut self, session: Session, cycle: Option<u32>) {
        let start = session.tick;
        let marks = (self.session.outputs.clone(), self.session.picture);
        self.session = session;
        // the output cell and picture box the page gave (`data-still`) stay
        if self.session.outputs.is_empty() {
            self.session.outputs = marks.0;
        }
        if self.session.picture.is_none() {
            self.session.picture = marks.1;
        }
        let mut look = scene::viewer_look(&self.session);
        look.lit_marker = true;
        self.look = look;
        self.history = vec![Default::default(); self.session.outputs.len()];
        self.history_tick = None;
        self.pressed = vec![None; self.session.circuit_inputs.len()];
        self.cycle = Some(cycle);
        // a new run: the old run's input log and snapshots are gone
        self.input_log.clear();
        self.snapshots.clear();
        self.activity_runs.clear();
        self.activity_version += 1;
        self.high_water = start;
        if let Some(d) = self.pending_demo.take() {
            self.session.attach_demo(d);
        }
        // the auto-demo's skipped wait, then `data-tick`: played through over frames
        let mut target = self.session.tick;
        let mut fresh = false;
        if std::mem::take(&mut self.pending_skip)
            && let Some(to) = self.demo_skip_target()
        {
            self.demo_from = to;
            target = to;
            fresh = true;
        }
        if let Some(t) = self.pending_run.take() {
            target += t;
        }
        self.woke = true;
        self.follow_machine();
        self.record();
        self.invalidate();
        if target > self.session.tick {
            self.fast_forward = Some((target, fresh));
            if self.clock.is_none() {
                self.advance_fast_forward(&mut || false);
            }
        } else {
            self.run_pending();
        }
    }

    /// Ticks played through unseen ([`Player::fast_forward`]) until `out_of_time`; true once
    /// there (the picture then moves on, the history starting there for a skipped wait, and what
    /// the person did meanwhile is done).
    fn advance_fast_forward(&mut self, out_of_time: &mut dyn FnMut() -> bool) -> bool {
        let Some((to, fresh)) = self.fast_forward else { return true };
        let mut n = 0;
        while self.session.tick < to && self.session.halted.is_none() {
            if n > 0 && out_of_time() {
                return false;
            }
            self.session.step();
            n += 1;
        }
        self.fast_forward = None;
        if fresh {
            self.history = vec![Default::default(); self.session.outputs.len()];
            self.history_tick = None;
        }
        self.record();
        self.follow_machine();
        self.invalidate();
        self.run_pending();
        true
    }

    /// What the person did while the view was loading, in order.
    fn run_pending(&mut self) {
        for f in std::mem::take(&mut self.pending) {
            f(self);
        }
    }

    /// The view is loading (a still that wakes now, its session being built, ticks played
    /// through): `f` waits for it (true); else nothing waits (false: do it now). While
    /// replaying (the timelapse), nothing waits: the replayed action runs at once.
    fn defer(&mut self, f: impl FnOnce(&mut Player) + 'static) -> bool {
        if self.replaying {
            return false;
        }
        if self.lazy.is_some() {
            self.wake();
        }
        if self.waking.is_some() || self.fast_forward.is_some() {
            self.pending.push(Box::new(f));
            return true;
        }
        false
    }

    /// Did the view just wake up (once)?
    pub fn take_woke(&mut self) -> bool {
        std::mem::take(&mut self.woke)
    }

    /// Attach the auto-demo (`data-stimulus`): to the session that plays, when it does.
    pub fn attach_demo(&mut self, d: crate::circuit::Schedule) {
        if self.lazy.is_some() || self.waking.is_some() {
            self.pending_demo = Some(d);
        } else {
            self.session.attach_demo(d);
            // the run's starting conditions changed: its snapshot at this tick is refreshed,
            // so a rewind keeps the demo
            self.snapshot_now();
        }
    }

    /// Start at tick `t` (`data-tick`) once the view plays.
    pub fn run_on_wake(&mut self, t: u32) {
        if self.lazy.is_some() || self.waking.is_some() {
            self.pending_run = Some(t);
        } else {
            self.session.run(t);
        }
    }

    /// The auto-demo waiting for the view to play (`Player::attach_demo` before it woke).
    pub fn pending_demo(&self) -> Option<&crate::circuit::Schedule> {
        self.pending_demo.as_ref()
    }

    /// The auto-demo starts 1 s before its first change when that is more than 3 s away (a BUD
    /// switch's judge waits 12 s before its first event: critique, "demo: waiting" at 4.6 s): the
    /// session plays there unseen (every tick simulated; the counter shows the real tick: over
    /// frames with the browser's clock, at once without), and a reset comes back there.
    pub fn skip_demo_wait(&mut self) {
        // a view that has not played yet skips it when it does
        if self.lazy.is_some() || self.waking.is_some() {
            self.pending_skip = true;
            return;
        }
        let Some(to) = self.demo_skip_target() else { return };
        self.demo_from = to;
        self.fast_forward = Some((to, true));
        if self.clock.is_none() {
            self.advance_fast_forward(&mut || false);
        }
    }

    /// Where a skipped wait goes ([`Player::skip_demo_wait`]): 1 s before the demo's first change
    /// when that is more than 3 s away.
    fn demo_skip_target(&self) -> Option<u32> {
        let d = self.session.demo.as_ref()?;
        // a driven input's switch goes 2 gt before its edge (`Session::inputs_due`)
        let first = d.edges.iter().flatten().map(|(t, _)| t.saturating_sub(2)).chain(d.events.iter().map(|(t, _, _)| *t)).chain(d.levels.iter().map(|(t, _, _)| *t)).min()?;
        (first > 60 && self.session.tick + 20 < first).then(|| first - 20)
    }

    /// A travelling design (a flying machine): move the camera with its centre (moving blocks at
    /// their drawn offset, `stop:` blocks left out).
    /// A flying machine's centre: while it plays the camera follows it (it leaves the frame
    /// otherwise); nothing else ever moves the camera by itself. `move_camera` false only updates
    /// the reference (paused stepping), so the next follow does not jump.
    fn follow_machine_with(&mut self, move_camera: bool) {
        if self.session.corridor.is_none() || !self.is_flying_machine() {
            return;
        }
        let blocks = self.session.render_blocks(self.partial);
        let stops: Vec<Pos> = self.session.design.stops.iter().map(|(p, _)| *p).collect();
        let (mut c, mut n) = ([0f32; 3], 0usize);
        for b in blocks.iter().filter(|b| !(b.offset == [0.0; 3] && stops.contains(&b.cell))) {
            for i in 0..3 {
                c[i] += b.cell[i] as f32 + b.offset[i] + 0.5;
            }
            n += 1;
        }
        if n == 0 {
            return;
        }
        let c = c.map(|v| v / n as f32);
        if let Some(prev) = self.follow
            && move_camera
        {
            for i in 0..3 {
                self.camera.target[i] += c[i] - prev[i];
            }
        }
        self.follow = Some(c);
    }

    fn follow_machine(&mut self) {
        self.follow_machine_with(true);
    }

    /// A flying machine (`circuit: flying-machine`, or a `travel:` design without a kind): the
    /// only design the camera follows. A piston extender's payload comes back: the camera stays.
    fn is_flying_machine(&self) -> bool {
        self.session.design.meta.get("circuit").is_none_or(|k| k.trim_start().starts_with("flying-machine"))
    }

    /// Advance the clock by `dt` seconds; true when the picture changed.
    pub fn update(&mut self, dt: f32) -> bool {
        self.update_with(dt, &mut || false)
    }

    /// [`Player::update`] with a frame's time budget: it stops stepping once `out_of_time` says
    /// so (the browser: the frame's ~12 ms; the coordinator 2026-10-21: never block the page over
    /// ~50 ms a frame), the ticks it could not play dropped.
    pub fn update_with(&mut self, dt: f32, out_of_time: &mut dyn FnMut() -> bool) -> bool {
        if !self.playing {
            return std::mem::take(&mut self.dirty);
        }
        // played: the still becomes the session that plays (its seeding then, not at load), built
        // over frames with the browser's clock: a slice a frame, the still shown meanwhile
        self.wake();
        if self.waking.is_some() {
            if self.poll_wake(WAKE_SLICE_MS) {
                // built: its first picture now (the ticks start next frame)
                return true;
            }
            return std::mem::take(&mut self.dirty);
        }
        // ticks played through unseen (a skipped wait), within the frame's time
        if self.fast_forward.is_some() {
            if self.advance_fast_forward(out_of_time) {
                return true;
            }
            return std::mem::take(&mut self.dirty);
        }
        if !self.session.playable() {
            return std::mem::take(&mut self.dirty);
        }
        // a flying machine at its judge's end (the corridor's end), or the auto-demo at the end
        // of the judge's stimulus: a pause, then again
        let end = match (&self.session.demo, self.session.end_tick) {
            (Some(d), _) => Some(d.ticks),
            (None, e) => e,
        };
        if let Some(e) = end
            && self.session.tick >= e
        {
            self.hold += dt.min(0.25);
            if self.hold < 1.0 {
                return std::mem::take(&mut self.dirty);
            }
            self.hold = 0.0;
            self.reset();
            return true;
        }
        self.acc += dt.min(0.25) * self.tps;
        let mut n = 0;
        // a few ticks a frame, never a long run on the main thread (the coordinator 2026-10-21: no
        // view may block the page): at most MAX_STEPS_PER_FRAME, the rest dropped
        while self.acc >= 1.0 && n < MAX_STEPS_PER_FRAME && self.end().is_none_or(|e| self.session.tick < e) {
            if n > 0 && out_of_time() {
                self.acc = self.acc.min(1.0);
                break;
            }
            self.step_once();
            self.acc -= 1.0;
            n += 1;
        }
        if n >= MAX_STEPS_PER_FRAME {
            self.acc = self.acc.min(1.0);
        }
        // interpolate moving blocks only at watchable speeds
        self.partial = if self.tps <= 40.0 { self.acc.clamp(0.0, 0.999) } else { 0.0 };
        self.follow_machine();
        self.dirty = false;
        true
    }

    /// Where the run ends: the auto-demo's end, or a flying machine's (its judge's corridor end).
    fn end(&self) -> Option<u32> {
        match (&self.session.demo, self.session.end_tick) {
            (Some(d), _) => Some(d.ticks),
            (None, e) => e,
        }
    }

    /// A flying machine's end found as it flies (the browser: no run ahead to it,
    /// `Session::fly_end_due`): the tick it gets there.
    fn note_fly_end(&mut self) {
        if self.session.fly_end_due() && self.session.at_fly_end() {
            self.session.end_tick = Some(self.session.tick);
        }
    }

    /// Append the outputs' levels to the history (once per tick).
    fn record(&mut self) {
        if self.history_tick == Some(self.session.tick) {
            return;
        }
        self.history_tick = Some(self.session.tick);
        let outs = self.session.outputs.clone();
        if self.history.len() != outs.len() {
            self.history = vec![Default::default(); outs.len()];
        }
        for (h, o) in self.history.iter_mut().zip(&outs) {
            h.push_back(self.session.output_powered(*o));
            while h.len() > HISTORY {
                h.pop_front();
            }
        }
        self.maybe_snapshot();
    }

    /// How many ticks a recording from tick 0 shows (`max_ticks` at most): the auto-demo's whole
    /// run, a door demo's cycle (`door_cycle`, [`door_demo_cycle`]), a clock's warmup and whole
    /// periods (at least one, about 8 s at its speed), a flying machine's flight; else 10 s.
    pub fn recording_ticks(&self, door_cycle: Option<u32>, max_ticks: u32) -> u32 {
        let s = &self.session;
        let secs = |x: f32| (x * self.tps).round() as u32;
        let t = if let Some(d) = &s.demo {
            // what is left of it (it may start after its long wait: `skip_demo_wait`)
            d.ticks.saturating_sub(s.tick)
        } else if let Some(c) = door_cycle {
            c
        } else if let Some(e) = s.end_tick {
            e + secs(1.0)
        } else if s.circuit_inputs.is_empty() && !s.mode.is_door() && !s.design.meta.contains_key("circuit") && s.simulated() {
            match s.period(20_000) {
                Ok(p) if p.period > 0 => {
                    let n = (secs(8.0) as f32 / p.period as f32).ceil().max(1.0) as u32;
                    p.warmup + n * p.period
                }
                _ => secs(10.0),
            }
        } else {
            secs(10.0)
        };
        t.clamp(1, max_ticks.max(1))
    }

    /// The checker's phase label at this tick (the auto-demo's badge).
    pub fn phase(&self) -> Option<&str> {
        let t = self.session.tick;
        self.phases.iter().find(|(a, b, _)| *a <= t && t < *b).map(|(_, _, l)| l.as_str())
    }

    /// Any input from the person ends the auto-demo (the inputs are theirs from then on).
    pub fn stop_demo(&mut self) {
        // the person's first input ends the auto-demo (logged before the input itself, at
        // this tick: a deferred stop still cleared it at call time)
        self.log_op(InputOp::StopDemo);
        self.session.demo = None;
        self.pending_demo = None;
        self.pending_skip = false;
        self.door_demo = None;
    }

    /// Start the door demo (a door viewer's play: the page's test on a loop): its lever flipped
    /// like the record videos (`door video`: a 1.5 s hold, the door opening until it settles,
    /// 1.2 s open, closing until it settles, 1.5 s closed, again). Lever uses only: what runs is
    /// redsim's.
    pub fn start_door_demo(&mut self) {
        if self.session.mode.is_door() && self.session.simulated() {
            if self.defer(|p| p.start_door_demo()) {
                return;
            }
            self.door_demo = Some(DoorDemo::Wait(self.session.tick + DOOR_HOLD_CLOSED));
            self.log_op(InputOp::StartDoorDemo);
        }
    }

    /// The door demo after a tick: flip the lever when its stage is over.
    fn door_demo_tick(&mut self) {
        let Some(d) = self.door_demo else { return };
        let Mode::Door { lever, .. } = self.session.mode else { return };
        let t = self.session.tick;
        let settled = self.session.idle() && self.session.since_input() >= 2;
        let next = match d {
            DoorDemo::Wait(at) | DoorDemo::Closed(at) if t >= at => {
                self.session.toggle(lever);
                DoorDemo::Opening
            }
            DoorDemo::Opening if settled => DoorDemo::Open(t + DOOR_HOLD_OPEN),
            DoorDemo::Open(at) if t >= at => {
                self.session.toggle(lever);
                DoorDemo::Closing
            }
            DoorDemo::Closing if settled => DoorDemo::Closed(t + DOOR_HOLD_CLOSED),
            d => d,
        };
        if next != d {
            self.invalidate();
        }
        self.door_demo = Some(next);
    }

    /// A tap on circuit input `i`: a short pulse (pulse inputs) or a flip (level inputs). A pulse
    /// input the demo left on is let go instead (a pulse cannot start on an input already on:
    /// the tap did nothing at all, and stopped the demo; QA at phone size, dff-lock's CLK).
    pub fn tap_input(&mut self, i: usize) -> bool {
        self.stop_demo();
        if self.defer(move |p| {
            p.tap_input(i);
        }) {
            return true;
        }
        self.log_op(InputOp::TapInput(i));
        let ok = if self.pulse_inputs {
            if self.session.input_on.get(i).copied().unwrap_or(false) { self.session.set_input(i, false) } else { self.session.pulse_input(i, 2) }
        } else {
            self.session.set_input(i, !self.session.input_on[i])
        };
        if ok {
            self.invalidate();
        }
        ok
    }

    /// A press on input `i`'s button (momentary for pulse inputs: on while held, at least 2 gt;
    /// a flip for level inputs). A pulse input the demo left on: the press lets it go (as
    /// [`Player::tap_input`]).
    pub fn input_down(&mut self, i: usize) -> bool {
        if !self.pulse_inputs {
            return self.tap_input(i);
        }
        self.stop_demo();
        if self.defer(move |p| {
            p.input_down(i);
        }) {
            return true;
        }
        self.log_op(InputOp::InputDown(i));
        if self.session.input_on.get(i).copied().unwrap_or(false) {
            let ok = self.session.set_input(i, false);
            if ok {
                self.invalidate();
            }
            return ok;
        }
        if i < self.pressed.len() && self.session.set_input(i, true) {
            self.pressed[i] = Some(self.session.tick);
            self.invalidate();
            return true;
        }
        false
    }

    /// The release of input `i`'s button.
    pub fn input_up(&mut self, i: usize) {
        if !self.pulse_inputs {
            return;
        }
        // its press waits for the view to load: so does the release
        if (self.waking.is_some() || self.fast_forward.is_some()) && self.defer(move |p| p.input_up(i)) {
            return;
        }
        self.log_op(InputOp::InputUp(i));
        if i >= self.pressed.len() {
            return;
        }
        if let Some(at) = self.pressed[i].take() {
            if self.session.tick >= at + 2 {
                self.session.set_input(i, false);
            } else {
                // a tap: it stays on for 2 gt
                self.session.release_input_at(i, at + 2);
            }
            self.invalidate();
        }
    }

    /// One tick (paused stepping).
    pub fn step(&mut self) {
        if self.defer(|p| p.step()) {
            return;
        }
        self.step_once();
        self.acc = 0.0;
        self.partial = 0.0;
        // paused: the camera stays (only a playing flying machine is followed)
        self.follow_machine_with(false);
        self.invalidate();
    }

    /// One tick of the run: the session steps, the history, the door demo and the fly end
    /// advance with it. [`Player::step`] and [`Player::update_with`] both go through here, so
    /// a replayed run (the timelapse) takes the same ticks as the played one.
    fn step_once(&mut self) {
        let demo = self.door_demo;
        let inputs = self.session.input_on.clone();
        self.session.step();
        self.record();
        self.door_demo_tick();
        self.note_fly_end();
        // the tick just landed on changed something when the world is not idle after it, a
        // demo edge landed on it (its input changed), or the door demo changed on it (its
        // lever flipped, or the door finished opening/closing)
        let active = !self.session.idle() || self.door_demo != demo || self.session.input_on != inputs;
        self.record_activity(active);
    }

    /// Jump to a tick of this run (the scrubber): back rewinds (the future past it is
    /// dropped, like [`Player::step_back`]), forward steps (re-simulating after a
    /// rewind). Clamped to the run's earliest reachable tick. False when already there
    /// or the view has not played. A pending fast-forward is dropped: the visitor took
    /// control of the tick.
    pub fn seek_to(&mut self, target: u32) -> bool {
        if self.lazy.is_some() || self.waking.is_some() || !self.session.simulated() {
            return false;
        }
        let (floor, _) = self.scrub_range();
        let target = target.max(floor);
        if target == self.session.tick {
            return false;
        }
        self.fast_forward = None;
        if target < self.session.tick {
            self.seek(target);
        } else {
            while self.session.tick < target {
                self.step_once();
            }
        }
        self.acc = 0.0;
        self.partial = 0.0;
        self.follow_machine_with(false);
        self.invalidate();
        true
    }

    /// The scrubber's range: the earliest reachable tick (the oldest snapshot's), and the
    /// furthest tick reached this run.
    pub fn scrub_range(&self) -> (u32, u32) {
        (self.rewind_floor(), self.high_water.max(self.session.tick))
    }

    /// The run's (start, end, active) stretches for the scrubber's strip, covering
    /// [`Player::scrub_range`]: consecutive ticks with the same activity, inactive
    /// stretches (waits, holds, a settled world) dim. `end` is exclusive.
    pub fn activity_runs(&self) -> Vec<(u32, u32, bool)> {
        let (floor, high) = self.scrub_range();
        let end = high + 1;
        let mut out: Vec<(u32, u32, bool)> = Vec::new();
        let push = |out: &mut Vec<(u32, u32, bool)>, s: u32, e: u32, a: bool| {            if s >= e {
                return;
            }
            if let Some((_, pe, pa)) = out.last_mut() {
                if *pa == a && *pe == s {
                    *pe = e;
                    return;
                }
            }
            out.push((s, e, a));
        };
        let mut cur = floor;
        for &(s, e, a) in &self.activity_runs {
            if e <= floor {
                continue;
            }
            if s > cur {
                push(&mut out, cur, s.min(end), false);
            }
            push(&mut out, s.max(cur), e.min(end), a);
            cur = e.min(end);
            if cur >= end {
                break;
            }
        }
        push(&mut out, cur, end, false);
        out
    }

    /// A counter bumped when [`Player::activity_runs`] changes (the strip's redraw signal).
    pub fn activity_version(&self) -> u64 {
        self.activity_version
    }

    /// Extend the activity stretches to cover tick `t` (and any gap before it, a
    /// fast-forward jump: never displayed, inactive).
    fn record_activity(&mut self, active: bool) {
        let t = self.session.tick;
        self.high_water = self.high_water.max(t);
        self.extend_activity(t, t + 1, active);
    }

    /// Extend the activity stretches to cover [s, e) with one activity, merging with the
    /// last stretch when it matches and meets it.
    fn extend_activity(&mut self, s: u32, e: u32, active: bool) {
        if s >= e {
            return;
        }
        if let Some((_, pe, pa)) = self.activity_runs.last_mut() {
            if *pa == active && *pe == s {
                *pe = e;
                self.activity_version += 1;
                return;
            }
        }
        self.activity_runs.push((s, e, active));
        self.activity_version += 1;
    }

    /// Mark tick `t` active (an input the visitor or a tap landed on it: logged in
    /// [`Player::log_op`): split the stretch covering it.
    fn mark_tick_active(&mut self, t: u32) {
        // a gap before it (an input while waking, before any step): never displayed
        if let Some((_, end, _)) = self.activity_runs.last() {
            if *end < t {
                self.extend_activity(*end, t, false);
            }
        }
        let Some(idx) = self.activity_runs.iter().position(|&(s, e, _)| s <= t && t < e) else {
            self.extend_activity(t, t + 1, true);
            return;
        };
        if self.activity_runs[idx].2 {
            return;
        }
        let (s, e, _) = self.activity_runs[idx];
        self.activity_runs.remove(idx);
        let mut at = idx;
        if s < t {
            self.activity_runs.insert(at, (s, t, false));
            at += 1;
        }
        self.activity_runs.insert(at, (t, t + 1, true));
        if t + 1 < e {
            self.activity_runs.insert(at + 1, (t + 1, e, false));
        }
        self.activity_version += 1;
    }

    /// Drop the activity stretches at or past `to` (a rewind: the run is rebuilt from
    /// there).
    fn truncate_activity(&mut self, to: u32) {
        let mut changed = false;
        while let Some(&(s, e, _)) = self.activity_runs.last() {
            if s >= to {
                self.activity_runs.pop();
                changed = true;
            } else if e > to {
                self.activity_runs.last_mut().unwrap().1 = to;
                changed = true;
                break;
            } else {
                break;
            }
        }
        if changed {
            self.activity_version += 1;
        }
    }

    /// Drop the activity stretches before `floor` (the oldest snapshot fell off: the
    /// rewind floor moved past them).
    fn drain_activity(&mut self, floor: u32) {
        let mut changed = false;
        while let Some(&(s, e, _)) = self.activity_runs.first() {
            if e <= floor {
                self.activity_runs.remove(0);
                changed = true;
            } else if s < floor {
                self.activity_runs.first_mut().unwrap().0 = floor;
                changed = true;
                break;
            } else {
                break;
            }
        }
        if changed {
            self.activity_version += 1;
        }
    }

    /// Whether [`Player::step_back`] can go back: the run has a tick behind it, back to the
    /// oldest snapshot.
    pub fn can_step_back(&self) -> bool {
        self.lazy.is_none() && self.waking.is_none() && self.session.simulated() && self.session.tick > self.rewind_floor()
    }

    /// The earliest tick the timelapse reaches: the oldest snapshot's (a fresh run's start).
    fn rewind_floor(&self) -> u32 {
        self.snapshots.first().map(|(t, _)| *t).unwrap_or(self.session.tick)
    }

    /// One tick back in time (the timelapse): the run is rebuilt from its nearest snapshot
    /// and replayed to the tick before this one, with the person's inputs replayed to there;
    /// later inputs are dropped (that future is gone: stepping forward again replays the
    /// world without them, until the person acts again). False at the run's start, or for a
    /// view that has not played: nothing to go back to. The camera stays.
    pub fn step_back(&mut self) -> bool {
        if !self.can_step_back() {
            return false;
        }
        let target = self.session.tick.saturating_sub(1);
        self.seek(target);
        self.acc = 0.0;
        self.partial = 0.0;
        self.follow_machine_with(false);
        self.invalidate();
        true
    }

    /// Rebuild the run at `target` (at or behind the tick now): restore the nearest snapshot
    /// at or before it, replay the logged inputs to it. The log and snapshots past `target`
    /// are dropped: that future is gone.
    fn seek(&mut self, target: u32) {
        let idx = self.snapshots.iter().rposition(|(t, _)| *t <= target).expect("the floor snapshot");
        let snap_tick = self.snapshots[idx].0;
        let snap = self.snapshots[idx].1.clone();
        self.snapshots.truncate(idx + 1);
        self.input_log.retain(|(t, _)| *t < target);
        // the activity past the restored snapshot is rebuilt by the replay below
        self.truncate_activity(snap_tick);
        self.pending.clear();
        self.session.restore(snap.run);
        self.history = snap.history;
        self.history_tick = snap.history_tick;
        self.door_demo = snap.door_demo;
        self.pressed = snap.pressed;
        self.hold = 0.0;
        self.replaying = true;
        while self.session.tick < target {
            let t = self.session.tick;
            let ops: Vec<InputOp> = self.input_log.iter().filter(|(at, _)| *at == t).map(|(_, op)| op.clone()).collect();
            for op in ops {
                self.apply_input_op(op);
            }
            self.step_once();
        }
        self.replaying = false;
        self.follow_machine();
    }

    /// Run a logged input action again (the timelapse's replay): the same method the person
    /// ran, so it lands the same way (its own log call is suppressed: [`Player::replaying`]).
    fn apply_input_op(&mut self, op: InputOp) {
        match op {
            InputOp::StopDemo => self.stop_demo(),
            InputOp::TapInput(i) => {
                let _ = self.tap_input(i);
            }
            InputOp::InputDown(i) => {
                let _ = self.input_down(i);
            }
            InputOp::InputUp(i) => self.input_up(i),
            InputOp::HoldInput(i, on) => {
                let _ = self.hold_input(i, on);
            }
            InputOp::Toggle(pos) => {
                self.stop_demo();
                let _ = self.session.toggle(pos);
            }
            InputOp::StartDoorDemo => self.start_door_demo(),
        }
    }

    /// Log the person's input action at this tick (the timelapse). Nothing while replaying.
    /// The tick is marked active (the scrubber's strip): the input visibly changed the run.
    fn log_op(&mut self, op: InputOp) {
        if !self.replaying {
            self.input_log.push((self.session.tick, op));
            self.mark_tick_active(self.session.tick);
        }
    }

    /// Snapshot the run now, if due (every [`SNAPSHOT_EVERY`] ticks): the timelapse's
    /// restore point. Called after every tick's [`Player::record`], and on a fresh run.
    fn maybe_snapshot(&mut self) {
        let t = self.session.tick;
        if self.snapshots.last().is_some_and(|(s, _)| *s == t) {
            return;
        }
        if self.snapshots.is_empty() || t >= self.snapshots.last().map(|(s, _)| *s).unwrap_or(0) + SNAPSHOT_EVERY {
            self.snapshots.push((t, self.snapshot_state()));
            while self.snapshots.len() > MAX_SNAPSHOTS {
                self.snapshots.remove(0);
            }
            // the rewind floor moved past the oldest snapshot: its activity is unreachable
            if let Some(&(floor, _)) = self.snapshots.first() {
                self.drain_activity(floor);
            }
        }
    }

    /// Snapshot the run now, replacing the snapshot at this tick if there is one (the run's
    /// starting conditions changed after the tick's snapshot: [`Player::attach_demo`]).
    fn snapshot_now(&mut self) {
        let t = self.session.tick;
        self.snapshots.retain(|(s, _)| *s != t);
        self.snapshots.push((t, self.snapshot_state()));
        while self.snapshots.len() > MAX_SNAPSHOTS {
            self.snapshots.remove(0);
        }
    }

    /// The run's state now (the session's plus the player's), for a snapshot.
    fn snapshot_state(&self) -> Snapshot {
        Snapshot { run: self.session.snapshot(), history: self.history.clone(), history_tick: self.history_tick, door_demo: self.door_demo, pressed: self.pressed.clone() }
    }

    pub fn reset(&mut self) {
        // a view that never played is still its picture (or still being built)
        if self.lazy.is_some() || self.waking.is_some() {
            self.invalidate();
            return;
        }
        // a new run: the old run's input log and snapshots are gone
        self.input_log.clear();
        self.snapshots.clear();
        self.activity_runs.clear();
        self.activity_version += 1;
        self.session.reset();
        self.high_water = self.session.tick;
        self.fast_forward = None;
        // the auto-demo comes back where it is shown from (its long wait skipped: played through
        // unseen, over frames with the browser's clock)
        if self.session.demo.is_some() && self.demo_from > self.session.tick {
            self.fast_forward = Some((self.demo_from, true));
            if self.clock.is_none() {
                self.advance_fast_forward(&mut || false);
            }
        }
        self.history = vec![Default::default(); self.session.outputs.len()];
        self.history_tick = None;
        self.pressed = vec![None; self.session.circuit_inputs.len()];
        // the door demo starts over, closed
        if self.door_demo.is_some() {
            self.door_demo = Some(DoorDemo::Wait(self.session.tick + DOOR_HOLD_CLOSED));
        }
        self.record();
        self.acc = 0.0;
        self.partial = 0.0;
        self.follow_machine();
        self.invalidate();
    }

    /// The block under a point of the view (x, y in -1..1, y up).
    pub fn pick(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let blocks = self.session.render_blocks(0.0);
        crate::pick::pick(atlas, &blocks, o, d, self.camera.dist * 4.0 + 50.0)
    }

    /// A click: flips the lever / presses the button under the pointer, paused or playing (the
    /// frame is rebuilt at once: the cached mesh is keyed by tick, which a click does not move).
    /// Returns the cell.
    /// The circuit input under a point of the view (x, y in -1..1): its cell (often empty: the
    /// judge writes a block there) or the device in it, when nothing nearer covers it.
    pub fn input_at(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<usize> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let picked = self.pick(atlas, x, y, aspect);
        let hit_dist = |c: Pos| -> Option<f32> { ray_box(o, d, [c[0] as f32, c[1] as f32, c[2] as f32], [c[0] as f32 + 1.0, c[1] as f32 + 1.0, c[2] as f32 + 1.0]) };
        let picked_dist = picked.and_then(hit_dist).unwrap_or(f32::MAX);
        self.session
            .circuit_inputs
            .iter()
            .enumerate()
            .filter_map(|(i, ci)| hit_dist(ci.pos).map(|t| (i, ci.pos, t)))
            .filter(|(_, p, t)| Some(*p) == picked || *t <= picked_dist + 1e-3)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, _, _)| i)
    }

    /// Press-and-hold on circuit input `i` in the view: on while held (`on`), off when released
    /// (a pulse input stays on at least 2 gt, like its button).
    pub fn hold_input(&mut self, i: usize, on: bool) -> bool {
        if i >= self.session.circuit_inputs.len() {
            return false;
        }
        self.stop_demo();
        if self.defer(move |p| {
            p.hold_input(i, on);
        }) {
            return true;
        }
        self.log_op(InputOp::HoldInput(i, on));
        if on {
            if !self.session.set_input(i, true) {
                return false;
            }
            if let Some(p) = self.pressed.get_mut(i) {
                *p = Some(self.session.tick);
            }
        } else {
            let at = self.pressed.get_mut(i).and_then(|p| p.take()).unwrap_or(0);
            if self.pulse_inputs && self.session.tick < at + 2 {
                self.session.release_input_at(i, at + 2);
            } else {
                self.session.set_input(i, false);
            }
        }
        self.invalidate();
        true
    }

    pub fn click(&mut self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        if let Some(i) = self.input_at(atlas, x, y, aspect) {
            let p = self.session.circuit_inputs[i].pos;
            return self.tap_input(i).then_some(p);
        }
        let c = self.pick(atlas, x, y, aspect)?;
        // only a block a click uses wakes the view (a lever, a button, a door, a note block)
        if !matches!(self.session.world.get(crate::session::bp(c)), redsim::Block::Lever { .. } | redsim::Block::Button { .. } | redsim::Block::Note { .. } | redsim::Block::Trapdoor { iron: false, .. } | redsim::Block::Door { iron: false, .. }) {
            return None;
        }
        self.stop_demo();
        if self.defer(move |p| {
            p.log_op(InputOp::Toggle(c));
            if p.session.toggle(c) {
                p.invalidate();
            }
        }) {
            return Some(c);
        }
        self.log_op(InputOp::Toggle(c));
        if self.session.toggle(c) {
            self.invalidate();
            Some(c)
        } else {
            None
        }
    }

    /// Where on the view (x, y in -1..1, y up) a click picks `cell` (tests click there); a
    /// circuit input's cell (a click there drives it, even when empty) at its projected centre.
    pub fn screen_point(&self, atlas: &Atlas, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
        if self.session.circuit_inputs.iter().any(|c| c.pos == cell) {
            let (f, r, u) = self.camera.basis();
            let eye = self.camera.eye();
            let t = (self.camera.vfov.to_radians() / 2.0).tan();
            let v = [cell[0] as f32 + 0.5 - eye[0], cell[1] as f32 + 0.5 - eye[1], cell[2] as f32 + 0.5 - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f);
            if z > 0.05 {
                let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
                if x.abs() < 0.98 && y.abs() < 0.98 {
                    return Some((x, y));
                }
            }
            return None;
        }
        scene::click_point(atlas, &self.session, &self.camera, cell, aspect)
    }

    /// Turn the camera (and its home) to a side every lever and button can be seen and clicked
    /// from, if the pictures' side hides one.
    /// The camera from the side every input can be seen from, its build in `window` (a live
    /// viewer: [`scene::live_window`] of its height); the home view too.
    pub fn show_inputs(&mut self, atlas: &Atlas, aspect: f32, window: (f32, f32, f32)) {
        self.camera = scene::camera_showing_inputs(atlas, &self.session, &self.look, aspect, window);
        self.home = self.camera;
        self.dirty = true;
    }

    /// The live viewer's default camera at its CSS size (`scene::live_camera`: on a phone, clear
    /// of the Record pill when the build would come out under it); returns the window it used.
    pub fn frame_live(&mut self, atlas: &Atlas, w: f32, h: f32) -> (f32, f32, f32) {
        let (cam, window) = scene::live_camera(atlas, &self.session, &self.look, w, h);
        self.camera = cam;
        self.home = cam;
        self.dirty = true;
        window
    }

    /// Flip a door's lever (or the first input).
    pub fn flip(&mut self) -> bool {
        self.stop_demo();
        if self.defer(|p| {
            p.flip();
        }) {
            return true;
        }
        if !self.session.circuit_inputs.is_empty() {
            return self.tap_input(0);
        }
        let target = match self.session.mode {
            Mode::Door { lever, .. } => Some(lever),
            _ => self.session.inputs.first().copied(),
        };
        let ok = target.is_some_and(|p| {
            self.log_op(InputOp::Toggle(p));
            self.session.toggle(p)
        });
        if ok {
            self.invalidate();
        }
        ok
    }

    /// The frame's triangles (solid, translucent sorted for the camera).
    pub fn triangles(&mut self, atlas: &Atlas) -> (Vec<Vertex>, Vec<Vertex>) {
        let key = (self.session.tick, (self.partial * 64.0) as u32);
        if self.mesh.is_none() || key != self.mesh_key {
            self.mesh = Some(scene::build(atlas, &self.session, self.partial, &self.look));
            self.mesh_key = key;
        }
        scene::triangles(self.mesh.as_ref().unwrap(), &self.camera)
    }

    /// Forget the cached mesh (the look changed).
    pub fn invalidate(&mut self) {
        self.mesh = None;
        self.dirty = true;
    }

    /// After moving the camera: keep it above the ground (never under the floor): the pictures'
    /// grass, or a TNT duper's real ground.
    pub fn keep_camera_above_ground(&mut self) {
        if let Some(g) = self.look.ground {
            self.camera.keep_above(g);
        } else if let Some(f) = &self.session.floor {
            self.camera.keep_above(f.hi[1] as f32 + 1.0);
        }
    }

    /// The tick counter: `6.15 s · 123 ticks` (plain words: seconds first), plus the ticks since
    /// the last flip for a door.
    pub fn label(&self) -> String {
        let s = &self.session;
        let t = s.tick.saturating_sub(s.rest_tick);
        let mut out = format!("{} · {t} ticks", seconds(t));
        // the person's flip of a door: the ticks since (the door demo's badge says its stage)
        if s.last_input.is_some() && s.mode.is_door() && self.door_demo.is_none() {
            out.push_str(&format!(" · flip +{}", s.since_input()));
        }
        out
    }
}

/// Where a ray (origin, direction) enters a box, if it does (slab test).
fn ray_box(o: [f32; 3], d: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// `6.15 s`, `1 min 2.50 s` (always two decimals: the counter keeps its width).
pub fn seconds(gt: u32) -> String {
    let s = gt as f32 / 20.0;
    if s < 60.0 {
        format!("{s:.2} s")
    } else {
        let m = (s / 60.0).floor();
        format!("{m} min {:05.2} s", s - m * 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The door demo flips the lever like the record videos: the door opens (its opening
    /// empties), stays open, closes again (its 9 blocks back), and starts over.
    #[test]
    fn the_door_demo_opens_and_closes() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let mut p = Player::new(&text, 1.5).unwrap();
        p.start_door_demo();
        p.playing = true;
        let mut seen: Vec<usize> = Vec::new();
        for _ in 0..12_000 {
            p.update(0.05);
            let n = p.session.door_blocks().unwrap();
            if seen.last() != Some(&n) {
                seen.push(n);
            }
            if seen.len() > 1 && seen.first() == Some(&9) && seen.contains(&0) && seen.last() == Some(&9) {
                break;
            }
        }
        assert_eq!(seen.first(), Some(&9), "closed at first: {seen:?}");
        assert!(seen.contains(&0), "opens: {seen:?}");
        assert_eq!(seen.last(), Some(&9), "closes again: {seen:?}");
        assert!(p.door_demo.is_some(), "still running");
        p.flip();
        assert!(p.door_demo.is_none(), "the person's flip stops it");
    }

    /// A page's view plays only when played (the coordinator 2026-10-21: "a design page's view
    /// simulates on demand only (play/press), incrementally in real time ... keep the exact-twin
    /// seeding, just run it lazily"): it loads as a still, nothing runs while it is paused, and
    /// played it becomes the session the judge seeds (the same frames as the native viewer's);
    /// a flying machine's end is found as it flies, at the tick the look-ahead finds it.
    #[test]
    fn a_page_view_plays_only_when_played() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/fly/archive");
        let f = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("fly-down"))).unwrap();
        let text = std::fs::read_to_string(&f).unwrap();
        let mut p = Player::new_lazy(&text, 1.5, None).unwrap();
        assert!(p.is_still() && p.session.still && p.session.tick == 0);
        p.update(1.0);
        assert!(p.is_still(), "paused: still a picture");
        p.playing = true;
        p.tps = 1000.0;
        p.update(0.05);
        assert!(!p.is_still() && !p.session.still, "played: the session that plays");
        let native = Session::new(&text, &Options::default()).unwrap();
        let want = native.end_tick.expect("the look-ahead's end");
        for _ in 0..20_000 {
            if p.session.end_tick.is_some() {
                break;
            }
            p.update(0.05);
        }
        assert_eq!(p.session.end_tick, Some(want), "the end found as it flies, where the look-ahead finds it");
    }

    /// A 2x2 door (clk-item's `door-size: 2`, the coordinator 2026-10-21: "door viewer on a 2x2
    /// opening"): its opening is its 4 cells, the demo opens it (all 4 empty) and closes it again.
    #[test]
    fn a_2x2_door_demo_opens_and_closes_its_4_cells() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-2x2/archive/classic6-20b.txt")).unwrap();
        let mut p = Player::new(&text, 1.5).unwrap();
        assert!(matches!(p.session.mode, Mode::Door { size: 2, .. }), "{:?}", p.session.mode);
        p.start_door_demo();
        p.playing = true;
        let mut seen: Vec<usize> = Vec::new();
        for _ in 0..12_000 {
            p.update(0.05);
            let n = p.session.door_blocks().unwrap();
            if seen.last() != Some(&n) {
                seen.push(n);
            }
            if seen.len() > 1 && seen.first() == Some(&4) && seen.contains(&0) && seen.last() == Some(&4) {
                break;
            }
        }
        assert_eq!(seen.first(), Some(&4), "closed at first: {seen:?}");
        assert!(seen.contains(&0), "opens: {seen:?}");
        assert_eq!(seen.last(), Some(&4), "closes again: {seen:?}");
    }

    /// A flying machine started by its own lever (site 2026-10-21: the start-lever picks stood at
    /// "0.00 s · 0 ticks", the run's end found at placement undoing every flip): it has no end
    /// before its input; a press makes it play on.
    #[test]
    fn a_lever_started_flying_machine_plays_after_its_flip() {
        // an archived docking machine (the first by name: the docking records get renamed as they
        // improve; fly-docking-15b became -15b-narrow and -17b-61gt on 2026-10-21)
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/fly/archive");
        let mut docking: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("fly-docking-") && n.ends_with(".txt")))
            .collect();
        docking.sort();
        let path = docking.first().expect("an archived docking machine").clone();
        let text = std::fs::read_to_string(&path).unwrap();
        // the docking machine without its kick: only its lever left
        let text: String = text.lines().filter(|l| !l.starts_with("port start:")).map(|l| format!("{l}\n")).collect();
        let mut p = Player::new(&text, 1.5).unwrap();
        assert!(p.session.starts_on_input() && p.session.end_tick.is_none());
        assert!(p.tap_input(0), "the lever flips");
        p.playing = true;
        let t0 = p.session.tick;
        for _ in 0..40 {
            p.update(0.05);
        }
        assert!(p.session.tick > t0 + 20, "it plays on after the flip: tick {} from {t0}", p.session.tick);
        // the kick-started docking machine keeps its judge's end
        let kicked = Player::new(&std::fs::read_to_string(&path).unwrap(), 1.5).unwrap();
        assert!(!kicked.session.starts_on_input() && kicked.session.end_tick.is_some());
    }

    /// Critique: a BUD switch's demo said "waiting" for 12 s (its judge's first event at 240 gt):
    /// the demo is shown from 1 s before its first change, and a reset comes back there; a demo
    /// whose first change comes soon starts at placement.
    #[test]
    fn a_long_demo_wait_is_skipped() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bud/archive/bud-observer.txt")).unwrap();
        let schedule = |events: &str, ticks: u32, n: usize| crate::circuit::Schedule { edges: vec![Vec::new(); n], ticks, events: crate::circuit::parse_events(events).unwrap(), levels: Vec::new() };
        let mut p = Player::new(&text, 1.5).unwrap();
        let t0 = p.session.tick;
        assert!(t0 < 200, "{t0}");
        let n = p.session.circuit_inputs.len();
        p.session.attach_demo(schedule("240 place 0 1 0 minecraft:oak_log[axis=y]; 340 clear 0 1 0", 380, n));
        p.skip_demo_wait();
        assert_eq!((p.demo_from, p.session.tick), (220, 220));
        assert_eq!(p.recording_ticks(None, 1000), 160, "what is left of the demo");
        p.step();
        p.reset();
        assert_eq!(p.session.tick, 220, "a reset comes back where the demo is shown from");
        let mut q = Player::new(&text, 1.5).unwrap();
        q.session.attach_demo(schedule("50 place 0 1 0 minecraft:oak_log[axis=y]", 120, n));
        q.skip_demo_wait();
        assert_eq!((q.demo_from, q.session.tick), (0, t0));
    }

    /// With the browser's clock a page's view builds its session over frames (`Player::wake`; the
    /// coordinator 2026-10-22: pressing play ran one 300-450 ms task): the still stays while it is
    /// built (the badge says so), a press meanwhile waits for it, the demo's skipped wait is
    /// played through over frames too, a reset comes back there the same way, and what plays is
    /// what the at-once wake plays, tick for tick.
    #[test]
    fn a_page_view_wakes_over_frames() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bud/archive/bud-observer.txt")).unwrap();
        let schedule = || crate::circuit::Schedule { edges: vec![Vec::new(); 1], ticks: 380, events: crate::circuit::parse_events("240 place 0 1 0 minecraft:oak_log[axis=y]; 340 clear 0 1 0").unwrap(), levels: Vec::new() };
        // at once (no clock): the reference
        let mut a = Player::new_lazy(&text, 1.5, None).unwrap();
        a.attach_demo(schedule());
        a.skip_demo_wait();
        a.playing = true;
        a.tps = 20.0;
        a.update(0.0);
        assert!(!a.is_still() && a.session.tick == 220, "{}", a.session.tick);
        // over frames: a clock that runs out after a few calls
        let t = std::rc::Rc::new(std::cell::Cell::new(0.0));
        let clock = {
            let t = t.clone();
            std::rc::Rc::new(move || {
                t.set(t.get() + 1.0);
                t.get()
            })
        };
        let mut p = Player::new_lazy(&text, 1.5, None).unwrap();
        p.set_clock(clock);
        p.attach_demo(schedule());
        p.skip_demo_wait();
        p.playing = true;
        p.tps = 20.0;
        let mut frames = 0;
        while p.is_still() {
            p.update_with(0.0, &mut || true);
            frames += 1;
            assert!(frames < 10_000);
        }
        assert!(frames > 1, "built over {frames} frames");
        let mut ff = 0;
        while p.session.tick < 220 {
            p.update_with(0.0, &mut || true);
            ff += 1;
            assert!(ff < 10_000);
        }
        assert!(ff > 10, "the skipped wait played through over {ff} frames");
        assert_eq!((p.demo_from, p.session.tick), (220, 220));
        assert!(p.session.world.blocks() == a.session.world.blocks(), "the same session at the demo's start");
        for _ in 0..120 {
            a.session.step();
            p.session.step();
            assert!(p.session.world.blocks() == a.session.world.blocks(), "tick {}", a.session.tick);
        }
        // a reset comes back to the demo's start over frames
        p.reset();
        let mut n = 0;
        while p.session.tick < 220 {
            p.update_with(0.0, &mut || true);
            n += 1;
            assert!(n < 10_000);
        }
        assert_eq!(p.session.tick, 220);
        // a press while it is being built waits for it
        let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/memory/archive/dff-lock.txt")).unwrap();
        let mut q = Player::new_lazy(&lock, 1.5, None).unwrap();
        q.set_clock(std::rc::Rc::new(|| 1.0));
        q.pulse_inputs = false;
        assert!(q.flip(), "the press is taken");
        assert!(q.is_waking() && q.session.still, "still building");
        q.playing = true;
        let mut n = 0;
        while q.is_still() {
            q.update_with(0.0, &mut || true);
            n += 1;
            assert!(n < 10_000);
        }
        assert!(!q.session.still && q.session.input_on.first() == Some(&true), "the press done once it plays: {:?}", q.session.input_on);
    }

    /// QA at phone size (dff-lock's CLK, the dividers' IN): a press on a pulse input the demo
    /// left on did nothing at all (a pulse cannot start on an input already on) and stopped the
    /// demo. It lets the input go; the next press is a pulse again.
    #[test]
    fn a_press_on_an_input_the_demo_left_on_lets_it_go() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/memory/archive/dff-lock.txt")).unwrap();
        let mut p = Player::new(&text, 0.8).unwrap();
        p.pulse_inputs = true;
        let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
        let clk = names.iter().position(|n| n == "clk").unwrap();
        let demo = crate::circuit::parse_stimulus("clk 40+ 900-", &names, 40).unwrap();
        p.session.attach_demo(demo);
        while p.session.tick < 60 {
            p.step();
        }
        assert!(p.session.input_on[clk], "the demo holds CLK on");
        assert!(p.input_down(clk), "the press does something");
        assert!(!p.session.input_on[clk] && p.session.demo.is_none(), "CLK let go, the demo stopped");
        p.input_up(clk);
        p.step();
        assert!(!p.session.input_on[clk]);
        // the next press: on while held, off after its release
        assert!(p.input_down(clk));
        assert!(p.session.input_on[clk]);
        for _ in 0..3 {
            p.step();
        }
        p.input_up(clk);
        p.step();
        assert!(!p.session.input_on[clk]);
        // a tap the same way
        p.session.set_input(clk, true);
        assert!(p.tap_input(clk) && !p.session.input_on[clk]);
    }

    /// Recording QA at phone size: a lock's video was 10 s of a lock standing still (its page has
    /// no demo). Its recording turns each dial to its code (the lamp stays off until the last
    /// turn), holds it open, and shuts it with one more turn; every archived lock.
    #[test]
    fn a_locks_recording_opens_it() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/lock/archive");
        let mut locks: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        locks.sort();
        assert!(!locks.is_empty());
        for f in &locks {
            let text = std::fs::read_to_string(f).unwrap();
            if !crate::session::tests::is_frame_lock(&text) {
                continue;
            }
            let mut p = Player::new(&text, 0.8).unwrap();
            let (demo, phases) = lock_demo(&p.session).unwrap_or_else(|| panic!("{}: a lock demo", f.display()));
            assert_eq!(phases.last().map(|x| x.2.as_str()), Some("one more turn: shut"));
            let open_at = phases[phases.len() - 2].0;
            let shut_at = phases[phases.len() - 1].0;
            let out = p.session.outputs[0];
            p.session.attach_demo(demo.clone());
            p.phases = phases;
            let ticks = p.recording_ticks(None, 2000);
            assert_eq!(ticks, demo.ticks - p.session.tick, "{}", f.display());
            assert!(ticks <= 20 * 20, "{}: {ticks} ticks fit a 20 s recording", f.display());
            let mut lit = Vec::new();
            while p.session.tick < demo.ticks {
                p.session.step();
                if p.session.output_powered(out) {
                    lit.push(p.session.tick);
                }
            }
            assert!(!lit.is_empty(), "{}: it opens", f.display());
            assert!(lit[0] + 1 >= open_at && lit[0] < open_at + 20, "{}: lit at {} (the last turn at {})", f.display(), lit[0], open_at - 1);
            assert!(*lit.last().unwrap() < shut_at + 20, "{}: shut by {} (the extra turn at {shut_at})", f.display(), lit.last().unwrap());
            assert!(!p.session.output_powered(out), "{}: shut at the end", f.display());
        }
    }

    /// The Record button's length (from tick 0): a door's demo cycle (closed, opening, open,
    /// closing, closed again), a clock's warmup and whole periods (at least one), the cap.
    #[test]
    fn recording_lengths() {
        let door = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let cycle = door_demo_cycle(&door).expect("a door's demo cycle");
        assert!(cycle > DOOR_HOLD_CLOSED + DOOR_HOLD_OPEN && cycle < 1000, "{cycle}");
        let p = Player::new(&door, 1.5).unwrap();
        assert_eq!(p.recording_ticks(Some(cycle), 400), cycle.min(400));
        let clock = "name: c\nport start: 0 0 0 last\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";
        let p = Player::new(clock, 1.5).unwrap();
        let per = p.session.period(20_000).unwrap();
        let t = p.recording_ticks(None, 400);
        assert!(t >= per.warmup + per.period && (t - per.warmup) % per.period == 0, "{t} ticks for warmup {} period {}", per.warmup, per.period);
        assert_eq!(p.recording_ticks(None, 10), 10, "the cap");
        assert!(door_demo_cycle(clock).is_none(), "not a door");
    }

    #[test]
    fn plays_a_clock_and_flips_a_door() {
        let clock = "name: c\nport start: 0 0 0 last\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";
        let mut p = Player::new(clock, 1.5).unwrap();
        p.playing = true;
        for _ in 0..4 {
            assert!(p.update(0.25));
        }
        assert_eq!(p.session.tick, 20);
        assert_eq!(seconds(123), "6.15 s");
        assert_eq!(seconds(20), "1.00 s");
        assert_eq!(seconds(1250), "1 min 02.50 s");
    }

    #[test]
    fn a_paused_click_shows_at_once() {
        // the lever flips while paused: the next frame shows it (no stale mesh of the same tick)
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let atlas = Atlas::build();
        let mut p = Player::new(gadget, 1.5).unwrap();
        assert_eq!(p.session.inputs, vec![[0, 1, 0]]);
        let before = p.triangles(&atlas).0;
        let (x, y) = p.screen_point(&atlas, [0, 1, 0], 1.5).expect("the lever is visible");
        assert_eq!(p.click(&atlas, x, y, 1.5), Some([0, 1, 0]));
        assert!(p.update(0.0), "a paused click asks for a frame");
        let after = p.triangles(&atlas).0;
        assert!(before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| a.pos != b.pos), "the frame changed");
        assert!(p.session.is_on([0, 1, 0]));
    }

    /// The timelapse: going back in time tick by tick, then forward again, shows the same
    /// states as the run that played straight through; back before a flip, the flip is
    /// dropped (that future is gone) and forward again stays off.
    #[test]
    fn rewind_then_forward_replays_the_run() {
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let powered = |p: &Player| p.session.design.blocks.keys().filter(|c| !p.session.inputs.contains(c) && p.session.is_on(**c)).count();
        let mut a = Player::new(gadget, 1.5).unwrap();
        a.step();
        assert!(a.flip(), "the lever flips");
        let mut states = Vec::new();
        for _ in 0..10 {
            states.push((a.session.tick, a.session.is_on([0, 1, 0]), a.session.is_on([1, 0, 0]), powered(&a)));
            a.step();
        }
        let mut b = Player::new(gadget, 1.5).unwrap();
        b.step();
        assert!(b.flip());
        for _ in 0..10 {
            b.step();
        }
        assert_eq!(b.session.tick, 11);
        for _ in 0..6 {
            assert!(b.step_back(), "back to tick 5");
        }
        assert_eq!(b.session.tick, 5);
        for (tick, lever, bulb, n) in states.iter().skip(4) {
            assert_eq!((b.session.tick, b.session.is_on([0, 1, 0]), b.session.is_on([1, 0, 0]), powered(&b)), (*tick, *lever, *bulb, *n), "tick {tick}");
            b.step();
        }
        assert_eq!(b.session.tick, 11);
        while b.session.tick > 1 {
            assert!(b.step_back());
        }
        assert_eq!(b.session.tick, 1);
        assert!(!b.session.is_on([0, 1, 0]), "back before the flip at tick 1: the flip is dropped");
        for _ in 0..5 {
            b.step();
        }
        assert!(!b.session.is_on([0, 1, 0]), "forward again without it: still off");
    }

    /// The timelapse at the run's start: nothing to go back to.
    #[test]
    fn no_rewind_at_run_start() {
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let mut p = Player::new(gadget, 1.5).unwrap();
        assert!(!p.can_step_back());
        assert!(!p.step_back());
        assert_eq!(p.session.tick, 0);
    }

    /// The timelapse across a snapshot (every 512 ticks): back from 600, past 512, to the
    /// flip at tick 10 — the flip survives until the rewind passes it.
    #[test]
    fn rewind_across_a_snapshot() {
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let mut p = Player::new(gadget, 1.5).unwrap();
        for _ in 0..10 {
            p.step();
        }
        assert!(p.flip(), "the lever flips at tick 10");
        for _ in 0..590 {
            p.step();
        }
        assert_eq!(p.session.tick, 600);
        while p.session.tick > 11 {
            assert!(p.step_back());
        }
        assert_eq!(p.session.tick, 11);
        assert!(p.session.is_on([0, 1, 0]), "the flip at tick 10 is still in");
        assert!(p.step_back());
        assert_eq!(p.session.tick, 10);
        assert!(!p.session.is_on([0, 1, 0]), "back to tick 10: the flip at 10 is dropped");
    }

    /// The scrubber: jump between states both ways (back drops the future, forward
    /// replays without it), and the idle stretches are marked (the settled tail dim).
    #[test]
    fn scrubber_seeks_between_states_and_marks_idle() {
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let mut p = Player::new(gadget, 1.5).unwrap();
        assert_eq!(p.scrub_range(), (0, 0));
        p.step();
        assert!(p.flip(), "the lever flips at tick 1");
        for _ in 0..6 {
            p.step();
        }
        assert_eq!(p.session.tick, 7);
        assert_eq!(p.scrub_range(), (0, 7));
        let runs = p.activity_runs();
        assert_eq!(runs.first().map(|r| r.0), Some(0));
        assert_eq!(runs.last().map(|r| r.1), Some(8));
        for w in runs.windows(2) {
            assert_eq!(w[0].1, w[1].0, "contiguous: {runs:?}");
        }
        assert!(runs.iter().any(|r| r.2), "the flip and the settling are active: {runs:?}");
        assert!(!runs.last().unwrap().2, "settled: the last stretch is idle: {runs:?}");
        // back to tick 2: the flip at tick 1 is kept, the lever stays on
        assert!(p.seek_to(2));
        assert_eq!(p.session.tick, 2);
        assert!(p.session.is_on([0, 1, 0]));
        // forward again: still on, the bulb still lights
        assert!(p.seek_to(7));
        assert_eq!(p.session.tick, 7);
        assert!(p.session.is_on([0, 1, 0]));
        assert!(p.session.is_on([1, 0, 0]));
        // back before the flip: it is dropped, the lever stays off going forward
        assert!(p.seek_to(1));
        assert!(!p.session.is_on([0, 1, 0]), "the flip at tick 1 is dropped");
        assert!(p.seek_to(7));
        assert!(!p.session.is_on([0, 1, 0]));
        // back to the floor: the furthest tick stays (the scrubber's end never rewinds)
        assert!(p.seek_to(0));
        assert_eq!(p.session.tick, 0);
        assert_eq!(p.scrub_range(), (0, 7));
        assert!(!p.seek_to(0), "already there");
    }

    /// The scrubber with the door demo: the waits are long idle stretches, the
    /// opening/closing active; scrubbing back then forward again lands on the same
    /// states as the runs that played straight there.
    #[test]
    fn scrubber_door_demo_marks_waits() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let mut mk = |n: u32| {
            let mut p = Player::new(&text, 1.5).unwrap();
            p.start_door_demo();
            for _ in 0..n {
                p.step();
            }
            p
        };
        let mut a = mk(200);
        let start = a.session.tick - 200;
        assert_eq!(a.scrub_range(), (start, start + 200));
        let runs = a.activity_runs();
        assert!(runs.iter().any(|&(s, e, act)| !act && e - s >= 20), "a wait is a long idle stretch: {runs:?}");
        assert!(runs.iter().any(|r| r.2), "the opening/closing is active: {runs:?}");
        let b = mk(100);
        let c = mk(200);
        let Mode::Door { lever, .. } = a.session.mode else { panic!("a door") };
        assert!(a.seek_to(start + 100));
        assert_eq!((a.session.tick, a.session.door_blocks(), a.session.is_on(lever)), (b.session.tick, b.session.door_blocks(), b.session.is_on(lever)));
        assert!(a.seek_to(start + 200));
        assert_eq!((a.session.tick, a.session.door_blocks(), a.session.is_on(lever)), (c.session.tick, c.session.door_blocks(), c.session.is_on(lever)));
    }

    /// The timelapse with the door demo: rewound, then forward again, the door is where the
    /// run that played straight through has it.
    #[test]
    fn rewind_replays_the_door_demo() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/piston-door-3x3/archive/b38h-obs7-45b.txt")).unwrap();
        let mut a = Player::new(&text, 1.5).unwrap();
        let start = a.session.tick;
        a.start_door_demo();
        for _ in 0..60 {
            a.step();
        }
        let mut b = Player::new(&text, 1.5).unwrap();
        b.start_door_demo();
        for _ in 0..30 {
            b.step();
        }
        for _ in 0..5 {
            assert!(b.step_back());
        }
        assert_eq!(b.session.tick, start + 25);
        for _ in 0..35 {
            b.step();
        }
        assert_eq!(b.session.tick, start + 60);
        let Mode::Door { lever, .. } = a.session.mode else { panic!("a door") };
        assert_eq!(b.session.is_on(lever), a.session.is_on(lever), "the lever");
        assert_eq!(b.session.door_blocks(), a.session.door_blocks(), "the door");
    }

    /// The timelapse with the auto-demo: rewound, then forward again, the demo's held input
    /// is where the run that played straight through has it.
    #[test]
    fn rewind_replays_the_auto_demo() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/memory/archive/dff-lock.txt")).unwrap();
        let mut mk = |pulse: bool| {
            let mut p = Player::new(&text, 0.8).unwrap();
            p.pulse_inputs = pulse;
            let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
            let clk = names.iter().position(|n| n == "clk").unwrap();
            let demo = crate::circuit::parse_stimulus("clk 40+ 900-", &names, 40).unwrap();
            p.attach_demo(demo);
            (p, clk)
        };
        let (mut a, clk) = mk(true);
        let start = a.session.tick;
        for _ in 0..100 {
            a.step();
        }
        let (mut b, _) = mk(true);
        for _ in 0..60 {
            b.step();
        }
        for _ in 0..10 {
            assert!(b.step_back());
        }
        assert_eq!(b.session.tick, start + 50);
        assert!(b.session.demo.is_some(), "the demo survived the rewind");
        for _ in 0..50 {
            b.step();
        }
        assert_eq!(b.session.tick, start + 100);
        assert_eq!(b.session.input_on[clk], a.session.input_on[clk], "the demo holds CLK the same");
        assert_eq!(b.session.demo.as_ref().map(|d| d.ticks), a.session.demo.as_ref().map(|d| d.ticks));
    }
}
