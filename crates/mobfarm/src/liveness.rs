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
    /// A save/stop/start between two ticks, load tick 1 (the official load's held attempts): every chunk
    /// of the part saved and unloaded (`redsim::reload::unload_all`) and loaded again
    /// (`load_all_ordered`) in this entity order: the scheduled block and fluid ticks back with their
    /// remaining delays, the block entities registered again, every entity back as a new object under a
    /// new id with its motion, passengers and codec'd memories, its uncodec'd memories lost and its
    /// sensor timers redrawn, as redsim's bridged load models them; then the load's block-only tick (game
    /// time + 1, blocks and redstone ticking, no entity moving: [`restart`]). (Block events never cross a
    /// tick boundary: none is pending between two ticks.)
    Load(redsim::reload::Order),
    /// The same load with load tick 0 (the official load's moved attempts, tick-exact; ironspec 21:47
    /// (2)): no block-only tick, so the first tick after the load ticks the entities too; every reload
    /// effect on the entities kept (the new objects, the dry first tick, the unsaved memories, the
    /// re-phased triggers).
    Load0(redsim::reload::Order),
}

impl Restart {
    /// `save`, `reverse` or `seed:K` (K < 1000), load tick 1; with `/t0` load tick 0 (`/t1` names 1).
    pub fn parse(s: &str) -> Option<Restart> {
        let s = s.trim();
        let (o, tick) = match s.rsplit_once('/') {
            Some((o, "t0")) => (o, 0),
            Some((o, "t1")) => (o, 1),
            Some(_) => return None,
            None => (s, 1),
        };
        redsim::reload::Order::parse(o.trim()).map(|o| if tick == 0 { Restart::Load0(o) } else { Restart::Load(o) })
    }

    /// The row's entity load order.
    pub fn order(&self) -> redsim::reload::Order {
        match *self {
            Restart::Load(o) | Restart::Load0(o) => o,
        }
    }

    /// The load's block-only ticks: 1 or 0 (ironspec 21:47 (2): `load_tick: 0 | 1`).
    pub fn load_tick(&self) -> i64 {
        match self {
            Restart::Load(_) => 1,
            Restart::Load0(_) => 0,
        }
    }

    pub fn label(&self) -> String {
        let o = match self.order() {
            redsim::reload::Order::Save => "save".to_string(),
            redsim::reload::Order::Reverse => "reverse".to_string(),
            redsim::reload::Order::Seed(k) => format!("seed:{k}"),
        };
        if self.load_tick() == 0 { format!("{o}/t0") } else { o }
    }

    /// The rows a judge runs at every phase: the save order, its reverse and 8 seeded orders
    /// (ironspec's reload-order rule), each with load tick 1 and with load tick 0 (ironspec 21:47 (2):
    /// every restart row set runs both branches, crossed with the orders).
    pub fn rows() -> Vec<Restart> {
        let mut orders = vec![redsim::reload::Order::Save, redsim::reload::Order::Reverse];
        orders.extend((1..=8).map(redsim::reload::Order::Seed));
        orders.iter().map(|o| Restart::Load(*o)).chain(orders.iter().map(|o| Restart::Load0(*o))).collect()
    }
}

/// `--restart T,ORDER` (repeatable in a bench's CLI): the tick and the [`Restart`].
pub fn parse_restart(s: &str) -> Result<(i64, Restart), String> {
    let (t, how) = s.split_once(',').ok_or(format!("--restart {s}: T,ORDER (ORDER save, reverse or seed:K, /t0 for load tick 0)"))?;
    let t = t.trim().parse::<i64>().map_err(|e| format!("--restart {s}: {e}"))?;
    let how = Restart::parse(how).ok_or(format!("--restart {s}: ORDER is save, reverse or seed:K (/t0: load tick 0)"))?;
    Ok((t, how))
}

/// What a [`restart`] did (ironspec 21:13: nothing silently dropped): the entities' new ids (old -> new;
/// it reads as that map), the game time after it (`at`: the load's block-only tick's, or the load's own
/// with load tick 0), the entity-free ticks the restart itself ran (`entity_free`: its load tick, 1 or 0,
/// measured as the ticks run with no entity ticking) and the entities that tick held (`held`: each noted
/// frozen at `at` in `World::frozen`, kept there, so a freeze that goes on from the load tick reads as
/// starting on it).
#[derive(Clone, Debug, Default)]
pub struct Restarted {
    pub ids: std::collections::HashMap<u32, u32>,
    pub at: i64,
    pub entity_free: i64,
    pub held: usize,
    /// The world has a Nether part (`World::nether`): the reload model doesn't save it, so it ticks normally
    /// through the load tick and keeps its in-memory state; the row counts as unmodelled until the reload
    /// model saves and loads both dimensions (ironspec 21:23/21:24: said in the note, not silent).
    pub nether: bool,
}

impl std::ops::Deref for Restarted {
    type Target = std::collections::HashMap<u32, u32>;
    fn deref(&self) -> &Self::Target {
        &self.ids
    }
}

impl Restarted {
    /// The row's entity-free ticks with an `f`-gt freeze going on from the load tick (the restart x
    /// 1..4-gt freeze rows: a load longer than 1 tick): the load's 1 (or 0) plus `f`.
    pub fn with_freeze(&self, f: i64) -> i64 {
        self.entity_free + f.max(0)
    }

    /// The text every restart row prints (ironspec 21:13): `entity-free ticks N (the load's block-only
    /// tick at gt T holding H entities[ + an F-gt freeze from it])`, with load tick 0 `entity-free ticks
    /// N (load tick 0 at gt T: no block-only tick[ + …])`, and with a Nether part `; Nether restart not
    /// modelled in redsim (…): this row UNMODELLED` (ironspec 21:24).
    pub fn note(&self, f: i64) -> String {
        let load = if self.entity_free == 0 { format!("load tick 0 at gt {}: no block-only tick", self.at) } else { format!("the load's block-only tick at gt {} holding {} entities", self.at, self.held) };
        format!(
            "entity-free ticks {} ({load}{}){}",
            self.with_freeze(f),
            if f > 0 { format!(" + a {f}-gt freeze from it") } else { String::new() },
            if self.nether { "; Nether restart not modelled in redsim (the Nether part ticked normally through the load tick and kept its in-memory state): this row UNMODELLED" } else { "" }
        )
    }
}

/// Restart world `w` between two ticks ([`Restart`]): the official load (tick-exact, farmopt 2026-11-12
/// 21:07; ironspec 20:35 (2a), accepted 21:13): the world saved and stopped, then loaded in `how`'s entity
/// order, then with load tick 1 ([`Restart::Load`]) one block-only tick (the loaded world ticks once before
/// its entities do: game time + 1, the scheduled ticks, block events and redstone run, no entity moves),
/// with load tick 0 ([`Restart::Load0`]) none (ironspec 21:47 (2): officially 0 or 1), the load order kept
/// for the ticks after. Returns [`Restarted`]: the entities' new ids (old -> new) for the bench to follow its carts and
/// riders by (any id since also follows through [`follow`]) and the entity-free ticks every restart row
/// prints ([`Restarted::note`]); an error when the reload model refuses the world (a player, falling
/// blocks: `w.errors`) or a partial unload is active (the block-only tick needs every chunk).
pub fn restart(w: &mut World, how: Restart) -> Result<Restarted, String> {
    let errors = w.errors.len();
    let before: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
    redsim::reload::unload_all(w);
    redsim::reload::load_all_ordered(w, how.order());
    if w.errors.len() > errors {
        return Err(format!("restart {}: {}", how.label(), w.errors[errors..].join("; ")));
    }
    let ids = |w: &World| before.iter().map(|id| (*id, follow(w, *id))).collect();
    if how.load_tick() == 0 {
        return Ok(Restarted { ids: ids(w), at: w.time, entity_free: 0, held: 0, nether: w.nether.is_some() });
    }
    if !w.unforced.is_empty() {
        return Err(format!("restart {}: a partial unload is active; the load's block-only tick needs every chunk loaded", how.label()));
    }
    // (the block-only tick: an empty entity-ticking range, so `entity_ticks_at` is false everywhere and no
    // entity ticks, its place in the list kept; the freezes it notes stay in `World::frozen`, counted)
    let (ec, frozen) = (w.entity_chunks, w.frozen.len());
    w.entity_chunks = Some([1, 1, 0, 0]);
    w.tick();
    w.entity_chunks = ec;
    if w.errors.len() > errors {
        return Err(format!("restart {}: the load's block-only tick: {}", how.label(), w.errors[errors..].join("; ")));
    }
    // (measured, not assumed: every entity alive after the tick was held in it (noted frozen at its game
    // time; a block may still remove one, a hopper taking an item); one that ticked would make it no
    // block-only tick)
    let at = w.time;
    let noted: std::collections::HashSet<u32> = w.frozen[frozen..].iter().filter(|(t, _)| *t == at).map(|(_, id)| *id).collect();
    let ticked: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed && !noted.contains(&e.id)).map(|e| e.id).collect();
    if !ticked.is_empty() {
        return Err(format!("restart {}: the load's block-only tick ticked entities {ticked:?}", how.label()));
    }
    Ok(Restarted { ids: ids(w), at, entity_free: 1, held: noted.len(), nether: w.nether.is_some() })
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
        let ids = restart(&mut w, Restart::Load(redsim::reload::Order::Reverse)).unwrap();
        let now = follow(&w, cart);
        assert_ne!(now, cart);
        assert_eq!(ids.get(&cart), Some(&now));
        assert!(live_cart(&w, now));
        assert_eq!(w.ticks.count(), n0, "a save/stop/start keeps the pending tick");
        restart(&mut w, Restart::Load(redsim::reload::Order::Seed(2))).unwrap();
        let again = follow(&w, cart);
        assert_ne!(again, now, "a second restart renumbers again");
        assert!(live_cart(&w, again), "the first id follows to a live minecart");
        assert_eq!(follow(&w, now), again, "the id between follows to the same one");
        assert_eq!(w.ticks.count(), n0);
        // (both branches, crossed with the orders (ironspec 21:47 (2)): 10 orders x load tick 1 and 0)
        let rows = Restart::rows();
        assert_eq!(rows.len(), 20);
        assert_eq!((rows.iter().filter(|r| r.load_tick() == 1).count(), rows.iter().filter(|r| r.load_tick() == 0).count()), (10, 10));
        for r in rows {
            assert_eq!(Restart::parse(&r.label()), Some(r));
        }
        assert_eq!(parse_restart("120,seed:3"), Ok((120, Restart::Load(redsim::reload::Order::Seed(3)))));
        assert_eq!(parse_restart("120,seed:3/t1"), Ok((120, Restart::Load(redsim::reload::Order::Seed(3)))));
        assert_eq!(parse_restart("120,reverse/t0"), Ok((120, Restart::Load0(redsim::reload::Order::Reverse))));
        assert_eq!(Restart::Load0(redsim::reload::Order::Save).label(), "save/t0");
        assert!(parse_restart("120,cold").is_err() && parse_restart("120,save/t2").is_err());
    }

    /// The official load (tick-exact, farmopt 21:07): a restart is one tick for the blocks and none for the
    /// entities. Against a clone of the same world given one normal tick: the same game time, the same block
    /// states and pending ticks (a repeater powered from behind on its way on), while the moving cart holds
    /// its place across the load (the clone's cart moves), its freeze at the load tick counted and kept
    /// (ironspec 21:13); the next ticks keep the blocks in lockstep with the clone, and move the cart again.
    #[test]
    fn restart_is_a_block_only_tick() {
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
        let mut w2 = w.clone();
        w2.tick();
        let (t0, frozen0) = (w.time, w.frozen.len());
        let p0 = w.entities.list[i].pos;
        let r = restart(&mut w, Restart::Load(redsim::reload::Order::Save)).unwrap();
        let now = follow(&w, cart);
        let pos_of = |w: &World, id: u32| w.entities.list[redsim::entity::index_of(w, id).unwrap()].pos;
        assert_eq!(w.time, t0 + 1, "a load advances game time by exactly 1");
        assert_eq!(w.time, w2.time);
        assert_eq!(w.get(rp), w2.get(rp), "the blocks ticked as in a normal tick");
        assert_eq!(w.ticks.count(), w2.ticks.count(), "the same pending ticks as after a normal tick");
        let (p1, q1) = (pos_of(&w, now), pos_of(&w2, cart));
        assert!((p1.x - p0.x).abs() < 1e-9 && (p1.y - p0.y).abs() < 1e-9 && (p1.z - p0.z).abs() < 1e-9, "the moving cart held across the load: {p0:?} -> {p1:?}");
        assert!(q1.x > p0.x, "the clone's cart moved in its normal tick: {p0:?} -> {q1:?}");
        // (counted, not dropped (ironspec 21:13): the cart noted frozen at the load tick and kept; the row's
        // entity-free ticks: the load's 1, plus an F-gt freeze from it)
        assert_eq!((r.at, r.entity_free, r.held), (t0 + 1, 1, 1));
        assert_eq!(w.frozen[frozen0..].to_vec(), vec![(t0 + 1, now)], "the load tick's freeze kept in World::frozen");
        assert_eq!(r.get(&cart), Some(&now), "it reads as the id map");
        assert_eq!(r.note(0), format!("entity-free ticks 1 (the load's block-only tick at gt {} holding 1 entities)", t0 + 1));
        assert_eq!((r.with_freeze(3), r.note(3).starts_with("entity-free ticks 4 ")), (4, true));
        assert!(r.note(3).ends_with(" + a 3-gt freeze from it)"), "{}", r.note(3));
        // (a world with a Nether part: the note says the row is unmodelled (ironspec 21:24))
        let mut wn = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        wn.nether = Some(Box::new(World::new(BlockPos::new(-8, -4, -8), [16, 8, 16])));
        let rn = restart(&mut wn, Restart::Load(redsim::reload::Order::Save)).unwrap();
        assert!(rn.nether && rn.note(0).contains("; Nether restart not modelled in redsim (") && rn.note(0).ends_with(": this row UNMODELLED"), "{}", rn.note(0));
        assert!(!r.nether && !r.note(0).contains("Nether"));
        let mut on = false;
        for _ in 0..6 {
            w.tick();
            w2.tick();
            assert_eq!(w.get(rp), w2.get(rp), "the blocks stay in lockstep with the clone");
            on |= w.get(rp).to_string().contains("powered=true");
        }
        assert!(on, "the repeater turned on: the block side did tick");
        assert!(pos_of(&w, now).x > p1.x, "the next normal ticks move the cart again");
    }

    /// Load tick 0 (ironspec 21:47 (2), the official load's moved attempts): no block-only tick: the game
    /// time stays, the blocks don't tick (the repeater's turn-on still pending as before the restart), the
    /// entities come back renumbered (the reload's effects kept) and the first tick after the load moves
    /// the cart; its note reads 0 entity-free ticks.
    #[test]
    fn restart_load_tick_0() {
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
        let (t0, n0, s0, frozen0) = (w.time, w.ticks.count(), w.get(rp), w.frozen.len());
        let p0 = w.entities.list[i].pos;
        let r = restart(&mut w, Restart::Load0(redsim::reload::Order::Save)).unwrap();
        let now = follow(&w, cart);
        assert_ne!(now, cart, "the reload's effects kept: a new object under a new id");
        assert_eq!((w.time, w.ticks.count(), w.get(rp), w.frozen.len()), (t0, n0, s0, frozen0), "no block-only tick: time, pending ticks and blocks as before");
        assert_eq!((r.at, r.entity_free, r.held), (t0, 0, 0));
        assert_eq!(r.note(0), format!("entity-free ticks 0 (load tick 0 at gt {t0}: no block-only tick)"));
        assert_eq!(r.with_freeze(2), 2);
        w.tick();
        let p1 = w.entities.list[redsim::entity::index_of(&w, now).unwrap()].pos;
        assert!(p1.x > p0.x, "the first tick after a load-tick-0 restart moves the cart: {p0:?} -> {p1:?}");
    }
}
