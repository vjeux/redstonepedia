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
//! With `router`, the router is emulated: it toggles only on an actual (gated) launch (ironspec
//! via farmopt, 2026-11-11: never on GO), the head checks the permissions of the stand it points
//! to at each GO and holds (no launch, no toggle) otherwise. Stand A is this bench's stand (its
//! real latches); stand B is virtual (its launches toggle the router and send no train; its
//! permission can be held for a span of cycles), so A gets a train every second launch, the
//! double stand's cadence.
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
    /// The stand the router points to isn't ready: no launch, no toggle.
    Hold,
    /// The router's virtual stand B takes this launch: no train here, the router toggles.
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
    pub router: u8,
    to_a: bool,
    b_hold: Option<(i64, i64)>,
    router_log: Vec<String>,
    holds: usize,
}

impl HeadPerm {
    #[allow(clippy::too_many_arguments)]
    pub fn new(d_arr: [i32; 3], dh: [i32; 3], xb: i32, shift: [i32; 2], rel_cell: Option<BlockPos>, gate: bool, router: u8, b_hold: Option<(i64, i64)>) -> HeadPerm {
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
            to_a: true,
            b_hold,
            router_log: Vec::new(),
            holds: 0,
        }
    }

    pub fn permit(&self) -> bool {
        self.arr && self.emp
    }

    /// The head's decision at a GO of cycle `c`: the stand the router points to, launched if its
    /// permission is set, else held. Router 1 (toggle on launch): the router toggles on each
    /// actual launch, a hold retries the same stand at the next GO. Router 2 (phase-locked,
    /// 7b90c3f1's): the router points at the stand whose tick this GO follows (A on even cycles,
    /// B on odd), a held stand just skips its slot.
    pub fn go(&mut self, c: i64) -> Go {
        let to_a = if self.router == 2 { c.rem_euclid(2) == 0 } else { self.to_a };
        let (who, d) = if to_a {
            ("A", if !self.gate || self.permit() { Go::Launch } else { Go::Hold })
        } else {
            ("B", if self.b_hold.is_some_and(|(c0, n)| c >= c0 && c < c0 + n) { Go::Hold } else { Go::VirtualB })
        };
        if d == Go::Hold {
            self.holds += 1;
        } else if self.router == 1 {
            self.to_a = !self.to_a;
        }
        if self.router_log.len() < 60 {
            self.router_log.push(format!("{c}:{who}{}", match d {
                Go::Launch => "+",
                Go::VirtualB => "+",
                Go::Hold => "(held)",
            }));
        }
        d
    }

    /// The head's launch for this stand: the latches read (a bind if either is unset), then both
    /// reset.
    pub fn on_launch(&mut self, t: i64, slot: i64) {
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
        if dh_rise {
            self.flag_a = true;
        }
        if !self.emp && self.flag_a && !da && !dh {
            self.emp = true;
        }
        let mut waiting = false;
        for (_, p, v) in carts {
            if self.hold.iter().any(|c| (p.x - (c[0] as f64 + 0.5)).abs() < 0.99 && (p.z - (c[2] as f64 + 0.5)).abs() < 0.99 && p.y > c[1] as f64 - 0.6 && p.y < c[1] as f64 + 1.2) && (v.x * v.x + v.z * v.z).sqrt() < 0.01 {
                waiting = true;
            }
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
            if self.router > 0 { if self.router == 2 { ", router phase-locked (A here, B virtual)" } else { ", router toggling on launches (A here, B virtual)" } } else { "" },
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
        if self.router > 0 {
            out.push(format!("head-perm router ({}): {} holds; GO by GO: {}", if self.router == 2 { "phase-locked" } else { "toggles on actual launches only" }, self.holds, self.router_log.join(" ")));
        }
        (out, problems)
    }

    pub fn detail(&self) -> Vec<String> {
        self.tracks
            .iter()
            .map(|tr| format!("head-perm track slot {}: launch +{}: ARR {} EMP(A) {} EMP(C) {}{}", tr.slot, tr.launch - tr.slot * 100, tr.arr.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), tr.emp_a.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), tr.emp_c.map_or("-".to_string(), |x| format!("+{}", x - tr.launch)), if tr.stopped { " (a cart stopped in the hold)" } else { "" }))
            .collect()
    }
}
