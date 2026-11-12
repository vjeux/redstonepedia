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
}

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
    /// detector rail, world coordinates) or `virtual:X,Z[,PHASE]` (world coordinates); else the
    /// `fallback` the bench gives (a virtual ring in its column's chunk).
    pub fn from_config(fallback: Ring) -> Self {
        let ring = std::env::var("MCTAS_ALIVE_RING").ok().and_then(|v| {
            let (kind, rest) = v.split_once(':')?;
            let n: Vec<f64> = rest.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            match (kind, n.as_slice()) {
                ("probe", [x, y, z]) => Some(Ring::Probe(BlockPos::new(*x as i32, *y as i32, *z as i32))),
                ("virtual", [x, z]) => Some(Ring::Virtual { x: *x, z: *z, period: 30, high: 20, phase: 0 }),
                ("virtual", [x, z, p]) => Some(Ring::Virtual { x: *x, z: *z, period: 30, high: 20, phase: *p as i64 }),
                _ => None,
            }
        });
        Liveness::new(ring.unwrap_or(fallback))
    }

    /// The ring's label for the report.
    pub fn ring_label(&self) -> String {
        match &self.ring {
            Ring::Probe(p) => format!("the ring's detector rail at {},{},{} (probed)", p.x, p.y, p.z),
            Ring::Virtual { x, z, period, high, phase } => format!("a virtual sealed ring in the chunk of {x:.1},{z:.1} (period {period}, high {high}, phase {phase}; its lap advances only while that chunk ticks entities)"),
        }
    }

    fn ring_on(&mut self, w: &World) -> bool {
        match self.ring.clone() {
            Ring::Probe(p) => w.get(p).to_string().contains("powered=true"),
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
            Ring::Virtual { x, z, .. } => (*x, *z),
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
}
