//! S3(ii)'s per-stand head-launch permissions, emulated on one stand (`queue-loop-run
//! --head-perm`; farmopt, ironspec, 51b2aacd and 7b90c3f1 2026-11-11). The register's head
//! launches a train for stand X only while both of X's latches are set:
//!
//! - ARR ("the train launched for X arrived"): set by the rising edge of D_arr, the detector rail
//!   on the hold's upper descent slope (7b90c3f1's (xb, 4, 2): every arriving cart crosses it,
//!   carts 2 and 3 rest pressed in its box, the front cart rests below it, touching H); reset by
//!   the launch for X.
//! - EMP ("X's hold emptied since X's last launch"), rule A (7b90c3f1's): set once D_arr and Dh
//!   (pocket 3's entry slope (xb + 1, 1, 0), past H) both read off after a Dh rising edge since the
//!   launch; reset by the launch. Rule B (printed for comparison only) also resets its flag on a
//!   D_arr rising edge (a cart entering after the Dh evidence): it can wedge unset when carts pass
//!   unmetered (D_arr re-rises between them while Dh stays on). The front cell can't carry a
//!   detector (it touches H), so EMP leans on the meter releasing the front cart before Dh's
//!   off-lag (20 gt) runs out: the hand-off's condition, measured here against the carts'
//!   positions (a cart stopped in the hold when EMP sets is a false set; a released cart still
//!   crossing the snap is counted apart).
//!
//! Both detectors are read as the world's own detector rails when the design has them there
//! (`--meter-hold` converts them), else ideally (`comb::VDet`: on while a cart's box meets the
//! search box, re-checked every 20 gt). With `gate`, the head's release waits on ARR and EMP
//! (c4336062's `head_pending` path); without, the latches are only observed and printed.

use crate::comb::{VDet, meets};
use redsim::entity::Vec3;
use redsim::pos::BlockPos;
use redsim::world::World;

#[derive(Clone, Debug)]
pub struct HeadPerm {
    /// D_arr and Dh, in the world's frame (the design's shift applied).
    d_arr: [i32; 3],
    dh: [i32; 3],
    /// The hold's cells (the descent's two slopes and the snap turn), world frame: a cart in any
    /// is still in the hold (the truth EMP is measured against).
    hold: [[i32; 3]; 3],
    pub gate: bool,
    pub arr: bool,
    pub emp: bool,
    flag: bool,
    flag_a: bool,
    emp_b: bool,
    binds_b: usize,
    ideal: [VDet; 2],
    prev: [bool; 2],
    launch_at: Option<i64>,
    arr_at: Option<i64>,
    emp_at: Option<i64>,
    /// The truth: the last tick a cart was in the hold since the launch.
    hold_last: Option<i64>,
    pub launches: usize,
    pub binds: Vec<String>,
    slack: Vec<(i64, i64, i64)>,
    false_emp: Vec<String>,
    transit_emp: usize,
    late_emp: Vec<i64>,
    lines: Vec<String>,
    /// Each launch's latches followed to completion (the next launch on a one-stand bench comes
    /// at +100, before a two-stand X's at +200): (slot, launch t, Dh edge seen, ARR t, EMP t,
    /// EMP with a cart stopped in the hold).
    tracks: Vec<(i64, i64, bool, Option<i64>, Option<i64>, bool)>,
}

impl HeadPerm {
    pub fn new(d_arr: [i32; 3], dh: [i32; 3], xb: i32, shift: [i32; 2], gate: bool) -> HeadPerm {
        let w = |c: [i32; 3]| [c[0] + shift[0], c[1], c[2] + shift[1]];
        HeadPerm {
            d_arr: w(d_arr),
            dh: w(dh),
            hold: [w([xb, 4, 2]), w([xb, 3, 1]), w([xb, 2, 0])],
            gate,
            // (the run starts with a batch in the stand and nothing held: both set)
            arr: true,
            emp: true,
            flag: false,
            flag_a: false,
            emp_b: true,
            binds_b: 0,
            ideal: [VDet::default(); 2],
            prev: [false; 2],
            launch_at: None,
            arr_at: None,
            emp_at: None,
            hold_last: None,
            launches: 0,
            binds: Vec::new(),
            slack: Vec::new(),
            false_emp: Vec::new(),
            transit_emp: 0,
            late_emp: Vec::new(),
            lines: Vec::new(),
            tracks: Vec::new(),
        }
    }

    pub fn permit(&self) -> bool {
        self.arr && self.emp
    }

    /// The head's launch for this stand: the latches read (a bind if either is unset), the slack
    /// since each was set, then both reset.
    pub fn on_launch(&mut self, t: i64, slot: i64) {
        if let Some(l) = self.launch_at {
            let a = self.arr_at.map_or(-1, |x| t - x);
            let e = self.emp_at.map_or(-1, |x| t - x);
            self.slack.push((slot, a, e));
            self.lines.push(format!("head-perm slot {slot}: launch +{} (prev +{}): ARR set {} EMP set {}{}", t - slot * 100, l - (slot - 1) * 100, self.arr_at.map_or("-".to_string(), |x| format!("+{}", x - l)), self.emp_at.map_or("-".to_string(), |x| format!("+{}", x - l)), if self.permit() { "" } else { " BIND" }));
        }
        if !self.permit() {
            self.binds.push(format!("slot {slot} (ARR {} EMP {})", self.arr, self.emp));
        }
        if !(self.arr && self.emp_b) {
            self.binds_b += 1;
        }
        self.launches += 1;
        self.launch_at = Some(t);
        self.tracks.push((slot, t, false, None, None, false));
        self.arr = false;
        self.emp = false;
        self.emp_b = false;
        self.flag = false;
        self.flag_a = false;
        self.arr_at = None;
        self.emp_at = None;
        self.hold_last = None;
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

    /// Before the world's tick: the two detectors, the latches, and the truth (a cart in the hold).
    pub fn pre_tick(&mut self, w: &World, carts: &[(u32, Vec3, Vec3)], t: i64, slot: i64) {
        let (da, dh) = (self.read(w, carts, 0, self.d_arr, t), self.read(w, carts, 1, self.dh, t));
        let (da_rise, dh_rise) = (da && !self.prev[0], dh && !self.prev[1]);
        self.prev = [da, dh];
        if da_rise {
            if !self.arr {
                self.arr = true;
                self.arr_at = Some(t);
            }
            self.flag = false;
        }
        if dh_rise {
            self.flag = true;
            self.flag_a = true;
        }
        // (rule B, for comparison: the flag reset by a cart entering the queue after it)
        if !self.emp_b && self.flag && !da && !dh {
            self.emp_b = true;
        }
        // (the truth: a cart in the hold's cells, stopped (waiting at H) or moving (released))
        let mut waiting = false;
        let mut moving = false;
        for (_, p, v) in carts {
            if self.hold.iter().any(|c| (p.x - (c[0] as f64 + 0.5)).abs() < 0.99 && (p.z - (c[2] as f64 + 0.5)).abs() < 0.99 && p.y > c[1] as f64 - 0.6 && p.y < c[1] as f64 + 1.2) {
                if (v.x * v.x + v.z * v.z).sqrt() < 0.01 {
                    waiting = true;
                } else {
                    moving = true;
                }
            }
        }
        let in_hold = waiting || moving;
        if in_hold {
            self.hold_last = Some(t);
        }
        // (each launch followed to completion, rule A, within 300 gt of its launch)
        for tr in self.tracks.iter_mut().filter(|tr| tr.4.is_none() && t - tr.1 < 300) {
            if da_rise && tr.3.is_none() {
                tr.3 = Some(t);
            }
            if dh_rise {
                tr.2 = true;
            }
            if tr.2 && !da && !dh {
                tr.4 = Some(t);
                tr.5 = waiting;
            }
        }
        // (rule A, 7b90c3f1's: D_arr and Dh both off after a Dh edge since the launch)
        if !self.emp && self.flag_a && !da && !dh {
            self.emp = true;
            self.emp_at = Some(t);
            if waiting && self.false_emp.len() < 20 {
                self.false_emp.push(format!("slot {slot} +{} (a cart stopped in the hold)", t - slot * 100));
            } else if moving {
                self.transit_emp += 1;
            }
            if let Some(h) = self.hold_last.filter(|_| !in_hold) {
                self.late_emp.push(t - h);
            }
        }
    }

    pub fn report(&self, cycle: i64) -> (Vec<String>, Vec<String>) {
        let mut out = Vec::new();
        let mut problems = Vec::new();
        let mn = |v: &mut dyn Iterator<Item = i64>| v.min().map_or("-".to_string(), |m| m.to_string());
        let mx = |v: &mut dyn Iterator<Item = i64>| v.max().map_or("-".to_string(), |m| m.to_string());
        out.push(format!(
            "head-perm (S3(ii)'s per-stand latches, {}): {} launches, binds {}{}",
            if self.gate { "gating the head" } else { "observed only" },
            self.launches,
            self.binds.len(),
            if self.binds.is_empty() { String::new() } else { format!(" ({})", self.binds.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }
        ));
        out.push(format!("head-perm: EMP by rule A (D_arr and Dh off after a Dh edge since the launch; gates the head); rule B (its flag also reset by a D_arr rising edge) would have bound {} launches", self.binds_b));
        let arr_set: Vec<i64> = self.slack.iter().filter(|s| s.1 >= 0).map(|s| cycle - s.1).collect();
        let emp_set: Vec<i64> = self.slack.iter().filter(|s| s.2 >= 0).map(|s| cycle - s.2).collect();
        out.push(format!(
            "head-perm: ARR set +{}..+{} after its launch; EMP set +{}..+{}; slack to the next launch of this stand (+{cycle}): ARR min {} EMP min {} gt; to +200 (stands alternating): EMP min {} gt",
            mn(&mut arr_set.iter().copied()),
            mx(&mut arr_set.iter().copied()),
            mn(&mut emp_set.iter().copied()),
            mx(&mut emp_set.iter().copied()),
            mn(&mut self.slack.iter().filter(|s| s.1 >= 0).map(|s| s.1)),
            mn(&mut self.slack.iter().filter(|s| s.2 >= 0).map(|s| s.2)),
            mn(&mut emp_set.iter().map(|e| 200 - e)),
        ));
        out.push(format!("head-perm: EMP set with a cart stopped in the hold: {}{}; with a released cart still crossing the snap: {}; EMP's lag after the hold emptied: min {} max {} gt", self.false_emp.len(), if self.false_emp.is_empty() { String::new() } else { format!(" ({})", self.false_emp.join("; ")) }, self.transit_emp, mn(&mut self.late_emp.iter().copied()), mx(&mut self.late_emp.iter().copied())));
        if !self.false_emp.is_empty() {
            problems.push(format!("head-perm: EMP set {} times with a cart stopped in the hold", self.false_emp.len()));
        }
        (out, problems)
    }

    pub fn detail(&self) -> Vec<String> {
        let mut out = self.lines.clone();
        for (slot, l, _, a, e, stopped) in &self.tracks {
            out.push(format!("head-perm track slot {slot}: launch at +{}: ARR +{} EMP +{}{}", l - slot * 100, a.map_or("-".to_string(), |x| (x - l).to_string()), e.map_or("-".to_string(), |x| (x - l).to_string()), if *stopped { " (a cart stopped in the hold)" } else { "" }));
        }
        out
    }

    /// Each launch's latches followed to completion: ARR and EMP after the launch, and EMP's slack
    /// before X's next launch with stands alternating (+200).
    pub fn tracks_line(&self) -> String {
        let done: Vec<&(i64, i64, bool, Option<i64>, Option<i64>, bool)> = self.tracks.iter().filter(|tr| tr.4.is_some()).collect();
        let a: Vec<i64> = self.tracks.iter().filter_map(|tr| tr.3.map(|x| x - tr.1)).collect();
        let e: Vec<i64> = done.iter().filter_map(|tr| tr.4.map(|x| x - tr.1)).collect();
        format!(
            "head-perm (each launch to completion): {} of {} launches' EMP set; ARR +{}..+{}, EMP +{}..+{} after the launch; slack before X's next launch at +200 (stands alternating): ARR min {} EMP min {} gt; EMP with a cart stopped in the hold: {}",
            done.len(),
            self.tracks.len(),
            a.iter().min().map_or("-".into(), |x| x.to_string()),
            a.iter().max().map_or("-".into(), |x| x.to_string()),
            e.iter().min().map_or("-".into(), |x| x.to_string()),
            e.iter().max().map_or("-".into(), |x| x.to_string()),
            a.iter().max().map_or("-".into(), |x| (200 - x).to_string()),
            e.iter().max().map_or("-".into(), |x| (200 - x).to_string()),
            done.iter().filter(|tr| tr.5).count()
        )
    }
}
