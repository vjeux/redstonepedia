//! `mctas mobfarm f1q2-run` (onechunk, Nov 12; ironspec's deciding test for every rider's bed,
//! the runtime run of condition (i)): the assembled F1 + Q2 module run with REAL brains (the exact
//! villager brain: the mob AI on) and every villager's bed claim measured, as brief rule (c) reads
//! it (ironspec 05:26, G1's reading of 26.3): at night a calm rider's stamp
//! (`CANT_REACH_WALK_TARGET_SINCE`) clears only by a nap or by `MoveToTargetSink` reaching / completing
//! a path to its walk target; panic pauses REST without resetting it; HOME is released at the first
//! calm REST tick with no walk target once the stamp is over 1,200, and the release resets the stamp.
//!
//! Printed: every villager's stamp age at each calm REST tick with no walk target (its largest, where
//! and when, and the ticks over 1,000), every clear (its cause: a nap, the target reached, a complete
//! path; its target: the bed, a flee point, a stroll point; where), HOME tracked every tick (a loss:
//! its tick and place) and HOME at dawn. Bar: the stamp <= 1,000 at every calm REST tick at night and
//! HOME kept by every rider. The positive control (`--control-rider N`): the N-th villager's clears
//! undone every tick, so it must release HOME.
//!
//! Phase 1 (this file's first form): the riders seated where the module places them (the station's
//! pockets and holds) plus the lanes' stored carts (`--lane-carts`: R_1 and the 5 slopes of each lane,
//! 8 a lane with the pocket and the hold), the scarer set and a fix set pasted in as parts of the
//! module (`mobfarm column`), no drive yet: every cart at rest. The controller (day-1's table v6) and
//! the parts' emulated drives come next; every drive will be labelled EMULATED.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use std::collections::BTreeMap;

/// The run's options.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: i64,
    /// Game ticks to run (from `day0`).
    pub ticks: i64,
    /// The day time at the start.
    pub day0: i64,
    /// The lanes' stored carts added (R_1 and the 5 slopes of lanes x -1 / 3 / 7).
    pub lane_carts: bool,
    /// Riders a lane at the start, the pocket and the hold included (8: R_1 and 5 slopes; 7:
    /// ironspec's steady start, the top slope empty).
    pub per_lane: usize,
    /// tick-exact's official lane x 3 scene (07:54) instead of the module's night: the night held at
    /// day time 18,000, the file's carts left out (its boats, the scarers, kept; the bed stop's
    /// zombie at (-4.5,-18,3.5)), 7 riders entering lane x 3 one per 30 gt in a cart at rest at
    /// (3.6, 24.0, 6.49), and every 100 gt the cart on H's trapdoor (3,16,0) and its rider taken out
    /// and a rider re-entering at the top (a FRESH villager: redsim seats a villager in a cart only at
    /// its spawn; the official run re-seats the same one), R_1 (3,17,1) powered without updates for
    /// 12 gt.
    pub official_lane3: bool,
    /// Phase 3a: a napper seeded each night at the stop's entry (3,-16,-5), running west at 0.2,
    /// HOME the bed, its lane rotating -1 / 3 / 7; the stop, the hold, K and the siding driven
    /// (EMULATED: day-1's events 2026-11-12 10:00, 58f770cc's stop).
    pub napper: bool,
    /// (ironspec 11:15, 9fc3155f's statics) S-bend seats whose stamp resets are counted: `NAME`,
    /// the seat's rail cell, its clearing candidates (a complete draw to one of them while a rider
    /// sits on the seat resets its stamp).
    pub seats: Vec<(String, [i32; 3], Vec<[i32; 3]>)>,
    /// tick-exact's per-draw log of the official lane run (its lines `t<tick> r<k> rider cell
    /// (x,y,z) ... | target (x,y,z) <outcome> | path +<delay> complete|PARTIAL (...)`, `| null` or
    /// `never pathed`): its per-block tally printed beside redsim's.
    pub compare_official: Option<String>,
    /// Restarts (liveness::restart, between two ticks) at these ticks from the start.
    pub restarts: Vec<i64>,
    pub restart: crate::liveness::Restart,
    /// The positive control: this villager (its index among the villagers at the start) has every
    /// clear of its stamp undone.
    pub control_rider: Option<usize>,
    /// A trace line every N gt (0: none).
    pub trace: i64,
    /// (ironfarm 11:19) The napper's per-tick trace over these ticks from the start (FROM, TO).
    pub nap_trace: Option<(i64, i64)>,
    /// (ironspec 11:29) A neighbour copy's offset (the east neighbour: 32,0,0): its pad golems
    /// EMULATED as ours moved by it (the same cycle, the same phase: the worst case), each setting
    /// GOLEM_DETECTED_RECENTLY on every rider whose 16-block box it meets (GolemSensor over
    /// NEAREST_LIVING_ENTITIES); and its pad's detection box (the pad cells moved, inflated 16).
    pub neighbour: Option<[i32; 3]>,
    /// (ironfarm 11:40) A cart's cells over FROM..TO: (cart id, FROM, TO); and each lane's
    /// occupancy (carts in H..V: x = the lane, y 16..22, z 0..6) at every release in FROM..TO,
    /// with the ticks V (x,22,6) held a cart.
    pub cart_trace: Option<(u32, i64, i64)>,
    /// The shared bed's head (the module's `home` port).
    pub bed: [i32; 3],
    /// Phase 2: the station, the lanes' R line and the split's latches driven (every drive
    /// EMULATED, day-1's table v6 at s 12: the golem pulse starts a cycle, REL = golem + 4, the
    /// pockets launched at REL + 0 / 12 / 24).
    pub drive: bool,
}

/// The three lanes (x) and their launch offsets after REL (s 12).
const LANES: [i32; 3] = [-1, 3, 7];
const OFFS: [i64; 3] = [0, 12, 24];

/// A rider's place kind by its cell: 0 the pocket (x,0,0), 1 the lane (R_1 (x,17,1) and the slopes
/// (x,18..22,2..6)), 2 the siding (the S-bend's K (x-2,25,9), F (x-2,25,8), R (x-2,24,7)), 3
/// elsewhere.
fn place_of(c: [i32; 3]) -> usize {
    for x in LANES {
        if c == [x, 0, 0] {
            return 0;
        }
        if c == [x, 17, 1] || (0..5).any(|k| c == [x, 18 + k, 2 + k]) {
            return 1;
        }
        if c == [x - 2, 24, 7] || c == [x - 2, 25, 8] || c == [x - 2, 25, 9] {
            return 2;
        }
    }
    3
}

/// Phase 2's controller and the parts' emulated drives (every one labelled EMULATED in the
/// report): the station's L1 events (dfb89499: each pocket's switched support (x,-1,0) a redstone
/// block at REL + offset for 16 gt; its hold's trapdoor (x,16,0) opened 4 gt after the pocket's exit
/// detector (x,0,2) rises past REL + offset, or at REL + offset + 4 when the pocket string (x,1,0) is
/// off then (the cold start), shut 12 gt after; all three shut: + 2 gt, the R line: every R_1's side
/// power (x+1,17,1) a redstone block for 30 gt (ironfarm)), the split's latches (7b90c3f1: J(-1)
/// set at idle; lane -1's branch rise (-1,24,8) clears it and sets J(3); lane 3's (3,24,8) clears
/// J(3); the third cart's rise at (5,24,10) sets J(-1); J(7) stays turned), and the cycle (day-1's
/// v6 at s 12: a golem's appearance is the golem pulse; REL = its tick + 2 (into the pad portal) + 4;
/// a pulse while a cycle runs is latched (P) and starts the next one when it ends).
/// (ironspec 09:59) Villagers within 16 of the bed head: the cells, the villager-ticks, and the
/// calm (not panicking) REST ticks while the bed is occupied (the bar: 0).
#[derive(Default)]
struct Near {
    occupied_ticks: i64,
    cells: BTreeMap<[i32; 3], i64>,
    ticks: i64,
    calm_rest_occupied: i64,
    calm_rest_rows: Vec<String>,
}

/// (ironspec 11:02) The staleness bar's entries: into a hidden-within-6.05 position, the gap from
/// the rider's last in-sight-within-8 position (bar >= 40 gt).
#[derive(Default)]
struct Stale {
    entries: i64,
    bad: i64,
    least: Option<i64>,
    rows: Vec<String>,
}

/// (ironspec 11:15) A seat's stamp resets: rider-ticks on it, complete draws to its candidates
/// from it (each a reset), by candidate, and the longest stretch a rider sat without one.
#[derive(Default, Clone)]
struct SeatStat {
    occupied: i64,
    resets: i64,
    by_cand: BTreeMap<[i32; 3], i64>,
    longest: i64,
}

/// (ironspec 11:29) GOLEM_DETECTED_RECENTLY at the stand arrivals.
#[derive(Default)]
struct Gdr {
    arrivals: i64,
    blocked: i64,
    late: i64,
    least_margin: Option<i64>,
    neighbour_sets: i64,
    least_box_gap: Option<i64>,
    box_short: i64,
    rows: Vec<String>,
    box_rows: Vec<String>,
    napper_rows: Vec<String>,
}

/// (farmopt 12:10) The summons from the pockets: attempts (a scared, slept villager free of the
/// memory in a pocket at a summon tick), villagers rejected for GOLEM_DETECTED_RECENTLY, and the
/// summons those rejections blocked (fewer than 3 counted, 3 or more with them).
#[derive(Default)]
struct Summons {
    attempts: i64,
    rejected: i64,
    blocked: i64,
    rows: Vec<String>,
    /// Callers in a pocket at a summon tick with the memory still set (they can't call), the
    /// least margin of a free caller's memory (T - (last set + 599)), each lane's least rotation
    /// at a summon tick.
    callers_blocked: i64,
    least_caller_margin: Option<i64>,
    min_rotation: [Option<i64>; 3],
    /// (ironspec 12:48) the blocked callers by their memory's last setter.
    blocked_by: BTreeMap<&'static str, i64>,
}

/// (ironspec 12:23) GOLEM_DETECTED_RECENTLY's settings by setter: a summon's 10-box stamp, or the
/// GolemSensor (how many within 600 gt after the rider's own summon's stamp).
#[derive(Default)]
struct Renewals {
    stamps: i64,
    sensor: i64,
    neighbour: i64,
    after_own: i64,
    rows: Vec<String>,
    /// (ironspec 12:48) the stamps split by recipient; each bystander's expiry against its next
    /// summon.
    summoner_stamps: i64,
    bystander_stamps: i64,
    bystander_margin: Option<i64>,
    bystander_rows: Vec<String>,
}

#[derive(Default)]
struct Ctl {
    golems: std::collections::BTreeSet<u32>,
    busy: bool,
    pending: bool,
    rel: Option<i64>,
    support_off: [Option<i64>; 3],
    d_prev: [bool; 3],
    armed: [bool; 3],
    trap_open: [Option<i64>; 3],
    trap_shut: [Option<i64>; 3],
    shut_done: [bool; 3],
    r_on: Option<i64>,
    r_off: Option<i64>,
    /// (the built drive) the repeater's input block off.
    r_input_off: Option<i64>,
    branch_prev: [bool; 2],
    row_prev: bool,
    cycles: i64,
    log: Vec<String>,
    // phase 3a, the nappers (day-1's events 2026-11-12 10:00; 58f770cc's stop): EMULATED
    napper: bool,
    /// The stop's cells' y offset against rev 3's (the bed head's y + 18: -9 with the stop 9 down).
    stop_dy: i32,
    /// This cycle's ARM3: the main line's detector (5,0,7) risen at or after REL + 24 (the
    /// batch's third cart, lane 7's).
    arm3: Option<i64>,
    det5_prev: bool,
    /// The stop: the bed's lie-down edge, F (the hatch's controls off: the zombie seen) 10 gt
    /// after it, the release (the park's supports to redstone blocks) 30 gt after F; R's detector
    /// re-arms it (the park braked, the hatch open).
    bed_prev: bool,
    f_at: Option<i64>,
    release_at: Option<i64>,
    rdet_prev: bool,
    /// The hold (c' as built): the waiting RS set by the lift exit (7,-3,4) (its delayed Q 38 gt
    /// later), reset by the fired pulse; the input (7,-1,8) on at ARM3 + 10 for 12 gt.
    rs_set: Option<i64>,
    hold_on: Option<i64>,
    hold_off: Option<i64>,
    /// The waiting napper's lane (an index into LANES) and its cycle's ARM3.
    nap_lane: usize,
    nap_arm3: Option<i64>,
    /// K(n) set at ARM3 + 93 / 108 / 124 (+ the driver's 4), reset at ARM3 + 125 / 140 / 156 (+ 6),
    /// that reset's edge setting PRESENT_l.
    k_on: Option<i64>,
    k_off: Option<i64>,
    present_at: Option<i64>,
    present: [bool; 3],
    /// J(-1) kept from re-arming from ARM3 + 72 to ARM3 + 127 for an eastbound napper (lanes 3, 7),
    /// re-armed at its end.
    j_block: Option<(i64, i64)>,
    /// The siding's release: D's rise at (x,24,8) while PRESENT_l: the input (x-2,23,9) on 4 gt
    /// later for 30 gt.
    siding_on: [Option<i64>; 3],
    siding_off: [Option<i64>; 3],
    d7_prev: bool,
    branch_prev_nap: [bool; 2],
    /// A ridden cart on the hold last tick (the departure's edge).
    hold_prev: bool,
    /// The napper's arrival on the hold and its first tick at rest there; pulses fired with the
    /// hold empty (bar 0).
    hold_enter: Option<i64>,
    hold_rest: Option<i64>,
    empty_pulses: i64,
    /// The events, for the napper's lines.
    nap_events: Vec<String>,
}

fn set(w: &mut redsim::world::World, p: [i32; 3], state: &str, flags: u32) -> Result<(), String> {
    let b = redsim::block::Block::parse(state).map_err(|e| format!("{state}: {e}"))?;
    w.set_block(redsim::pos::BlockPos::new(p[0], p[1], p[2]), b, flags);
    Ok(())
}

fn powered(w: &redsim::world::World, p: [i32; 3]) -> bool {
    w.get(redsim::pos::BlockPos::new(p[0], p[1], p[2])).to_string().contains("powered=true")
}

impl Ctl {
    fn note(&mut self, t: i64, s: String) {
        if self.log.len() < 400 {
            self.log.push(format!("t {t}: {s}"));
        }
    }

    /// One tick's drives, after the world's tick at `now` (`rel` the run's tick).
    fn step(&mut self, w: &mut redsim::world::World, rel: i64) -> Result<(), String> {
        use redsim::world::flags;
        let now = rel;
        // the golem pulse: a golem appeared
        let new: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::IronGolem))).map(|e| e.id).filter(|id| !self.golems.contains(id)).collect();
        for id in new {
            self.golems.insert(id);
            if self.busy {
                self.pending = true;
                self.note(now, format!("golem {id}: the pulse latched (P), a cycle running"));
            } else {
                self.start(now + 2 + 4);
                self.note(now, format!("golem {id}: the pulse; REL at t {}", now + 6));
            }
        }
        if let Some(r) = self.rel {
            for k in 0..3 {
                let x = LANES[k];
                if now == r + OFFS[k] {
                    set(w, [x, -1, 0], "minecraft:redstone_block", flags::ALL)?;
                    self.support_off[k] = Some(now + 16);
                    self.armed[k] = true;
                    // (the cold start: no cart on the pocket's string, the trapdoor opens anyway)
                    if !powered(w, [x, 1, 0]) {
                        self.trap_open[k] = Some(now + 4);
                        self.armed[k] = false;
                    }
                }
                if self.support_off[k] == Some(now) {
                    set(w, [x, -1, 0], "minecraft:stone", flags::ALL)?;
                    self.support_off[k] = None;
                }
                let d = powered(w, [x, 0, 2]);
                if self.armed[k] && d && !self.d_prev[k] && now >= r + OFFS[k] {
                    self.trap_open[k] = Some(now + 4);
                    self.armed[k] = false;
                }
                self.d_prev[k] = d;
                if self.trap_open[k] == Some(now) {
                    set(w, [x, 16, 0], "minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]", flags::ALL)?;
                    self.trap_shut[k] = Some(now + 12);
                    self.trap_open[k] = None;
                }
                if self.trap_shut[k] == Some(now) {
                    set(w, [x, 16, 0], "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]", flags::ALL)?;
                    self.trap_shut[k] = None;
                    self.shut_done[k] = true;
                }
            }
            if self.shut_done.iter().all(|s| *s) && self.r_on.is_none() && self.r_off.is_none() {
                self.r_on = Some(now + 2);
            }
        }
        if self.r_on == Some(now) {
            for x in LANES {
                // (dfb89499's built drive, b4549f35a: R_1's repeater at (x+1,17,1), its input
                // (x+2,17,1) a redstone block for 10 gt; the old file: the block in (x+1,17,1))
                if w.get(redsim::pos::BlockPos::new(x + 1, 17, 1)).to_string().contains("repeater") {
                    set(w, [x + 2, 17, 1], "minecraft:redstone_block", flags::ALL)?;
                    self.r_input_off = Some(now + 10);
                } else {
                    set(w, [x + 1, 17, 1], "minecraft:redstone_block", flags::ALL)?;
                }
            }
            self.r_on = None;
            self.r_off = Some(now + 30);
        }
        if self.r_input_off == Some(now) {
            for x in LANES {
                set(w, [x + 2, 17, 1], "minecraft:air", flags::ALL)?;
            }
            self.r_input_off = None;
        }
        if self.r_off == Some(now) {
            for x in LANES {
                if !w.get(redsim::pos::BlockPos::new(x + 1, 17, 1)).to_string().contains("repeater") {
                    set(w, [x + 1, 17, 1], "minecraft:glass", flags::ALL)?;
                }
            }
            self.r_off = None;
            self.busy = false;
            self.rel = None;
            self.shut_done = [false; 3];
            self.cycles += 1;
            self.note(now, format!("cycle {} done (the R line off)", self.cycles));
            if self.pending {
                self.pending = false;
                self.start(now + 4);
                self.note(now, format!("the latched pulse starts the next cycle: REL at t {}", now + 4));
            }
        }
        // the split's latches (7b90c3f1's K re-lay on split9, 7b21f939a: the row along z 9 at y 25,
        // each junction J (x,25,9) and K (x-2,25,9) switched by its lever (x,26,9), the latch's
        // output (EMULATED: the lever toggled with flags ALL, redsim settling the junction, as the
        // split's own bench does); the branch detectors are the detector slopes D (x,24,8); the
        // row's own detectors are gone, so the batch's third cart is seen at lane 7's D (7,24,8),
        // which re-arms J(-1); the K's stay off (through) with no napper in the run)
        let b0 = powered(w, [-1, 24, 8]);
        let b1 = powered(w, [3, 24, 8]);
        let row = powered(w, [7, 24, 8]);
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        if b0 && !self.branch_prev[0] {
            set(w, [-1, 26, 9], &lever(false), flags::ALL)?;
            set(w, [3, 26, 9], &lever(true), flags::ALL)?;
        }
        if b1 && !self.branch_prev[1] {
            set(w, [3, 26, 9], &lever(false), flags::ALL)?;
        }
        // (an eastbound napper's window keeps J(-1) from re-arming)
        let j_blocked = self.j_block.is_some_and(|(a, b)| now >= a && now < b);
        if row && !self.row_prev && !j_blocked {
            set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
        }
        self.branch_prev = [b0, b1];
        self.row_prev = row;
        if self.napper {
            self.nap_step(w, now, [b0, b1, row])?;
        }
        Ok(())
    }

    /// Phase 3a's drives (EMULATED; day-1's events 2026-11-12 10:00, 58f770cc's stop): the stop's
    /// F, release and re-arm; the hold's presence-gated pulse; K(n) for the napper's lane; J(-1)'s
    /// block for an eastbound napper; the siding's release. `d` the branch detectors' states
    /// (lanes -1 / 3 / 7) this tick.
    fn nap_step(&mut self, w: &mut redsim::world::World, now: i64, d: [bool; 3]) -> Result<(), String> {
        use redsim::world::flags;
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        // ARM3: the main line's detector risen at or after REL + 24
        let det5 = powered(w, [5, 0, 7]);
        if det5 && !self.det5_prev && self.arm3.is_none() && self.rel.is_some_and(|r| now >= r + 24) {
            self.arm3 = Some(now);
        } else if det5 && !self.det5_prev {
            // (ironspec 11:05) any other rise of (5,0,7) (the napper rolling over it after the hold,
            // or a rise before REL + 24): logged with the ARM3 consumers' states around it; the
            // emulation latches ARM3 once a cycle, so a second rise changes nothing here (the
            // built circuit's consumers: day-1's list)
            let lv = |x: i32| w.get(redsim::pos::BlockPos::new(x, 26, 9)).to_string().contains("powered=true");
            self.nap_events.push(format!(
                "t {now}: (5,0,7) rose again ({}): the emulation keeps ARM3 latched; the levers J/K (x,26,9) at x -3 {}, -1 {}, 1 {}, 3 {}, 5 {}; the hold input (7,-1,8) {}; a cycle {}",
                self.arm3.map_or_else(|| self.rel.map_or("no cycle".to_string(), |r| format!("REL + {}", now - r)), |a| format!("ARM3 + {}", now - a)),
                lv(-3),
                lv(-1),
                lv(1),
                lv(3),
                lv(5),
                if w.get(redsim::pos::BlockPos::new(7, -1, 8)).to_string().contains("redstone_block") { "on" } else { "off" },
                if self.busy { "running" } else { "idle" }
            ));
        }
        self.det5_prev = det5;
        // the stop (its cells moved with it: `stop_dy`, the bed head's y + 18; ironspec 12:23 put
        // the stop 9 down)
        let sy = self.stop_dy;
        let bed = matches!(w.get(redsim::pos::BlockPos::new(-3, -18 + sy, 3)).to_string(), s if s.contains("occupied=true"));
        if bed && !self.bed_prev {
            self.f_at = Some(now + 10);
            self.nap_events.push(format!("t {now}: lie-down (the bed occupied)"));
        }
        self.bed_prev = bed;
        if self.f_at == Some(now) {
            set(w, [-4, -18 + sy, 2], "minecraft:stone", flags::ALL)?;
            set(w, [-4, -16 + sy, 3], "minecraft:stone", flags::ALL)?;
            self.release_at = Some(now + 30);
            self.f_at = None;
            self.nap_events.push(format!("t {now}: F (the hatch's controls off)"));
        }
        if self.release_at == Some(now) {
            set(w, [1, -18 + sy, -5], "minecraft:redstone_block", flags::ALL)?;
            set(w, [0, -19 + sy, -5], "minecraft:redstone_block", flags::ALL)?;
            self.release_at = None;
            self.nap_events.push(format!("t {now}: the stop's release (the park's supports on)"));
        }
        let rdet = powered(w, [4, -19 + sy, 4]);
        if rdet && !self.rdet_prev {
            set(w, [1, -18 + sy, -5], "minecraft:stone", flags::ALL)?;
            set(w, [0, -19 + sy, -5], "minecraft:stone", flags::ALL)?;
            set(w, [-4, -18 + sy, 2], "minecraft:redstone_block", flags::ALL)?;
            set(w, [-4, -16 + sy, 3], "minecraft:redstone_block", flags::ALL)?;
            self.nap_events.push(format!("t {now}: R's detector: the stop re-armed"));
        }
        self.rdet_prev = rdet;
        // (ironspec's hold release (c') as built, 11:49) the lift exit (7,-3,4) as a detector rail
        // sets the waiting RS; its Q delayed 38 gt; E samples live Q AND delayed Q at its edge (ARM3
        // + 2); locked: the 12-gt pulse fires (the input (7,-1,8) at ARM3 + 10, the rail at ARM3 +
        // 12) and resets the RS (the departing napper's second ARM3 rise fires nothing); the fired
        // pulse keys K (on ARM3 + 93 / 108 / 124, off ARM3 + 125 / 140 / 156: 7b90c3f1's numbers),
        // PRESENT_l at the reset, and J(-1)'s block ARM3 + 72..127 for an eastbound napper
        let at_exit = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [7, -3, 4]);
        if at_exit && self.rs_set.is_none() {
            self.rs_set = Some(now);
            self.nap_events.push(format!("t {now}: the lift exit (7,-3,4): the waiting RS set (its delayed Q at t {})", now + 38));
        }
        let on_hold = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [9, 0, 8]);
        if on_hold && !self.hold_prev {
            self.hold_enter = Some(now);
            self.hold_rest = None;
        }
        if on_hold
            && self.hold_rest.is_none()
            && w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [9, 0, 8] && e.delta_movement.x.abs() + e.delta_movement.z.abs() < 1e-3)
        {
            self.hold_rest = Some(now);
            if let Some(s) = self.rs_set {
                self.nap_events.push(format!("t {now}: the napper at rest on the hold; the delayed Q reaches the sampler at t {} ({:+} gt against the rest)", s + 38, s + 38 - now));
            }
        }
        if let Some(a) = self.arm3
            && now == a + 2
            && let Some(s) = self.rs_set
            && now >= s + 38
        {
            let k = self.nap_lane;
            self.hold_on = Some(a + 10);
            self.hold_off = Some(a + 22);
            self.rs_set = None;
            self.nap_arm3 = Some(a);
            self.k_on = Some(a + [93, 108, 124][k]);
            self.k_off = Some(a + [125, 140, 156][k]);
            self.present_at = Some(a + [125, 140, 156][k]);
            if k > 0 {
                self.j_block = Some((a + 72, a + 127));
            }
            if !on_hold {
                self.empty_pulses += 1;
            }
            self.nap_events.push(format!(
                "t {now}: E's rise (ARM3 t {a} + 2): live Q and delayed Q, the 12-gt pulse fires and resets the RS{}; K / PRESENT / J keyed to it (lane x {})",
                if on_hold { self.hold_rest.map_or(", the napper on the hold still ROLLING".to_string(), |r| format!(", the napper at rest on the hold since t {r}")) } else { ", the HOLD EMPTY (a failure)".to_string() },
                LANES[k]
            ));
        }
        // (the departure, logged against E's rise)
        let past_hold = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [9, 0, 7]);
        if self.hold_prev && !on_hold && past_hold {
            self.nap_events.push(format!("t {now}: the napper off the hold{}", self.arm3.map(|a| format!(" (E's rise + {})", now - a - 2)).unwrap_or_default()));
        }
        self.hold_prev = on_hold;
        if self.hold_on == Some(now) {
            set(w, [7, -1, 8], "minecraft:redstone_block", flags::ALL)?;
            self.hold_on = None;
        }
        if self.hold_off == Some(now) {
            set(w, [7, -1, 8], "minecraft:air", flags::ALL)?;
            self.hold_off = None;
        }
        let kx = LANES[self.nap_lane] - 2;
        if self.k_on == Some(now) {
            set(w, [kx, 26, 9], &lever(true), flags::ALL)?;
            self.k_on = None;
            self.nap_events.push(format!("t {now}: K({kx}) on"));
        }
        if self.present_at == Some(now) {
            self.present[self.nap_lane] = true;
            self.present_at = None;
        }
        if self.k_off == Some(now) {
            set(w, [kx, 26, 9], &lever(false), flags::ALL)?;
            self.k_off = None;
            self.nap_events.push(format!("t {now}: K({kx}) off"));
        }
        if let Some((_, b)) = self.j_block
            && now == b
        {
            set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
            self.j_block = None;
            self.nap_events.push(format!("t {now}: J(-1) re-armed (the napper's block ends)"));
        }
        // the siding: D's rise while PRESENT_l (the branch detectors' rises, this tick)
        let prev = [self.branch_prev_nap[0], self.branch_prev_nap[1], self.d7_prev];
        for k in 0..3 {
            if d[k] && !prev[k] && self.present[k] {
                self.siding_on[k] = Some(now + 4);
                self.present[k] = false;
            }
            let sx = LANES[k] - 2;
            if self.siding_on[k] == Some(now) {
                set(w, [sx, 23, 9], "minecraft:redstone_block", flags::ALL)?;
                self.siding_on[k] = None;
                self.siding_off[k] = Some(now + 30);
                self.nap_events.push(format!("t {now}: the siding's release, lane x {} (the input ({sx},23,9) on 30 gt)", LANES[k]));
            }
            if self.siding_off[k] == Some(now) {
                set(w, [sx, 23, 9], "minecraft:air", flags::ALL)?;
                self.siding_off[k] = None;
            }
        }
        self.branch_prev_nap = [d[0], d[1]];
        self.d7_prev = d[2];
        if self.nap_arm3.is_some() && self.k_off.is_none() && self.present.iter().all(|p| !p) && self.siding_off.iter().all(|s| s.is_none()) && self.present_at.is_none() {
            self.nap_arm3 = None;
        }
        Ok(())
    }

    fn start(&mut self, rel: i64) {
        self.busy = true;
        self.rel = Some(rel);
        self.armed = [false; 3];
        self.arm3 = None;
    }

    /// The idle state at the start (the split's latches): J(-1) on (into lane -1), J(3) off
    /// (through), J(7) on (into lane 7).
    fn init(&mut self, w: &mut redsim::world::World) -> Result<(), String> {
        use redsim::world::flags;
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        for (x, on) in [(-1, true), (3, false), (7, true)] {
            if !w.get(redsim::pos::BlockPos::new(x, 26, 9)).to_string().contains("lever") {
                return Err(format!("no junction lever at ({x},26,9): the drive takes c85331aaf's in-chunk split"));
            }
            set(w, [x, 26, 9], &lever(on), flags::ALL)?;
        }
        Ok(())
    }
}

/// One villager's record.
#[derive(Clone, Debug, Default)]
struct Rec {
    label: String,
    /// The largest stamp age at a calm REST tick with no walk target: (age, tick, cell).
    peak: Option<(i64, i64, [i32; 3])>,
    calm_rest_ticks: i64,
    over_1000: i64,
    clears: Vec<String>,
    home_lost: Vec<String>,
    home_at_start: Option<[i32; 3]>,
    home_at_dawn: Option<Option<[i32; 3]>>,
    prev_stamp: Option<i64>,
    prev_home: Option<[i32; 3]>,
    prev_target: Option<[f64; 3]>,
    naps: i64,
    /// The pocket turn in progress.
    turn: Option<Turn>,
    prev_panic: bool,
    /// The stamp's age at dusk (the first REST tick): None before it, Some(None) when unarmed.
    dusk: Option<Option<i64>>,
    /// Daytime clears (outside REST) by place: the pocket, the lane (R_1 and the slopes), the
    /// siding (the S-bend's K, F, R), elsewhere.
    day_clears: [i64; 4],
    laps: i64,
    /// Every clear by place (pocket, lane, siding, elsewhere) and cause (a nap, the target reached,
    /// a complete path, no target on record).
    clears_by: [[i64; 4]; 4],
    last_clear: Option<i64>,
    /// The longest stretch between two clears: (gt, from, to); and of those ending in REST.
    worst_gap: Option<(i64, i64, i64)>,
    worst_night_gap: Option<(i64, i64, i64)>,
    /// By place: draws (a new walk target) whose path, one tick on, was complete / partial (the
    /// sink stuck: WALK_TARGET held, no complete path) / none; and the ticks WALK_TARGET was held on
    /// a partial path.
    draws_by: [[i64; 3]; 4],
    /// The same for the draws landed while panicking (the clears' draws).
    draws_by_panic: [[i64; 3]; 4],
    stuck_ticks: [i64; 4],
    /// A draw awaiting its classification: (its place, the tick it landed, landed in panic, its
    /// target's cell, the rider's cell then).
    pending_draw: Option<(usize, i64, bool, [i32; 3], [i32; 3])>,
    /// The pending draw's NEAREST_HOSTILE and the rider's cell at the draw.
    draw_hostile: Option<u32>,
    draw_cell: Option<[i32; 3]>,
    /// The rider's exact position at the draw.
    draw_pos: [f64; 3],
    /// The draw tick's lane state (its cart, the lane's other carts, their velocities).
    draw_ctx: String,
    /// A partial draw's hold being measured: (its row in the table, its target, its tick).
    holding: Option<(usize, [i32; 3], i64)>,
    /// The target's identity last tick and the pending draw's (a cell, or the entity followed).
    prev_key: Option<(u8, i64, i64, i64)>,
    /// (the staleness bar) the rider's last tick within 8 of a zombie in its sight, and whether it
    /// was hidden within 6.05 of one last tick.
    last_seen8: Option<i64>,
    prev_hidden605: bool,
    /// (the seats) the tick this rider sat on a seat or had its stamp reset there, and which seat.
    seat_since: Option<i64>,
    seat_last: Option<usize>,
    /// (a napper) its cells in order from its first K until its lane's top.
    route: Vec<[i32; 3]>,
    /// (ironspec 11:29) GOLEM_DETECTED_RECENTLY: the last tick it was set (any source), the last
    /// tick the neighbour's golems set it (EMULATED), the last tick inside the neighbour pad's
    /// detection box, whether the cart was at the stand last tick; a napper's exit from the bed's
    /// 16-ball after R, in gt.
    gdr_set: Option<i64>,
    /// (ironspec 12:23) the tick of the rider's own summon's stamp (the memory set the tick a golem
    /// was born).
    last_stamp: Option<i64>,
    /// (ironspec 12:13) the lane whose slopes the rider last rode (x = the lane, y 16..22, z 0..6).
    lane: Option<i32>,
    /// (ironspec 12:48) who set the memory last; a bystander's stamp (tick, position) pending its
    /// next summon.
    gdr_setter: Option<&'static str>,
    bystander_stamp: Option<(i64, [f64; 3])>,
    gdr_neighbour: Option<i64>,
    in_neighbour_box: Option<i64>,
    prev_at_stand: bool,
    ball_exit_after_r: Option<i64>,
    pending_key: Option<(u8, i64, i64, i64)>,
    /// A restart (liveness::restart) just reloaded it: its next missing stamp is the reload's.
    reloaded: bool,
    reload_drops: i64,
    /// A napper's milestones (phase 3a): what, the tick.
    milestones: Vec<(String, i64)>,
}

/// One pocket turn of a rider.
#[derive(Clone, Debug, Default)]
struct Turn {
    lane: usize,
    start: i64,
    panic: i64,
    landed_reach: i64,
    landed_unreach: i64,
    nopath: i64,
    cleared: bool,
    /// The panic's onset: its tick, the WALK_TARGET holder then, the sink's remainingCooldown.
    onset: Option<(i64, String, Option<i32>)>,
    /// The first tick after the onset with no WALK_TARGET.
    freed: Option<i64>,
}

/// A lane's pocket turns.
#[derive(Clone, Copy, Debug, Default)]
struct Turns {
    turns: i64,
    cleared: i64,
    draws: i64,
    panic: i64,
    landed_reach: i64,
    landed_unreach: i64,
    nopath: i64,
}

/// tick-exact's per-draw log: per rider block, [null, complete, PARTIAL, never pathed or no path].
fn parse_official(path: &str) -> BTreeMap<[i32; 3], [i64; 4]> {
    let mut m: BTreeMap<[i32; 3], [i64; 4]> = BTreeMap::new();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let cell_after = |l: &str, key: &str| -> Option<[i32; 3]> {
        let i = l.find(key)? + key.len();
        let rest = &l[i..];
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        let v: Vec<i32> = rest[open + 1..close].split(',').filter_map(|t| t.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    for l in text.lines() {
        if !l.starts_with('t') || !l.contains("rider cell") {
            continue;
        }
        let Some(c) = cell_after(l, "rider cell") else { continue };
        let k = if l.trim_end().ends_with("| null") {
            0
        } else if l.contains("PARTIAL") {
            2
        } else if l.contains(" complete (path") {
            1
        } else {
            3
        };
        m.entry(c).or_insert([0; 4])[k] += 1;
    }
    m
}

/// The lanes' stored carts: R_1 (x,17,1) and the slopes (x,18..22,2..6) of lanes x -1 / 3 / 7, a
/// villager seated in each (the lanes file places no cart); `slopes` of the 5 from the bottom (4:
/// ironspec's steady start state, 7 riders a lane with the pocket and the hold, the top slope empty).
pub fn lane_carts(slopes: usize) -> Vec<(String, [f64; 3], Option<String>)> {
    let mut v = Vec::new();
    for x in [-1, 3, 7] {
        // (on a slope a cart rides half a block over its cell's floor)
        v.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, 17.5625, 1.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string())));
        for k in 0..slopes.min(5) {
            v.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, 18.0 + k as f64 + 0.5625, 2.5 + k as f64], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string())));
        }
    }
    v
}

fn cell(p: redsim::entity::Vec3) -> [i32; 3] {
    [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
}

/// One run: the report.
pub fn run(spec: &FarmSpec, o: &Opts) -> Result<String, String> {
    let mut sp = spec.clone();
    if o.official_lane3 {
        // (tick-exact's scene: the module's carts out, its boats (the scarers) kept, the bed stop's
        // zombie where the official run seats it)
        sp.entities.retain(|e| e.0.contains("boat"));
        for e in sp.entities.iter_mut() {
            // (the bed stop's boat alone, matched by its cell: rev 2's at (-5,-18,3.5))
            if e.1[0].floor() as i32 == -5 && e.1[1].floor() as i32 == -18 && e.1[2].floor() as i32 == 3 {
                e.1 = [-4.5, -18.0, 3.5];
            }
        }
    } else if o.lane_carts {
        // (ironfarm 11:40: the napper is one of its lane's 8 carts; phase 3a seeds it at the stop,
        // so its first lane starts one slope cart short)
        let nap_lane = o.napper.then(|| LANES[o.seed.rem_euclid(3) as usize % 3]);
        let mut carts = lane_carts(o.per_lane.saturating_sub(3));
        if let Some(x) = nap_lane
            && let Some(k) = carts.iter().rposition(|c| (c.1[0] - (x as f64 + 0.5)).abs() < 0.01 && c.1[1] > 18.0)
        {
            carts.remove(k);
        }
        sp.entities.extend(carts);
    }
    anyhow_free(sp.has_bed(), "the module holds no bed: no villager gets a HOME")?;
    let ro = RunOpts { ai: true, day0: if o.official_lane3 { 18_000 } else { o.day0 }, slept: true, ..RunOpts::steady(o.seed, (o.ticks + 200) as u32, WalkMode::Random) };
    // (the shared bed, as the station bench sets it: the farm built homeless, the bed's POI taken,
    // every villager's HOME the bed and LAST_SLEPT 100 gt back)
    let mut run = Run::new_homeless(&sp, &ro)?;
    let bed = redsim::pos::BlockPos::new(o.bed[0], o.bed[1], o.bed[2]);
    redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    let t0 = run.world.time;
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        if e.removed || !matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager)) {
            continue;
        }
        let b = redsim::entity::mob::ai::brain::brain_mut(&mut run.world, i);
        b.home = Some(bed);
        b.last_slept = Some(t0 - 100);
    }
    let mut recs: BTreeMap<u32, Rec> = BTreeMap::new();
    let villagers = |w: &redsim::world::World| -> Vec<u32> { w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager))).map(|e| e.id).collect() };
    for (k, id) in villagers(&run.world).into_iter().enumerate() {
        let i = redsim::entity::index_of(&run.world, id).ok_or("a villager vanished")?;
        let e = &run.world.entities.list[i];
        let b = redsim::entity::mob::ai::brain::brain(&run.world, i);
        let home = b.home.map(|h| [h.x, h.y, h.z]);
        recs.insert(id, Rec { label: format!("v{k} (id {id}, seated at {:?})", cell(e.pos)), home_at_start: home, prev_home: home, ..Default::default() });
    }
    let control_id: Option<u32> = o.control_rider.and_then(|k| recs.keys().nth(k).copied());
    // (the world as built against the file: every rail's shape kept (a pasted design isn't settled;
    // a top slope that settled flat would stall the arrivals))
    let rail_mismatch: Vec<String> = sp
        .blocks
        .iter()
        .filter(|(_, b)| redsim::rails::is_rail(b))
        .filter_map(|(p, b)| {
            let now = run.world.get(*p);
            (now != *b).then(|| format!("({},{},{}) {} built as {}", p.x, p.y, p.z, b, now))
        })
        .collect();
    let mut log: Vec<String> = Vec::new();
    let mut nap_trace_rows: Vec<String> = Vec::new();
    let mut cart_cells: Vec<(i64, [i32; 3])> = Vec::new();
    let mut occupancy_rows: Vec<String> = Vec::new();
    let mut v_held: BTreeMap<i32, i64> = BTreeMap::new();
    // (the stop's cells move with it: its bed head's y against rev 3's -18)
    let sy = o.bed[1] + 18;
    let mut ctl = Ctl { napper: o.napper, stop_dy: sy, ..Ctl::default() };
    if o.drive {
        ctl.init(&mut run.world)?;
    }
    // (phase 3a: the nappers seeded, one a night; ironspec 09:59: every villager within 16 of the
    // bed head with its state at each occupied-bed tick; ironspec 10:02: a cart resting on another
    // cart's top is a failure)
    let mut nappers: Vec<(u32, usize, i64)> = Vec::new();
    let mut seeded_night: Option<i64> = None;
    let mut near = Near::default();
    let mut stale = Stale::default();
    let mut seat_stats: Vec<SeatStat> = vec![SeatStat::default(); o.seats.len()];
    let mut gdr = Gdr::default();
    let mut summons = Summons::default();
    let mut golem_born: BTreeMap<u32, i64> = BTreeMap::new();
    let mut renew = Renewals::default();
    let mut willing_at: (i64, std::collections::BTreeSet<u32>) = (-1, std::collections::BTreeSet::new());
    let mut callers_now: Vec<(u32, i64)> = Vec::new();
    let mut stacked: Vec<String> = Vec::new();
    let mut stacked_ticks = 0i64;
    let mut turns = [Turns::default(); 3];
    let mut turn_lines: Vec<String> = Vec::new();
    // (the panic draws whose first path was partial: (place, target cell, rider cell) -> count)
    let mut partial_list: BTreeMap<(usize, [i32; 3], [i32; 3]), i64> = BTreeMap::new();
    // (and each one's detail row)
    let mut partial_rows: Vec<String> = Vec::new();
    // (the official scene's per-rest-block tally of the panic draws: complete, partial, never)
    let mut by_cell: BTreeMap<[i32; 3], [i64; 3]> = BTreeMap::new();
    // (the panic draws by the target's kind: [a point or a block, an entity walk] x [complete,
    // partial, never pathed])
    let mut by_kind = [[0i64; 3]; 2];
    let mut r1_off: Option<i64> = None;
    let mut entered = 0usize;
    let mut prev_day = run.world.day_time();
    for rel in 0..o.ticks {
        if o.official_lane3 {
            use redsim::world::flags;
            // the night held at 18,000 (`advance_time false`)
            run.world.mob.day_offset = 18_000 - run.world.time;
            let w = &mut run.world;
            let seat = |w: &mut redsim::world::World, recs: &mut BTreeMap<u32, Rec>, label: String| -> Result<(), String> {
                let cart = redsim::entity::spawn(w, "minecraft:minecart", [3.6, 24.0, 6.49], Some("{Passengers:[{id:\"minecraft:villager\"}]}"))?;
                let ci = redsim::entity::index_of(w, cart).ok_or("the entering cart vanished")?;
                let vid = *w.entities.list[ci].passengers.first().ok_or("the entering cart has no rider")?;
                let vi = redsim::entity::index_of(w, vid).ok_or("the entering rider vanished")?;
                let tnow = w.time;
                let b = redsim::entity::mob::ai::brain::brain_mut(w, vi);
                b.home = Some(bed);
                b.last_slept = Some(tnow - 100);
                recs.insert(vid, Rec { label, home_at_start: Some([bed.x, bed.y, bed.z]), prev_home: Some([bed.x, bed.y, bed.z]), ..Default::default() });
                Ok(())
            };
            // 7 riders entering one per 30 gt
            if rel % 30 == 0 && entered < 7 {
                seat(w, &mut recs, format!("r{entered}"))?;
                entered += 1;
            }
            // every 100 gt: the cart on H's trapdoor out with its rider, a rider in at the top, and
            // R_1 powered without updates for 12 gt
            if rel >= 100 && rel % 100 == 0 {
                let on_h: Vec<(u32, Vec<u32>)> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [3, 16, 0]).map(|e| (e.id, e.passengers.clone())).collect();
                for (cart, riders) in on_h {
                    for rid in riders {
                        redsim::entity::kill(w, rid)?;
                    }
                    redsim::entity::kill(w, cart)?;
                    seat(w, &mut recs, format!("r{entered}"))?;
                    entered += 1;
                }
                set(w, [3, 17, 1], "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]", flags::RAW)?;
                r1_off = Some(rel + 12);
            }
            if r1_off == Some(rel) {
                set(w, [3, 17, 1], "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]", flags::RAW)?;
                r1_off = None;
            }
        }
        if o.napper {
            // one napper a night at the stop's entry, its lane rotating
            let night = run.world.day_time().div_euclid(24_000);
            let want = 13_800 + (o.seed * 37).rem_euclid(400);
            if run.world.day_time().rem_euclid(24_000) == want && seeded_night != Some(night) {
                seeded_night = Some(night);
                // (the lane rotates from the seed's: seeds 1 / 2 / 3 start at lanes 3 / 7 / -1)
                let k = (nappers.len() + o.seed.rem_euclid(3) as usize) % 3;
                let w = &mut run.world;
                let start = [3.5, -16.0 + sy as f64 + 0.0625, -4.5];
                let cart = redsim::entity::spawn(w, "minecraft:minecart", start, Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?;
                let ci = redsim::entity::index_of(w, cart).ok_or("the napper's cart vanished")?;
                w.entities.list[ci].delta_movement = redsim::entity::Vec3::new(-0.2, 0.0, 0.0);
                let vid = *w.entities.list[ci].passengers.first().ok_or("the napper's cart has no rider")?;
                let vi = redsim::entity::index_of(w, vid).ok_or("the napper vanished")?;
                redsim::entity::position_rider(w, ci, vi);
                let b = redsim::entity::mob::ai::brain::brain_mut(w, vi);
                b.home = Some(bed);
                recs.insert(vid, Rec { label: format!("napper{} (id {vid}, lane x {}, seeded at the stop's entry)", nappers.len(), LANES[k]), home_at_start: Some([bed.x, bed.y, bed.z]), prev_home: Some([bed.x, bed.y, bed.z]), ..Default::default() });
                nappers.push((vid, k, rel));
                ctl.nap_lane = k;
                ctl.nap_events.push(format!("t {rel}: napper{} seeded at the stop's entry (lane x {})", nappers.len() - 1, LANES[k]));
            }
        }
        if o.restarts.contains(&rel) {
            let m = crate::liveness::restart(&mut run.world, o.restart)?;
            let old = std::mem::take(&mut recs);
            for (id, mut r) in old {
                // (G1 on f1507f5dc: a reload drops CANT_REACH_WALK_TARGET_SINCE, no codec: the
                // next tick's missing stamp is booked as "a reload", not as a clear)
                r.reloaded = true;
                recs.insert(m.get(&id).copied().unwrap_or(id), r);
            }
            // (the controller's golems renumbered too, so a golem alive across the restart isn't
            // read as a new pulse)
            ctl.golems = ctl.golems.iter().map(|id| m.get(id).copied().unwrap_or(*id)).collect();
            log.push(format!("t {rel}: restart (liveness::restart, order {})", o.restart.label()));
        }
        // (farmopt 12:10: summon participations blocked by detection) before the tick whose
        // VillagerPanicTrigger calls `spawn_golem_if_needed` (game time % 100 == 0): for every
        // scared villager in a pocket cell (x,0,0) that can summon (slept within 24,000, no
        // GOLEM_DETECTED_RECENTLY), the villagers within its box inflated by 10: counted (slept, no
        // memory) or rejected for GOLEM_DETECTED_RECENTLY (slept, the memory set)
        if (run.world.time + 1) % 100 == 0 {
            let w = &run.world;
            let t = w.time + 1;
            willing_at = (t, std::collections::BTreeSet::new());
            callers_now.clear();
            let villagers: Vec<usize> = (0..w.entities.list.len()).filter(|&k| !w.entities.list[k].removed && matches!(&w.entities.list[k].kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager && m.brain.is_some())).collect();
            let brain_of = |k: usize| match &w.entities.list[k].kind {
                EntityKind::Mob(m) => m.brain.as_deref(),
                _ => None,
            };
            let slept = |k: usize| brain_of(k).is_some_and(|b| b.last_slept.is_some_and(|s| t - s < redsim::entity::mob::villager::SLEPT_WITHIN));
            // (present at the summon: the brain forgets a memory whose ttl is 0 before its
            // behaviours run, so only a ttl of 1 or more before the tick blocks)
            let gdr = |k: usize| brain_of(k).is_some_and(|b| b.golem_detected.is_some_and(|t| t >= 1));
            for &i in &villagers {
                let c = cell(w.entities.list[i].pos);
                let in_pocket = c[1] == 0 && c[2] == 0 && LANES.contains(&c[0]);
                if !in_pocket || !brain_of(i).is_some_and(|b| b.scared()) || !slept(i) {
                    continue;
                }
                // (ironspec 12:13) each lane's rotation at the summon tick: its carts in x = the
                // lane, y 0..22, z 0..6 (the pocket, the drop, H, R_1, the slopes, V)
                for (li, x) in LANES.iter().enumerate() {
                    // (a lane's rotation: the riders whose last lane is it, its napper out while
                    // away: from its seeding until it is back in its lane)
                    let n = recs
                        .iter()
                        .filter(|(vid, r)| r.lane == Some(*x) && !(nappers.iter().any(|n| n.0 == **vid) && !r.milestones.iter().any(|(w, _)| w == "back in its lane")))
                        .count() as i64;
                    summons.min_rotation[li] = Some(summons.min_rotation[li].map_or(n, |m: i64| m.min(n)));
                }
                // (the caller's memory at T: absent, expired by how much; present: the caller is
                // blocked itself)
                let set = recs.get(&w.entities.list[i].id).and_then(|r| r.gdr_set);
                callers_now.push((w.entities.list[i].id, t));
                if gdr(i) {
                    summons.callers_blocked += 1;
                    let setter = recs.get(&w.entities.list[i].id).and_then(|r| r.gdr_setter).unwrap_or("?");
                    *summons.blocked_by.entry(setter).or_insert(0) += 1;
                    if summons.rows.len() < 20 {
                        let name = recs.get(&w.entities.list[i].id).map_or(format!("#{}", w.entities.list[i].id), |r| r.label.split(' ').next().unwrap_or("").to_string());
                        summons.rows.push(format!("t {}: {name} in the pocket ({},0,0) can't call: GOLEM_DETECTED_RECENTLY set (ttl {}, set t {} by {setter})", rel + 1, c[0], brain_of(i).and_then(|b| b.golem_detected).unwrap_or(0), set.map_or("?".to_string(), |s| (s - t0).to_string())));
                    }
                    continue;
                }
                if let Some(s) = set {
                    let margin = t - (s + redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                    summons.least_caller_margin = Some(summons.least_caller_margin.map_or(margin, |m: i64| m.min(margin)));
                }
                let bb = w.entities.list[i].bb.inflate(10.0, 10.0, 10.0);
                let (mut counted, mut rejected) = (0usize, Vec::new());
                for &j in &villagers {
                    if !bb.intersects(w.entities.list[j].bb) || !slept(j) {
                        continue;
                    }
                    if gdr(j) {
                        rejected.push(j);
                    } else {
                        counted += 1;
                        // (ironspec 12:48) the villagers a summon this tick counts: a stamp on any
                        // other is a bystander's
                        willing_at.0 = t;
                        willing_at.1.insert(w.entities.list[j].id);
                    }
                }
                summons.attempts += 1;
                summons.rejected += rejected.len() as i64;
                if counted.min(5) < 3 && counted + rejected.len() >= 3 {
                    summons.blocked += 1;
                }
                if !rejected.is_empty() && summons.rows.len() < 20 {
                    let name = |k: usize| recs.get(&w.entities.list[k].id).map_or(format!("#{}", w.entities.list[k].id), |r| r.label.split(' ').next().unwrap_or("").to_string());
                    summons.rows.push(format!(
                        "t {}: {} in the pocket ({},0,0): {counted} counted, rejected for GOLEM_DETECTED_RECENTLY {}{}",
                        rel + 1,
                        name(i),
                        c[0],
                        rejected.iter().map(|&k| format!("{} (ttl {})", name(k), brain_of(k).and_then(|b| b.golem_detected).unwrap_or(0))).collect::<Vec<_>>().join(", "),
                        if counted.min(5) < 3 && counted + rejected.len() >= 3 { " (the summon BLOCKED)" } else { "" }
                    ));
                }
            }
        }
        // (ironspec 12:48) each caller's pending bystander stamp against its summon tick
        for (id, t) in callers_now.drain(..) {
            if let Some(r) = recs.get_mut(&id)
                && let Some((s, p)) = r.bystander_stamp.take()
            {
                let margin = t - (s + redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                renew.bystander_margin = Some(renew.bystander_margin.map_or(margin, |m: i64| m.min(margin)));
                if renew.bystander_rows.len() < 24 {
                    renew.bystander_rows.push(format!("{} stamped as a bystander at t {} at ({:.1},{:.1},{:.1}), its next summon t {}: {margin:+} gt past the expiry", r.label.split(' ').next().unwrap_or(""), s - t0, p[0], p[1], p[2], t - t0));
                }
            }
        }
        run.tick();
        if o.drive {
            ctl.step(&mut run.world, rel)?;
        }
        let w = &mut run.world;
        let now = w.time;
        let day = w.day_time();
        let dawn = day < prev_day || (prev_day < 23_999 && day >= 23_999);
        prev_day = day;
        // (ironspec 09:59) the bed occupied this tick: a villager asleep in it
        let bed_occupied = w.entities.list.iter().any(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.sleeping_pos.is_some_and(|p| [p.x, p.y, p.z] == o.bed)));
        // (ironspec 11:02) the zombies where they sit this tick (a boated zombie seated: the boat's
        // y + its passenger attachment + the zombie's riding offset)
        let zombies_now = crate::fleecheck::zombies_seated(w, spec);
        // (ironspec 12:23) every iron golem's first tick seen (its birth)
        for e in w.entities.list.iter() {
            if !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem) {
                golem_born.entry(e.id).or_insert(now);
            }
        }
        // (ironspec 11:29) the neighbour's pad golems this tick (EMULATED: ours moved by the
        // neighbour's offset)
        let neighbour_golems: Vec<redsim::entity::Vec3> = match o.neighbour {
            Some(d) => w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem))
                .map(|e| redsim::entity::Vec3::new(e.pos.x + d[0] as f64, e.pos.y + d[1] as f64, e.pos.z + d[2] as f64))
                .collect(),
            None => Vec::new(),
        };
        if bed_occupied {
            near.occupied_ticks += 1;
        }
        // (ironspec 10:02) a cart resting on another cart's top is a failure
        {
            let carts: Vec<(u32, redsim::entity::Vec3)> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, e.pos)).collect();
            for (a, pa) in &carts {
                for (b, pb) in &carts {
                    // (on top: nearly the same x and z, its box a cart's height higher; two carts
                    // touching on a slope are 0.5 apart in z as well as in y, so they don't count)
                    if a != b && (pa.x - pb.x).abs() < 0.35 && (pa.z - pb.z).abs() < 0.35 && pa.y - pb.y > 0.5 && pa.y - pb.y < 0.9 {
                        stacked_ticks += 1;
                        if stacked.len() < 20 {
                            stacked.push(format!("t {}: cart {a} at ({:.2},{:.2},{:.2}) on cart {b} at ({:.2},{:.2},{:.2})", now - t0, pa.x, pa.y, pa.z, pb.x, pb.y, pb.z));
                        }
                    }
                }
            }
        }
        let ids: Vec<u32> = recs.keys().copied().collect();
        for id in ids {
            let Some(i) = redsim::entity::index_of(w, id) else { continue };
            let pos = w.entities.list[i].pos;
            let (home, activity_rest, panic, sleeping, stamp, target, last_slept, reach, sink_cd, path_target, path_end, hostile, target_kind, activity, target_key, path_from) = {
                use redsim::entity::mob::ai::brain::Act;
                let b = redsim::entity::mob::ai::brain::brain(w, i);
                let ex = b.ex.as_deref();
                // (the exact brain's own activity: REST, PANIC; the legacy fields aren't kept with
                // the mob AI on)
                let rest = ex.map_or(matches!(b.activity, redsim::entity::mob::villager::Activity::Rest), |x| x.active == Act::Rest);
                let panic = ex.map_or(b.panic_running, |x| x.active == Act::Panic);
                let sleeping = matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.sleeping_pos.is_some());
                let target = ex.and_then(|x| x.mem.walk_target.as_ref()).and_then(|t| match t.target {
                    redsim::entity::mob::ai::brain::Tracker::Block(p) => Some([p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5]),
                    redsim::entity::mob::ai::brain::Tracker::Point(v) => Some([v.x, v.y, v.z]),
                    redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => redsim::entity::index_of(w, id).map(|k| {
                        let q = w.entities.list[k].pos;
                        [q.x, q.y, q.z]
                    }),
                });
                // (what kind of target: a point (a flee or a stroll), a block (HOME, a POI) or another
                // entity (the walk to a villager seen within 8: SetWalkTargetFromLookTarget), and the
                // activity it was drawn in)
                let target_kind = ex.and_then(|x| x.mem.walk_target.as_ref()).map(|t| match t.target {
                    redsim::entity::mob::ai::brain::Tracker::Block(_) => "a block".to_string(),
                    redsim::entity::mob::ai::brain::Tracker::Point(_) => "a point (a flee or a stroll)".to_string(),
                    redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => format!("entity {id} (a walk to another mob)"),
                });
                // (a draw's identity: a point's or a block's cell, or the entity followed: an
                // entity walk is one draw however many cells its mob crosses)
                let target_key: Option<(u8, i64, i64, i64)> = ex.and_then(|x| x.mem.walk_target.as_ref()).map(|t| match t.target {
                    redsim::entity::mob::ai::brain::Tracker::Block(p) => (0, p.x as i64, p.y as i64, p.z as i64),
                    redsim::entity::mob::ai::brain::Tracker::Point(v) => (0, v.x.floor() as i64, v.y.floor() as i64, v.z.floor() as i64),
                    redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => (1, id as i64, 0, 0),
                });
                let activity = ex.map(|x| format!("{:?}", x.active)).unwrap_or_default();
                // (the path the sink holds: complete (`canReach`) or not)
                let reach = ex.and_then(|x| x.mem.path.as_ref()).map(|p| p.can_reach());
                // (the path's own target: a path computed for an earlier target is stale)
                let path_target = ex.and_then(|x| x.mem.path.as_ref()).map(|p| [p.target.x, p.target.y, p.target.z]);
                let path_end = ex.and_then(|x| x.mem.path.as_ref()).and_then(|p| p.nodes.last().map(|n| [n.x, n.y, n.z]));
                // (the path's first node and length: where it was computed from)
                let path_from = ex.and_then(|x| x.mem.path.as_ref()).map(|p| (p.nodes.first().map(|n| [n.x, n.y, n.z]), p.nodes.len()));
                let hostile = b.nearest_hostile;
                // (MoveToTargetSink's remainingCooldown)
                let sink_cd = ex.and_then(|x| x.nodes.iter().find_map(|n| match &n.kind {
                    redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { remaining_cooldown, .. } => Some(*remaining_cooldown),
                    _ => None,
                }));
                (b.home.map(|h| [h.x, h.y, h.z]), rest, panic, sleeping, ex.and_then(|x| x.mem.cant_reach_since), target, b.last_slept, reach, sink_cd, path_target, path_end, hostile, target_kind, activity, target_key, path_from)
            };
            let r = recs.get_mut(&id).unwrap();
            let c = cell(pos);
            // (the rider's lane: the lane whose slopes it rides)
            if LANES.contains(&c[0]) && (16..=22).contains(&c[1]) && (0..=6).contains(&c[2]) {
                r.lane = Some(c[0]);
            }
            // (the seats: occupancy, and each rider's stretch on a seat since it sat or was reset)
            if let Some(si) = o.seats.iter().position(|(_, s, _)| *s == c) {
                seat_stats[si].occupied += 1;
                if r.seat_since.is_none() {
                    r.seat_since = Some(now);
                }
            } else if let Some(since) = r.seat_since.take() {
                if let Some(si) = r.seat_last {
                    seat_stats[si].longest = seat_stats[si].longest.max(now - since);
                }
            }
            r.seat_last = o.seats.iter().position(|(_, s, _)| *s == c).or(r.seat_last);
            // (ironspec 11:02, G1's staleness condition: NEAREST_HOSTILE up to ~39 gt stale, the
            // flee testing only presence and distance < 6) every entry into a position within 6.05
            // of a zombie hidden from it, at least 40 gt after the rider's last position within 8 of
            // a zombie in its sight (the game's own sight: `hasLineOfSight` from the rider's eye
            // (+1.62) to the zombie's (+1.74)); the stop's waker places (the napper at the stop)
            // are declared and left out
            {
                let eye = redsim::entity::Vec3::new(pos.x, pos.y + 1.62, pos.z);
                let at_stop = (-6..=4).contains(&c[0]) && (-20 + sy..=-14 + sy).contains(&c[1]) && (-6..=6).contains(&c[2]);
                let mut seen8 = false;
                let mut hidden605 = false;
                for z in &zombies_now {
                    let d = ((pos.x - z.x).powi(2) + (pos.y - z.y).powi(2) + (pos.z - z.z).powi(2)).sqrt();
                    if d >= 8.0 {
                        continue;
                    }
                    let sees = redsim::entity::mob::los::has_line_of_sight(w, eye, redsim::entity::Vec3::new(z.x, z.y + 1.74, z.z));
                    if sees {
                        seen8 = true;
                    } else if d < 6.05 {
                        hidden605 = true;
                    }
                }
                if seen8 {
                    r.last_seen8 = Some(now);
                }
                if hidden605 && !r.prev_hidden605 && !at_stop {
                    let gap = r.last_seen8.map(|t| now - t);
                    stale.entries += 1;
                    if gap.is_some_and(|g| g < 40) {
                        stale.bad += 1;
                        if stale.rows.len() < 20 {
                            stale.rows.push(format!("t {}: {} at ({:.2},{:.2},{:.2}) entered a hidden-within-6.05 position {} gt after its last in-sight-within-8 position", now - t0, r.label.split(' ').next().unwrap_or(""), pos.x, pos.y, pos.z, gap.unwrap_or(0)));
                        }
                    }
                    if let Some(g) = gap {
                        stale.least = Some(stale.least.map_or(g, |l: i64| l.min(g)));
                    }
                }
                r.prev_hidden605 = hidden605 && !at_stop;
            }
            // (ironspec 11:29) GOLEM_DETECTED_RECENTLY at the stand: the neighbour's golems
            // (EMULATED) set it on a rider whose 16-block box meets one; each rider's last set (any
            // source) and its last tick inside the neighbour pad's detection box; at every stand
            // arrival (its cart entering a pocket cell (x,0,0)) the memory must have expired at
            // least 20 gt before, and the last tick in the neighbour's box must lie >= 619 gt back
            {
                let villager_box = |g: &redsim::entity::Vec3| (g.x - pos.x).abs() <= 17.0 && g.y <= pos.y + 17.95 && g.y + 2.7 >= pos.y - 16.0 && (g.z - pos.z).abs() <= 17.0;
                if neighbour_golems.iter().any(villager_box) {
                    redsim::entity::mob::ai::brain::brain_mut(w, i).golem_detected = Some(redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                    gdr.neighbour_sets += 1;
                    r.gdr_neighbour = Some(now);
                }
                if let Some(d) = o.neighbour {
                    // (the neighbour's pad: our pad's cells, x -3..7, y -6, z -6..-1, moved)
                    let inside = (-3..=7).any(|x| {
                        let gx = x as f64 + 0.5 + d[0] as f64;
                        (gx - pos.x).abs() <= 17.0
                    }) && (-6.0 + d[1] as f64) <= pos.y + 17.95
                        && (-6.0 + d[1] as f64 + 2.7) >= pos.y - 16.0
                        && (-6..=-1).any(|z| ((z as f64 + 0.5 + d[2] as f64) - pos.z).abs() <= 17.0);
                    if inside {
                        r.in_neighbour_box = Some(now);
                    }
                }
                let ttl = redsim::entity::mob::ai::brain::brain(w, i).golem_detected;
                if ttl == Some(redsim::entity::mob::villager::GOLEM_MEMORY_TTL) {
                    r.gdr_set = Some(now);
                    // (ironspec 12:23) the setter: a summon's 10-box stamp (a golem born this
                    // tick) or the GolemSensor reading the living list (the nearest golem in the
                    // rider's 16-block box, or none alive: a departed one in a stale list)
                    let born_now = golem_born.values().any(|t| *t == now);
                    if r.gdr_neighbour == Some(now) {
                        renew.neighbour += 1;
                        r.gdr_setter = Some("the neighbour's golems");
                    } else if born_now {
                        renew.stamps += 1;
                        r.last_stamp = Some(now);
                        // (ironspec 12:48) a summoner of that golem (counted by this tick's
                        // summon) or a bystander in the 10-box
                        let id_me = w.entities.list[i].id;
                        if willing_at.0 == now && willing_at.1.contains(&id_me) {
                            renew.summoner_stamps += 1;
                            r.gdr_setter = Some("a summon's stamp (its summoner)");
                        } else {
                            renew.bystander_stamps += 1;
                            r.gdr_setter = Some("a summon's stamp (a bystander)");
                            r.bystander_stamp = Some((now, [pos.x, pos.y, pos.z]));
                        }
                    } else {
                        renew.sensor += 1;
                        r.gdr_setter = Some("the GolemSensor");
                        let src = w
                            .entities
                            .list
                            .iter()
                            .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem))
                            .filter(|e| (e.pos.x - pos.x).abs() <= 17.0 && e.pos.y <= pos.y + 17.95 && e.pos.y + 2.7 >= pos.y - 16.0 && (e.pos.z - pos.z).abs() <= 17.0)
                            .min_by(|a, b| a.pos.distance_to_sqr(pos).total_cmp(&b.pos.distance_to_sqr(pos)))
                            .map(|e| (e.id, golem_born.get(&e.id).copied()));
                        let after_own = r.last_stamp.is_some_and(|s| now - s < 600);
                        if after_own {
                            renew.after_own += 1;
                        }
                        if renew.rows.len() < 24 {
                            renew.rows.push(format!(
                                "t {}: {} at ({:.1},{:.1},{:.1}): the GolemSensor, {}{}",
                                now - t0,
                                r.label.split(' ').next().unwrap_or(""),
                                pos.x,
                                pos.y,
                                pos.z,
                                match src {
                                    Some((g, Some(b))) => format!("golem {g} (born t {})", b - t0),
                                    Some((g, None)) => format!("golem {g}"),
                                    None => "no golem alive in its box (a departed one in the stale living list)".to_string(),
                                },
                                r.last_stamp.map_or(String::new(), |s| format!(", {} gt after its own summon's stamp", now - s))
                            ));
                        }
                    }
                }
                let at_stand = c[1] == 0 && c[2] == 0 && LANES.contains(&c[0]);
                if at_stand && !r.prev_at_stand {
                    gdr.arrivals += 1;
                    let label = r.label.split(' ').next().unwrap_or("").to_string();
                    if ttl.is_some() {
                        gdr.blocked += 1;
                        if gdr.rows.len() < 20 {
                            gdr.rows.push(format!("t {}: {label} arrived at the stand ({},0,0) with GOLEM_DETECTED_RECENTLY still set (ttl {}, set t {})", now - t0, c[0], ttl.unwrap_or(0), r.gdr_set.map_or("?".to_string(), |s| (s - t0).to_string())));
                        }
                    } else if let Some(s) = r.gdr_set {
                        let margin = now - (s + redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                        gdr.least_margin = Some(gdr.least_margin.map_or(margin, |m: i64| m.min(margin)));
                        if margin < 20 {
                            gdr.late += 1;
                        }
                    }
                    if let Some(b) = r.in_neighbour_box {
                        let gap = now - b;
                        gdr.least_box_gap = Some(gdr.least_box_gap.map_or(gap, |m: i64| m.min(gap)));
                        if gap < 619 {
                            gdr.box_short += 1;
                            if gdr.box_rows.len() < 20 {
                                gdr.box_rows.push(format!("t {}: {label} at the stand {gap} gt after its last tick in the neighbour pad's detection box", now - t0));
                            }
                        }
                    }
                    if nappers.iter().any(|x| x.0 == id) {
                        gdr.napper_rows.push(format!(
                            "t {}: {label} at the stand: GOLEM_DETECTED_RECENTLY {}, last set t {}, the neighbour's last set t {}, its last tick in the neighbour's box t {}",
                            now - t0,
                            ttl.map_or("expired".to_string(), |t| format!("still set (ttl {t})")),
                            r.gdr_set.map_or("never".to_string(), |s| (s - t0).to_string()),
                            r.gdr_neighbour.map_or("never".to_string(), |s| (s - t0).to_string()),
                            r.in_neighbour_box.map_or("never".to_string(), |s| (s - t0).to_string())
                        ));
                    }
                }
                r.prev_at_stand = at_stand;
                // (the departing napper's exit from the 16-ball around the bed head after R)
                if let Some(n) = nappers.iter().position(|x| x.0 == id) {
                    let _ = n;
                    let at_r = r.milestones.iter().find(|(w, _)| w == "at R").map(|(_, t)| *t);
                    let bh = [o.bed[0] as f64 + 0.5, o.bed[1] as f64 + 0.5, o.bed[2] as f64 + 0.5];
                    let d = ((pos.x - bh[0]).powi(2) + (pos.y - bh[1]).powi(2) + (pos.z - bh[2]).powi(2)).sqrt();
                    if let Some(rt) = at_r
                        && d > 16.0
                        && !r.milestones.iter().any(|(w, _)| w == "out of the bed's 16-ball")
                    {
                        r.milestones.push(("out of the bed's 16-ball".to_string(), now - t0));
                        r.ball_exit_after_r = Some(now - t0 - rt);
                    }
                }
            }
            // (ironspec 09:59) within 16 of the bed head
            let bh = [o.bed[0] as f64 + 0.5, o.bed[1] as f64 + 0.5, o.bed[2] as f64 + 0.5];
            if ((pos.x - bh[0]).powi(2) + (pos.y - bh[1]).powi(2) + (pos.z - bh[2]).powi(2)).sqrt() <= 16.0 {
                *near.cells.entry(c).or_insert(0) += 1;
                near.ticks += 1;
                if bed_occupied && activity_rest && !panic && !sleeping {
                    near.calm_rest_occupied += 1;
                    if near.calm_rest_rows.len() < 30 {
                        near.calm_rest_rows.push(format!("t {}: {} calm in REST at ({:.2},{:.2},{:.2}), the bed occupied", now - t0, r.label.split(' ').next().unwrap_or(""), pos.x, pos.y, pos.z));
                    }
                }
            }
            // (phase 3a: a napper's milestones)
            if let Some(n) = nappers.iter().position(|x| x.0 == id) {
                let mark = |r: &mut Rec, what: &str| {
                    if !r.milestones.iter().any(|(w, _)| w == what) {
                        r.milestones.push((what.to_string(), now - t0));
                    }
                };
                let k = nappers[n].1;
                let lx = LANES[k];
                if sleeping {
                    mark(r, "asleep in the bed");
                }
                if c == [4, -19 + sy, 4] {
                    mark(r, "at R");
                }
                if c[0] == 6 && (-19 + sy..=-4).contains(&c[1]) && c[2] == 4 {
                    mark(r, "in the napper lift");
                }
                if c == [9, 0, 8] {
                    mark(r, "on the hold (9,0,8)");
                }
                if c[1] == 0 && c[2] == 7 && c[0] <= 4 {
                    mark(r, "on the merge row");
                }
                if c[0] == -5 && c[2] == 7 && c[1] >= 1 {
                    mark(r, "in the shared lift");
                }
                if c == [lx - 2, 25, 9] {
                    mark(r, "on its K");
                }
                if c == [lx - 2, 24, 7] {
                    mark(r, "on its siding's R");
                }
                if c[0] == lx && (18..=22).contains(&c[1]) && c[2] >= 2 && c[2] <= 6 {
                    mark(r, "back in its lane");
                }
                if c[2] == 9 && c[1] == 25 && c[0] != lx - 2 && c[0] > lx - 2 {
                    mark(r, "past its K on the row (a wrong lane?)");
                }
                // (ironspec 11:19: the napper's route against the designed one) its cells in order,
                // from its first K until its lane's top: designed K (x-2,25,9), F (x-2,25,8), R
                // (x-2,24,7), c1 (x-2,24,6), c2 (x-1,24,6), c3 (x-1,24,7), C (x,24,7) (ironfarm 11:21)
                let at_k = r.milestones.iter().any(|(w, _)| w == "on its K");
                let in_lane = r.milestones.iter().any(|(w, _)| w == "back in its lane");
                if at_k && !in_lane && r.route.last() != Some(&c) {
                    r.route.push(c);
                }
            }
            // (the control: its clears undone)
            if Some(id) == control_id && r.prev_stamp.is_some() && stamp.is_none() && home.is_some() {
                if let Some(ex) = redsim::entity::mob::ai::brain::brain_mut(w, i).ex.as_deref_mut() {
                    ex.mem.cant_reach_since = r.prev_stamp;
                }
                r.prev_target = target;
                r.prev_key = target_key;
                continue;
            }
            if activity_rest && r.dusk.is_none() {
                r.dusk = Some(stamp.map(|s| now - s));
            }
            if activity_rest && !panic && target.is_none() {
                r.calm_rest_ticks += 1;
                if let Some(s) = stamp {
                    let age = now - s;
                    if r.peak.is_none_or(|p| age > p.0) {
                        r.peak = Some((age, now - t0, c));
                    }
                    if age > 1000 {
                        r.over_1000 += 1;
                    }
                }
            }
            if r.reloaded {
                // (a reload: the stamp isn't saved, so its loss here is no clear)
                if r.prev_stamp.is_some() && stamp.is_none() {
                    r.clears.push(format!("t {} at {:?}: a reload (CANT_REACH_WALK_TARGET_SINCE isn't saved: not a clear)", now - t0, c));
                    r.reload_drops += 1;
                }
                r.reloaded = false;
            } else if let (Some(_), None) = (r.prev_stamp, stamp) {
                let napped = sleeping || last_slept == Some(now);
                let what = match r.prev_target {
                    Some(t) => {
                        let d = ((pos.x - t[0]).powi(2) + (pos.y - t[1]).powi(2) + (pos.z - t[2]).powi(2)).sqrt();
                        let tc = [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32];
                        let kind = if r.prev_home.is_some_and(|h| h == tc) { "the bed" } else if panic { "a flee point" } else { "a stroll point" };
                        if napped { format!("a nap (target {kind})") } else if d <= 1.5 { format!("its target reached ({kind}, {d:.2} away)") } else { format!("a complete path to its target ({kind}, {d:.2} away)") }
                    }
                    None => if napped { "a nap".to_string() } else { "no target on record (a model case to report)".to_string() },
                };
                r.clears.push(format!("t {} at {:?}: {what}", now - t0, c));
                if !activity_rest {
                    r.day_clears[place_of(c)] += 1;
                }
                // (the clears by place and cause, and the stretch since the last clear)
                let cause = if what.starts_with("a nap") { 0 } else if what.starts_with("its target reached") { 1 } else if what.starts_with("a complete path") { 2 } else { 3 };
                r.clears_by[place_of(c)][cause] += 1;
                if let Some(l) = r.last_clear {
                    let gap = now - t0 - l;
                    if r.worst_gap.is_none_or(|g| gap > g.0) {
                        r.worst_gap = Some((gap, l, now - t0));
                    }
                    if activity_rest && r.worst_night_gap.is_none_or(|g| gap > g.0) {
                        r.worst_night_gap = Some((gap, l, now - t0));
                    }
                }
                r.last_clear = Some(now - t0);
                if let Some(t) = r.turn.as_mut() {
                    t.cleared = true;
                }
            }
            if sleeping && last_slept == Some(now) {
                r.naps += 1;
            }
            if r.prev_home.is_some() && home.is_none() {
                r.home_lost.push(format!("t {} at {:?} (day time {day})", now - t0, c));
            }
            if dawn {
                r.home_at_dawn = Some(home);
            }
            // (the pocket turns: a rider seated in pocket k's cell (x,0,0); its draws (a new walk
            // target while panicking: landed with a complete path, or with none) and panic ticks while
            // there, the WALK_TARGET holder and the sink's remainingCooldown at the panic's onset, the
            // tick WALK_TARGET frees after it, the panic ticks spent on a landed draw with no path,
            // and whether a clear came)
            let in_pocket = LANES.iter().position(|x| c == [*x, 0, 0]);
            let close = |t: &Turn, turns: &mut [Turns; 3], lines: &mut Vec<String>, label: &str, end: i64| {
                let lane = &mut turns[t.lane];
                lane.turns += 1;
                lane.cleared += t.cleared as i64;
                lane.draws += t.landed_reach + t.landed_unreach;
                lane.panic += t.panic;
                lane.landed_reach += t.landed_reach;
                lane.landed_unreach += t.landed_unreach;
                lane.nopath += t.nopath;
                if lines.len() < 600 {
                    lines.push(format!(
                        "lane x {} {label}: t {}..{} ({} gt), panic ticks {}, onset {}, WALK_TARGET free at {}, draws landed reachable {} / no path {}, no-path panic ticks {}, {}",
                        LANES[t.lane],
                        t.start,
                        end,
                        end - t.start,
                        t.panic,
                        t.onset.as_ref().map(|(at, h, cd)| format!("t {at} (holder {h}, sink cooldown {})", cd.map(|v| v.to_string()).unwrap_or("-".to_string()))).unwrap_or("none".to_string()),
                        t.freed.map(|f| format!("t {f}")).unwrap_or("-".to_string()),
                        t.landed_reach,
                        t.landed_unreach,
                        t.nopath,
                        if t.cleared { "CLEARED" } else { "no clear (a miss)" }
                    ));
                }
            };
            match (r.turn.as_ref().map(|t| t.lane), in_pocket) {
                (None, Some(k)) => r.turn = Some(Turn { lane: k, start: now - t0, ..Default::default() }),
                (Some(l), Some(k)) if l == k => {}
                (Some(_), _) => {
                    let t = r.turn.take().unwrap();
                    close(&t, &mut turns, &mut turn_lines, &r.label, now - t0);
                    r.laps += 1;
                    r.turn = in_pocket.map(|k| Turn { lane: k, start: now - t0, ..Default::default() });
                }
                (None, None) => {}
            }
            if let Some(t) = r.turn.as_mut() {
                if panic {
                    t.panic += 1;
                    if !r.prev_panic && t.onset.is_none() {
                        let holder = match (r.prev_target, r.prev_home) {
                            (None, _) => "none".to_string(),
                            (Some(p), Some(h)) if [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32] == h => "the bed walk".to_string(),
                            (Some(p), _) => format!("a target at ({:.1},{:.1},{:.1})", p[0], p[1], p[2]),
                        };
                        t.onset = Some((now - t0, holder, sink_cd));
                    }
                    let new_cell = target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]) != r.prev_target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]);
                    if target.is_some() && new_cell {
                        if reach == Some(true) {
                            t.landed_reach += 1;
                        } else {
                            t.landed_unreach += 1;
                        }
                    }
                    if target.is_some() && reach == Some(false) {
                        t.nopath += 1;
                    }
                }
                if t.onset.is_some() && t.freed.is_none() && target.is_none() {
                    t.freed = Some(now - t0);
                }
            }
            // (every draw by place, classified at the first path the sink computes for its own
            // target (complete or partial), or "never pathed" when the target changes or clears
            // first; the panic draws' partial ones listed by target and rider cell)
            let tc = target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]);
            if let Some((p, at, was_panic, want, rider)) = r.pending_draw {
                // (the draw changed or cleared before a path: never pathed; a path computed for
                // the target's current cell (an entity walk follows its mob): complete or partial)
                let k = if target_key != r.pending_key {
                    Some(2)
                } else if path_target.is_some() && path_target == tc {
                    match reach {
                        Some(true) => Some(0),
                        Some(false) => Some(1),
                        None => None,
                    }
                } else {
                    None
                };
                if let Some(k) = k {
                    let row = if was_panic { &mut r.draws_by_panic } else { &mut r.draws_by };
                    row[p][k] += 1;
                    if was_panic {
                        by_cell.entry(rider).or_insert([0; 3])[k] += 1;
                        // (the panic draws by the target's kind: a point or a block, an entity walk)
                        let e = usize::from(r.pending_key.is_some_and(|x| x.0 == 1));
                        by_kind[e][k] += 1;
                    }
                    // (a seat's stamp reset: a complete draw to one of its candidates by a rider on
                    // the seat)
                    if k == 0 {
                        for (si, (_, seat, cands)) in o.seats.iter().enumerate() {
                            if rider == *seat && cands.contains(&want) {
                                let st = &mut seat_stats[si];
                                st.resets += 1;
                                *st.by_cand.entry(want).or_insert(0) += 1;
                                if let Some(since) = r.seat_since {
                                    st.longest = st.longest.max(now - since);
                                }
                                r.seat_since = Some(now);
                            }
                        }
                    }
                    if k == 1 && was_panic {
                        *partial_list.entry((p, want, rider)).or_insert(0) += 1;
                        // (the draw's detail: its scarer, the path's end, whether the rider's cart
                        // was at rest or rolling; the hold measured below)
                        if partial_rows.len() < 400 {
                            let speed = {
                                let e = &w.entities.list[i];
                                e.vehicle.and_then(|v| redsim::entity::index_of(w, v)).map(|vi| {
                                    let d = w.entities.list[vi].delta_movement;
                                    (d.x * d.x + d.z * d.z).sqrt()
                                })
                            }
                            .unwrap_or(0.0);
                            let scarer = r.draw_hostile.and_then(|h| redsim::entity::index_of(w, h)).map(|hi| {
                                let q = w.entities.list[hi].pos;
                                let name = if (q.x - 1.5).abs() < 1.0 && q.z < 0.0 && q.y > 20.0 {
                                    "F3's pillager"
                                } else if q.y < -15.0 {
                                    "the bed stop's zombie"
                                } else if q.y > -2.5 {
                                    "S1 (the pillager)"
                                } else {
                                    "S2 (the zombie)"
                                };
                                format!("{name} at ({:.1},{:.1},{:.1})", q.x, q.y, q.z)
                            });
                            r.holding = Some((partial_rows.len(), want, at));
                            let cart_v = {
                                let e = &w.entities.list[i];
                                e.vehicle.and_then(|v| redsim::entity::index_of(w, v)).map(|vi| w.entities.list[vi].delta_movement)
                            };
                            partial_rows.push(format!(
                                "t {at}: {} (lane x {}), rider cell {:?} ({}, {}; rider at ({:.4},{:.4},{:.4}){}), scarer {}, target {:?}, the partial path ends at {:?}{}",
                                r.label.split(' ').next().unwrap_or(""),
                                LANES.iter().min_by_key(|x| (*x - rider[0]).abs()).copied().unwrap_or(0),
                                rider,
                                ["the pocket", "the lane's rest (R_1, the slopes)", "the siding", "elsewhere"][p],
                                if speed < 0.01 { "at rest" } else { "rolling" },
                                r.draw_pos[0],
                                r.draw_pos[1],
                                r.draw_pos[2],
                                cart_v.map(|d| format!(", its cart's velocity now ({:.4},{:.4},{:.4})", d.x, d.y, d.z)).unwrap_or_default(),
                                scarer.unwrap_or_else(|| "none on record".to_string()),
                                want,
                                path_end,
                                // (where the path was computed: its first node, its length, the tick
                                // and the rider's position then; a rolling rider computes from
                                // where its cart has carried it)
                                path_from.map(|(first, n)| format!(" (the path computed at t {} (+{} gt after the draw) from {:?}, {n} nodes, the rider then at ({:.2},{:.2},{:.2}))", now - t0, now - t0 - at, first, pos.x, pos.y, pos.z)).unwrap_or_default()
                            ));
                            if let Some(row) = partial_rows.last_mut() {
                                row.push_str(&format!("; {}", r.draw_ctx));
                            }
                        }
                    }
                    r.pending_draw = None;
                }
            }
            // (a partial draw's hold: WALK_TARGET kept on its target; when it goes, its length and
            // whether the rider had left its cell by then (the hold lasted over the place's window))
            if let Some((row, want, from)) = r.holding {
                if tc != Some(want) {
                    let moved = r.draw_cell.is_some_and(|dc| dc != c);
                    if let Some(line) = partial_rows.get_mut(row) {
                        line.push_str(&format!("; WALK_TARGET held {} gt{}", now - t0 - from, if moved { ", past the rider's stay at its cell (the hold overlapped the place's window to its end)" } else { "" }));
                    }
                    r.holding = None;
                }
            }
            // (one draw per new target: a point's or a block's cell, or the entity followed; a
            // target re-set on the same cell, or an entity walk whose mob moves, is the same draw)
            if let (Some(t), true) = (tc, target_key.is_some() && target_key != r.prev_key) {
                if r.pending_draw.is_none() {
                    r.pending_draw = Some((place_of(c), now - t0, panic, t, c));
                    r.pending_key = target_key;
                    r.draw_hostile = hostile;
                    r.draw_cell = Some(c);
                    r.draw_pos = [pos.x, pos.y, pos.z];
                    // (tick-exact 08:56: the draw tick's state for an official replay: the rider's
                    // cart and its velocity, and every other cart in its lane (x within 1.5 of the
                    // lane, y 15..27))
                    let lane_x = LANES.iter().min_by_key(|x| (*x - c[0]).abs()).copied().unwrap_or(0) as f64 + 0.5;
                    let my_cart = w.entities.list[i].vehicle;
                    let carts: Vec<String> = w
                        .entities
                        .list
                        .iter()
                        .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && (e.pos.x - lane_x).abs() < 1.5 && e.pos.y > 15.0 && e.pos.y < 27.0)
                        .map(|e| format!("{}({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}){}", if Some(e.id) == my_cart { "ITS CART " } else { "" }, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.passengers.is_empty() { " empty" } else { "" }))
                        .collect();
                    // (tick-exact 11:05: who set an entity walk: the game's own case is an IDLE
                    // neighbour's InteractWith / TradeWithVillager locking gaze and walk on both
                    // (BehaviorUtils.setWalkAndLookTargetMemories, in the neighbour's tick); the
                    // followed mob's INTERACTION_TARGET, WALK_TARGET, activity and running
                    // behaviours, and the rider's own running behaviours)
                    let ex_of = |k: usize| -> Option<&redsim::entity::mob::ai::brain::Exact> {
                        match &w.entities.list[k].kind {
                            EntityKind::Mob(m) => m.brain.as_deref().and_then(|b| b.ex.as_deref()),
                            _ => None,
                        }
                    };
                    let running = |k: usize| -> String {
                        ex_of(k).map_or(String::new(), |x| {
                            let v: Vec<&str> = x.nodes.iter().filter(|n| n.running).map(|n| n.class_name()).collect();
                            v.join("+")
                        })
                    };
                    let me_id = w.entities.list[i].id;
                    let setter = match target_key {
                        Some((1, jid, _, _)) => match redsim::entity::index_of(w, jid as u32) {
                            Some(k) => match ex_of(k) {
                                Some(x) => {
                                    let its_walk = x.mem.walk_target.as_ref().map_or("none".to_string(), |t| match t.target {
                                        redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } if id == me_id => "this rider".to_string(),
                                        redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => format!("entity {id}"),
                                        _ => "a point or block".to_string(),
                                    });
                                    let locked = x.mem.interaction_target == Some(me_id) && its_walk == "this rider";
                                    format!(
                                        "; the followed mob {jid}: its INTERACTION_TARGET {}, its WALK_TARGET {its_walk}, in {:?}, running {}{}; the rider's own running {}",
                                        x.mem.interaction_target.map_or("none".to_string(), |t| if t == me_id { "this rider".to_string() } else { format!("entity {t}") }),
                                        x.active,
                                        running(k),
                                        if locked { " (both locked on each other: the neighbour's setWalkAndLookTargetMemories)" } else { "" },
                                        running(i)
                                    )
                                }
                                None => format!("; the followed mob {jid} has no villager brain; the rider's own running {}", running(i)),
                            },
                            None => format!("; the followed mob {jid} is gone; the rider's own running {}", running(i)),
                        },
                        _ => String::new(),
                    };
                    r.draw_ctx = format!("the target {} drawn in {activity}{setter}; the lane's carts at the draw tick: {}", target_kind.as_deref().unwrap_or("?"), carts.join("; "));
                }
            }
            if target.is_some() && reach == Some(false) && path_target == target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]) {
                r.stuck_ticks[place_of(c)] += 1;
            }
            r.prev_panic = panic;
            r.prev_stamp = stamp;
            r.prev_home = home;
            r.prev_target = target;
            r.prev_key = target_key;
        }
        // (ironfarm 11:40) the traced cart's cells, and each lane's H..V occupancy at every release
        if let Some((cid, a, b)) = o.cart_trace
            && rel >= a
            && rel <= b
        {
            if let Some(ci) = redsim::entity::index_of(w, cid) {
                let e = &w.entities.list[ci];
                let cc = cell(e.pos);
                if cart_cells.last().map(|(_, c)| *c) != Some(cc) {
                    cart_cells.push((rel, cc));
                }
            }
            let held: Vec<i32> = LANES.iter().copied().filter(|x| w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [*x, 22, 6])).collect();
            for x in held {
                *v_held.entry(x).or_insert(0) += 1;
            }
            if ctl.rel == Some(rel) {
                let occ: Vec<String> = LANES
                    .iter()
                    .map(|x| {
                        let n = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && {
                            let c = cell(e.pos);
                            c[0] == *x && (16..=22).contains(&c[1]) && (0..=6).contains(&c[2])
                        }).count();
                        format!("lane {x}: {n}")
                    })
                    .collect();
                occupancy_rows.push(format!("t {rel} (a release): {}", occ.join(", ")));
            }
        }
        // (ironfarm 11:19) the napper's per-tick trace over --nap-trace FROM,TO: its cart's position
        // and velocity, its lane's R (x-2,24,7) powered or not, every cart within 1.5 of it
        if let Some((a, b)) = o.nap_trace
            && rel >= a
            && rel <= b
        {
            for (vid, k, _) in &nappers {
                let Some(vi) = redsim::entity::index_of(w, *vid) else { continue };
                let Some(ci) = w.entities.list[vi].vehicle.and_then(|v| redsim::entity::index_of(w, v)) else { continue };
                let cart = &w.entities.list[ci];
                let x = LANES[*k];
                let r_on = w.get(redsim::pos::BlockPos::new(x - 2, 24, 7)).to_string().contains("powered=true");
                let near: Vec<String> = w
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && e.id != cart.id && matches!(e.kind, EntityKind::Minecart(_)) && e.pos.distance_to_sqr(cart.pos) <= 2.25)
                    .map(|e| format!("cart {} at ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}){}", e.id, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.passengers.is_empty() { " empty" } else { "" }))
                    .collect();
                nap_trace_rows.push(format!(
                    "t {rel}: napper cart {} at ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}); R ({},24,7) {}; within 1.5: {}",
                    cart.id,
                    cart.pos.x,
                    cart.pos.y,
                    cart.pos.z,
                    cart.delta_movement.x,
                    cart.delta_movement.y,
                    cart.delta_movement.z,
                    x - 2,
                    if r_on { "powered" } else { "unpowered" },
                    if near.is_empty() { "none".to_string() } else { near.join("; ") }
                ));
            }
        }
        if o.trace > 0 && rel % o.trace == 0 {
            log.push(format!("t {rel}: day time {day}"));
        }
        if !w.errors.is_empty() {
            return Err(format!("a model error at t {rel}: {}", w.errors.join("; ")));
        }
    }
    let mut s = format!(
        "f1q2-run ({}; REAL brains, the mob AI on): {} villagers, {} gt from day time {} (seed {}){}{}\n",
        if o.drive { "phase 2: the station, the R line and the split's latches driven, every drive EMULATED (day-1's v6 at s 12)" } else { "phase 1: every cart at rest, no drive" },
        recs.len(),
        o.ticks,
        o.day0,
        o.seed,
        if o.restarts.is_empty() { String::new() } else { format!(", {} restarts (order {})", o.restarts.len(), o.restart.label()) },
        control_id.map(|_| format!(", the positive control on v{}", o.control_rider.unwrap_or(0))).unwrap_or_default()
    );
    s.push_str(&format!(
        "the world as built: {} rails whose shape differs from the file (0 required: the file pasted unsettled){}\n",
        rail_mismatch.len(),
        rail_mismatch.iter().take(6).map(|m| format!("; {m}")).collect::<String>()
    ));
    if o.drive {
        s.push_str(&format!("the controller (EMULATED): {} golems seen, {} cycles completed\n", ctl.golems.len(), ctl.cycles));
        for (k, t) in turns.iter().enumerate() {
            s.push_str(&format!(
                "  lane x {}: pocket turns {}, with a clear {}, without (a miss) {}; draws while panicking in the pocket: landed with a complete path {}, landed with none {} (null draws aren't seen from outside the behaviour); panic ticks in the pocket {}, of them on a landed draw with no path (non-clearing) {}\n",
                LANES[k],
                t.turns,
                t.cleared,
                t.turns - t.cleared,
                t.landed_reach,
                t.landed_unreach,
                t.panic,
                t.nopath
            ));
        }
        for l in turn_lines.iter().take(120) {
            s.push_str(&format!("    {l}\n"));
        }
    }
    let worst = recs.values().filter_map(|r| r.peak.map(|p| p.0)).max().unwrap_or(0);
    let lost: usize = recs.values().filter(|r| !r.home_lost.is_empty()).count();
    let over: i64 = recs.values().map(|r| r.over_1000).sum();
    let dusk_max = recs.values().filter_map(|r| r.dusk.flatten()).max();
    let dusk_unarmed = recs.values().filter(|r| r.dusk == Some(None)).count();
    s.push_str(&format!(
        "the stamp at dusk (each rider's first REST tick, the day's carry-over): the largest {}, unarmed at dusk {} of {}\n",
        dusk_max.map(|v| v.to_string()).unwrap_or("-".to_string()),
        dusk_unarmed,
        recs.len()
    ));
    let mut dc = [0i64; 4];
    let laps: i64 = recs.values().map(|r| r.laps).sum();
    for r in recs.values() {
        for k in 0..4 {
            dc[k] += r.day_clears[k];
        }
    }
    s.push_str(&format!(
        "daytime clears (outside REST) by place, all riders: the pocket {}, the lane (R_1, the slopes) {}, the siding (K, F, R) {}, elsewhere {}; over {laps} laps (pocket turns): {:.2} / {:.2} / {:.2} / {:.2} a lap\n",
        dc[0],
        dc[1],
        dc[2],
        dc[3],
        dc[0] as f64 / laps.max(1) as f64,
        dc[1] as f64 / laps.max(1) as f64,
        dc[2] as f64 / laps.max(1) as f64,
        dc[3] as f64 / laps.max(1) as f64
    ));
    // (every clear by place and cause, a lap = a pocket turn)
    let mut by = [[0i64; 4]; 4];
    for r in recs.values() {
        for p in 0..4 {
            for c in 0..4 {
                by[p][c] += r.clears_by[p][c];
            }
        }
    }
    let lapsf = laps.max(1) as f64;
    for (p, name) in ["the pocket", "the lane (R_1, the slopes)", "the siding (K, F, R)", "elsewhere"].iter().enumerate() {
        s.push_str(&format!(
            "clears at {name}, all riders: a nap {} ({:.2} a lap), the target reached {} ({:.2}), a complete path {} ({:.2}), no target on record {}\n",
            by[p][0],
            by[p][0] as f64 / lapsf,
            by[p][1],
            by[p][1] as f64 / lapsf,
            by[p][2],
            by[p][2] as f64 / lapsf,
            by[p][3]
        ));
    }
    let mut dby = [[0i64; 3]; 4];
    let mut dbp = [[0i64; 3]; 4];
    let mut stuck = [0i64; 4];
    for r in recs.values() {
        for p in 0..4 {
            for k in 0..3 {
                dby[p][k] += r.draws_by[p][k];
                dbp[p][k] += r.draws_by_panic[p][k];
            }
            stuck[p] += r.stuck_ticks[p];
        }
    }
    for (p, name) in ["the pocket", "the lane (R_1, the slopes)", "the siding (K, F, R)", "elsewhere"].iter().enumerate() {
        s.push_str(&format!(
            "draws at {name}, all riders (a new walk target, classified at the first path the sink computes for it, or never pathed when the target changes or clears first): while panicking: complete {}, PARTIAL (the sink stuck, WALK_TARGET held) {}, never pathed {}; calm: complete {}, PARTIAL {}, never pathed {}; ticks WALK_TARGET held on a path for it that isn't complete {}\n",
            dbp[p][0], dbp[p][1], dbp[p][2], dby[p][0], dby[p][1], dby[p][2], stuck[p]
        ));
    }
    // (the list for F3's lane candidates: every panic draw whose first path was partial, by its
    // place, its target's cell and the rider's cell, the most frequent first)
    let mut pl: Vec<(&(usize, [i32; 3], [i32; 3]), &i64)> = partial_list.iter().collect();
    pl.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    s.push_str(&format!("the panic draws whose first path was PARTIAL, by place, target cell and rider cell ({} distinct, {} draws):\n", pl.len(), pl.iter().map(|x| *x.1).sum::<i64>()));
    for ((p, t, rider), n) in pl.iter().take(80) {
        s.push_str(&format!(
            "  {} x{n}: target {:?} from the rider at {:?}\n",
            ["pocket", "lane", "siding", "elsewhere"][*p],
            t,
            rider
        ));
    }
    // (the panic draws by the target's kind)
    s.push_str(&format!(
        "GOLEM_DETECTED_RECENTLY at the stand (ironspec 11:29/11:30: every stand arrival with the memory expired at least 20 gt before; the neighbour's golems {}): {} stand arrivals, {} with the memory still set (blocked from a summon; bar 0), {} expiring within 20 gt of the arrival, the least margin (arrival - (last set + 599)) {}; {}{}{}\n",
        match o.neighbour {
            Some(d) => format!("EMULATED at ours + ({},{},{}), {} sets", d[0], d[1], d[2], gdr.neighbour_sets),
            None => "not modelled".to_string(),
        },
        gdr.arrivals,
        gdr.blocked,
        gdr.late,
        gdr.least_margin.map_or("none".to_string(), |m| format!("{m} gt")),
        if o.neighbour.is_some() {
            format!(
                "the least time from a rider's last tick in the neighbour pad's detection box to its next stand arrival {} (bar >= 619), {} under",
                gdr.least_box_gap.map_or("none (never in the box)".to_string(), |g| format!("{g} gt")),
                gdr.box_short
            )
        } else {
            String::new()
        },
        if gdr.blocked + gdr.late + gdr.box_short == 0 { " (PASS)" } else { " (FAIL)" },
        {
            let mut v: Vec<String> = gdr.rows.clone();
            v.extend(gdr.box_rows.iter().cloned());
            v.extend(gdr.napper_rows.iter().cloned());
            if v.is_empty() { String::new() } else { format!(": {}", v.join("; ")) }
        }
    ));
    s.push_str(&format!(
        "GOLEM_DETECTED_RECENTLY's settings by setter (ironspec 12:23): {} by a summon's 10-box stamp (a golem born that tick: {} on a summoner it counted, {} on a bystander; a bystander's least expiry margin at its next summon {}), {} by the GolemSensor reading the living list ({} of them within 600 gt after the rider's own summon's stamp), {} by the neighbour's golems (EMULATED){}{}\n",
        renew.stamps,
        renew.summoner_stamps,
        renew.bystander_stamps,
        renew.bystander_margin.map_or("none seen".to_string(), |m| format!("{m:+} gt")),
        renew.sensor,
        renew.after_own,
        renew.neighbour,
        if renew.rows.is_empty() { String::new() } else { format!(": {}", renew.rows.join("; ")) },
        if renew.bystander_rows.is_empty() { String::new() } else { format!("; the bystanders: {}", renew.bystander_rows.join("; ")) }
    ));
    s.push_str(&format!(
        "the summons from the pockets (farmopt 12:10, ironspec 12:13: the stand is the pocket; at each summon tick T every scared, slept villager in a pocket cell (x,0,0) calls, the villagers in its box inflated by 10 counted or rejected; a memory present at T: a ttl of 1 or more before the tick): {} calls, {} callers blocked by their own GOLEM_DETECTED_RECENTLY (by setter: {}), {} villagers in the boxes rejected for it (not callers: informational), {} summons blocked by those rejections (participations blocked: callers blocked + summons blocked, bar 0: {}); the least free caller's margin (T - (last set + 599)) {}; each lane's least rotation at a summon tick (the riders whose last lane is it, a napper out while away; bar >= 7): {}; the most nappers away from one lane at once: {}{}\n",
        summons.attempts,
        summons.callers_blocked,
        if summons.blocked_by.is_empty() { "none".to_string() } else { summons.blocked_by.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ") },
        summons.rejected,
        summons.blocked,
        if summons.blocked + summons.callers_blocked == 0 { "PASS" } else { "FAIL" },
        summons.least_caller_margin.map_or("none seen".to_string(), |m| format!("{m} gt")),
        LANES.iter().zip(summons.min_rotation.iter()).map(|(x, m)| format!("lane {x} {}", m.map_or("-".to_string(), |v| v.to_string()))).collect::<Vec<_>>().join(", "),
        usize::from(!nappers.is_empty()),
        if summons.rows.is_empty() { String::new() } else { format!(": {}", summons.rows.join("; ")) }
    ));
    for (vid, _, _) in &nappers {
        if let Some(r) = recs.get(vid)
            && let Some(at_r) = r.milestones.iter().find(|(w, _)| w == "at R").map(|(_, t)| *t)
        {
            s.push_str(&format!(
                "the departing napper {} (ironspec 11:29: out of the bed's 16-ball within 152 gt of reaching R): at R t {at_r}, out of the ball {}\n",
                r.label.split(' ').next().unwrap_or(""),
                r.ball_exit_after_r.map_or("never (FAIL)".to_string(), |g| format!("{g} gt later{}", if g <= 152 { " (PASS)" } else { " (FAIL)" }))
            ));
        }
    }
    for (si, (name, seat, cands)) in o.seats.iter().enumerate() {
        let st = &seat_stats[si];
        s.push_str(&format!(
            "the seat {name} ({},{},{}), candidates {}: {} rider-ticks on it, {} stamp resets (complete draws to a candidate from it; {}), {:.2} per 1,000 rider-ticks, the longest stretch on it without a reset {} gt\n",
            seat[0],
            seat[1],
            seat[2],
            cands.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "),
            st.occupied,
            st.resets,
            cands.iter().map(|c| format!("({},{},{}) {}", c[0], c[1], c[2], st.by_cand.get(c).copied().unwrap_or(0))).collect::<Vec<_>>().join(", "),
            if st.occupied > 0 { st.resets as f64 * 1000.0 / st.occupied as f64 } else { 0.0 },
            st.longest
        ));
    }
    s.push_str(&format!(
        "the staleness bar (ironspec 11:02, G1: NEAREST_HOSTILE up to ~39 gt stale; every entry into a position within 6.05 of a zombie hidden from it at least 40 gt after the rider's last position within 8 of a zombie in its sight; the stop's waker places left out): {} entries, the least gap {}, {} under 40 gt ({}){}\n",
        stale.entries,
        stale.least.map_or("none (never seen within 8 before)".to_string(), |g| format!("{g} gt")),
        stale.bad,
        if stale.bad == 0 { "PASS" } else { "FAIL" },
        if stale.rows.is_empty() { String::new() } else { format!(": {}", stale.rows.join("; ")) }
    ));
    s.push_str(&format!(
        "the panic draws by the target's kind (complete / PARTIAL / never pathed): a point or a block (the flee, the strolls, HOME) {} / {} / {}; an entity walk (to another villager seen within 8: one draw however many cells its mob crosses) {} / {} / {}\n",
        by_kind[0][0], by_kind[0][1], by_kind[0][2], by_kind[1][0], by_kind[1][1], by_kind[1][2]
    ));
    // (ironspec 10:02) carts on carts; (ironspec 09:59) villagers near the bed; (phase 3a) nappers
    s.push_str(&format!(
        "a cart resting on another cart's top (a failure): {stacked_ticks} cart-ticks{}\n",
        if stacked.is_empty() { " (PASS)".to_string() } else { format!(" (FAIL): {}", stacked.join("; ")) }
    ));
    s.push_str(&format!(
        "villagers within 16 of the bed head ({},{},{}): {} villager-ticks over {} cells; the bed occupied {} ticks; calm (not panicking) REST ticks within 16 while occupied: {} (the bar: 0){}\n",
        o.bed[0],
        o.bed[1],
        o.bed[2],
        near.ticks,
        near.cells.len(),
        near.occupied_ticks,
        near.calm_rest_occupied,
        if near.calm_rest_rows.is_empty() { String::new() } else { format!(": {}", near.calm_rest_rows.join("; ")) }
    ));
    if !near.cells.is_empty() {
        let mut cs: Vec<(&[i32; 3], &i64)> = near.cells.iter().collect();
        cs.sort_by_key(|x| -x.1);
        s.push_str(&format!("  the cells (villager-ticks): {}\n", cs.iter().take(40).map(|(c, n)| format!("{c:?} {n}")).collect::<Vec<_>>().join(", ")));
    }
    if o.napper {
        s.push_str(&format!("phase 3a, the nappers (EMULATED drives: day-1's events 10:00, 58f770cc's stop): {} seeded\n", nappers.len()));
        for (vid, k, at) in &nappers {
            if let Some(r) = recs.get(vid) {
                let x = LANES[*k];
                let designed: Vec<[i32; 3]> = vec![[x - 2, 25, 9], [x - 2, 25, 8], [x - 2, 24, 7], [x - 2, 24, 6], [x - 1, 24, 6], [x - 1, 24, 7], [x, 24, 7]];
                // (the observed cells up to the lane's top: the route's cells from K, the lane's
                // own slope cells after C cut off)
                let seen: Vec<[i32; 3]> = match r.route.iter().position(|c| *c == [x, 24, 7]) {
                    Some(i) => r.route[..=i].to_vec(),
                    None => r.route.clone(),
                };
                let verdict = if r.route.is_empty() {
                    "never on its K".to_string()
                } else if seen == designed {
                    "identical to the designed K, F, R, c1, c2, c3, C (PASS)".to_string()
                } else {
                    format!("DIFFERS from the designed {} (FAIL)", designed.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "))
                };
                s.push_str(&format!(
                    "  {} (seeded t {at}, lane x {}): {}; slept {}; HOME at the end {}; its route from K: {}: {verdict}\n",
                    r.label.split(' ').next().unwrap_or(""),
                    LANES[*k],
                    if r.milestones.is_empty() { "no milestone".to_string() } else { r.milestones.iter().map(|(w, t)| format!("{w} t {t}")).collect::<Vec<_>>().join(", ") },
                    r.naps,
                    if r.home_lost.is_empty() { "kept" } else { "LOST" },
                    r.route.iter().take(40).map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ")
                ));
            }
        }
        s.push_str(&format!("  the hold's pulses fired with the hold empty: {} (bar 0)\n", ctl.empty_pulses));
        s.push_str(&format!("  the drives: {}\n", ctl.nap_events.iter().take(120).cloned().collect::<Vec<_>>().join("; ")));
        if !nap_trace_rows.is_empty() {
            s.push_str("  the napper's trace (per tick):\n");
            for row in &nap_trace_rows {
                s.push_str(&format!("    {row}\n"));
            }
        }
    }
    if let Some((cid, a, b)) = o.cart_trace {
        s.push_str(&format!(
            "cart {cid}'s cells over t {a}..{b}: {}\n",
            if cart_cells.is_empty() { "not found".to_string() } else { cart_cells.iter().map(|(t, c)| format!("t {t} ({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ") }
        ));
        s.push_str(&format!(
            "the lanes' H..V occupancy (carts in x = the lane, y 16..22, z 0..6) at each release over t {a}..{b}: {}; V (x,22,6) held: {}\n",
            if occupancy_rows.is_empty() { "no release".to_string() } else { occupancy_rows.join("; ") },
            LANES.iter().map(|x| format!("lane {x} {} ticks", v_held.get(x).copied().unwrap_or(0))).collect::<Vec<_>>().join(", ")
        ));
    }
    // (per rest block, the panic draws that landed: tick-exact's official table's rows; its null
    // draws don't land, so they aren't seen here)
    let official = o.compare_official.as_deref().map(parse_official);
    s.push_str(&format!(
        "the panic draws that landed, by the rider's block (redsim: complete / PARTIAL / never pathed{}):\n",
        if official.is_some() { "; the official log: null / complete / PARTIAL / never pathed or no path" } else { "" }
    ));
    let mut cells: std::collections::BTreeSet<[i32; 3]> = by_cell.keys().copied().collect();
    if let Some(off) = &official {
        cells.extend(off.keys().copied());
    }
    for c in &cells {
        let n = by_cell.get(c).copied().unwrap_or([0; 3]);
        let off = official.as_ref().map(|m| m.get(c).copied().unwrap_or([0; 4]));
        s.push_str(&format!(
            "  {c:?}: {} / {} / {}{}\n",
            n[0],
            n[1],
            n[2],
            off.map(|f| format!("   official {} / {} / {} / {}", f[0], f[1], f[2], f[3])).unwrap_or_default()
        ));
    }
    s.push_str(&format!("each PARTIAL panic draw (the first {} of {}): the tick, the rider and its lane, its cell and place (the cart at rest or rolling), the scarer (NEAREST_HOSTILE at the draw), the target, the partial path's end, how long WALK_TARGET was held and whether the hold outlasted the rider's stay at its cell:\n", partial_rows.len(), pl.iter().map(|x| *x.1).sum::<i64>()));
    for row in partial_rows.iter().take(150) {
        s.push_str(&format!("    {row}\n"));
    }
    let wg = recs.values().filter_map(|r| r.worst_gap.map(|g| (g, r.label.clone()))).max_by_key(|x| x.0 .0);
    let wn = recs.values().filter_map(|r| r.worst_night_gap.map(|g| (g, r.label.clone()))).max_by_key(|x| x.0 .0);
    s.push_str(&format!(
        "the longest stretch between two clears (a rider's last clear to its next): {}; of those ending in REST: {} (ironspec's structural bar <= 1,000)\n",
        wg.map(|((g, a, b), l)| format!("{g} gt ({l}, t {a}..{b})")).unwrap_or("-".to_string()),
        wn.map(|((g, a, b), l)| format!("{g} gt ({l}, t {a}..{b})")).unwrap_or("-".to_string())
    ));
    s.push_str(&format!(
        "the bar: the largest stamp age at a calm REST tick with no walk target {worst} (<= 1,000: {}); calm REST ticks over 1,000: {over}; HOME lost by {lost} of {} villagers{}\n",
        if worst <= 1000 { "PASS" } else { "FAIL" },
        recs.len(),
        if lost == 0 { " (PASS)" } else { " (FAIL)" }
    ));
    for r in recs.values() {
        s.push_str(&format!(
            "  {}: HOME at the start {:?}, at dawn {}; the stamp at dusk {}; calm REST ticks with no walk target {}, the largest stamp age {}; over 1,000: {}; naps {}; laps {}; the longest stretch between clears {} (ending in REST {}); clears {}{}; HOME lost {}{}\n",
            r.label,
            r.home_at_start,
            match &r.home_at_dawn {
                Some(h) => format!("{h:?}"),
                None => "(no dawn in the run)".to_string(),
            },
            match r.dusk {
                Some(Some(a)) => a.to_string(),
                Some(None) => "unarmed".to_string(),
                None => "(no dusk)".to_string(),
            },
            r.calm_rest_ticks,
            r.peak.map(|(a, t, c)| format!("{a} (t {t} at {c:?})")).unwrap_or_else(|| "none".to_string()),
            r.over_1000,
            r.naps,
            r.laps,
            r.worst_gap.map(|(g, a, b)| format!("{g} (t {a}..{b})")).unwrap_or("-".to_string()),
            r.worst_night_gap.map(|(g, a, b)| format!("{g} (t {a}..{b})")).unwrap_or("-".to_string()),
            r.clears.len(),
            r.clears.iter().take(4).map(|c| format!("; {c}")).collect::<String>(),
            r.home_lost.len(),
            r.home_lost.iter().map(|c| format!("; {c}")).collect::<String>()
        ));
    }
    for l in log.iter().take(40) {
        s.push_str(&format!("  {l}\n"));
    }
    for l in ctl.log.iter().take(60) {
        s.push_str(&format!("  ctl {l}\n"));
    }
    Ok(s)
}

fn anyhow_free(ok: bool, why: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.to_string()) }
}
