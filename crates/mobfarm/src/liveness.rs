//! Day-1's gravity liveness gate, emulated (farmopt Nov 11 17:13, day-1's semantics): one sealed
//! empty-cart ring per column (7b90c3f1's 3 x 3 ring: period 30 gt, its detector high ~20 / low
//! ~10), a rising-edge detector on the ring's detector rail into a 32-gt stretcher = ALIVE. Every
//! cart-moving pulse is ANDed with ALIVE at its rise: a pulse that has risen completes on its own
//! timing (redstone runs at level 32), a timer sequence that hasn't fired its pulses when ALIVE
//! falls is dropped, not resumed, until its next start; RS latches stay live (frozen carts make no
//! edges); the cycle restarts at the next golem event (a level-32 freeze stops the villagers: no
//! summon, no golem, no release while frozen).
//!
//! One [`Liveness`] per run, shared by the bench and the timers it emulates (queue-loop-run's
//! `--alive-gate`, 7b90c3f1's stand timer fill, gq-bench's cycle clock, the descent bench's T+3
//! release):
//! - [`Liveness::alive`]: ALIVE at tick `t` (call it every tick: it advances the ring once per
//!   `t`, cached for further calls in the same tick);
//! - a timer sequence: [`Liveness::start`] at its start (the hold's release, the golem event, the
//!   fill), then [`Liveness::rise`] (name, offset, t), true on the tick the pulse at `offset` gt
//!   rises, ANDed with ALIVE; when ALIVE falls with the sequence started and not finished, the
//!   module drops its unfired remainder (logged) and `rise` returns false until the next `start`;
//! - a one-off pulse: [`Liveness::allow_rise`];
//! - [`Liveness::lines`]: ALIVE's falls and rises, each freeze's detection latency, the pulses
//!   fired, refused and dropped, and the pulses that rose with a frozen cart in the column (the
//!   latency window; must be 0).
//!
//! `t` is the caller's tick count (any clock advancing by one per game tick, the same for every
//! caller in a run). [`freeze32`] / [`thaw32`] freeze the whole part at level 32 (a forced ring of
//! chunks around it: redstone running, entities frozen).

use redsim::pos::BlockPos;
use redsim::world::World;

/// The ring: a real one (its detector rail's cell, read each tick) or a virtual one (a cart going
/// round in the chunk of a point, its lap advancing only while that chunk ticks entities: a frozen
/// ring stops mid-lap and goes on from there).
#[derive(Clone, Debug)]
pub enum Ring {
    Probe(BlockPos),
    Virtual { x: f64, z: f64, period: i64, high: i64, phase: i64 },
    /// A virtual ring read as its detectors' edge train: a lap of `lap` gt (advancing only while
    /// the chunk of x,z ticks entities) with a one-tick pulse at each offset of `edges` (day-1's
    /// fast ring: 8 detectors, 2 ring carts, an edge every ~3.75 gt, with a ~5-gt stretcher).
    Train { x: f64, z: f64, lap: i64, edges: Vec<i64> },
}

impl Ring {
    /// Day-1's fast ring (8 detectors, 2 ring carts): an edge every 3.75 gt on average (offsets 0,
    /// 4, 8, 11 of a 15-gt lap), to go with a 5-gt stretcher ([`FAST_STRETCH`]).
    pub fn fast(x: f64, z: f64) -> Ring {
        Ring::Train { x, z, lap: 15, edges: vec![0, 4, 8, 11] }
    }
}

/// The stretcher that goes with [`Ring::fast`] (a detection latency of at most ~6 gt).
pub const FAST_STRETCH: i64 = 5;

#[derive(Clone, Debug, Default)]
struct Seq {
    t0: i64,
    risen: Vec<i64>,
    offsets: Option<Vec<i64>>,
    dropped: bool,
}

/// Day-1's ALIVE and the pulses it gates (one per run).
#[derive(Clone, Debug)]
pub struct Liveness {
    pub ring: Ring,
    pub stretch: i64,
    ring_t: i64,
    prev_on: bool,
    last_t: Option<i64>,
    /// the tick of the ring's last rising edge
    pub last_edge: Option<i64>,
    alive_now: bool,
    /// ALIVE's falls and rises (ticks)
    pub falls: Vec<i64>,
    pub rises: Vec<i64>,
    seqs: std::collections::BTreeMap<String, Seq>,
    /// the sequences dropped when ALIVE fell: (tick, name and what had risen)
    pub drops: Vec<(i64, String)>,
    /// the pulses that rose, those refused at their rise (ALIVE low), and those that rose with a
    /// frozen cart in the column (ALIVE still high in the latency window): (tick, what)
    pub fired: Vec<(i64, String)>,
    pub refused: Vec<(i64, String)>,
    pub fired_on_frozen: Vec<(i64, String)>,
    /// the freezes' starts (ticks), for the detection latency
    pub freezes: Vec<i64>,
    /// a bench that emulates its freeze itself (gq-bench's `--freeze`: every cart's position and
    /// motion restored each tick, no chunk unloaded) says each tick whether the column is frozen:
    /// the virtual ring then stops on it, and the frozen-cart check reads it
    pub column_frozen: Option<bool>,
}

impl Liveness {
    pub fn new(ring: Ring) -> Self {
        Liveness { ring, stretch: 32, ring_t: 0, prev_on: false, last_t: None, last_edge: None, alive_now: true, falls: Vec::new(), rises: Vec::new(), seqs: Default::default(), drops: Vec::new(), fired: Vec::new(), refused: Vec::new(), fired_on_frozen: Vec::new(), freezes: Vec::new(), column_frozen: None }
    }

    /// The module's own choice of ring: `MCTAS_ALIVE_RING` = `probe:X,Y,Z` (a real ring's
    /// detector rail, world coordinates), `virtual:X,Z[,PHASE]` (world coordinates), or `fast`
    /// / `fast:X,Z` (day-1's fast ring, [`Ring::fast`], with its 5-gt stretcher; bare `fast` in
    /// the fallback's chunk); else the `fallback` the bench gives (a virtual ring in its column's
    /// chunk).
    pub fn from_config(fallback: Ring) -> Self {
        let fb_xz = match &fallback {
            Ring::Virtual { x, z, .. } | Ring::Train { x, z, .. } => (*x, *z),
            Ring::Probe(p) => (p.x as f64 + 0.5, p.z as f64 + 0.5),
        };
        let ring = std::env::var("MCTAS_ALIVE_RING").ok().and_then(|v| {
            if v.trim() == "fast" {
                return Some(Ring::fast(fb_xz.0, fb_xz.1));
            }
            let (kind, rest) = v.split_once(':')?;
            let n: Vec<f64> = rest.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            match (kind, n.as_slice()) {
                ("probe", [x, y, z]) => Some(Ring::Probe(BlockPos::new(*x as i32, *y as i32, *z as i32))),
                ("virtual", [x, z]) => Some(Ring::Virtual { x: *x, z: *z, period: 30, high: 20, phase: 0 }),
                ("virtual", [x, z, p]) => Some(Ring::Virtual { x: *x, z: *z, period: 30, high: 20, phase: *p as i64 }),
                ("fast", [x, z]) => Some(Ring::fast(*x, *z)),
                _ => None,
            }
        });
        let ring = ring.unwrap_or(fallback);
        let fast = matches!(ring, Ring::Train { .. });
        let mut l = Liveness::new(ring);
        if fast {
            l.stretch = FAST_STRETCH;
        }
        l
    }

    /// The ring's label for the report.
    pub fn ring_label(&self) -> String {
        match &self.ring {
            Ring::Probe(p) => format!("the ring's detector rail at {},{},{} (probed)", p.x, p.y, p.z),
            Ring::Virtual { x, z, period, high, phase } => format!("a virtual sealed ring in the chunk of {x:.1},{z:.1} (period {period}, high {high}, phase {phase}; its lap advances only while that chunk ticks entities)"),
            Ring::Train { x, z, lap, edges } => format!("day-1's fast ring, virtual, in the chunk of {x:.1},{z:.1} (a {lap}-gt lap with edges at {edges:?}: an edge every {:.2} gt; its lap advances only while that chunk ticks entities)", *lap as f64 / edges.len().max(1) as f64),
        }
    }

    fn ring_on(&mut self, w: &World) -> bool {
        match self.ring.clone() {
            Ring::Probe(p) => w.get(p).to_string().contains("powered=true"),
            Ring::Train { x, z, lap, edges } => {
                let ticks = match self.column_frozen {
                    Some(f) => !f,
                    None => w.entity_ticks_at(x, z),
                };
                if ticks {
                    self.ring_t += 1;
                }
                // (a one-tick pulse at each edge offset, only while the ring moves: a frozen ring
                // holds its detector state, and a held pulse makes no new edge)
                ticks && edges.contains(&self.ring_t.rem_euclid(lap.max(1)))
            }
            Ring::Virtual { x, z, period, high, phase } => {
                let ticks = match self.column_frozen {
                    Some(f) => !f,
                    None => w.entity_ticks_at(x, z),
                };
                if ticks {
                    self.ring_t += 1;
                }
                (self.ring_t + phase).rem_euclid(period.max(1)) < high
            }
        }
    }

    /// ALIVE at tick `t` (the first call for a `t` reads the ring; a run starts ALIVE, the
    /// stretcher taken as charged at the first tick). When ALIVE falls, every started sequence
    /// with pulses still to fire is dropped.
    pub fn alive(&mut self, w: &World, t: i64) -> bool {
        if self.last_t == Some(t) {
            return self.alive_now;
        }
        self.last_t = Some(t);
        if self.last_edge.is_none() {
            self.last_edge = Some(t);
        }
        let on = self.ring_on(w);
        if on && !self.prev_on {
            self.last_edge = Some(t);
        }
        self.prev_on = on;
        let now = self.last_edge.is_some_and(|e| t - e <= self.stretch);
        if now != self.alive_now {
            if now {
                self.rises.push(t);
            } else {
                self.falls.push(t);
                for (name, s) in self.seqs.iter_mut() {
                    let pending = match &s.offsets {
                        Some(o) => o.iter().any(|x| !s.risen.contains(x)),
                        None => true,
                    };
                    if !s.dropped && pending {
                        s.dropped = true;
                        if self.drops.len() < 200 {
                            self.drops.push((t, format!("{name} started at {} (risen: {})", s.t0, if s.risen.is_empty() { "none".to_string() } else { s.risen.iter().map(|x| format!("+{x}")).collect::<Vec<_>>().join(",") })));
                        }
                    }
                }
            }
        }
        self.alive_now = now;
        now
    }

    /// A timer sequence starts at `t` (the hold's release, the golem event, the fill's start):
    /// any earlier run of it is forgotten.
    pub fn start(&mut self, name: &str, t: i64) {
        self.seqs.insert(name.to_string(), Seq { t0: t, ..Default::default() });
    }

    /// [`Self::start`] with the sequence's offsets declared: a fall after all of them rose drops
    /// nothing.
    pub fn start_with(&mut self, name: &str, t: i64, offsets: &[i64]) {
        self.seqs.insert(name.to_string(), Seq { t0: t, offsets: Some(offsets.to_vec()), ..Default::default() });
    }

    /// Whether the named sequence was dropped (ALIVE fell, or a pulse was refused, before its
    /// last pulse rose): its owner restarts it at the next golem event.
    pub fn dropped(&self, name: &str) -> bool {
        self.seqs.get(name).is_some_and(|s| s.dropped)
    }

    /// Whether the named sequence is running: started, not dropped, a declared pulse still to rise.
    pub fn pending(&self, name: &str) -> bool {
        self.seqs.get(name).is_some_and(|s| !s.dropped && s.offsets.as_ref().is_none_or(|o| o.iter().any(|x| !s.risen.contains(x))))
    }

    /// True on the tick the sequence's pulse at `offset` gt rises: the sequence started and not
    /// dropped, `t` its start + `offset`, ALIVE (a refusal drops the sequence's remainder too).
    pub fn rise(&mut self, w: &World, name: &str, offset: i64, t: i64) -> bool {
        let alive = self.alive(w, t);
        let Some(s) = self.seqs.get(name).cloned() else { return false };
        if s.dropped || t - s.t0 != offset {
            return false;
        }
        let what = format!("{name} +{offset}");
        if !alive {
            if let Some(s) = self.seqs.get_mut(name) {
                s.dropped = true;
            }
            if self.refused.len() < 200 {
                self.refused.push((t, what));
            }
            return false;
        }
        if let Some(s) = self.seqs.get_mut(name) {
            s.risen.push(offset);
        }
        self.note_fired(w, t, what);
        true
    }

    /// A one-off cart-moving pulse wants to rise at `t`: allowed only while ALIVE.
    pub fn allow_rise(&mut self, w: &World, what: &str, t: i64) -> bool {
        if !self.alive(w, t) {
            if self.refused.len() < 200 {
                self.refused.push((t, what.to_string()));
            }
            return false;
        }
        self.note_fired(w, t, what.to_string());
        true
    }

    fn note_fired(&mut self, w: &World, t: i64, what: String) {
        // (a cart frozen in the column at the rise: the pulse fires into it, ALIVE still high in
        // the latency window)
        let column = match &self.ring {
            Ring::Probe(p) => (p.x as f64 + 0.5, p.z as f64 + 0.5),
            Ring::Virtual { x, z, .. } | Ring::Train { x, z, .. } => (*x, *z),
        };
        let (ccx, ccz) = ((column.0.floor() as i64).div_euclid(16), (column.1.floor() as i64).div_euclid(16));
        let frozen = match self.column_frozen {
            Some(f) => f && w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))),
            None => w.entities.list.iter().any(|e| {
                !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_)) && (e.pos.x.floor() as i64).div_euclid(16) == ccx && (e.pos.z.floor() as i64).div_euclid(16) == ccz && !w.entity_ticks_at(e.pos.x, e.pos.z)
            }),
        };
        if frozen && self.fired_on_frozen.len() < 200 {
            self.fired_on_frozen.push((t, what.clone()));
        }
        if self.fired.len() < 400 {
            self.fired.push((t, what));
        }
    }

    /// A freeze starts at `t` (for the detection latency line).
    pub fn freeze_started(&mut self, t: i64) {
        self.freezes.push(t);
    }

    /// The detection latency of a freeze starting at `t0`: ALIVE's first fall at or after it.
    pub fn latency(&self, t0: i64) -> Option<i64> {
        self.falls.iter().find(|f| **f >= t0).map(|f| f - t0)
    }

    /// The report lines and problems.
    pub fn lines(&self) -> (Vec<String>, Vec<String>) {
        let mut out = Vec::new();
        let mut problems = Vec::new();
        let mut ev: Vec<(i64, String)> = self.falls.iter().map(|f| (*f, format!("fell at {f}"))).chain(self.rises.iter().map(|r| (*r, format!("rose at {r}")))).collect();
        ev.sort();
        let lat: Vec<String> = self.freezes.iter().map(|f| format!("freeze at {f}: {}", self.latency(*f).map_or("ALIVE never fell".to_string(), |l| format!("{l} gt"))) ).collect();
        out.push(format!(
            "liveness gate (day-1's: {}, a rising-edge detector into a {} gt stretcher = ALIVE; every cart-moving pulse ANDed with ALIVE at its rise, a risen pulse completes, an unfired timer sequence dropped, latches live): ALIVE {}; detection latency {}",
            self.ring_label(),
            self.stretch,
            if ev.is_empty() { "never fell".to_string() } else { ev.into_iter().map(|e| e.1).collect::<Vec<_>>().join(", ") },
            if lat.is_empty() { "- (no freeze)".to_string() } else { lat.join("; ") }
        ));
        out.push(format!(
            "liveness gate pulses: fired {}, refused at their rise {}{}, sequences dropped {}{}",
            self.fired.len(),
            self.refused.len(),
            if self.refused.is_empty() { String::new() } else { format!(" ({})", self.refused.iter().take(10).map(|(t, w)| format!("{w} at {t}")).collect::<Vec<_>>().join("; ")) },
            self.drops.len(),
            if self.drops.is_empty() { String::new() } else { format!(" ({})", self.drops.iter().take(10).map(|(t, w)| format!("{w}, at {t}")).collect::<Vec<_>>().join("; ")) }
        ));
        out.push(format!(
            "liveness gate: pulses fired into a frozen cart (ALIVE still high in the latency window; must be 0): {}{}",
            self.fired_on_frozen.len(),
            if self.fired_on_frozen.is_empty() { String::new() } else { format!(" ({})", self.fired_on_frozen.iter().take(8).map(|(t, w)| format!("{w} at {t}")).collect::<Vec<_>>().join("; ")) }
        ));
        if !self.fired_on_frozen.is_empty() {
            problems.push(format!("liveness gate: {} pulses fired into a frozen cart", self.fired_on_frozen.len()));
        }
        (out, problems)
    }
}

/// The whole part frozen at level 32 (the player just outside its chunks): a forced ring of chunks
/// kept around the part's chunk box, the part's own chunks unforced (level 32 next to the ring,
/// redstone running, entities frozen; 33 further in, nothing). Returns the part's chunk box, to
/// pass to [`thaw32`]. (cc767b78, G1 2026-11-12) Asserts that every chunk of the part reads level 32
/// ([`levels`]): a part three or more chunks across in both axes has an interior past the ring at 33,
/// which is no freeze a judge may count.
pub fn freeze32(w: &mut World) -> [i32; 4] {
    if w.entity_chunks.is_none() {
        let (mn, sz) = (w.min, w.size);
        w.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
    }
    let pb = w.entity_chunks.unwrap_or([0; 4]);
    w.entity_chunks = Some([pb[0] - 1, pb[1] - 1, pb[2] + 1, pb[3] + 1]);
    redsim::reload::unload_some(w, pb);
    let (lo, hi) = levels(w, pb);
    assert!(lo == 32 && hi == 32, "freeze32: the part's chunks {pb:?} read levels {lo}..{hi}, not all 32");
    pb
}

/// [`freeze32`]'s end: the part's chunks forced again, the ring dropped.
pub fn thaw32(w: &mut World, pb: [i32; 4]) {
    redsim::reload::load_some(w, pb);
    if w.unforced.is_empty() {
        w.entity_chunks = Some(pb);
    }
}

/// The levels a box of chunks is at now (min, max).
pub fn levels(w: &World, b: [i32; 4]) -> (i32, i32) {
    let lv: Vec<i32> = (b[0]..=b[2]).flat_map(|x| (b[1]..=b[3]).map(move |z| (x, z))).map(|(x, z)| w.chunk_level(x, z)).collect();
    (lv.iter().copied().min().unwrap_or(0), lv.iter().copied().max().unwrap_or(0))
}

/// (cc767b78; ironspec's L1‴ rows, farmopt 2026-11-12 00:39: the farm never freezes in normal
/// operation and a restart loads its chunks before its first tick) How a restart brings a bench's
/// world back. Every bench wires it as `--restart T,ORDER` (between ticks T - 1 and T) and calls
/// [`restart`]; the judges drive it at every phase in each of [`Restart::rows`]. The cold start is not
/// a restart (G1 2026-11-12 01:05): it is its own row set, the design loaded fresh (blocks as written,
/// entities as the build guide places them, the controller initial) and started at each phase of the
/// cycle, which only the bench can build (e.g. dfb89499's `--dep-riders`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    /// The official load at a world open (ironspec 23:12 / 23:23, 16 of 16 official loads; tick-exact's
    /// control run): every chunk of the part saved and unloaded (`redsim::reload::unload_all`) and loaded
    /// again (`load_all_ordered`) in this entity `order` at the save's game time (26.3: the forced chunks
    /// unpack during server startup, `prepareLevels` -> `startTickingChunk` -> `unpackTicks(gameTime)`, while
    /// game time is still the save's): the scheduled block and fluid ticks back at their save-time due times,
    /// the block entities registered again, every entity back as a new object under a new id with its
    /// motion, passengers and codec'd memories, its uncodec'd memories lost and its sensor timers redrawn, as
    /// redsim's bridged load models them; then `idle` whole-chunk idle ticks (game time and day time
    /// advance, the level's weather with them; nothing of the world ticks); then normal ticks: at the first
    /// ticking tick every tick due inside the window fires (by due time, then priority, then sub-tick order,
    /// the saved ticks' negative sub-ticks first), then the entities tick; later ticks keep their game-time
    /// due times, so pending redstone runs ahead of the entities by `idle` (1 or 2 at a world open). A world
    /// open is the lead's only load case; the pearl alternative's unload case is not modelled here.
    /// A minecart's first move after the load is dry (ironspec 03:34, tick-exact's official lift-top row):
    /// wasTouchingWater and the fluid heights aren't saved, and `AbstractMinecart.tick` updates the fluid
    /// state after its move (redsim's `entity::minecart::tick` likewise), so a cart loaded in water moves one
    /// tick as on land (no water speed cap or drag), and that tick's difference carries on.
    /// (Block events never cross a tick boundary: none is pending between two ticks.)
    Load { order: redsim::reload::Order, idle: i64 },
    /// STRESS, not observed in 16 official loads (ironspec 23:12 (3): tolerance prints only, never bars):
    /// loaded at once, then 1 + `extra` block-only ticks (blocks and redstone, no entity, no block entity),
    /// which puts all redstone ahead of the entities by 1 + `extra`.
    Stress { order: redsim::reload::Order, extra: i64 },
}

/// The whole-chunk idle ticks of a world open (ironspec 23:23 (1): 1 or 2): the default, the larger.
pub const WORLD_OPEN_IDLE: i64 = 2;

/// The idle counts a world open's restart rows run (ironspec 23:23 (2): N = 1 and N = 2).
pub const WORLD_OPEN_IDLES: [i64; 2] = [1, 2];

impl Restart {
    /// The official load in `order` at a world open ([`WORLD_OPEN_IDLE`] idle ticks).
    pub fn load(order: redsim::reload::Order) -> Restart {
        Restart::Load { order, idle: WORLD_OPEN_IDLE }
    }

    /// The STRESS variant in `order`: one block-only tick (`with_extra` for more).
    pub fn stress(order: redsim::reload::Order) -> Restart {
        Restart::Stress { order, extra: 0 }
    }

    /// `save`, `reverse` or `seed:K` (K < 1000): the official load at a world open with 2 idle ticks;
    /// `/idle:N` with N; `/stress:F` the STRESS variant's 1 + F block-only ticks.
    pub fn parse(s: &str) -> Option<Restart> {
        let s = s.trim();
        let (o, tail) = s.split_once('/').map_or((s, None), |(o, t)| (o, Some(t.trim())));
        let order = redsim::reload::Order::parse(o.trim())?;
        let num = |t: &str| t.trim().parse::<i64>().ok().filter(|n| *n >= 0);
        match tail {
            None => Some(Restart::load(order)),
            Some(t) => match t.split_once(':') {
                Some(("idle", n)) => Some(Restart::Load { order, idle: num(n).filter(|n| *n >= 1)? }),
                Some(("stress", f)) => Some(Restart::Stress { order, extra: num(f)? }),
                _ => None,
            },
        }
    }

    /// The row's entity load order.
    pub fn order(&self) -> redsim::reload::Order {
        match *self {
            Restart::Load { order, .. } | Restart::Stress { order, .. } => order,
        }
    }

    /// The ticks the restart itself runs: the official load's idle ticks, or the stress variant's 1 + F.
    pub fn load_ticks(&self) -> i64 {
        match *self {
            Restart::Load { idle, .. } => idle.max(1),
            Restart::Stress { extra, .. } => 1 + extra.max(0),
        }
    }

    /// The same official load with `n` idle ticks (a world open's 1 or 2).
    pub fn with_idle(self, n: i64) -> Restart {
        Restart::Load { order: self.order(), idle: n.max(1) }
    }

    /// The STRESS variant with `f` extra block-only ticks.
    pub fn with_extra(self, f: i64) -> Restart {
        Restart::Stress { order: self.order(), extra: f.max(0) }
    }

    /// The STRESS variant (not observed in 16 official loads): its rows print, never bar.
    pub fn is_stress(&self) -> bool {
        matches!(self, Restart::Stress { .. })
    }

    pub fn label(&self) -> String {
        let o = match self.order() {
            redsim::reload::Order::Save => "save".to_string(),
            redsim::reload::Order::Reverse => "reverse".to_string(),
            redsim::reload::Order::Seed(k) => format!("seed:{k}"),
        };
        match *self {
            Restart::Load { idle, .. } if idle == WORLD_OPEN_IDLE => o,
            Restart::Load { idle, .. } => format!("{o}/idle:{idle}"),
            Restart::Stress { extra, .. } => format!("{o}/stress:{extra}"),
        }
    }

    /// The rows a judge runs at every phase: the save order, its reverse and 8 seeded orders
    /// (ironspec's reload-order rule), each the official load at a world open with N = 1 and N = 2 idle
    /// ticks (one physical branch, ironspec 22:20 (2); the world-open rule, 23:23 (2)).
    pub fn rows() -> Vec<Restart> {
        let mut orders = vec![redsim::reload::Order::Save, redsim::reload::Order::Reverse];
        orders.extend((1..=8).map(redsim::reload::Order::Seed));
        WORLD_OPEN_IDLES.iter().flat_map(|n| orders.iter().map(move |o| Restart::Load { order: *o, idle: *n })).collect()
    }
}

/// `--restart T,ORDER` (repeatable in a bench's CLI): the tick and the [`Restart`].
pub fn parse_restart(s: &str) -> Result<(i64, Restart), String> {
    let (t, how) = s.split_once(',').ok_or(format!("--restart {s}: T,ORDER (ORDER save, reverse or seed:K, /idle:N for N idle ticks, /stress:F for the stress variant)"))?;
    let t = t.trim().parse::<i64>().map_err(|e| format!("--restart {s}: {e}"))?;
    let how = Restart::parse(how).ok_or(format!("--restart {s}: ORDER is save, reverse or seed:K (/idle:N, N >= 1; /stress:F)"))?;
    Ok((t, how))
}

/// What a [`restart`] did (ironspec 21:13: nothing silently dropped): the entities' new ids (old -> new;
/// it reads as that map), the restart's game times (`first..=at`), the entity-free ticks the restart
/// itself ran (`entity_free`: a world open's N idle ticks, or the stress variant's 1 + F block-only ticks)
/// and the entities they held (`held`: for the official load the entities it brought back, present and
/// unticked through its idle ticks; for the STRESS variant each noted frozen at its first load tick in
/// `World::frozen`, kept there, so a freeze that goes on from the load reads as starting on it).
#[derive(Clone, Debug, Default)]
pub struct Restarted {
    pub ids: std::collections::HashMap<u32, u32>,
    pub first: i64,
    pub at: i64,
    pub entity_free: i64,
    pub held: usize,
    /// Entities created in the STRESS variant's load ticks (a dispenser's or dropper's output, a block's
    /// drop: G1's note (a), ironspec 22:16 (2)): not there to hold, listed here and in the note, exempt from
    /// the held check (none in the official load's idle ticks: nothing ticks).
    pub created: Vec<u32>,
    /// The STRESS variant (not observed in 16 official loads): the row prints, never bars.
    pub stress: bool,
    /// The world has a Nether part (`World::nether`): the reload model doesn't save it, so it keeps its
    /// in-memory state through the restart, its clock advanced with the overworld's (game time is global,
    /// ironspec 23:58 (2)); the row counts as unmodelled until the reload model saves and loads both
    /// dimensions (ironspec 21:23/21:24: said in the note, not silent).
    pub nether: bool,
}

impl std::ops::Deref for Restarted {
    type Target = std::collections::HashMap<u32, u32>;
    fn deref(&self) -> &Self::Target {
        &self.ids
    }
}

impl Restarted {
    /// The row's entity-free ticks with an `f`-gt level-32 freeze going on from the load's end (a genuine
    /// freeze, block entities ticking): the load's ticks plus `f`.
    pub fn with_freeze(&self, f: i64) -> i64 {
        self.entity_free + f.max(0)
    }

    /// The text every restart row prints (ironspec 21:13): the official load's `entity-free ticks N (a world
    /// open: N whole-chunk idle ticks at gt A..=B, nothing ticked, game and day time advanced, the ticks due in
    /// them firing at the first ticking tick; H entities back; the lead's only load case; the pearl alternative's unload case not modelled here)`; the
    /// STRESS variant's `STRESS (not observed in 16 official loads; prints only): entity-free ticks 1 + F = N
    /// (block-only ticks at gt A..=B holding H entities[, C created in them])`; `[ + a F-gt level-32 freeze
    /// from it]`; and with a Nether part `; Nether restart not modelled in redsim (…): this row UNMODELLED`
    /// (ironspec 21:24).
    pub fn note(&self, f: i64) -> String {
        let span = if self.first == self.at { format!("gt {}", self.at) } else { format!("gt {}..={}", self.first, self.at) };
        let body = if self.stress {
            let n = if self.entity_free > 1 { format!("1 + {} = {}", self.entity_free - 1, self.with_freeze(f)) } else { self.with_freeze(f).to_string() };
            format!(
                "STRESS (not observed in 16 official loads; prints only): entity-free ticks {n} (block-only ticks at {span} holding {} entities{}",
                self.held,
                if self.created.is_empty() { String::new() } else { format!(", {} created in them {:?}", self.created.len(), self.created) }
            )
        } else {
            format!(
                "entity-free ticks {} (a world open: {} whole-chunk idle ticks at {span}, nothing ticked, game and day time advanced, the ticks due in them firing at the first ticking tick; {} entities back; the lead's only load case; the pearl alternative's unload case not modelled here",
                self.with_freeze(f),
                self.entity_free,
                self.held
            )
        };
        format!(
            "{body}{}){}",
            if f > 0 { format!(" + a {f}-gt level-32 freeze from it") } else { String::new() },
            match (self.nether, self.stress) {
                (false, _) => "",
                (true, false) => "; Nether restart not modelled in redsim (the Nether part kept its in-memory state, its clock advanced with the overworld's): this row UNMODELLED",
                (true, true) => "; Nether restart not modelled in redsim (the Nether part ticked normally through the block-only ticks and kept its in-memory state): this row UNMODELLED",
            }
        )
    }
}

/// The held check of the STRESS variant's load ticks (measured, not assumed): every entity alive after them
/// that was there before them (`present`) must have been held (noted frozen in them: `noted`; a block may
/// still remove one, a hopper taking an item); one created in them (a dispenser's output, a drop) is
/// listed, exempt. Returns (held, created) or the entities that ticked.
pub fn load_held(present: &std::collections::HashSet<u32>, live: &[u32], noted: &std::collections::HashSet<u32>) -> Result<(usize, Vec<u32>), Vec<u32>> {
    let ticked: Vec<u32> = live.iter().copied().filter(|id| present.contains(id) && !noted.contains(id)).collect();
    if !ticked.is_empty() {
        return Err(ticked);
    }
    Ok((live.iter().filter(|id| present.contains(id)).count(), live.iter().copied().filter(|id| !present.contains(id)).collect()))
}

/// Restart world `w` between two ticks ([`Restart`]).
///
/// The official load at a world open (ironspec 23:23, tick-exact's official control run: 16 loads, the
/// positive control passing): the world saved and unloaded, then loaded in `how`'s entity order at the
/// save's game time (26.3: the forced chunks unpack during server startup, `prepareLevels` ->
/// `startTickingChunk` -> `unpackTicks(gameTime)`, while game time is still the save's), so the saved
/// scheduled and fluid ticks keep their save-time due times (redsim's `ticks.load` with no time away), the
/// block entities and entities back; then N whole-chunk idle ticks (game time + 1 each and day time with
/// it (`getDayTime` = the day offset + game time), the level's weather; nothing of the world ticks: no block
/// tick, scheduled or fluid tick, block event, random tick, block entity or entity; a daylight detector's
/// 20-gt check falling in them is lost); then the bench's normal ticks: at the first ticking tick every
/// tick due inside the window fires (by due time, then priority, then sub-tick order, as vanilla's
/// `LevelTicks` sorts them: redsim's `collect` takes every due trigger in that order, a chunk's saved ticks
/// carrying negative sub-ticks in saved order, ahead of new ones), then the entities tick; later ticks keep their
/// game-time due times, so pending redstone runs ahead of the entities by N (1 or 2). A world open is the
/// lead's only load case; the pearl alternative's unload case is not modelled here.
///
/// The STRESS variant (not observed in 16 official loads; tolerance prints only, never bars): loaded at
/// once, then 1 + F block-only ticks: game time + 1 each, the scheduled ticks, block events and redstone
/// run, no entity (an empty entity-ticking range, so the load order stays) and no block entity
/// (`World::skip_block_entities` gates the tick's three block-entity calls, all of redsim's block-entity
/// tickers: `redstone::container::tick_hoppers`' list (hoppers, crafters, daylight detectors via
/// `daylight::tick`, jukeboxes, furnaces, brewing stands), `sculk::tick_all` and
/// `piston::tick_block_entities`); the freezes they note stay in `World::frozen`; an entity there before
/// them must have been held ([`load_held`]), one created in them is listed.
///
/// Returns [`Restarted`]: the entities' new ids (old -> new) for the bench to follow its carts and riders
/// by (any id since also follows through [`follow`]) and the entity-free ticks every restart row prints
/// ([`Restarted::note`]); an error when the reload model refuses the world (a player, falling blocks:
/// `w.errors`), a partial unload is active, or an entity ticked in a stress tick.
pub fn restart(w: &mut World, how: Restart) -> Result<Restarted, String> {
    let errors = w.errors.len();
    let before: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
    if !w.unforced.is_empty() {
        return Err(format!("restart {}: a partial unload is active; a restart needs every chunk loaded", how.label()));
    }
    let first = w.time + 1;
    redsim::reload::unload_all(w);
    // (loaded at the save's game time: the saved ticks keep their save-time due times)
    redsim::reload::load_all_ordered(w, how.order());
    if w.errors.len() > errors {
        return Err(format!("restart {}: {}", how.label(), w.errors[errors..].join("; ")));
    }
    let ids = |w: &World| before.iter().map(|id| (*id, follow(w, *id))).collect();
    if let Restart::Load { idle, .. } = how {
        // (the world open's idle ticks: game time and day time advance, the level's weather with them;
        // nothing of the world ticks, so every tick due inside them fires at the first ticking tick)
        for _ in 0..idle.max(1) {
            // (the tick's own time phase: game time, the day time while `advance_time` holds, the
            // weather; the Nether part's clock with the overworld's: game time is global)
            w.tick_time();
            if let Some(n) = w.nether.as_mut() {
                n.time = w.time;
            }
        }
        let held = w.entities.list.iter().filter(|e| !e.removed).count();
        return Ok(Restarted { ids: ids(w), first, at: w.time, entity_free: how.load_ticks(), held, created: Vec::new(), stress: false, nether: w.nether.is_some() });
    }
    // (the STRESS variant's block-only ticks: an empty entity-ticking range, so `entity_ticks_at` is false
    // everywhere and no entity ticks, its place in the list kept; the freezes they note stay in
    // `World::frozen`, counted; no block entity ticks either)
    let (ec, frozen) = (w.entity_chunks, w.frozen.len());
    let present: std::collections::HashSet<u32> = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
    w.entity_chunks = Some([1, 1, 0, 0]);
    w.skip_block_entities = true;
    for _ in 0..how.load_ticks() {
        w.tick();
    }
    w.skip_block_entities = false;
    w.entity_chunks = ec;
    if w.errors.len() > errors {
        return Err(format!("restart {}: the stress ticks: {}", how.label(), w.errors[errors..].join("; ")));
    }
    let at = w.time;
    let noted: std::collections::HashSet<u32> = w.frozen[frozen..].iter().filter(|(t, _)| *t >= first && *t <= at).map(|(_, id)| *id).collect();
    let live: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
    let (held, created) = load_held(&present, &live, &noted).map_err(|t| format!("restart {}: the stress ticks ticked entities {t:?}", how.label()))?;
    Ok(Restarted { ids: ids(w), first, at, entity_free: how.load_ticks(), held, created, stress: true, nether: w.nether.is_some() })
}

/// An entity's current id after any restarts (from the id it had once, or any since).
pub fn follow(w: &World, id: u32) -> u32 {
    w.renumbered.get(&id).copied().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stretcher, a sequence dropped by a fall, the next start rising again: a virtual ring in
    /// a world with no partial unload always ticks (ALIVE never falls); with the ring's chunk
    /// unforced it stops and ALIVE falls `stretch` gt after its last edge at the latest.
    #[test]
    fn alive_and_sequences() {
        let mut w = World::new(BlockPos::new(0, 0, 0), [32, 4, 32]);
        let mut g = Liveness::new(Ring::Virtual { x: 8.0, z: 8.0, period: 30, high: 20, phase: 0 });
        for t in 0..100 {
            assert!(g.alive(&w, t));
        }
        g.start_with("fill", 100, &[5, 40]);
        assert!(!g.rise(&w, "fill", 5, 104));
        assert!(g.rise(&w, "fill", 5, 105));
        // (the ring frozen: its chunk unforced, a ring of forced chunks kept around it)
        w.entity_chunks = Some([0, 0, 1, 1]);
        redsim::reload::unload_some(&mut w, [0, 0, 0, 0]);
        let mut fell = None;
        for t in 106..200 {
            if !g.alive(&w, t) && fell.is_none() {
                fell = Some(t);
            }
        }
        let f = fell.expect("ALIVE falls once the ring stops");
        assert!(f <= 106 + 32 + 1, "fell at {f}");
        assert_eq!(g.drops.len(), 1, "the fill's +40 was still to fire: dropped");
        assert!(!g.rise(&w, "fill", 40, 140));
        redsim::reload::load_some(&mut w, [0, 0, 0, 0]);
        let mut back = None;
        for t in 200..260 {
            if g.alive(&w, t) && back.is_none() {
                back = Some(t);
            }
        }
        assert!(back.is_some(), "ALIVE rises at the ring's first edge after the thaw");
        g.start("fill", 260);
        assert!(g.rise(&w, "fill", 0, 260));
    }

    /// Day-1's fast ring: ALIVE never falls while the ring runs (edges 3..4 gt apart under a 5-gt
    /// stretcher); frozen at any phase of its lap, ALIVE falls within 6 gt; a dropped sequence
    /// reads as dropped, a running one as pending.
    #[test]
    fn fast_ring() {
        let w = World::new(BlockPos::new(0, 0, 0), [32, 4, 32]);
        for phase in 0..15 {
            let mut g = Liveness::new(Ring::fast(8.0, 8.0));
            g.stretch = FAST_STRETCH;
            g.column_frozen = Some(false);
            for t in 0..(100 + phase) {
                assert!(g.alive(&w, t), "phase {phase}: ALIVE low at {t} with the ring running");
            }
            g.start_with("fill", 100 + phase, &[50]);
            assert!(g.pending("fill"));
            let f0 = 100 + phase;
            g.column_frozen = Some(true);
            let fell = (f0..f0 + 40).find(|t| !g.alive(&w, *t)).expect("ALIVE falls once the ring stops");
            assert!(fell - f0 <= FAST_STRETCH + 1, "phase {phase}: latency {}", fell - f0);
            assert!(g.dropped("fill") && !g.pending("fill"));
        }
    }

    /// A restart between two ticks: the cart back under a new id that `follow` finds (from its first
    /// id and from any since) through two restarts, a pending scheduled tick kept across both, the
    /// rows' orders parsed back from their labels.
    #[test]
    fn restart_rows() {
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        w.set_block(BlockPos::new(0, -1, 0), redsim::block::Block::parse("minecraft:stone").unwrap(), redsim::world::flags::ALL);
        let rail = redsim::block::Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").unwrap();
        w.set_block(BlockPos::new(0, 0, 0), rail.clone(), redsim::world::flags::ALL);
        let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.0625, 0.5], None).unwrap();
        let rep = redsim::block::Block::parse("minecraft:repeater[delay=1,facing=north,locked=false,powered=false]").unwrap();
        w.set_block(BlockPos::new(2, 0, 2), rep.clone(), redsim::world::flags::ALL);
        w.schedule_tick(BlockPos::new(2, 0, 2), rep.kind(), 4, redsim::ticks::Priority::Normal);
        let n0 = w.ticks.count();
        assert!(n0 >= 1);
        let live_cart = |w: &World, id: u32| w.entities.list.iter().any(|e| e.id == id && !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_)));
        let ids = restart(&mut w, Restart::load(redsim::reload::Order::Reverse)).unwrap();
        let now = follow(&w, cart);
        assert_ne!(now, cart);
        assert_eq!(ids.get(&cart), Some(&now));
        assert!(live_cart(&w, now));
        assert_eq!(w.ticks.count(), n0, "a save/stop/start keeps the pending tick");
        restart(&mut w, Restart::load(redsim::reload::Order::Seed(2))).unwrap();
        let again = follow(&w, cart);
        assert_ne!(again, now, "a second restart renumbers again");
        assert!(live_cart(&w, again), "the first id follows to a live minecart");
        assert_eq!(follow(&w, now), again, "the id between follows to the same one");
        assert_eq!(w.ticks.count(), n0);
        // (one physical branch (ironspec 22:20 (2)): the 10 orders at a world open with N = 1 and N = 2 idle
        // ticks (23:23 (2)); /idle:N labels the non-default N; the STRESS variant /stress:F)
        let rows = Restart::rows();
        assert_eq!(rows.len(), 20);
        assert!(rows.iter().all(|r| !r.is_stress()) && rows.iter().filter(|r| r.load_ticks() == 1).count() == 10 && rows.iter().filter(|r| r.load_ticks() == 2).count() == 10);
        for r in rows.iter().copied().chain([Restart::stress(redsim::reload::Order::Reverse).with_extra(2)]) {
            assert_eq!(Restart::parse(&r.label()), Some(r));
        }
        assert_eq!(parse_restart("120,seed:3"), Ok((120, Restart::load(redsim::reload::Order::Seed(3)))));
        assert_eq!(parse_restart("120,seed:3/idle:1"), Ok((120, Restart::Load { order: redsim::reload::Order::Seed(3), idle: 1 })));
        assert_eq!(parse_restart("120,reverse/stress:2"), Ok((120, Restart::Stress { order: redsim::reload::Order::Reverse, extra: 2 })));
        assert_eq!((Restart::stress(redsim::reload::Order::Save).with_extra(3).label(), Restart::stress(redsim::reload::Order::Save).with_extra(3).load_ticks()), ("save/stress:3".to_string(), 4));
        assert!(parse_restart("120,cold").is_err() && parse_restart("120,save/t0").is_err() && parse_restart("120,save+2").is_err() && parse_restart("120,save/idle:0").is_err());
    }

    /// The scene of the restart tests: a moving cart on a rail, a repeater powered from behind on its way on
    /// (a scheduled tick pending), a daylight detector reading noon, a hopper over a chest holding an ingot,
    /// a crafter one tick from the end of its crafting.
    struct Scene {
        w: World,
        cart: u32,
        rp: BlockPos,
        dd: BlockPos,
        hp: BlockPos,
        chest: BlockPos,
        cp: BlockPos,
    }

    fn scene(time: i64) -> Scene {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        // (the clock first, so every tick the scene schedules is due relative to it)
        w.time = time;
        for x in -3..=3 {
            w.set_block(BlockPos::new(x, -1, 0), stone.clone(), redsim::world::flags::ALL);
            w.set_block(BlockPos::new(x, 0, 0), redsim::block::Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        }
        let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.0625, 0.5], None).unwrap();
        let i = redsim::entity::index_of(&w, cart).unwrap();
        w.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.3, 0.0, 0.0);
        let rp = BlockPos::new(2, 0, 4);
        w.set_block(rp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(rp, redsim::block::Block::parse("minecraft:repeater[delay=2,facing=north,locked=false,powered=false]").unwrap(), redsim::world::flags::ALL);
        for z in [3, 5] {
            w.set_block(BlockPos::new(2, 0, z), redsim::block::Block::parse("minecraft:redstone_block").unwrap(), redsim::world::flags::ALL);
        }
        let dd = BlockPos::new(-2, 0, 4);
        w.set_block(dd.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(dd, redsim::block::Block::parse("minecraft:daylight_detector[inverted=false,power=0]").unwrap(), redsim::world::flags::ALL);
        let (hp, chest) = (BlockPos::new(-4, 1, 4), BlockPos::new(-4, 0, 4));
        w.set_block(chest.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(chest, redsim::block::Block::parse("minecraft:chest[facing=north,type=single,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        w.set_block(hp, redsim::block::Block::parse("minecraft:hopper[enabled=true,facing=down]").unwrap(), redsim::world::flags::ALL);
        let ingot = redsim::redstone::container::entity_items("{Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:1}]}").unwrap();
        redsim::redstone::container::set_contents(&mut w, hp, &ingot);
        let cp = BlockPos::new(-6, 0, 4);
        w.set_block(cp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(cp, redsim::block::Block::parse("minecraft:crafter[crafting=true,orientation=north_up,triggered=false]").unwrap(), redsim::world::flags::ALL);
        w.containers.map.get_mut(&cp).expect("the crafter's block entity").crafting_ticks = 1;
        Scene { w, cart, rp, dd, hp, chest, cp }
    }

    fn held_in(w: &World, p: BlockPos) -> u32 {
        w.containers.map.get(&p).map_or(0, |c| c.slots.iter().flatten().map(|s| s.count as u32).sum::<u32>())
    }

    fn pos_of(w: &World, id: u32) -> redsim::entity::Vec3 {
        w.entities.list[redsim::entity::index_of(w, id).unwrap()].pos
    }

    /// The official load at a world open (ironspec 23:23; tick-exact's official control run): loaded at the
    /// save's game time, then N whole-chunk idle ticks, then normal ticks. Game time and day time + N with
    /// nothing ticking (the repeater's pending turn-on not fired, the water's pending spread not run, the
    /// detector's 20-gt check falling in the idle lost (DAY up to 20 late), the hopper keeps its ingot, the
    /// crafter stays crafting, the cart is back where it was saved); the saved ticks keep their save-time due
    /// times (not re-based); entities and block entities resume together on the first ticking tick (the cart
    /// moves, the hopper pushes, the crafter finishes); a tick due after the window fires at its game-time due
    /// time.
    #[test]
    fn restart_is_a_world_open() {
        // (the idle ticks at 5999..=6000: the detector's check at 6000 inside them; the repeater's turn-on due
        // at 6002 and the water's first spread at 6003, both after them)
        let Scene { mut w, cart, rp, dd, hp, chest, cp } = scene(5998);
        let water = BlockPos::new(5, 0, -4);
        for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            w.set_block(BlockPos::new(water.x + dx, -1, water.z + dz), redsim::block::Block::parse("minecraft:stone").unwrap(), redsim::world::flags::ALL);
        }
        w.set_block(water, redsim::block::Block::parse("minecraft:water[level=0]").unwrap(), redsim::world::flags::ALL);
        let fluid_due = w.fluid_ticks.all().into_iter().find(|t| t.pos == water).map(|t| t.trigger).expect("the water's pending spread");
        let crafting = |w: &World| w.get(cp).to_string().contains("crafting=true");
        let pend = |w: &World, p: BlockPos| w.ticks.all().into_iter().find(|t| t.pos == p).map(|t| t.trigger);
        let (t0, d0, p0, s0, n0) = (w.time, w.day_time(), pos_of(&w, cart), w.get(rp), w.ticks.count());
        let rp_due = pend(&w, rp).expect("the repeater's pending turn-on");
        assert!(rp_due > t0 + WORLD_OPEN_IDLE && fluid_due > t0 + WORLD_OPEN_IDLE, "both due after the window: {rp_due}, {fluid_due}");
        let r = restart(&mut w, Restart::load(redsim::reload::Order::Save)).unwrap();
        let now = follow(&w, cart);
        assert_eq!((w.time, w.day_time()), (t0 + WORLD_OPEN_IDLE, d0 + WORLD_OPEN_IDLE), "game time and day time advance by N");
        assert_eq!((r.first, r.at, r.entity_free, r.held, r.stress), (t0 + 1, t0 + WORLD_OPEN_IDLE, WORLD_OPEN_IDLE, 1, false), "{}", r.note(0));
        assert_eq!((w.get(rp), w.ticks.count()), (s0, n0), "nothing ticked: the repeater's turn-on still pending");
        assert_eq!(pend(&w, rp), Some(rp_due), "the saved tick keeps its save-time due time (not re-based)");
        assert_eq!(w.fluid_ticks.all().into_iter().find(|t| t.pos == water).map(|t| t.trigger), Some(fluid_due), "the fluid tick too, not run");
        assert!(w.get(BlockPos::new(water.x + 1, 0, water.z)).is_air(), "the water didn't spread in the idle");
        assert_eq!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the detector's check in the idle lost");
        assert!(held_in(&w, hp) == 1 && held_in(&w, chest) == 0 && crafting(&w), "no block entity ticked");
        let p1 = pos_of(&w, now);
        assert!((p1.x - p0.x).abs() < 1e-9 && (p1.z - p0.z).abs() < 1e-9, "the cart back where it was saved: {p0:?} -> {p1:?}");
        assert_eq!(r.note(0), format!("entity-free ticks 2 (a world open: 2 whole-chunk idle ticks at gt {}..={}, nothing ticked, game and day time advanced, the ticks due in them firing at the first ticking tick; 1 entities back; the lead's only load case; the pearl alternative's unload case not modelled here)", t0 + 1, t0 + 2));
        // (the first ticking tick: entities and block entities resume together)
        w.tick();
        assert!(pos_of(&w, now).x > p1.x, "the cart moves on the first ticking tick");
        assert!(held_in(&w, hp) == 0 && held_in(&w, chest) == 1 && !crafting(&w), "the hopper pushes and the crafter finishes on the first ticking tick");
        assert_eq!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the detector's next check is at 6020");
        // (the repeater's tick, due after the window, fires at its game-time due time)
        let mut on_at = None;
        while w.time < rp_due + 4 && on_at.is_none() {
            if w.get(rp).to_string().contains("powered=true") {
                on_at = Some(w.time);
            } else {
                w.tick();
            }
        }
        assert_eq!(on_at, Some(rp_due), "the repeater turns on at its save-time due time");
    }

    /// The world-open rule's tick-exact rows (ironspec 23:23 (2)): a delay-4 repeater pending at the save
    /// fires at save + 8 with N = 1 and N = 2 (the idle ticks count toward it); a 1-gt tick due inside the
    /// window fires at the first ticking tick (late, bunched), not in the idle; two ticks due inside the
    /// window keep their saved order at the first ticking tick.
    #[test]
    fn world_open_overdue_ticks() {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let rs = redsim::block::Block::parse("minecraft:redstone_block").unwrap();
        let powered = |w: &World, p: BlockPos| w.get(p).to_string().contains("powered=true");
        for n in WORLD_OPEN_IDLES {
            // (a delay-4 repeater whose input turned on at the save: its turn-on due at save + 8)
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            w.time = 1000;
            let rp = BlockPos::new(0, 0, 0);
            w.set_block(rp.below(), stone.clone(), redsim::world::flags::ALL);
            w.set_block(rp, redsim::block::Block::parse("minecraft:repeater[delay=4,facing=north,locked=false,powered=false]").unwrap(), redsim::world::flags::ALL);
            for z in [-1, 1] {
                w.set_block(BlockPos::new(0, 0, z), rs.clone(), redsim::world::flags::ALL);
            }
            let save = w.time;
            assert_eq!(w.ticks.all().into_iter().find(|t| t.pos == rp).map(|t| t.trigger), Some(save + 8));
            restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(n)).unwrap();
            let mut on_at = None;
            while w.time < save + 12 && on_at.is_none() {
                if powered(&w, rp) {
                    on_at = Some(w.time);
                } else {
                    w.tick();
                }
            }
            assert_eq!(on_at, Some(save + 8), "N = {n}: the delay-4 repeater fires at save + 8");
            // (two ticks due inside the window (the first at save + 1, both in saved order, in one chunk: a
            // chunk's saved ticks unpack in their saved order): they fire late at the first ticking tick, the
            // earlier saved first)
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            w.time = 2000;
            let (a, b) = (BlockPos::new(1, 0, 0), BlockPos::new(4, 0, 0));
            let r1 = redsim::block::Block::parse("minecraft:repeater[delay=1,facing=north,locked=false,powered=false]").unwrap();
            let mut cells = vec![(a.below(), stone.clone()), (b.below(), stone.clone()), (a, r1.clone()), (b, r1.clone())];
            for p in [a, b] {
                cells.push((BlockPos::new(p.x, 0, p.z - 1), rs.clone()));
                cells.push((BlockPos::new(p.x, 0, p.z + 1), rs.clone()));
            }
            w.place_raw(&cells);
            w.schedule_tick(b, r1.kind(), 1, redsim::ticks::Priority::High);
            w.schedule_tick(a, r1.kind(), 1, redsim::ticks::Priority::High);
            let save = w.time;
            let before: Vec<BlockPos> = w.ticks.all().into_iter().map(|t| t.pos).collect();
            assert_eq!(before, vec![b, a], "scheduled b then a, both due at save + 1");
            restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(n)).unwrap();
            assert!(!powered(&w, a) && !powered(&w, b), "N = {n}: nothing fired in the idle");
            let order: Vec<BlockPos> = w.clone().ticks.collect(w.time + 1).into_iter().map(|t| t.pos).collect();
            assert_eq!(order, vec![b, a], "N = {n}: the overdue ticks keep their saved order");
            w.tick();
            assert_eq!(w.time, save + n + 1);
            assert!(powered(&w, a) && powered(&w, b), "N = {n}: both fire at the first ticking tick (save + {})", n + 1);
            // (two ticks overdue at the first ticking tick with different priorities, scheduled (and saved) in
            // the opposite order: the higher priority fires first (ironspec 23:29: due time, then priority,
            // then sub-tick order))
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            w.time = 3000;
            let (lo, hi) = (BlockPos::new(1, 0, 0), BlockPos::new(4, 0, 0));
            let mut cells = vec![(lo.below(), stone.clone()), (hi.below(), stone.clone()), (lo, r1.clone()), (hi, r1.clone())];
            for p in [lo, hi] {
                cells.push((BlockPos::new(p.x, 0, p.z - 1), rs.clone()));
                cells.push((BlockPos::new(p.x, 0, p.z + 1), rs.clone()));
            }
            w.place_raw(&cells);
            w.schedule_tick(lo, r1.kind(), 1, redsim::ticks::Priority::High);
            w.schedule_tick(hi, r1.kind(), 1, redsim::ticks::Priority::ExtremelyHigh);
            restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(n)).unwrap();
            // (asserted through the drain the tick itself runs (`LevelTicks::collect` at the first ticking
            // tick's game time), on a copy)
            let due: Vec<(BlockPos, redsim::ticks::Priority)> = w.clone().ticks.collect(w.time + 1).into_iter().map(|t| (t.pos, t.priority)).collect();
            assert_eq!(due, vec![(hi, redsim::ticks::Priority::ExtremelyHigh), (lo, redsim::ticks::Priority::High)], "N = {n}: the higher priority first, though saved second");
            w.tick();
            assert!(powered(&w, lo) && powered(&w, hi), "N = {n}: both fire at the first ticking tick");
        }
    }

    /// The idle runs the tick's own time phase (ironspec 23:58 (1)): with the day cycle held (the
    /// `advance_time` game rule off, as A6's night and the bed trap's 13,800 hold it) game time moves by N
    /// and the day time stays; with it on both move. Game time is global (23:58 (2)): a world's Nether part
    /// keeps its clock with the overworld's through the idle, and the note says what the code does.
    #[test]
    fn world_open_time_phase() {
        for (advance, dday) in [(false, 0), (true, 2)] {
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            w.time = 500;
            w.mob.day_offset = 13_300;
            w.mob.advance_time = advance;
            let (t0, d0) = (w.time, w.day_time());
            restart(&mut w, Restart::load(redsim::reload::Order::Save)).unwrap();
            assert_eq!((w.time - t0, w.day_time() - d0), (2, dday), "advance_time {advance}");
            // (and a normal tick keeps the rule)
            w.tick();
            assert_eq!((w.time - t0, w.day_time() - d0), (3, dday * 3 / 2), "advance_time {advance}, a tick after");
        }
        let mut wn = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        wn.time = 800;
        let mut nether = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        nether.time = 800;
        wn.nether = Some(Box::new(nether));
        let r = restart(&mut wn, Restart::load(redsim::reload::Order::Save).with_idle(1)).unwrap();
        assert_eq!((wn.time, wn.nether.as_ref().map(|n| n.time)), (801, Some(801)), "both clocks agree after the idle");
        assert!(r.note(0).ends_with("; Nether restart not modelled in redsim (the Nether part kept its in-memory state, its clock advanced with the overworld's): this row UNMODELLED"), "{}", r.note(0));
        wn.tick();
        assert_eq!((wn.time, wn.nether.as_ref().map(|n| n.time)), (802, Some(802)));
    }

    /// A world open drops every memory registered without a codec and keeps the codec'd ones (ironspec 01:49
    /// (4); tick-exact 01:36: 26.3's MemoryModuleType registers CANT_REACH_WALK_TARGET_SINCE without a codec
    /// (MemoryModuleType.java 63) and `Brain.pack` writes only the `canSerialize()` memories). redsim's load
    /// (`reload::rebuild_villager_brain`, its list from 26.3's registrations: HOME, LAST_SLEPT, LAST_WOKEN,
    /// GOLEM_DETECTED_RECENTLY with its time left, JOB_SITE, POTENTIAL_JOB_SITE, MEETING_POINT kept; the
    /// sensed ones, NEAREST_HOSTILE, HURT_BY, CANT_REACH_WALK_TARGET_SINCE, WALK_TARGET, PATH, LOOK_TARGET and
    /// SECONDARY_JOB_SITE dropped) builds a new brain from the codec'd memories alone, and `restore_fresh` a
    /// new navigation: one world open keeps HOME, LAST_SLEPT and GOLEM_DETECTED_RECENTLY (its TTL left) and
    /// drops CANT_REACH_WALK_TARGET_SINCE, PATH, WALK_TARGET and NEAREST_HOSTILE.
    #[test]
    fn world_open_drops_codecless_memories() {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        for x in -6..=6 {
            for z in -6..=6 {
                w.set_block(BlockPos::new(x, -1, z), stone.clone(), redsim::world::flags::ALL);
            }
        }
        w.mob.seed(7, 1000);
        w.mob.ai.on = true;
        let v = redsim::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, 0.5], None).unwrap();
        w.tick();
        let (t, i) = (w.time, redsim::entity::index_of(&w, v).unwrap());
        {
            let b = redsim::entity::mob::ai::brain::brain_mut(&mut w, i);
            b.home = Some(BlockPos::new(2, 0, 2));
            b.last_slept = Some(t - 100);
            b.golem_detected = Some(450);
            b.nearest_hostile = Some(v);
            let ex = b.ex.as_mut().expect("the villager's exact brain (AI on)");
            ex.mem.cant_reach_since = Some(t);
            ex.mem.walk_target = Some(redsim::entity::mob::ai::brain::WalkTarget::at_point(redsim::entity::Vec3::new(3.5, 0.0, 3.5), 0.5, 1));
            ex.mem.path = Some(redsim::entity::mob::ai::path::Path::new(Vec::new(), BlockPos::new(3, 0, 3), false));
        }
        // (no vacuous pass, G1: the four codec-less memories are present before the save)
        {
            let b = redsim::entity::mob::ai::brain::brain(&w, i);
            let ex = b.ex.as_ref().expect("the exact brain");
            assert_eq!((b.nearest_hostile, ex.mem.cant_reach_since, ex.mem.walk_target.is_some(), ex.mem.path.is_some()), (Some(v), Some(t), true, true), "NEAREST_HOSTILE, CANT_REACH_WALK_TARGET_SINCE, WALK_TARGET and PATH set before the world open");
        }
        restart(&mut w, Restart::load(redsim::reload::Order::Save)).unwrap();
        let i = redsim::entity::index_of(&w, follow(&w, v)).unwrap();
        let b = redsim::entity::mob::ai::brain::brain(&w, i);
        assert_eq!((b.home, b.last_slept, b.golem_detected), (Some(BlockPos::new(2, 0, 2)), Some(t - 100), Some(450)), "HOME, LAST_SLEPT and GOLEM_DETECTED_RECENTLY (its time left) kept");
        assert_eq!(b.nearest_hostile, None, "NEAREST_HOSTILE (no codec) dropped");
        let ex = b.ex.as_ref().expect("the exact brain rebuilt");
        assert_eq!((ex.mem.cant_reach_since, ex.mem.walk_target.is_some(), ex.mem.path.is_some()), (None, false, false), "CANT_REACH_WALK_TARGET_SINCE, WALK_TARGET and PATH (no codec) dropped");
    }

    /// ironspec 05:20's bench rule (the brain's activity at t1, REST at night): a villager summoned with the
    /// day already set is in REST on its first tick at night (13800) and IDLE by day (1000: the schedule's
    /// IDLE, from 10 to WORK's 2000), as a fresh Brain's first schedule update (lastScheduleUpdate −9999) is
    /// immediate; and a world open keeps that: the reloaded brain takes the schedule's activity at the load's
    /// own day time at once (reload's rebuild), so its first tick after the open is in the same activity.
    /// The wrong order (G1 on 0e91db73c), why the rule exists: summoned at day 1000, then the day set to 13800
    /// (`/time set`: the day time moves, the game time doesn't), the villager is still IDLE at t1 and turns to
    /// REST only at its next schedule update, more than 20 gt after the summon's (the t22 offset).
    #[test]
    fn world_open_activity_at_t1() {
        use redsim::entity::mob::ai::brain::{Act, brain};
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let active = |w: &World, i: usize| brain(w, i).ex.as_ref().expect("the exact brain (AI on)").active;
        let floor = |w: &mut World| {
            for x in -6..=6 {
                for z in -6..=6 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), redsim::world::flags::ALL);
                }
            }
        };
        for (day, want) in [(13800, Act::Rest), (1000, Act::Idle)] {
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            floor(&mut w);
            w.mob.seed(7, day);
            w.mob.ai.on = true;
            let v = redsim::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, 0.5], None).unwrap();
            w.tick();
            let i = redsim::entity::index_of(&w, v).unwrap();
            assert_eq!(active(&w, i), want, "day {day}: the activity on the first tick after the summon");
            restart(&mut w, Restart::load(redsim::reload::Order::Save)).unwrap();
            let i = redsim::entity::index_of(&w, follow(&w, v)).unwrap();
            assert_eq!(active(&w, i), want, "day {day}: the reloaded brain's activity at the open");
            w.tick();
            let i = redsim::entity::index_of(&w, follow(&w, v)).unwrap();
            assert_eq!(active(&w, i), want, "day {day}: the activity on the first tick after the open");
        }
        // (the wrong order: summoned by day, the night set after)
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        floor(&mut w);
        w.mob.seed(7, 1000);
        w.mob.ai.on = true;
        let v = redsim::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, 0.5], None).unwrap();
        let summoned = w.time;
        w.mob.day_offset = 13800 - w.time;
        w.tick();
        let i = redsim::entity::index_of(&w, v).unwrap();
        assert_eq!(active(&w, i), Act::Idle, "summoned by day, the night set after: IDLE at t1, not REST");
        let mut rest_at = None;
        for _ in 0..40 {
            w.tick();
            if active(&w, redsim::entity::index_of(&w, v).unwrap()) == Act::Rest {
                rest_at = Some(w.time - summoned);
                break;
            }
        }
        assert!(rest_at.is_some_and(|t| t > 20), "REST only at the next schedule update, more than 20 gt after the summon: {rest_at:?}");
    }

    /// The vanilla time order at a detector (tick-exact's six split rows): 26.3 advances the day time in
    /// tickChildren before the levels tick, then the level runs its weather, `updateSkyBrightness` at that
    /// day time, `tickTime` (game time + 1) and the block entities, so a daylight detector updating on a
    /// game time multiple of 20 reads this tick's advanced day time. At day times 12031 / 12687 / 13670 /
    /// 22331 / 23314 / 23970 (each a step of the detector's power) it reads 6 / 3 / 0 / 1 / 4 / 7: in a plain
    /// tick, and on the first ticking tick after a world open of N = 1 and 2 (the idle ticks move both
    /// clocks and update nothing).
    #[test]
    fn detector_split_rows() {
        let rows: [(i64, u8); 6] = [(12031, 6), (12687, 3), (13670, 0), (22331, 1), (23314, 4), (23970, 7)];
        let power_at = |day: i64, open: Option<i64>| -> u8 {
            let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
            let mut w = World::new(BlockPos::new(-4, -2, -4), [9, 12, 9]);
            for x in -2..=2 {
                for z in -2..=2 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), redsim::world::flags::ALL);
                }
            }
            w.set_block(BlockPos::new(0, 0, 0), redsim::block::Block::parse("minecraft:daylight_detector[inverted=false,power=15]").unwrap(), redsim::world::flags::ALL);
            w.mob.seed(7, 1000);
            // (the tick that reads: game time 1000 after its tickTime, day time `day` after its advance)
            let idle = open.unwrap_or(0);
            w.time = 999 - idle;
            w.mob.day_offset = day - 1 - 999;
            if let Some(n) = open {
                restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(n)).unwrap();
            }
            assert_eq!((w.time, w.day_time().rem_euclid(24000)), (999, (day - 1).rem_euclid(24000)), "the reading tick's eve");
            w.tick();
            assert!(w.errors.is_empty(), "{:?}", w.errors);
            match w.get(BlockPos::new(0, 0, 0)) {
                redsim::block::Block::DaylightDetector { power, .. } => power,
                b => panic!("{b}"),
            }
        };
        for (day, want) in rows {
            assert_eq!(power_at(day, None), want, "day time {day}, a plain tick");
            for n in [1, 2] {
                assert_eq!(power_at(day, Some(n)), want, "day time {day}, after a world open with N = {n}");
            }
        }
    }

    /// tick-exact's golem-ttl rows on the official 26.3 server (artifact 1798171348126791; ironspec 00:30):
    /// one golem stand (a glass floor at y 63, three villagers in carts at z 0.5 / 2.5 / 4.5 in brain order v0,
    /// v1, v2, a husk in a boat at (-4.5, 64, 2.5), a 5 x 3 cobblestone pad at x 2..4, z 0..4), T the first
    /// golem's tick. In brain order each villager's MemorySlot.tick runs (a TTL of 0 clears, else it counts
    /// down) before its panic tick, and a villager calls only when the three memories are clear; the setter
    /// stamps 599 on the three and those after it in that tick read 598. So the CALL (a GolemTry, the golem
    /// itself the pad's luck) comes: setter v0 or v1, at T + 600 by the setter; a world open at T + 300 (N = 1:
    /// one tick with no brain tick), none at T + 600 and the next at T + 700; the trio set at T + 100 to clear
    /// 1 tick before S = T + 400 (TTL 298) with a world open at T + 200, one call at S, by v2 alone; set to
    /// clear 2 ticks before (TTL 297), a call at S by v0; with no world open both call at S by v0.
    #[test]
    fn golem_ttl_order_dependence() {
        use redsim::block::Block;
        use redsim::entity::mob::ai::brain::brain_mut;
        let stand = |seed: i64| -> (World, [u32; 3]) {
            let (glass, cobble) = (Block::parse("minecraft:glass").unwrap(), Block::parse("minecraft:cobblestone").unwrap());
            let mut w = World::new(BlockPos::new(-14, 52, -10), [30, 24, 24]);
            for x in -10..=10 {
                for z in -6..=10 {
                    let b = if (2..=4).contains(&x) && (0..=4).contains(&z) { cobble.clone() } else { glass.clone() };
                    w.set_block(BlockPos::new(x, 63, z), b, redsim::world::flags::ALL);
                }
            }
            w.mob.seed(seed, 6000);
            let mut v = [0u32; 3];
            for (k, z) in [0.5, 2.5, 4.5].into_iter().enumerate() {
                let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 64.0, z], None).unwrap();
                let vil = redsim::entity::spawn(&mut w, "minecraft:villager", [0.5, 64.0625, z], None).unwrap();
                redsim::entity::mob::mount(&mut w, vil, cart).unwrap();
                v[k] = vil;
            }
            let boat = redsim::entity::spawn(&mut w, "minecraft:oak_boat", [-4.5, 64.0, 2.5], None).unwrap();
            // (redsim models no husk: a zombie, its sky roofed over at y 67 so it doesn't burn at noon,
            // `Mob.burnUndead`; the villagers' hostiles sensor takes either at 8)
            let husk = redsim::entity::spawn(&mut w, "minecraft:zombie", [-4.5, 64.0, 2.5], None).unwrap();
            w.set_block(BlockPos::new(-5, 67, 2), Block::parse("minecraft:stone").unwrap(), redsim::world::flags::ALL);
            redsim::entity::mob::mount(&mut w, husk, boat).unwrap();
            for id in v {
                let i = redsim::entity::index_of(&w, id).unwrap();
                brain_mut(&mut w, i).last_slept = Some(0);
            }
            (w, v)
        };
        // (one tick: the calls it made (tick, villager, a golem born), every golem killed before its second tick)
        let step = |w: &mut World, calls: &mut Vec<(i64, u32, bool)>| {
            w.tick();
            for e in std::mem::take(&mut w.mob.events) {
                if let redsim::entity::mob::MobEvent::GolemTry { t, villager, golem, .. } = e {
                    calls.push((t, villager, golem.is_some()));
                }
            }
            w.entities.list.retain(|e| !matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem));
        };
        let (mut w, v) = stand(3);
        let mut calls = Vec::new();
        let t = loop {
            step(&mut w, &mut calls);
            assert!(w.time < 3000, "no first golem by 3000: calls {calls:?}");
            if calls.iter().any(|c| c.2) {
                break w.time;
            }
        };
        let set = |w: &mut World, ttl: [i64; 3]| {
            for (k, id) in v.iter().enumerate() {
                let i = redsim::entity::index_of(w, follow(w, *id)).unwrap();
                brain_mut(w, i).golem_detected = Some(ttl[k]);
            }
        };
        // (a row: from T with the setter's stamps, an optional reset at T + 100, a world open at `open`; the
        // calls up to `until`)
        let row = |setter: usize, reset: Option<i64>, open: Option<i64>, until: i64| -> Vec<(i64, usize)> {
            let mut w = w.clone();
            set(&mut w, [0, 1, 2].map(|k| if k <= setter { 599 } else { 598 }));
            let mut calls = Vec::new();
            while w.time < t + until {
                if w.time == t + 100
                    && let Some(r) = reset
                {
                    set(&mut w, [r; 3]);
                }
                if Some(w.time - t) == open {
                    restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(1)).unwrap();
                    continue;
                }
                step(&mut w, &mut calls);
            }
            calls.into_iter().map(|(ct, id, _)| (ct - t, v.iter().position(|o| follow(&w, *o) == id).expect("a stand villager's call"))).collect()
        };
        let first = |c: &[(i64, usize)]| c.first().map(|x| x.0);
        let at = |c: &[(i64, usize)], s: i64| c.iter().filter(|x| x.0 == s).map(|x| x.1).collect::<Vec<_>>();
        for setter in [0, 1] {
            let c = row(setter, None, None, 650);
            assert_eq!((first(&c), at(&c, 600).first().copied()), (Some(600), Some(setter)), "c-{}: the setter v{setter} calls at T + 600: {c:?}", setter + 1);
            let r = row(setter, None, Some(300), 750);
            assert_eq!(first(&r), Some(700), "r-{}: a world open at T + 300: no call at T + 600, the next at T + 700: {r:?}", setter + 1);
        }
        for (m, ttl, open_who) in [(1, 298, vec![2]), (2, 297, vec![0])] {
            let c = row(0, Some(ttl), None, 450);
            assert_eq!((first(&c), at(&c, 400).first().copied()), (Some(400), Some(0)), "boundary m = {m}, no world open: v0 calls at S: {c:?}");
            let r = row(0, Some(ttl), Some(200), 450);
            assert_eq!(first(&r), Some(400), "boundary m = {m} with a world open at T + 200: {r:?}");
            assert_eq!(at(&r, 400)[..1], open_who[..], "boundary m = {m} with a world open: who calls first at S: {r:?}");
            if m == 1 {
                assert_eq!(at(&r, 400), vec![2], "m = 1: v2 alone calls at S");
            }
        }
    }

    /// G1's TTL point (ironspec 00:00 (2)): GOLEM_DETECTED_RECENTLY counts down once per brain tick
    /// (`ExpirableValue.tick`; redsim's `forget_outdated`) and is carried across a reload as its remaining
    /// TTL, so a tick in which the villager doesn't tick extends it by that tick in game time: a 600 memory
    /// frozen F ticks expires F later than the unfrozen control (T + 600 + F), across a world open N later
    /// (T + 600 + N).
    #[test]
    fn ttl_memory_extends() {
        let expiry = |freeze: i64, open: Option<i64>| -> i64 {
            let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            for x in -6..=6 {
                for z in -6..=6 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), redsim::world::flags::ALL);
                }
            }
            w.mob.seed(7, 1000);
            w.mob.ai.on = true;
            let v = redsim::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, 0.5], None).unwrap();
            let i = redsim::entity::index_of(&w, v).unwrap();
            redsim::entity::mob::ai::brain::brain_mut(&mut w, i).golem_detected = Some(600);
            let t0 = w.time;
            if freeze > 0 {
                let ec = w.entity_chunks;
                w.entity_chunks = Some([1, 1, 0, 0]);
                for _ in 0..freeze {
                    w.tick();
                }
                w.entity_chunks = ec;
            }
            if let Some(n) = open {
                restart(&mut w, Restart::load(redsim::reload::Order::Save).with_idle(n)).unwrap();
            }
            loop {
                let i = redsim::entity::index_of(&w, follow(&w, v)).unwrap();
                if redsim::entity::mob::ai::brain::brain(&w, i).golem_detected.is_none() {
                    return w.time - t0;
                }
                assert!(w.time - t0 < 2000, "the memory never expired");
                w.tick();
            }
        };
        let e0 = expiry(0, None);
        assert!((600..=602).contains(&e0), "the control expires at T + 600 (+ the countdown's own tick): {e0}");
        assert_eq!(expiry(30, None), e0 + 30, "a 30-gt freeze");
        assert_eq!(expiry(0, Some(1)), e0 + 1, "a world open, N = 1");
        assert_eq!(expiry(0, Some(2)), e0 + 2, "a world open, N = 2");
    }

    /// The STRESS variant (not observed in 16 official loads, ironspec 23:12 (3); 26.3's tick order, 22:16
    /// (1)): loaded at once, then a block-only tick: the blocks' scheduled ticks and redstone run, the
    /// entities and the block entities don't. Against a clone of the same world given one normal tick: the
    /// same game time, the same scheduled-tick side (a repeater powered from behind on its way on, the pending
    /// ticks), while the moving cart holds its place across the load (the clone's cart moves), its freeze at
    /// the load tick counted and kept (ironspec 21:13); a daylight detector whose 20-gt check falls on the
    /// load tick doesn't update in it (the clone's does) and a hopper doesn't push its item until the tick
    /// after (the clone's does in its first tick); then the scheduled-tick side stays in lockstep with the
    /// clone, the hopper delivers and the cart moves again.
    #[test]
    fn stress_is_a_block_only_tick() {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        for x in -3..=3 {
            w.set_block(BlockPos::new(x, -1, 0), stone.clone(), redsim::world::flags::ALL);
            w.set_block(BlockPos::new(x, 0, 0), redsim::block::Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        }
        let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.0625, 0.5], None).unwrap();
        let i = redsim::entity::index_of(&w, cart).unwrap();
        w.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.3, 0.0, 0.0);
        // (a repeater with a redstone block on both its north and south: powered from behind whichever way it
        // faces, so it schedules its turn-on)
        let rp = BlockPos::new(2, 0, 4);
        w.set_block(rp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(rp, redsim::block::Block::parse("minecraft:repeater[delay=2,facing=north,locked=false,powered=false]").unwrap(), redsim::world::flags::ALL);
        for z in [3, 5] {
            w.set_block(BlockPos::new(2, 0, z), redsim::block::Block::parse("minecraft:redstone_block").unwrap(), redsim::world::flags::ALL);
        }
        // (a daylight detector reading noon at its 20-gt check, which falls on the load tick: game time 6000;
        // a hopper over a chest holding one ingot)
        let dd = BlockPos::new(-2, 0, 4);
        w.set_block(dd.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(dd, redsim::block::Block::parse("minecraft:daylight_detector[inverted=false,power=0]").unwrap(), redsim::world::flags::ALL);
        let (hp, chest) = (BlockPos::new(-4, 1, 4), BlockPos::new(-4, 0, 4));
        w.set_block(chest.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(chest, redsim::block::Block::parse("minecraft:chest[facing=north,type=single,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        w.set_block(hp, redsim::block::Block::parse("minecraft:hopper[enabled=true,facing=down]").unwrap(), redsim::world::flags::ALL);
        let ingot = redsim::redstone::container::entity_items("{Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:1}]}").unwrap();
        redsim::redstone::container::set_contents(&mut w, hp, &ingot);
        // (a crafter one tick from the end of its crafting: its block entity's tick flips `crafting` off)
        let cp = BlockPos::new(-6, 0, 4);
        w.set_block(cp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(cp, redsim::block::Block::parse("minecraft:crafter[crafting=true,orientation=north_up,triggered=false]").unwrap(), redsim::world::flags::ALL);
        w.containers.map.get_mut(&cp).expect("the crafter's block entity").crafting_ticks = 1;
        let crafting = |w: &World| w.get(cp).to_string().contains("crafting=true");
        w.time = 5999;
        let held_in = |w: &World, p: BlockPos| w.containers.map.get(&p).map_or(0, |c| c.slots.iter().flatten().map(|s| s.count as u32).sum::<u32>());
        assert_eq!((held_in(&w, hp), held_in(&w, chest)), (1, 0));
        let mut w2 = w.clone();
        w2.tick();
        assert!(w2.get(dd).to_string().contains("power=") && !w2.get(dd).to_string().contains("power=0]") && !w2.get(dd).to_string().contains("power=0,"), "the clone's detector updated at its 20-gt check: {}", w2.get(dd));
        assert_eq!((held_in(&w2, hp), held_in(&w2, chest)), (0, 1), "the clone's hopper pushed in its first tick");
        assert!(crafting(&w) && !crafting(&w2), "the clone's crafter finished in its first tick");
        let (t0, frozen0) = (w.time, w.frozen.len());
        let p0 = w.entities.list[i].pos;
        let r = restart(&mut w, Restart::stress(redsim::reload::Order::Save)).unwrap();
        let now = follow(&w, cart);
        let pos_of = |w: &World, id: u32| w.entities.list[redsim::entity::index_of(w, id).unwrap()].pos;
        assert_eq!(w.time, t0 + 1, "the stress variant's block-only tick advances game time by exactly 1");
        assert_eq!(w.time, w2.time);
        assert_eq!(w.get(rp), w2.get(rp), "the scheduled-tick side ticked as in a normal tick");
        assert_eq!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the detector's check on the load tick skipped (no block entity ticks in it)");
        assert_eq!((held_in(&w, hp), held_in(&w, chest)), (1, 0), "the hopper didn't push in the load tick");
        assert!(crafting(&w), "the crafter's tick skipped in the load tick (its ticker in tick_hoppers' list)");
        assert_eq!(w.ticks.count(), w2.ticks.count(), "the same pending ticks as after a normal tick");
        let (p1, q1) = (pos_of(&w, now), pos_of(&w2, cart));
        assert!((p1.x - p0.x).abs() < 1e-9 && (p1.y - p0.y).abs() < 1e-9 && (p1.z - p0.z).abs() < 1e-9, "the moving cart held across the load: {p0:?} -> {p1:?}");
        assert!(q1.x > p0.x, "the clone's cart moved in its normal tick: {p0:?} -> {q1:?}");
        // (counted, not dropped (ironspec 21:13): the cart noted frozen at the load tick and kept; the row's
        // entity-free ticks: the load's 1, plus an F-gt level-32 freeze after it)
        assert_eq!((r.first, r.at, r.entity_free, r.held, r.created.len(), r.stress), (t0 + 1, t0 + 1, 1, 1, 0, true), "{}", r.note(0));
        assert!(w.frozen[frozen0..].contains(&(t0 + 1, now)), "the load tick's freeze kept in World::frozen: {:?}", &w.frozen[frozen0..]);
        assert_eq!(r.get(&cart), Some(&now), "it reads as the id map");
        assert_eq!(r.note(0), format!("STRESS (not observed in 16 official loads; prints only): entity-free ticks 1 (block-only ticks at gt {} holding 1 entities)", t0 + 1));
        assert_eq!((r.with_freeze(3), r.note(3).contains("entity-free ticks 4 ")), (4, true));
        assert!(r.note(3).ends_with(" + a 3-gt level-32 freeze from it)"), "{}", r.note(3));
        // (a world with a Nether part: the note says the row is unmodelled (ironspec 21:24))
        let mut wn = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        wn.nether = Some(Box::new(World::new(BlockPos::new(-8, -4, -8), [16, 8, 16])));
        let rn = restart(&mut wn, Restart::load(redsim::reload::Order::Save)).unwrap();
        assert!(rn.nether && rn.note(0).contains("; Nether restart not modelled in redsim (") && rn.note(0).ends_with(": this row UNMODELLED"), "{}", rn.note(0));
        assert!(!r.nether && !r.note(0).contains("Nether"));
        let mut on = false;
        for _ in 0..6 {
            w.tick();
            w2.tick();
            assert_eq!(w.get(rp), w2.get(rp), "the scheduled-tick side stays in lockstep with the clone");
            on |= w.get(rp).to_string().contains("powered=true");
        }
        assert!(on, "the repeater turned on: the block side did tick");
        assert_eq!((held_in(&w, hp), held_in(&w, chest)), (0, 1), "the hopper delivered from the tick after the load on");
        assert!(!crafting(&w), "the crafter finished from the tick after the load on");
        assert_eq!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the detector's next check is 20 gt after the skipped one");
        assert!(pos_of(&w, now).x > p1.x, "the next normal ticks move the cart again");
    }

    /// The STRESS variant's extra block-only ticks (`reverse/stress:2`, ironspec 23:12 (3): tolerance prints
    /// only): 3 block-only ticks, each with no entity and no block entity ticking and the scheduled ticks and
    /// redstone running: against a clone given 3 normal ticks, the same game time and scheduled-tick side, the
    /// cart held through all three, a daylight detector whose 20-gt check falls on the second skipped, a
    /// hopper holding its item until the tick after; the note reads STRESS, 1 + 2 = 3.
    #[test]
    fn stress_slow_load() {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        for x in -3..=3 {
            w.set_block(BlockPos::new(x, -1, 0), stone.clone(), redsim::world::flags::ALL);
            w.set_block(BlockPos::new(x, 0, 0), redsim::block::Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        }
        let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.0625, 0.5], None).unwrap();
        let i = redsim::entity::index_of(&w, cart).unwrap();
        w.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.3, 0.0, 0.0);
        let rp = BlockPos::new(2, 0, 4);
        w.set_block(rp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(rp, redsim::block::Block::parse("minecraft:repeater[delay=2,facing=north,locked=false,powered=false]").unwrap(), redsim::world::flags::ALL);
        for z in [3, 5] {
            w.set_block(BlockPos::new(2, 0, z), redsim::block::Block::parse("minecraft:redstone_block").unwrap(), redsim::world::flags::ALL);
        }
        let dd = BlockPos::new(-2, 0, 4);
        w.set_block(dd.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(dd, redsim::block::Block::parse("minecraft:daylight_detector[inverted=false,power=0]").unwrap(), redsim::world::flags::ALL);
        let (hp, chest) = (BlockPos::new(-4, 1, 4), BlockPos::new(-4, 0, 4));
        w.set_block(chest.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(chest, redsim::block::Block::parse("minecraft:chest[facing=north,type=single,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        w.set_block(hp, redsim::block::Block::parse("minecraft:hopper[enabled=true,facing=down]").unwrap(), redsim::world::flags::ALL);
        let ingot = redsim::redstone::container::entity_items("{Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:1}]}").unwrap();
        redsim::redstone::container::set_contents(&mut w, hp, &ingot);
        // (the load's ticks at 5999..=6001: the detector's check (6000) on the second)
        w.time = 5998;
        let held_in = |w: &World, p: BlockPos| w.containers.map.get(&p).map_or(0, |c| c.slots.iter().flatten().map(|s| s.count as u32).sum::<u32>());
        let mut w2 = w.clone();
        for _ in 0..3 {
            w2.tick();
        }
        let (t0, p0) = (w.time, w.entities.list[i].pos);
        let how = Restart::stress(redsim::reload::Order::Reverse).with_extra(2);
        let r = restart(&mut w, how).unwrap();
        let now = follow(&w, cart);
        let p1 = w.entities.list[redsim::entity::index_of(&w, now).unwrap()].pos;
        assert_eq!((w.time, r.first, r.at, r.entity_free, r.held), (t0 + 3, t0 + 1, t0 + 3, 3, 1), "{}", r.note(0));
        assert_eq!((w.time, w.get(rp), w.ticks.count()), (w2.time, w2.get(rp), w2.ticks.count()), "the scheduled-tick side as in 3 normal ticks");
        assert!((p1.x - p0.x).abs() < 1e-9 && (p1.z - p0.z).abs() < 1e-9, "the cart held through the slow load: {p0:?} -> {p1:?}");
        assert_eq!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the detector's check on a load tick skipped");
        assert_ne!(w2.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "the clone's detector updated at 6000");
        assert_eq!((held_in(&w, hp), held_in(&w, chest), held_in(&w2, chest)), (1, 0, 1), "the hopper held its item through the load (the clone's delivered)");
        assert_eq!(r.note(0), format!("STRESS (not observed in 16 official loads; prints only): entity-free ticks 1 + 2 = 3 (block-only ticks at gt {}..={} holding 1 entities)", t0 + 1, t0 + 3));
        assert_eq!(how.label(), "reverse/stress:2");
        w.tick();
        assert_eq!((held_in(&w, hp), held_in(&w, chest)), (0, 1), "the hopper delivers on the tick after the load");
    }

    /// A genuine level-32 freeze (ironspec 23:03 (2a)): the entity-ticking range empty and
    /// `skip_block_entities` OFF ticks the block entities as a normal tick does (the load's own state is the
    /// other semantics): a daylight detector's 20-gt check, a hopper's transfer and a crafter's tick happen
    /// in that tick, while the moving cart holds.
    #[test]
    fn level32_freeze_ticks_block_entities() {
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        for x in -3..=3 {
            w.set_block(BlockPos::new(x, -1, 0), stone.clone(), redsim::world::flags::ALL);
            w.set_block(BlockPos::new(x, 0, 0), redsim::block::Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        }
        let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.0625, 0.5], None).unwrap();
        let i = redsim::entity::index_of(&w, cart).unwrap();
        w.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.3, 0.0, 0.0);
        let dd = BlockPos::new(-2, 0, 4);
        w.set_block(dd.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(dd, redsim::block::Block::parse("minecraft:daylight_detector[inverted=false,power=0]").unwrap(), redsim::world::flags::ALL);
        let (hp, chest) = (BlockPos::new(-4, 1, 4), BlockPos::new(-4, 0, 4));
        w.set_block(chest.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(chest, redsim::block::Block::parse("minecraft:chest[facing=north,type=single,waterlogged=false]").unwrap(), redsim::world::flags::ALL);
        w.set_block(hp, redsim::block::Block::parse("minecraft:hopper[enabled=true,facing=down]").unwrap(), redsim::world::flags::ALL);
        let ingot = redsim::redstone::container::entity_items("{Items:[{Slot:0b,id:\"minecraft:iron_ingot\",count:1}]}").unwrap();
        redsim::redstone::container::set_contents(&mut w, hp, &ingot);
        let cp = BlockPos::new(-6, 0, 4);
        w.set_block(cp.below(), stone.clone(), redsim::world::flags::ALL);
        w.set_block(cp, redsim::block::Block::parse("minecraft:crafter[crafting=true,orientation=north_up,triggered=false]").unwrap(), redsim::world::flags::ALL);
        w.containers.map.get_mut(&cp).expect("the crafter's block entity").crafting_ticks = 1;
        w.time = 5999;
        let held_in = |w: &World, p: BlockPos| w.containers.map.get(&p).map_or(0, |c| c.slots.iter().flatten().map(|s| s.count as u32).sum::<u32>());
        let mut w2 = w.clone();
        w2.tick();
        let p0 = w.entities.list[i].pos;
        w.entity_chunks = Some([1, 1, 0, 0]);
        assert!(!w.skip_block_entities);
        w.tick();
        w.entity_chunks = None;
        let p1 = w.entities.list[redsim::entity::index_of(&w, cart).unwrap()].pos;
        assert!((p1.x - p0.x).abs() < 1e-9, "the cart frozen: {p0:?} -> {p1:?}");
        assert_eq!(w.get(dd), w2.get(dd), "the detector's check ran as in a normal tick");
        assert_ne!(w.get(dd).to_string(), "minecraft:daylight_detector[inverted=false,power=0]", "it did update");
        assert_eq!((held_in(&w, hp), held_in(&w, chest)), (0, 1), "the hopper transferred in the freeze");
        assert_eq!(w.get(cp), w2.get(cp), "the crafter ticked as in a normal tick");
        assert!(!w.get(cp).to_string().contains("crafting=true"), "it finished");
    }

    /// The held check (G1's note (a), ironspec 22:16 (2a)): an entity there before the load ticks and not
    /// held in them ticked (an error naming it); one created in them (a dispenser's output, a drop) is
    /// listed, exempt; one a block removed (a hopper taking an item) is just gone.
    #[test]
    fn load_held_exempts_created() {
        let set = |v: &[u32]| v.iter().copied().collect::<std::collections::HashSet<u32>>();
        assert_eq!(load_held(&set(&[1, 2, 3]), &[1, 2, 3], &set(&[1, 2, 3])), Ok((3, vec![])));
        assert_eq!(load_held(&set(&[1, 2, 3]), &[1, 3, 9], &set(&[1, 2, 3])), Ok((2, vec![9])), "2 removed by a block, 9 created");
        assert_eq!(load_held(&set(&[1, 2, 3]), &[1, 2, 3], &set(&[1, 3])), Err(vec![2]), "2 ticked");
    }
}
