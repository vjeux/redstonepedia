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
/// pass to [`thaw32`].
pub fn freeze32(w: &mut World) -> [i32; 4] {
    if w.entity_chunks.is_none() {
        let (mn, sz) = (w.min, w.size);
        w.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
    }
    let pb = w.entity_chunks.unwrap_or([0; 4]);
    w.entity_chunks = Some([pb[0] - 1, pb[1] - 1, pb[2] + 1, pb[3] + 1]);
    redsim::reload::unload_some(w, pb);
    // (G1: a part box 3 or more chunks wide on both axes would leave its middle at level 33, where
    // nothing runs: the freeze must read 32 on every part chunk, or the bench is not freezing what it
    // says)
    let (lo, hi) = levels(w, pb);
    assert!(lo == 32 && hi == 32, "freeze32: the part's chunks read levels {lo}..{hi}, not 32 (box {pb:?})");
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
/// [`restart`]; the judges drive it at every phase in each of [`Restart::rows`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    /// A save/stop/start between two ticks: every chunk of the part saved and unloaded
    /// (`redsim::reload::unload_all`) and loaded again (`load_all_ordered`) in this entity order
    /// before the next tick (no game time passes): the scheduled ticks back with their delays, the
    /// block entities registered again, every entity back as a new object under a new id, its
    /// stamps and sensor phases reset as redsim's load models them.
    Load(redsim::reload::Order),
    /// The cold start: a save/stop/start in the save order with nothing in flight surviving (the
    /// pending scheduled ticks and block events dropped): the farm as built, started at this phase.
    /// The bench starts its own controller (a bench-driven gate's timers and latches) again at its
    /// initial state.
    Cold,
}

impl Restart {
    /// `save`, `reverse`, `seed:K` (K < 1000) or `cold`.
    pub fn parse(s: &str) -> Option<Restart> {
        if s.trim() == "cold" {
            return Some(Restart::Cold);
        }
        redsim::reload::Order::parse(s.trim()).map(Restart::Load)
    }

    pub fn label(&self) -> String {
        match self {
            Restart::Load(redsim::reload::Order::Save) => "save".into(),
            Restart::Load(redsim::reload::Order::Reverse) => "reverse".into(),
            Restart::Load(redsim::reload::Order::Seed(k)) => format!("seed:{k}"),
            Restart::Cold => "cold".into(),
        }
    }

    /// The rows a judge runs at every phase: the save order, its reverse, 8 seeded orders
    /// (ironspec's reload-order rule) and the cold start.
    pub fn rows() -> Vec<Restart> {
        let mut v = vec![Restart::Load(redsim::reload::Order::Save), Restart::Load(redsim::reload::Order::Reverse)];
        v.extend((1..=8).map(|k| Restart::Load(redsim::reload::Order::Seed(k))));
        v.push(Restart::Cold);
        v
    }
}

/// `--restart T,ORDER` (repeatable in a bench's CLI): the tick and the [`Restart`].
pub fn parse_restart(s: &str) -> Result<(i64, Restart), String> {
    let (t, how) = s.split_once(',').ok_or(format!("--restart {s}: T,ORDER (ORDER save, reverse, seed:K or cold)"))?;
    let t = t.trim().parse::<i64>().map_err(|e| format!("--restart {s}: {e}"))?;
    let how = Restart::parse(how).ok_or(format!("--restart {s}: ORDER is save, reverse, seed:K or cold"))?;
    Ok((t, how))
}

/// Restart world `w` between two ticks ([`Restart`]). Returns the entities' new ids (old -> new) for
/// the bench to follow its carts and riders by (any id since also follows through [`follow`]); an
/// error when the reload model refuses the world (a player, falling blocks: `w.errors`).
pub fn restart(w: &mut World, how: Restart) -> Result<std::collections::HashMap<u32, u32>, String> {
    let errors = w.errors.len();
    let before: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
    redsim::reload::unload_all(w);
    if how == Restart::Cold {
        if let Some(a) = w.away.as_mut() {
            a.ticks.clear();
            a.fluid_ticks.clear();
        }
        w.events = Default::default();
    }
    let order = match how {
        Restart::Load(o) => o,
        Restart::Cold => redsim::reload::Order::Save,
    };
    redsim::reload::load_all_ordered(w, order);
    if w.errors.len() > errors {
        return Err(format!("restart {}: {}", how.label(), w.errors[errors..].join("; ")));
    }
    Ok(before.into_iter().map(|id| (id, follow(w, id))).collect())
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

    /// A restart between two ticks: the cart back under a new id that `follow` finds, the rows' orders
    /// parsed back from their labels; a save/stop/start keeps a pending scheduled tick, the cold start
    /// drops it.
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
        let ids = restart(&mut w, Restart::Load(redsim::reload::Order::Reverse)).unwrap();
        let now = follow(&w, cart);
        assert_ne!(now, cart);
        assert_eq!(ids.get(&cart), Some(&now));
        assert!(w.entities.list.iter().any(|e| e.id == now && !e.removed));
        assert_eq!(w.ticks.count(), n0, "a save/stop/start keeps the pending tick");
        restart(&mut w, Restart::Cold).unwrap();
        assert_eq!(w.ticks.count(), 0, "the cold start drops it");
        assert_ne!(follow(&w, cart), now, "a second restart renumbers again; the first id follows");
        let rows = Restart::rows();
        assert_eq!(rows.len(), 11);
        for r in rows {
            assert_eq!(Restart::parse(&r.label()), Some(r));
        }
        assert_eq!(parse_restart("120,seed:3"), Ok((120, Restart::Load(redsim::reload::Order::Seed(3)))));
    }
}
