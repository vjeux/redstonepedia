//! A bench's drive as data (`--drive-out F` on station-bench and linear-bench; mobviewer's live
//! viewer replays it tick-exact and checks its run against the bench's): everything the bench does
//! to the world besides ticking it, and what the bench saw happen.
//!
//! Format (text, one line each; `t` = ticks after the start, the start being `Run::start` with
//! `RunOpts::steady(seed, days * 24000, WalkMode::Random)` and the AI on, settled, then the
//! design's `entity:` lines spawned in order):
//! - `act <t> <action>`: applied when the world's time is t0 + t, before its next tick:
//!   `set x y z <state>` (`World::set_block`, flags ALL), `tp eN x y z` (the cart at rest there,
//!   its passengers positioned), `home eN.p0 x y z`, `last-slept eN.p0 <t'>` (LAST_SLEPT at
//!   t0 + t'), `poi-take x y z`;
//! - `expect <t> <event>`: `golem gK x y z` (the K-th golem, its summon tick and cell),
//!   `pulse gK <length>` (the counter pulse's rise), `cross gK` (into the Nether), `nether-death
//!   gK`;
//! - `expect-end <name> <value>`.
//!
//! Entities are named by the design's entity line: `eN` is line N's entity (a vehicle when it
//! carries one), `eN.pK` its passenger K.

use redsim::world::World;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct Drive {
    pub on: bool,
    pub t0: i64,
    names: HashMap<u32, String>,
    golems: RefCell<HashMap<u32, usize>>,
    next_golem: RefCell<usize>,
    lines: RefCell<Vec<String>>,
}

impl Drive {
    pub fn new(on: bool) -> Drive {
        Drive { on, t0: 0, names: HashMap::new(), golems: RefCell::new(HashMap::new()), next_golem: RefCell::new(0), lines: RefCell::new(Vec::new()) }
    }

    /// Name the design's entities: `spawned[n]` is the id `entity::spawn` returned for line n.
    pub fn name_entities(&mut self, w: &World, spawned: &[u32], t0: i64) {
        self.t0 = t0;
        if !self.on {
            return;
        }
        for (n, id) in spawned.iter().enumerate() {
            self.names.insert(*id, format!("e{n}"));
            if let Some(e) = w.entities.list.iter().find(|e| e.id == *id) {
                for (k, p) in e.passengers.iter().enumerate() {
                    self.names.insert(*p, format!("e{n}.p{k}"));
                }
            }
        }
    }

    pub fn name(&self, id: u32) -> String {
        self.names.get(&id).cloned().unwrap_or_else(|| format!("id{id}"))
    }

    /// An action at absolute world time `now` (before its next tick).
    pub fn act(&self, now: i64, what: String) {
        if self.on {
            self.lines.borrow_mut().push(format!("act {} {what}", now - self.t0));
        }
    }

    /// A golem's index (in summon order), assigned at its first mention.
    pub fn golem(&self, id: u32) -> usize {
        let mut g = self.golems.borrow_mut();
        let n = *self.next_golem.borrow();
        if let Some(k) = g.get(&id) {
            return *k;
        }
        g.insert(id, n);
        *self.next_golem.borrow_mut() = n + 1;
        n
    }

    /// The golem's copy in the Nether (`new`) takes its overworld self's index.
    pub fn alias(&self, new: u32, old: u32) {
        let k = self.golem(old);
        self.golems.borrow_mut().insert(new, k);
    }

    pub fn expect(&self, t: i64, what: String) {
        if self.on {
            self.lines.borrow_mut().push(format!("expect {} {what}", t - self.t0));
        }
    }

    pub fn expect_end(&self, what: String) {
        if self.on {
            self.lines.borrow_mut().push(format!("expect-end {what}"));
        }
    }

    /// A line already written with its time from the drive's start (a build's `act t set ...`).
    pub fn push(&self, line: String) {
        if self.on {
            self.lines.borrow_mut().push(line);
        }
    }

    pub fn text(&self, header: &[String]) -> String {
        let mut s = String::new();
        for h in header {
            s.push_str("# ");
            s.push_str(h);
            s.push('\n');
        }
        s.push_str("# expect t: the event happened in the tick that brought the world's time to t0 + t (an act at t comes before the tick to t0 + t + 1)\n");
        // in time order (stable: an act and an expect at one t keep the bench's order), the end's
        // lines last
        let lines = self.lines.borrow();
        let key = |l: &String| -> i64 {
            let mut it = l.split_whitespace();
            match (it.next(), it.next().and_then(|t| t.parse::<i64>().ok())) {
                (Some("act" | "expect"), Some(t)) => t,
                _ => i64::MAX,
            }
        };
        let mut sorted: Vec<&String> = lines.iter().collect();
        sorted.sort_by_key(|l| key(l));
        for l in sorted {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}
