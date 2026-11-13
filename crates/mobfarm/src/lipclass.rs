//! The lip arrivals' classes (ironspec Nov 13 12:08 (3), day-1 12:09 sub-bar (iv), G1 12:12 / 12:29 (3)):
//! ONE definition for every bench and composed run (onechunk 12:47).
//!
//! A DIAGNOSTIC (G1 14:09 L-b): it names the cause class of a lip stall from the ARRIVAL TICK's
//! occupants. It is not the V-empty bar: the ruled invariant (11:08) covers the arrival's whole pass
//! window and stays a windowed check of its own; a caller must not feed this tick sample to that bar.
//!
//! A LIP ARRIVAL is the first tick a lane cart's centre is in the drop cell (x, m, mz - 1) at the lip's
//! height (y >= m - 0.05: the lip's floor m less a margin for the off-rail floor's 24.000 against the
//! rail's 24.0625), entering from C (north) or from the napper's straight (east, under
//! `--sbend-straight`). At that tick every OTHER lane cart whose centre is in the lane's band (x - 0.6
//! .. x + 1.6) has its box and its rider's box tested against the LIP VOLUME: tick-exact's lane-3
//! swept volume (its 11:18 numbers: x 1.345..4.182, y 24.0..24.7, z 5.940..8.157; y re-derived
//! end-of-tick 12:18, unchanged on C's top) taken relative to lane 3 (x 3, m 24, mz 7): x - 1.655 ..
//! x + 1.182, y m .. m + 0.7, z mz - 1.060 .. mz + 1.157.
//!
//! THE LANE BAND IS A CARVE-OUT PENDING IRONSPEC'S RULING (G1 14:09 L-d): a napper resting on R
//! (lane 3: cart box x 1.01..1.99, y 24.41..25.69 at the lowest rest z 7.343 up to 24.99..26.27 at
//! 7.922, z 6.85..8.41) DOES meet the volume (x 1.345..1.99, y 24.41..24.7, z over 6.85..8.157), as
//! does its rider; the 11:08 rule counts any other cart's box in the volume. The band keeps it out
//! because the volume is the ARRIVING cart's swept path (R -> c1 -> c2 -> c3 -> C), which the napper
//! occupies only as its own path; whether that is allowed is ironspec's call.
//!
//! The arrival takes the FIRST class that holds:
//! - `NinthCart`: 8 other carts in the lane's slots (x .. x + 1, y 15.5 .. m - 1, z 0 .. mz - 0.5:
//!   slot 1's floor 16.19 and V's top slope at z mz - 1 inside; the drop cell out): the count
//!   sub-bar (1);
//! - `Rollback`: an occupant of the lip volume that had come to REST in the slots below V (stored speed
//!   under 0.001, y under m - 1, z under mz - 1.1: V's slot centre is mz - 0.98) since its own last lip
//!   arrival: sub-bar (4);
//! - `GapUnder(g)`: the C-GAP to the lane's previous lip arrival under `gap` (C-gap = Δ - 1 with Δ =
//!   t - t_prev, tick-exact 13:44; ironspec 12:39: Δ*_worst 13 + 10 = 23, so C-gap 22 is caught and 23
//!   is clear), or an occupant that itself reached the lip within 40 gt (two Δ*_worst + its descent's
//!   ~14 gt, a still-descending leader): sub-bar (3);
//! - `Unclassified`: any other occupant (a third mechanism);
//! - `SlowEntry`: no occupant, and the arrival's MOVED step that tick (its centre's move between the
//!   two end-of-tick positions) under `slow` 0.19 (tick-exact's quantized step bracket: stalls at
//!   steps <= 0.157, falls at >= 0.207; 0.19 sits between): G1 12:29 (3);
//! - `Clear`: none of the above.
//!
//! The tracker works in whatever frame its caller observes the carts in (a bench's design frame, a
//! composed run's world frame): positions, boxes and the lane's geometry must share it.

/// One lane cart as observed at the end of a tick.
#[derive(Clone, Debug)]
pub struct CartObs {
    /// A stable id (the entity id).
    pub id: u32,
    /// The cart's centre (x, y, z).
    pub pos: (f64, f64, f64),
    /// Its horizontal STORED speed (|delta_movement|, G1 14:09 L-f); the moved step is from positions.
    pub speed: f64,
    /// Its box: x0, x1, y0, y1, z0, z1.
    pub cart_box: [f64; 6],
    /// Its rider's box, if it has one.
    pub rider_box: Option<[f64; 6]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccKind {
    /// Had rested in the slots below V since its own last lip arrival.
    Rollback,
    /// Reached the lip itself within 40 gt.
    Descending,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LipClass {
    NinthCart,
    Rollback,
    GapUnder(i64),
    Unclassified,
    SlowEntry,
    Clear,
}

impl LipClass {
    pub fn name(self) -> String {
        match self {
            LipClass::NinthCart => "a 9th cart".to_string(),
            LipClass::Rollback => "rollback".to_string(),
            LipClass::GapUnder(g) => format!("a gap under {g}"),
            LipClass::Unclassified => "UNCLASSIFIED".to_string(),
            LipClass::SlowEntry => "slow entry".to_string(),
            LipClass::Clear => "clear".to_string(),
        }
    }
    /// Whether the V-empty sub-bars explain a stall after this arrival (a 9th cart, a rollback, a
    /// short gap, a slow entry); `Unclassified` and `Clear` don't.
    pub fn explains_a_stall(self) -> bool {
        !matches!(self, LipClass::Unclassified | LipClass::Clear)
    }
}

#[derive(Clone, Debug)]
pub struct LipArrival {
    pub tick: i64,
    pub cart: u32,
    /// The other carts in the lane's slots at the arrival.
    pub others_in_slots: usize,
    /// The gt since the lane's previous lip arrival.
    pub gap: Option<i64>,
    pub occupants: Vec<(u32, OccKind)>,
    /// The arrival's horizontal step that tick (NaN without the tick before).
    pub step: f64,
    pub class: LipClass,
}

/// One lane's lip, tracked tick by tick.
pub struct LipLane {
    pub x: f64,
    pub m: f64,
    pub mz: f64,
    pub volume: [f64; 6],
    pub gap: i64,
    pub slow: f64,
    settled_below: std::collections::HashMap<u32, bool>,
    in_drop: std::collections::HashMap<u32, bool>,
    last_arrival: std::collections::HashMap<u32, i64>,
    prev: std::collections::HashMap<u32, (f64, f64, f64)>,
    last_lane_arrival: Option<i64>,
    #[allow(clippy::type_complexity)]
    open: Vec<(LipArrival, std::collections::BTreeMap<u32, OccKind>)>,
}

impl LipLane {
    /// The lane at x (its drop cell (x, m, mz - 1), C at (x, m, mz)); `volume` None = tick-exact's
    /// lane-3 swept volume relative to the lane; the gap bar 23 and the slow-entry step 0.19.
    pub fn new(x: i32, m: i32, mz: i32, volume: Option<[f64; 6]>) -> LipLane {
        let (xf, mf, zf) = (x as f64, m as f64, mz as f64);
        LipLane {
            x: xf,
            m: mf,
            mz: zf,
            volume: volume.unwrap_or([xf - 1.655, xf + 1.182, mf, mf + 0.7, zf - 1.060, zf + 1.157]),
            gap: 23,
            slow: 0.19,
            settled_below: Default::default(),
            in_drop: Default::default(),
            last_arrival: Default::default(),
            prev: Default::default(),
            last_lane_arrival: None,
            open: Vec::new(),
        }
    }

    fn meets(&self, b: &[f64; 6]) -> bool {
        let v = &self.volume;
        b[1] > v[0] && b[0] < v[1] && b[3] > v[2] && b[2] < v[3] && b[5] > v[4] && b[4] < v[5]
    }

    fn in_lane(&self, p: (f64, f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + 1.0
    }

    /// One tick's observations of the lane's carts (every cart that can reach this lip). An arrival
    /// opens a PASS WINDOW (G1 14:09 L-b) from its arrival tick until its cart's centre falls below
    /// the lip (y < m - 0.5), leaves the observations, or 40 gt pass (a stall); the occupants are the
    /// union over that window, and the arrival is returned, classified, when the window closes (so
    /// later than its tick; `finish` closes the open ones at the end). An OCCUPANT is a cart whose
    /// centre is in the lane's band (x - 0.6 .. x + 1.6) and whose box, or its rider's, meets the lip
    /// volume (the band: a carve-out pending ironspec, see the module doc).
    pub fn tick(&mut self, t: i64, carts: &[CartObs]) -> Vec<LipArrival> {
        let occ: Vec<u32> = carts
            .iter()
            .filter(|c| c.pos.0 >= self.x - 0.6 && c.pos.0 < self.x + 1.6)
            .filter(|c| self.meets(&c.cart_box) || c.rider_box.as_ref().is_some_and(|r| self.meets(r)))
            .map(|c| c.id)
            .collect();
        for c in carts {
            let p = c.pos;
            if c.speed < 0.001 && self.in_lane(p) && p.1 < self.m - 1.0 && p.2 < self.mz - 1.1 {
                self.settled_below.insert(c.id, true);
            }
            let at = self.in_lane(p) && p.2 >= self.mz - 1.0 && p.2 < self.mz && p.1 >= self.m - 0.05;
            if at && !self.in_drop.get(&c.id).copied().unwrap_or(false) {
                let others_in_slots = carts.iter().filter(|o| o.id != c.id && self.in_lane(o.pos) && o.pos.1 > 15.5 && o.pos.1 < self.m - 1.0 && o.pos.2 > 0.0 && o.pos.2 < self.mz - 0.5).count();
                let gap = self.last_lane_arrival.map(|t0| t - t0);
                let step = self.prev.get(&c.id).map_or(f64::NAN, |q| (p.0 - q.0).hypot(p.2 - q.2));
                self.open.push((LipArrival { tick: t, cart: c.id, others_in_slots, gap, occupants: Vec::new(), step, class: LipClass::Clear }, std::collections::BTreeMap::new()));
                self.last_arrival.insert(c.id, t);
                self.last_lane_arrival = Some(t);
                self.settled_below.insert(c.id, false);
            }
            self.in_drop.insert(c.id, at);
            self.prev.insert(c.id, p);
        }
        // (every open window takes this tick's occupants; the strongest kind seen stays)
        let kind_of = |s: &LipLane, j: u32, t_arr: i64| -> OccKind {
            if s.settled_below.get(&j).copied().unwrap_or(false) && s.last_arrival.get(&j).is_none_or(|ta| *ta < t_arr) {
                OccKind::Rollback
            } else if s.last_arrival.get(&j).is_some_and(|ta| *ta != t_arr && (t_arr - ta).abs() <= 40) {
                OccKind::Descending
            } else {
                OccKind::Other
            }
        };
        let rank = |k: OccKind| match k {
            OccKind::Rollback => 2,
            OccKind::Descending => 1,
            OccKind::Other => 0,
        };
        let mut open = std::mem::take(&mut self.open);
        for (a, seen) in open.iter_mut() {
            for j in occ.iter().filter(|j| **j != a.cart) {
                let k = kind_of(self, *j, a.tick);
                let e = seen.entry(*j).or_insert(k);
                if rank(k) > rank(*e) {
                    *e = k;
                }
            }
        }
        let mut out = Vec::new();
        for (a, seen) in open {
            let fell = carts.iter().find(|c| c.id == a.cart).is_none_or(|c| c.pos.1 < self.m - 0.5);
            if fell || t - a.tick >= 40 {
                out.push(self.close(a, seen));
            } else {
                self.open.push((a, seen));
            }
        }
        out
    }

    /// Close every open pass window (the run's end), classified.
    pub fn finish(&mut self) -> Vec<LipArrival> {
        std::mem::take(&mut self.open).into_iter().map(|(a, seen)| self.close(a, seen)).collect()
    }

    fn close(&self, mut a: LipArrival, seen: std::collections::BTreeMap<u32, OccKind>) -> LipArrival {
        a.occupants = seen.into_iter().collect();
        a.class = if a.others_in_slots >= 8 {
            LipClass::NinthCart
        } else if a.occupants.iter().any(|o| o.1 == OccKind::Rollback) {
            LipClass::Rollback
        } else if a.gap.is_some_and(|g| g - 1 < self.gap) || a.occupants.iter().any(|o| o.1 == OccKind::Descending) {
            LipClass::GapUnder(self.gap)
        } else if !a.occupants.is_empty() {
            LipClass::Unclassified
        } else if a.step < self.slow {
            LipClass::SlowEntry
        } else {
            LipClass::Clear
        };
        a
    }
}

/// The world-frame observations of the carts `ids` (a composed run's caller): centre, horizontal
/// stored speed, box and the first passenger's box.
pub fn observe(w: &redsim::world::World, ids: &[u32]) -> Result<Vec<CartObs>, String> {
    let bx = |b: &redsim::entity::vec3::Aabb| [b.min_x, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z];
    // (G1 13:09 L-a: a cart or a rider that doesn't resolve is an ERROR, never a silent drop: a vanished
    // box would fail open on the very occupancy the lip bars judge)
    ids.iter()
        .map(|id| {
            let i = redsim::entity::index_of(w, *id).filter(|i| !w.entities.list[*i].removed).ok_or_else(|| format!("lipclass::observe: cart {id} is not in the world (removed or never spawned)"))?;
            let e = &w.entities.list[i];
            let rider_box = match e.passengers.first() {
                None => None,
                Some(r) => {
                    let ri = redsim::entity::index_of(w, *r).filter(|i| !w.entities.list[*i].removed).ok_or_else(|| format!("lipclass::observe: cart {id}'s rider {r} is not in the world"))?;
                    Some(bx(&w.entities.list[ri].bb))
                }
            };
            Ok(CartObs { id: *id, pos: (e.pos.x, e.pos.y, e.pos.z), speed: e.delta_movement.x.hypot(e.delta_movement.z), cart_box: bx(&e.bb), rider_box })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cart at (x, y, z) with its box (0.98 x 0.7 x 0.98) and a rider's box 1.95 tall over its floor.
    fn obs(id: u32, x: f64, y: f64, z: f64, speed: f64, rider: bool) -> CartObs {
        CartObs { id, pos: (x, y, z), speed, cart_box: [x - 0.49, x + 0.49, y, y + 0.7, z - 0.49, z + 0.49], rider_box: rider.then_some([x - 0.3, x + 0.3, y, y + 1.95, z - 0.3, z + 0.3]) }
    }

    /// The lane 3 lip (m 24, mz 7): an arrival from C (z 7.05) into the drop cell (z 7.05 - step).
    fn arrive(l: &mut LipLane, t: i64, id: u32, others: &[CartObs], step: f64) -> LipArrival {
        let mut before = others.to_vec();
        before.push(obs(id, 3.5, 24.0625, 7.05, 0.3, true));
        assert!(l.tick(t - 1, &before).is_empty(), "no arrival while on C");
        let mut now = others.to_vec();
        now.push(obs(id, 3.5, 24.0, 7.05 - step, 0.3, true));
        assert!(l.tick(t, &now).is_empty(), "the pass window opens");
        // (the next tick the cart has fallen below the lip: the window closes)
        let mut fell = others.to_vec();
        fell.push(obs(id, 3.5, 23.2, 6.5, 0.0, true));
        let a = l.tick(t + 1, &fell);
        assert_eq!(a.len(), 1, "{a:?}");
        assert_eq!(a[0].tick, t);
        assert!((a[0].step - step).abs() < 1e-9, "{a:?}");
        a[0].clone()
    }

    /// G1 14:09 L-b: an occupant that enters the lip volume AFTER the arrival tick, while the arriving
    /// cart is still in its pass window (stalled at the lip), counts; one that comes after the window
    /// closed (the cart fell) doesn't.
    #[test]
    fn occupants_over_the_pass_window() {
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = l.tick(1, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true), obs(6, 3.5, 24.0625, 7.05, 0.3, true)]);
        assert!(l.tick(2, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true), obs(6, 3.5, 24.0, 6.75, 0.3, true)]).is_empty());
        // (tick 5: cart 5 rolls up into V while cart 6 is still at the lip; tick 50: the window's 40 gt
        // pass with cart 6 stalled)
        assert!(l.tick(5, &[obs(5, 3.5, 22.5725, 6.51, 0.0, true), obs(6, 3.5, 24.0, 6.69, 0.0, true)]).is_empty());
        let a = l.tick(50, &[obs(5, 3.5, 22.5725, 6.51, 0.0, true), obs(6, 3.5, 24.0, 6.69, 0.0, true)]);
        assert_eq!(a.len(), 1, "{a:?}");
        assert_eq!((a[0].tick, a[0].class), (2, LipClass::Rollback), "{a:?}");
        // (after a clean fall the same rise is no occupant of that arrival)
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = l.tick(1, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true), obs(6, 3.5, 24.0625, 7.05, 0.3, true)]);
        let _ = l.tick(2, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true), obs(6, 3.5, 24.0, 6.75, 0.3, true)]);
        let a = l.tick(3, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true), obs(6, 3.5, 23.2, 6.5, 0.0, true)]);
        assert_eq!(a.iter().map(|x| x.class).collect::<Vec<_>>(), vec![LipClass::Clear]);
        assert!(l.tick(5, &[obs(5, 3.5, 22.5725, 6.51, 0.0, true), obs(6, 3.5, 22.083, 6.02, 0.0, true)]).is_empty());
        // (finish closes an open window)
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = l.tick(1, &[obs(6, 3.5, 24.0625, 7.05, 0.3, true)]);
        let _ = l.tick(2, &[obs(6, 3.5, 24.0, 6.75, 0.3, true)]);
        assert_eq!(l.finish().len(), 1);
    }

    /// G1 14:09 L-a: `observe` fails on a cart that left the world (an occupant removed mid-run), never
    /// dropping it from the occupancy.
    #[test]
    fn observe_refuses_a_missing_cart() {
        let mut w = redsim::world::World::new(redsim::pos::BlockPos::new(0, 0, 0), [8, 8, 8]);
        let id = redsim::entity::spawn(&mut w, "minecraft:minecart", [2.5, 2.0, 2.5], None).unwrap();
        assert_eq!(observe(&w, &[id]).map(|v| v.len()), Ok(1));
        redsim::entity::kill(&mut w, id).unwrap();
        assert!(observe(&w, &[id]).is_err());
    }

    /// Each class from its own construction, and the order: a 9th cart over a rollback.
    #[test]
    fn the_classes() {
        // (clear: nothing in the volume, a 0.30 step)
        let mut l = LipLane::new(3, 24, 7, None);
        assert_eq!(arrive(&mut l, 100, 1, &[], 0.30).class, LipClass::Clear);
        // (slow entry: a 0.11 step, nothing in the volume)
        let mut l = LipLane::new(3, 24, 7, None);
        assert_eq!(arrive(&mut l, 100, 1, &[], 0.11).class, LipClass::SlowEntry);
        // (a gap under 23: the previous arrival 20 gt before)
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = arrive(&mut l, 100, 1, &[], 0.30);
        let a = arrive(&mut l, 120, 2, &[obs(1, 3.5, 20.0, 4.0, 0.2, true)], 0.30);
        assert_eq!((a.class, a.gap), (LipClass::GapUnder(23), Some(20)));
        // (G1 14:09 L-c, the C-gap's edge (C-gap = Δ - 1): Δ 23 = C-gap 22 is caught, Δ 24 = C-gap 23
        // is clear)
        for (dt, want) in [(23, LipClass::GapUnder(23)), (24, LipClass::Clear)] {
            let mut l = LipLane::new(3, 24, 7, None);
            let _ = arrive(&mut l, 100, 1, &[], 0.30);
            let a = arrive(&mut l, 100 + dt, 2, &[obs(1, 3.5, 20.0, 4.0, 0.2, true)], 0.30);
            assert_eq!(a.class, want, "Δ {dt}: {a:?}");
        }
        // (rollback: cart 5 rested at slot 7 (z 4.81, y 20.87), then rose to V+ (z 6.51, y 22.57) with
        // its rider's head at 24.52 in the volume)
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = l.tick(10, &[obs(5, 3.5, 20.8695, 4.807, 0.0, true)]);
        let a = arrive(&mut l, 200, 6, &[obs(5, 3.5, 22.5725, 6.51, 0.0, true)], 0.30);
        assert_eq!(a.class, LipClass::Rollback, "{a:?}");
        assert_eq!(a.occupants, vec![(5, OccKind::Rollback)]);
        // (UNCLASSIFIED: an occupant that never rested below and never arrived)
        let mut l = LipLane::new(3, 24, 7, None);
        let a = arrive(&mut l, 200, 6, &[obs(7, 3.5, 22.5725, 6.51, 0.0, true)], 0.30);
        assert_eq!(a.class, LipClass::Unclassified);
        // (a napper resting on R (x 1.5, the lane's x - 1.5) has its box in the volume's x range, but it
        // is outside the lane's band: no occupant, even after it rested in the slots before)
        let mut l = LipLane::new(3, 24, 7, None);
        let _ = l.tick(10, &[obs(8, 3.5, 20.8695, 4.807, 0.0, true)]);
        let a = arrive(&mut l, 200, 6, &[obs(8, 1.5, 24.6, 7.6, 0.0, true)], 0.30);
        assert_eq!((a.class, a.occupants.len()), (LipClass::Clear, 0), "{a:?}");
        // (a 9th cart: 8 others in the slots (slot 1 at z 0.49, slots 2..8 at z 1.28 + 0.79 k, the 8th
        // at V (22.083, 6.020) with its rider's head over the lip's floor, an occupant): the count comes
        // first)
        let mut l = LipLane::new(3, 24, 7, None);
        let q: Vec<CartObs> = (0..8)
            .map(|k| {
                let z = if k == 0 { 0.49 } else { 1.28 + 0.79 * (k - 1) as f64 };
                obs(10 + k, 3.5, 22.083 - (6.02 - z), z, 0.0, true)
            })
            .collect();
        let _ = l.tick(10, &q);
        let a = arrive(&mut l, 200, 30, &q, 0.30);
        assert_eq!((a.class, a.others_in_slots), (LipClass::NinthCart, 8), "{a:?}");
        assert_eq!(a.occupants, vec![(17, OccKind::Other)], "V's rider is in the volume");
    }

    /// A cart outside the lane or under the lip's height is no arrival; the same cart is one arrival
    /// until it leaves the drop cell.
    #[test]
    fn arrivals_are_counted_once() {
        let mut l = LipLane::new(3, 24, 7, None);
        assert!(l.tick(1, &[obs(1, 2.4, 24.0, 6.6, 0.3, true)]).is_empty(), "west of the lane");
        assert!(l.tick(2, &[obs(1, 3.5, 23.5, 6.6, 0.3, true)]).is_empty(), "below the lip");
        assert!(l.tick(3, &[obs(1, 3.5, 24.0, 6.6, 0.3, true)]).is_empty(), "the window opens");
        assert!(l.tick(4, &[obs(1, 3.5, 24.0, 6.5, 0.1, true)]).is_empty(), "still in the drop cell: no second arrival");
        assert_eq!(l.tick(5, &[obs(1, 3.5, 23.2, 6.5, 0.1, true)]).len(), 1, "fell: one arrival");
        assert!(LipClass::SlowEntry.explains_a_stall() && !LipClass::Unclassified.explains_a_stall() && !LipClass::Clear.explains_a_stall());
    }
}
