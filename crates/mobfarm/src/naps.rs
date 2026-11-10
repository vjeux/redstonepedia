//! The sleep routine's nightly dispatch on the bench (the brief's part 3a, the day-1 lane's
//! scheduler): the yard's controls driven by time and by the bed's observer only, as the circuit
//! will drive them (the interface agreed with day-1 on 2026-11-10; ironspec's 4-advance ruling):
//! - inside the deep night (day time 13,670..22,330: the window's edges rain and thunder don't
//!   move), every 4th line fall from the window's first (4 and 7 coprime: 21 visits give each
//!   batch 3) the batch whose line just fell sends its rider of level (visit index div 7) (the
//!   stand-in order: day-1's circuit may use another, any order works here);
//! - at that fall T: the napper's hump (its hold control) pulsed T..T+100 and the activator
//!   powered T..T+170; the junctions and merges set for its lane and level at T+40 (the previous
//!   visit's return, if still out, has cleared them by then: a hold if not);
//! - the observer under the bed's head: OCCUPIED rising shuts the hatch (the dorm zombie seen),
//!   falling opens it and powers the park rail for 20 gt from 60 gt later (the boarding cooldown
//!   past; inside the 100 gt of LAST_WOKEN).
//! The bench watches each visit (dropped, asleep, woken, riding again, back past the junctions,
//! parked) for its length and its problems, each napper's HOME release and re-take, and each
//! villager's nap interval (the golem condition: under 24,000 gt).

use crate::hold::LaneOpts;
use crate::yard::{Stage, Visit, Yard};
use redsim::pos::BlockPos;
use std::collections::BTreeMap;

/// The night's dispatch and the yard's timed controls.
pub struct Naps {
    pub yard: Yard,
    pub lanes: LaneOpts,
    /// Each rider: (id, lane, level).
    pub riders: Vec<(u32, i32, i32)>,
    /// The visits out (dispatched, not yet parked back and panicking again): the next one may
    /// start while the previous one's return is still out.
    pub out: Vec<Visit>,
    /// This night (day time - 12000, / 24000), its line falls in the window, its visits.
    pub night: i64,
    pub falls: u32,
    pub dispatched: u32,
    /// The interface's timings: the dispatch window (day time), a visit every this many line
    /// falls in it, the route set this long after the dispatch, the hump's pulse, the
    /// activator's, the park rail's delay after the wake and its pulse, the visit's budget (its
    /// cart parked again by T + budget).
    pub window: (i64, i64),
    pub every: u32,
    pub route_after: i64,
    pub hump_for: i64,
    pub activator_for: i64,
    pub park_after: i64,
    pub park_for: i64,
    pub budget: i64,
    pending: Vec<(i64, BlockPos, &'static str)>,
    routes: Vec<(i64, i32, i32, u32)>,
    occupied: bool,
    /// Finished visits' lengths (T to the cart parked again), the junctions clear of each return
    /// (T to its cart back on its own extension), the log, problems.
    pub lengths: Vec<i64>,
    pub clear: Vec<i64>,
    pub log: Vec<String>,
    pub problems: Vec<String>,
    /// Routes set while another visit's return had not cleared the junctions (the scheduler
    /// would hold them).
    pub holds: u32,
    /// Each napper's HOME release in the yard: gt to its re-take and whether it was riding at
    /// the release; releases never re-taken during the visit.
    pub retakes: Vec<(i64, bool)>,
    pub lost_homes: u32,
    released: BTreeMap<u32, (i64, bool)>,
    /// Each villager's last sleep start, and the longest interval between two of a villager's.
    pub last_nap: BTreeMap<u32, i64>,
    pub worst_gap: i64,
    /// Print this napper's visit every tick (stderr).
    pub trace: Option<u32>,
}

fn block(s: &str) -> redsim::block::Block {
    redsim::block::Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

fn set(w: &mut redsim::world::World, p: BlockPos, s: &str) {
    w.set_block(p, block(s), redsim::world::flags::ALL);
}

/// The villager's LAST_SLEPT if it was set at or after `since` (a nap started then).
fn brain_slept_since(w: &redsim::world::World, i: usize, since: i64) -> Option<i64> {
    redsim::entity::mob::ai::brain::brain(w, i).last_slept.filter(|&s| s >= since)
}

impl Naps {
    pub fn new(yard: Yard, lanes: LaneOpts, riders: Vec<(u32, i32, i32)>) -> Naps {
        Naps {
            yard,
            lanes,
            riders,
            out: Vec::new(),
            night: i64::MIN,
            falls: 0,
            dispatched: 0,
            window: (13670, 22330),
            every: 4,
            route_after: 40,
            hump_for: 100,
            activator_for: 170,
            park_after: 60,
            park_for: 20,
            budget: 590,
            pending: Vec::new(),
            routes: Vec::new(),
            occupied: false,
            lengths: Vec::new(),
            clear: Vec::new(),
            log: Vec::new(),
            problems: Vec::new(),
            holds: 0,
            retakes: Vec::new(),
            lost_homes: 0,
            released: BTreeMap::new(),
            last_nap: BTreeMap::new(),
            worst_gap: 0,
            trace: None,
        }
    }

    /// The nappers out (exempt from the riders' checks until parked and panicking again).
    pub fn active(&self) -> Vec<u32> {
        self.out.iter().map(|v| v.id).collect()
    }

    fn index_of(w: &redsim::world::World, id: u32) -> Option<usize> {
        w.entities.list.iter().position(|e| e.id == id)
    }

    /// The junctions and merges set for a napper of `level` from lane `lane`.
    fn route(&self, w: &mut redsim::world::World, lane: i32, level: i32) {
        for (p, s) in self.yard.junctions[level as usize].iter().zip(self.yard.junction_states(lane)) {
            if let Some(on) = s {
                set(w, *p, if on { "redstone_block" } else { "target[power=0]" });
            }
        }
        let (m1, m2) = match level {
            0 => (true, true),
            1 => (false, true),
            _ => (false, false),
        };
        set(w, self.yard.merges[0], if m1 { "redstone_block" } else { "target[power=0]" });
        set(w, self.yard.merges[1], if m2 { "redstone_block" } else { "target[power=0]" });
    }

    /// A batch's line just fell (its riders come back): every `every`th fall inside the window
    /// dispatches one of them.
    pub fn on_line_fall(&mut self, w: &mut redsim::world::World, lane: i32, t: i64) {
        let dt = w.day_time();
        let tod = dt.rem_euclid(24000);
        let night = (dt - 12000).div_euclid(24000);
        if night != self.night {
            self.night = night;
            self.falls = 0;
            self.dispatched = 0;
        }
        if tod < self.window.0 || tod >= self.window.1 {
            return;
        }
        let k = self.falls;
        self.falls += 1;
        if k % self.every != 0 || self.dispatched >= 21 {
            return;
        }
        let level = (self.dispatched / 7) as i32;
        self.dispatched += 1;
        let Some(&(id, _, _)) = self.riders.iter().find(|r| r.1 == lane && r.2 == level) else {
            self.problems.push(format!("gt {t}: no rider at lane {lane} level {level}"));
            return;
        };
        if let Some(v) = self.out.iter().find(|v| v.id == id) {
            self.problems.push(format!("gt {t}: napper {id} dispatched again while still out (since gt {})", v.start));
            return;
        }
        let (hold, on, off) = self.lanes.hold_input(lane, level);
        set(w, hold, on);
        self.pending.push((t + self.hump_for, hold, off));
        set(w, self.yard.activator, "redstone_block");
        self.pending.push((t + self.activator_for, self.yard.activator, "glass"));
        self.routes.push((t + self.route_after, lane, level, id));
        let cart = Naps::index_of(w, id).and_then(|i| w.entities.list[i].vehicle);
        self.out.push(Visit { id, cart, lane, level, stage: Stage::Out, start: t, at: vec![(Stage::Out, t)], parked: None, clear: None });
    }

    /// Every tick: the timed controls due, the bed observer's edges, the visits watched.
    pub fn tick(&mut self, w: &mut redsim::world::World, t: i64) {
        let due: Vec<(i64, BlockPos, &'static str)> = self.pending.iter().filter(|p| p.0 <= t).cloned().collect();
        self.pending.retain(|p| p.0 > t);
        for (_, p, s) in due {
            set(w, p, s);
        }
        let routes: Vec<(i64, i32, i32, u32)> = self.routes.iter().filter(|r| r.0 <= t).cloned().collect();
        self.routes.retain(|r| r.0 > t);
        for (_, lane, level, id) in routes {
            if let Some(v) = self.out.iter().find(|v| v.id != id && v.clear.is_none()) {
                self.holds += 1;
                self.problems.push(format!("gt {t}: napper {id}'s route set while napper {}'s (dispatched gt {}, {:?}) is still in the junctions", v.id, v.start, v.stage));
            }
            self.route(w, lane, level);
        }
        let occupied = matches!(w.get(self.yard.bed_head), redsim::block::Block::Bed { occupied: true, .. });
        if occupied && !self.occupied {
            set(w, self.yard.hatch.below(), "glass");
        }
        if !occupied && self.occupied {
            set(w, self.yard.hatch.below(), "redstone_block");
            self.pending.push((t + self.park_after, self.yard.park, "redstone_block"));
            self.pending.push((t + self.park_after + self.park_for, self.yard.park, "glass"));
        }
        self.occupied = occupied;
        let out = std::mem::take(&mut self.out);
        for v in out {
            if let Some(v) = self.watch(w, t, v) {
                self.out.push(v);
            }
        }
    }

    /// One visit's tick: its stages, its HOME, its clear and parked times; `None` once it is over.
    fn watch(&mut self, w: &mut redsim::world::World, t: i64, mut v: Visit) -> Option<Visit> {
        let Some(i) = Naps::index_of(w, v.id) else {
            self.problems.push(format!("gt {t}: napper {} gone", v.id));
            return None;
        };
        let e = &w.entities.list[i];
        let (pos, riding) = (e.pos, e.vehicle.is_some());
        let asleep = redsim::entity::mob::mob(w, i).sleeping_pos.is_some();
        let hz = self.lanes.hold_slope_z(v.lane, v.level);
        let home = redsim::entity::mob::ai::brain::brain(w, i).home;
        if self.trace == Some(v.id) {
            let cart = v.cart.and_then(|c| w.entities.list.iter().find(|x| x.id == c)).map(|c| (c.pos, c.delta_movement));
            let b = redsim::entity::mob::ai::brain::brain(w, i);
            let act = redsim::entity::mob::ai::brain::has_exact(w, i).then(|| {
                let x = redsim::entity::mob::ai::brain::ex(w, i);
                format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since)
            });
            eprintln!(
                "yard gt {t}: napper {} {:?} at {:.2} {:.2} {:.2} riding {riding} asleep {asleep} cart {:?} | {:?} home {:?} hostile {:?} woken {:?} bed {:?}",
                v.id,
                v.stage,
                pos.x,
                pos.y,
                pos.z,
                cart.map(|(p, d)| {
                    // (the rail cell under the cart and the next one along its motion)
                    let c = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                    let n = BlockPos::new(c.x, c.y, c.z + if d.z < 0.0 { -1 } else { 1 });
                    format!("{:.2} {:.2} {:.2} d {:.3} {:.3} on {} next {}", p.x, p.y, p.z, d.x, d.z, w.get(c), w.get(n))
                }),
                act,
                b.home,
                b.nearest_hostile,
                b.last_woken,
                w.get(self.yard.bed_head)
            );
        }
        // HOME released in the yard (the first REST tick with an old CANT_REACH) and re-taken
        match (home, self.released.get(&v.id).copied()) {
            (None, None) => {
                self.released.insert(v.id, (t, riding));
            }
            (Some(_), Some((at, rode))) => {
                self.released.remove(&v.id);
                self.retakes.push((t - at, rode));
            }
            _ => {}
        }
        // every transition the tick allows (a one-tick nap: dropped, asleep and woken between two
        // of the bench's looks shows as a LAST_SLEPT set since the dispatch)
        let slept = brain_slept_since(w, i, v.start);
        loop {
            let next = match v.stage {
                Stage::Out => (!riding).then_some(Stage::AtBed),
                Stage::AtBed => (asleep || slept.is_some()).then_some(Stage::Asleep),
                Stage::Asleep => (!asleep).then_some(Stage::Woken),
                Stage::Woken => riding.then_some(Stage::Back),
                Stage::Back => None,
            };
            let Some(s) = next else { break };
            if s == Stage::Asleep {
                let at = slept.unwrap_or(t);
                if let Some(prev) = self.last_nap.insert(v.id, at) {
                    self.worst_gap = self.worst_gap.max(at - prev);
                }
            }
            v.stage = s;
            v.at.push((s, t));
        }
        let in_lane = (pos.x - (self.lanes.x(v.lane) as f64 + 0.5)).abs() < 0.3;
        if v.stage == Stage::Back && v.clear.is_none() && riding && in_lane && pos.z < (self.lanes.run + 4) as f64 - 0.5 {
            v.clear = Some(t);
            self.clear.push(t - v.start);
        }
        if v.stage == Stage::Back && v.parked.is_none() {
            let parked = riding && in_lane && pos.z > hz as f64 && pos.z < hz as f64 + 2.0;
            let still = e.vehicle.and_then(|c| w.entities.list.iter().find(|x| x.id == c)).is_some_and(|c| c.delta_movement.x.abs() + c.delta_movement.z.abs() < 0.01);
            if parked && still {
                v.parked = Some(t);
                self.lengths.push(t - v.start);
                self.log.push(format!("gt {t}: napper {} (lane {} level {}) home after {} gt (junctions clear at {:?}): {:?}", v.id, v.lane, v.level, t - v.start, v.clear.map(|c| c - v.start), v.at.iter().map(|(s, x)| (*s, x - v.start)).collect::<Vec<_>>()));
            }
        }
        if v.parked.is_none() && t > v.start + self.budget {
            self.problems.push(format!("gt {t}: napper {} (lane {} level {}) not parked back {} gt after its dispatch: {:?} at {:.2} {:.2} {:.2} (riding {riding}, asleep {asleep})", v.id, v.lane, v.level, self.budget, v.stage, pos.x, pos.y, pos.z));
            self.forget(v.id);
            return None;
        }
        // (the visit lasts until the napper panics again in the hold: every villager but the
        // napper panics at every night tick)
        if v.parked.is_some() && redsim::entity::mob::ai::brain::has_exact(w, i) && redsim::entity::mob::ai::brain::ex(w, i).active == redsim::entity::mob::ai::brain::Act::Panic {
            self.forget(v.id);
            return None;
        }
        Some(v)
    }

    /// A visit over: a HOME still released counts as lost.
    fn forget(&mut self, id: u32) {
        if self.released.remove(&id).is_some() {
            self.lost_homes += 1;
        }
    }
}
