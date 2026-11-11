//! S3(ii)'s per-stand head-launch permissions, emulated on one stand (`queue-loop-run
//! --head-perm`; farmopt, ironspec, 51b2aacd and 7b90c3f1 2026-11-11). The register's head
//! launches a train for stand X only while both of X's latches are set:
//!
//! - ARR ("the train launched for X arrived"): set by the rising edge of D_arr, the detector rail
//!   on the hold's upper descent slope (7b90c3f1's (xb, 4, 2): every arriving cart crosses it,
//!   carts 2 and 3 rest pressed in its box, the front cart rests below it, touching H); reset by
//!   the launch for X.
//! - EMP ("X's hold emptied since X's last launch"), rule A: set once D_arr and Dh (pocket 3's
//!   entry slope (xb + 1, 1, 0), past H, H's closing input) both read off after a Dh rising edge
//!   since the launch; reset by the launch. The front cell can't carry a detector (it touches H),
//!   so rule A leans on the meter releasing the front cart before Dh's 20-gt off-lag runs out
//!   (7b90c3f1: openings at the previous Dh edge + at most 16 gt). Rule C (7b90c3f1's
//!   alternative, printed for comparison): the third Dh rising edge since X's stand REL, with Dh
//!   off (exact when each opening passes one cart; undercounts, so never sets, when an opening
//!   passes two or a train is short, until the next REL resets the count).
//!
//! With `router`, the router is emulated phase-locked (7b90c3f1's; ironspec 2026-11-11, the
//! toggle-on-launch rule withdrawn): at each GO it points at the stand whose tick that GO follows
//! (A on even cycles, B on odd), never at a count, and the launch is gated by that stand's
//! permission; a held stand just skips its slot. Stand A is this bench's stand (its real
//! latches); stand B is virtual (its launches send no train; its permission can be held for a
//! span of cycles), so A gets a train every second GO, the double stand's cadence.
//!
//! Both detectors are read as the world's own detector rails when the design has them there
//! (`--meter-hold` converts them), else ideally (`comb::VDet`). With `gate`, the head's release
//! waits on ARR and EMP; without, the latches are only observed and printed.

use crate::comb::{VDet, meets};
use redsim::entity::Vec3;
use redsim::pos::BlockPos;
use redsim::world::World;

/// The head's decision at a GO (`HeadPerm::go`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    /// A train for this bench's stand (A).
    Launch,
    /// The stand the router points to isn't ready: no launch (that stand skips its slot).
    Hold,
    /// The router's virtual stand B takes this launch: no train here.
    VirtualB,
}

/// One launch followed to completion.
#[derive(Clone, Debug)]
struct Track {
    slot: i64,
    launch: i64,
    dh_seen: bool,
    arr: Option<i64>,
    emp_a: Option<i64>,
    emp_c: Option<i64>,
    /// EMP (rule A) set with a cart stopped in the hold.
    stopped: bool,
}

#[derive(Clone, Debug)]
pub struct HeadPerm {
    /// D_arr, Dh and the stand's REL input cell, in the world's frame.
    d_arr: [i32; 3],
    dh: [i32; 3],
    rel_cell: Option<[i32; 3]>,
    /// The hold's cells (the descent's two slopes and the snap turn), world frame: a cart in any
    /// is still in the hold (the truth EMP is measured against).
    hold: [[i32; 3]; 3],
    pub gate: bool,
    pub arr: bool,
    pub emp: bool,
    flag_a: bool,
    ideal: [VDet; 2],
    prev: [bool; 2],
    rel_prev: bool,
    dh_count: u32,
    pub launches: usize,
    pub binds: Vec<String>,
    tracks: Vec<Track>,
    /// The router (A = this stand, B = virtual), B's held cycles, and its log.
    pub router: bool,
    /// With the meter's VB: B's launches are real (parked by the bench), not virtual.
    pub b_physical: bool,
    skip_next_launch: bool,
    b_hold: Option<(i64, i64)>,
    router_log: Vec<String>,
    holds: usize,
    /// (day-1's RETRY-only head, farmopt 2026-11-11) this stand's REL only with a launch for it, or
    /// by RETRY (an EXPIRE GO, the stand not emptied since its last launch, no train in flight);
    /// the stand's raw pad pulses (the summon ticks with three unlocked panicking riders in the
    /// stand), A forced held for a span of its GOs (C0, N), a lost REL at one slot, and the log
    pub retry: bool,
    pub a_pulses: Vec<i64>,
    pub a_hold: Option<(i64, i64)>,
    pub rel_lost: Option<i64>,
    pub rel_log: Vec<String>,
    /// each summon tick: t: riders in the stand's carts / panicking / unlocked and panicking
    pub summon_log: Vec<String>,
    /// (the pulse-driven router) on; R points at A; B's missed ticks; B's pulses doubled onto a
    /// slot; a forced wrong GO; this GO's time, kind, EXPIRE and stand; the hold-at-tick check
    pub pulse_router: bool,
    pub r_a: bool,
    pub b_miss: Vec<i64>,
    pub b_double: Vec<i64>,
    pub wrong_go: Option<i64>,
    pub go_t: i64,
    pub last_kind: String,
    pub last_expire: bool,
    pub last_to_a: bool,
    pub tick_waits: Vec<String>,
    /// (day-1's latches, farmopt 2026-11-11: ARR_X re-armed by the launch (DEP_X later), cleared
    /// by D_arr's first edge after it; EMP_X (BLOCK_e) re-armed by the launch only while the stand
    /// holds a batch, cleared by SE_X; permission = both clear; RETRY = EXPIRE and EMP_X armed)
    pub day1: bool,
    pub arr_armed: bool,
    pub emp_armed: bool,
    pub se_seen: u64,
    pub launch_waits: Vec<String>,
    /// the most carts stopped on the descent at once, and the farthest back z (loop frame)
    pub held_max: (usize, f64),
    /// the last summon tick's snapshot (t, A's riders at rest in its pockets, carts stopped at H),
    /// each A GO's copy of it (slot, snapshot), and the fault's slot (cycles-to-clean)
    pub last_tick: (i64, usize, usize),
    pub ledger: Vec<(i64, (i64, usize, usize))>,
    pub fault_slot: Option<i64>,
    /// (day-1's latches) DEP_X: an ideal plane (loop z) on this stand's approach column that a cart
    /// crosses going down toward the hold (after VB's parking point): ARR/EMP re-armed on it, not
    /// on the launch; its re-arms logged (t, +E: EMP re-armed too)
    pub dep_x: Option<f64>,
    pub dep_log: Vec<String>,
    /// trains reaching H (ARR_X's clearing edge) while carts of an earlier one still waited there
    pub second_train: Vec<String>,
    /// D_arr's and Dh's levels at each change (t: A/a = D_arr on/off, H/h = Dh on/off)
    pub det_log: Vec<String>,
}

impl HeadPerm {
    #[allow(clippy::too_many_arguments)]
    pub fn new(d_arr: [i32; 3], dh: [i32; 3], xb: i32, shift: [i32; 2], rel_cell: Option<BlockPos>, gate: bool, router: bool, b_hold: Option<(i64, i64)>) -> HeadPerm {
        let w = |c: [i32; 3]| [c[0] + shift[0], c[1], c[2] + shift[1]];
        HeadPerm {
            d_arr: w(d_arr),
            dh: w(dh),
            rel_cell: rel_cell.map(|p| [p.x, p.y, p.z]),
            hold: [w([xb, 4, 2]), w([xb, 3, 1]), w([xb, 2, 0])],
            gate,
            // (the run starts with a batch in the stand and nothing held: both set)
            arr: true,
            emp: true,
            flag_a: false,
            ideal: [VDet::default(); 2],
            prev: [false; 2],
            rel_prev: false,
            dh_count: 0,
            launches: 0,
            binds: Vec::new(),
            tracks: Vec::new(),
            router,
            b_physical: false,
            skip_next_launch: false,
            b_hold,
            router_log: Vec::new(),
            holds: 0,
            retry: false,
            a_pulses: Vec::new(),
            a_hold: None,
            rel_lost: None,
            rel_log: Vec::new(),
            summon_log: Vec::new(),
            pulse_router: false,
            r_a: false,
            b_miss: Vec::new(),
            b_double: Vec::new(),
            wrong_go: None,
            go_t: 0,
            last_kind: String::new(),
            last_expire: false,
            last_to_a: false,
            tick_waits: Vec::new(),
            day1: false,
            arr_armed: false,
            emp_armed: false,
            se_seen: 0,
            launch_waits: Vec::new(),
            held_max: (0, 0.0),
            last_tick: (0, 0, 0),
            ledger: Vec::new(),
            fault_slot: None,
            dep_x: None,
            dep_log: Vec::new(),
            second_train: Vec::new(),
            det_log: Vec::new(),
        }
    }

    /// A raw pad pulse of this stand (A) at `t`.
    pub fn a_pulse(&mut self, t: i64) {
        self.a_pulses.push(t);
    }

    /// Whether A pulsed within the 60 gt before `t` (this GO's own pulse): else this GO is
    /// EXPIRE's (the timer restarted by every raw pulse runs out ~104 gt after the last one).
    pub fn a_pulsed_before(&self, t: i64) -> bool {
        self.a_pulses.iter().any(|p| *p <= t && t - *p < 60)
    }

    pub fn permit(&self) -> bool {
        if self.day1 {
            return !self.arr_armed && !self.emp_armed;
        }
        self.arr && self.emp
    }

    /// (day-1's latches) the launch re-arms ARR_X, and EMP_X only while the stand holds a batch
    /// (¬E_X): `occupied` the stand's occupancy at the launch, `waiting` the carts stopped at H then.
    /// With `dep_x` the launch only records `waiting`: DEP_X re-arms instead (`day1_dep`).
    pub fn day1_launch(&mut self, t: i64, slot: i64, occupied: bool, waiting: usize) {
        if self.dep_x.is_none() {
            self.arr_armed = true;
            self.emp_armed = occupied;
        }
        if waiting > 0 {
            self.launch_waits.push(format!("slot {slot} (t {t}): {waiting} carts stopped at H"));
        }
    }

    /// (day-1's latches with DEP_X) a cart crossing DEP_X toward this stand: ARR_X re-armed if
    /// clear (a train's later carts cross while it is still armed), EMP_X re-armed while the
    /// stand holds a batch (one-shot per re-arm).
    pub fn day1_dep(&mut self, t: i64, occupied: bool) {
        if !self.arr_armed {
            self.arr_armed = true;
            if occupied {
                self.emp_armed = true;
            }
            if self.dep_log.len() < 40 {
                self.dep_log.push(format!("{t}{}", if occupied { "+E" } else { "" }));
            }
        }
    }

    /// (day-1's latches) SE_X, the rearmost occupied pocket's departure edge: EMP_X cleared.
    pub fn day1_se(&mut self) {
        self.emp_armed = false;
    }

    /// The head's decision at a GO of cycle `c` (the router phase-locked, 7b90c3f1's, ironspec's
    /// ruling 2026-11-11: toggle-on-launch withdrawn): the router points at the stand whose tick
    /// this GO follows (A on even cycles, B on odd), never at a count; that stand is launched if its
    /// permission is set, else it just skips its slot.
    pub fn go(&mut self, c: i64) -> Go {
        // (the pulse-driven router, day-1's ports: R set by each stand's raw pad pulse (A's from
        // its summon ticks, B's virtual at its ticks unless missed, or doubled onto A's); with no
        // pulse, EXPIRE's GO toggles R; both at once (a double tick) toggles it too, one train;
        // a forced wrong GO flips it (the positive control))
        let to_a = if self.pulse_router {
            let a_p = self.a_pulses.iter().any(|p| self.go_t - *p >= 0 && self.go_t - *p < 60);
            let b_p = (c.rem_euclid(2) == 1 && !self.b_miss.contains(&c)) || self.b_double.contains(&c);
            let (r, kind) = match (a_p, b_p) {
                (true, false) => (true, "A's pulse"),
                (false, true) => (false, "B's pulse"),
                (true, true) => (!self.r_a, "a double tick, R toggled"),
                (false, false) => (!self.r_a, "EXPIRE, R toggled"),
            };
            let r = if self.wrong_go == Some(c) { !r } else { r };
            self.last_kind = format!("{kind}{}", if self.wrong_go == Some(c) { ", forced wrong" } else { "" });
            self.last_expire = !a_p && !b_p;
            self.r_a = r;
            r
        } else {
            self.last_expire = !self.a_pulses.iter().any(|p| self.go_t - *p >= 0 && self.go_t - *p < 60);
            c.rem_euclid(2) == 0
        };
        self.last_to_a = to_a;
        if to_a {
            self.ledger.push((c, self.last_tick));
        }
        let (who, d) = if to_a {
            let forced = self.a_hold.is_some_and(|(c0, n)| c >= c0 && c < c0 + 2 * n);
            ("A", if forced || (self.gate && !self.permit()) { Go::Hold } else { Go::Launch })
        } else {
            ("B", if self.b_hold.is_some_and(|(c0, n)| c >= c0 && c < c0 + n) {
                Go::Hold
            } else if self.b_physical {
                // (7b90c3f1's VB: B's train is launched for real and parked; this stand's latches
                // don't see that launch)
                self.skip_next_launch = true;
                Go::Launch
            } else {
                Go::VirtualB
            })
        };
        if d == Go::Hold {
            self.holds += 1;
        }
        if self.router_log.len() < 60 {
            self.router_log.push(format!("{c}:{who}{}{}", match d {
                Go::Launch => "+",
                Go::VirtualB => "+",
                Go::Hold => "(held)",
            }, if self.pulse_router { format!(" [{}]", self.last_kind) } else { String::new() }));
        }
        d
    }

    /// The head's launch for this stand: the latches read (a bind if either is unset), then both
    /// reset.
    /// Whether the next launch is B's (VB's real launch, unseen by A's latches).
    pub fn launching_b(&self) -> bool {
        self.skip_next_launch
    }

    pub fn on_launch(&mut self, t: i64, slot: i64) {
        if self.skip_next_launch {
            self.skip_next_launch = false;
            return;
        }
        if !self.permit() {
            self.binds.push(format!("slot {slot} (ARR {} EMP {})", self.arr, self.emp));
        }
        self.launches += 1;
        self.tracks.push(Track { slot, launch: t, dh_seen: false, arr: None, emp_a: None, emp_c: None, stopped: false });
        self.arr = false;
        self.emp = false;
        self.flag_a = false;
    }

    fn read(&mut self, w: &World, carts: &[(u32, Vec3, Vec3)], k: usize, c: [i32; 3], t: i64) -> bool {
        let b = w.get(BlockPos::new(c[0], c[1], c[2])).to_string();
        if b.contains("detector_rail") {
            return b.contains("powered=true");
        }
        let present = carts.iter().any(|(_, p, _)| meets(*p, c));
        self.ideal[k].step(present, t);
        self.ideal[k].on
    }

    /// Before the world's tick: the two detectors, the stand's REL, the latches, and the truth
    /// (a cart in the hold, stopped or moving).
    pub fn pre_tick(&mut self, w: &World, carts: &[(u32, Vec3, Vec3)], t: i64) {
        let (da, dh) = (self.read(w, carts, 0, self.d_arr, t), self.read(w, carts, 1, self.dh, t));
        let (da_rise, dh_rise) = (da && !self.prev[0], dh && !self.prev[1]);
        if (da != self.prev[0] || dh != self.prev[1]) && self.det_log.len() < 120 {
            self.det_log.push(format!("{t}:{}{}", if da { "A" } else { "a" }, if dh { "H" } else { "h" }));
        }
        self.prev = [da, dh];
        let rel_on = self.rel_cell.is_some_and(|r| w.get(BlockPos::new(r[0], r[1], r[2])).to_string().contains("redstone_block"));
        if rel_on && !self.rel_prev {
            self.dh_count = 0;
        }
        self.rel_prev = rel_on;
        if dh_rise {
            self.dh_count += 1;
        }
        if da_rise && !self.arr {
            self.arr = true;
        }
        // (ironspec's watch item: a train reaching H (ARR_X's clearing edge) while carts of an
        // earlier one still wait there; counted below with the carts stopped on the column)
        let arr_clearing = da_rise && self.arr_armed;
        if da_rise {
            self.arr_armed = false;
        }
        if dh_rise {
            self.flag_a = true;
        }
        if !self.emp && self.flag_a && !da && !dh {
            self.emp = true;
        }
        let mut waiting = false;
        let (mut n_held, mut z_back) = (0usize, 0.0f64);
        for (_, p, v) in carts {
            if self.hold.iter().any(|c| (p.x - (c[0] as f64 + 0.5)).abs() < 0.99 && (p.z - (c[2] as f64 + 0.5)).abs() < 0.99 && p.y > c[1] as f64 - 0.6 && p.y < c[1] as f64 + 1.2) && (v.x * v.x + v.z * v.z).sqrt() < 0.01 {
                waiting = true;
            }
            // (carts stopped on the descent's column, H to T and beyond)
            let col_x = self.hold[0][0] as f64 + 0.5;
            let z0 = self.hold[2][2] as f64;
            if (p.x - col_x).abs() < 0.45 && p.z > z0 + 0.5 && p.z < z0 + 7.0 && p.y > 1.5 && (v.x * v.x + v.z * v.z).sqrt() < 0.01 {
                n_held += 1;
                z_back = z_back.max(p.z - z0);
            }
        }
        if n_held > self.held_max.0 || (n_held == self.held_max.0 && z_back > self.held_max.1) {
            self.held_max = (n_held, z_back);
        }
        if arr_clearing && n_held > 0 && self.second_train.len() < 20 {
            self.second_train.push(format!("t {t}: {n_held} carts still stopped at H (back to z {z_back:.2})"));
        }
        // (each launch followed to completion within 300 gt)
        let rel_known = self.rel_cell.is_some();
        let count = self.dh_count;
        for tr in self.tracks.iter_mut().filter(|tr| t - tr.launch < 300) {
            if da_rise && tr.arr.is_none() {
                tr.arr = Some(t);
            }
            if dh_rise {
                tr.dh_seen = true;
            }
            if tr.emp_a.is_none() && tr.dh_seen && !da && !dh {
                tr.emp_a = Some(t);
                tr.stopped = waiting;
            }
            if rel_known && tr.emp_c.is_none() && count >= 3 && !dh && tr.arr.is_some() {
                tr.emp_c = Some(t);
            }
        }
    }

    pub fn report(&self) -> (Vec<String>, Vec<String>) {
        let mut out = Vec::new();
        let mut problems = Vec::new();
        let rng = |v: &[i64]| if v.is_empty() { "-".to_string() } else { format!("+{}..+{}", v.iter().min().unwrap_or(&0), v.iter().max().unwrap_or(&0)) };
        let arr: Vec<i64> = self.tracks.iter().filter_map(|tr| tr.arr.map(|x| x - tr.launch)).collect();
        let ea: Vec<i64> = self.tracks.iter().filter_map(|tr| tr.emp_a.map(|x| x - tr.launch)).collect();
        let ec: Vec<i64> = self.tracks.iter().filter_map(|tr| tr.emp_c.map(|x| x - tr.launch)).collect();
        let stopped: Vec<String> = self.tracks.iter().filter(|tr| tr.stopped).map(|tr| format!("slot {}", tr.slot)).collect();
        out.push(format!(
            "head-perm (S3(ii)'s per-stand latches, {}{}): {} launches of this stand, binds {}{}",
            if self.gate { "gating the head" } else { "observed only" },
            if self.router { ", router phase-locked (A here, B virtual)" } else { "" },
            self.launches,
            self.binds.len(),
            if self.binds.is_empty() { String::new() } else { format!(" ({})", self.binds.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }
        ));
        out.push(format!(
            "head-perm: each launch to completion: ARR {} after its launch ({} of {}); EMP rule A {} ({} of {}), rule C {} ({} of {}); slack before X's next launch at +200: ARR {} EMP(A) {} gt",
            rng(&arr),
            arr.len(),
            self.tracks.len(),
            rng(&ea),
            ea.len(),
            self.tracks.len(),
            if self.rel_cell.is_some() { rng(&ec) } else { "- (no REL cell)".to_string() },
            ec.len(),
            self.tracks.len(),
            arr.iter().max().map_or("-".into(), |x| (200 - x).to_string()),
            ea.iter().max().map_or("-".into(), |x| (200 - x).to_string()),
        ));
        out.push(format!("head-perm: EMP (rule A) set with a cart stopped in the hold: {}{}", stopped.len(), if stopped.is_empty() { String::new() } else { format!(" ({})", stopped.join(", ")) }));
        if !stopped.is_empty() {
            problems.push(format!("head-perm: EMP set {} times with a cart stopped in the hold", stopped.len()));
        }
        if self.router {
            out.push(format!("head-perm router (phase-locked): {} holds; GO by GO: {}", self.holds, self.router_log.join(" ")));
        }
        if self.retry {
            out.push(format!(
                "head-perm stand REL (day-1's RETRY-only head: a REL only with a launch for A, or RETRY on an EXPIRE GO with A's stand not emptied since its last launch and no train in flight): A's raw pad pulses at t {}; A's GOs: {}",
                if self.a_pulses.is_empty() { "-".to_string() } else { self.a_pulses.iter().take(16).map(|t| t.to_string()).collect::<Vec<_>>().join(",") },
                if self.rel_log.is_empty() { "-".to_string() } else { self.rel_log.join("; ") }
            ));
            out.push(format!("head-perm summon ticks (t: riders in the stand's carts / panicking / unlocked and panicking): {}", self.summon_log.join(" ")));
            out.push(format!("head-perm trains waiting in A's hold through A's tick (a summon tick with A's batch in the stand and carts stopped in the hold: inside the summoners' 10-box): {}{}", self.tick_waits.len(), if self.tick_waits.is_empty() { String::new() } else { format!(" ({})", self.tick_waits.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }));
            if self.day1 {
                // (cycles-to-clean: each A GO's last summon tick, clean = three riders at rest in
                // A's pockets and nothing stopped at H; clean from the first A GO after the fault
                // whose tick and every later one are clean)
                let led: Vec<String> = self.ledger.iter().map(|(c, (t, p, h))| format!("{c}:{p}/{h}{}", if *t == 0 { "?" } else { "" })).collect();
                let clean = |x: &(i64, (i64, usize, usize))| x.1 .1 >= 3 && x.1 .2 == 0;
                let from = self.fault_slot.unwrap_or(0);
                let after: Vec<&(i64, (i64, usize, usize))> = self.ledger.iter().filter(|x| x.0 > from).collect();
                let clean_from = (0..after.len()).find(|&i| after[i..].iter().all(|x| clean(x))).map(|i| after[i].0);
                out.push(format!(
                    "head-perm cycles-to-clean (each A GO: its tick's riders at rest in A's pockets / carts stopped at H): {}; clean from slot {} ({} A-cycles after the fault at slot {from})",
                    led.join(" "),
                    clean_from.map_or("- (never within the run)".to_string(), |s| s.to_string()),
                    clean_from.map_or("-".to_string(), |s| ((s - from) / 2).to_string())
                ));
                out.push(format!(
                    "head-perm day-1 latches (ARR_X by the launch, cleared by D_arr's first edge; EMP_X by the launch while the stand holds a batch, cleared by SE_X): launches for A with carts stopped at H: {}{}; the most carts stopped on the descent at once {} (the farthest back z {:.2} from the snap turn's cell){}",
                    self.launch_waits.len(),
                    if self.launch_waits.is_empty() { String::new() } else { format!(" ({})", self.launch_waits.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) },
                    self.held_max.0,
                    self.held_max.1,
                    self.dep_x.map_or(String::new(), |z| format!("; re-armed by DEP_X (ideal, the approach column crossing z {z} downward) at t {}", if self.dep_log.is_empty() { "-".to_string() } else { self.dep_log.join(",") }))
                ));
                out.push(format!(
                    "head-perm at H (ironspec's watch item): the most carts stopped at H at once {} (the farthest back z {:.2} from the snap turn's cell); a train reaching H while earlier carts still waited there: {}{}",
                    self.held_max.0,
                    self.held_max.1,
                    self.second_train.len(),
                    if self.second_train.is_empty() { String::new() } else { format!(" ({})", self.second_train.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
                ));
            }
            if !self.tick_waits.is_empty() {
                problems.push(format!("head-perm: {} trains waited in A's hold through A's tick", self.tick_waits.len()));
            }
        }
        (out, problems)
    }

    pub fn detail(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .tracks
            .iter()
            .map(|tr| format!("head-perm track slot {}: launch +{}: ARR {} EMP(A) {} EMP(C) {}{}", tr.slot, tr.launch - tr.slot * 100, tr.arr.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), tr.emp_a.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), tr.emp_c.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), if tr.stopped { " (a cart stopped in the hold)" } else { "" }))
            .collect();
        v.push(format!("head-perm detectors (t: A/a D_arr on/off, H/h Dh on/off): {}", self.det_log.join(" ")));
        v
    }
}
