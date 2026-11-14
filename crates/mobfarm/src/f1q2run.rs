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
    /// (ironspec 01:59 PT) The old seeding: the napper queued at the way-in from the start and its
    /// lane one slope cart short. Off (the default): the build guide's start, the module's queued
    /// cart dropped and every lane full.
    pub seed_queued_napper: bool,
    /// (ironspec 02:18 PT (2)) DAY's level: the daylight detector at this power or above is DAY
    /// (the design's L-6 line: 6; 1 is the positive control that must reproduce the lost window
    /// picks).
    pub day_threshold: u32,
    /// (ironspec 06:31 / 06:38 PT) State (ii): the bench takes the bed's single POI ticket at the
    /// start. Off (the default): the design start (i), every HOME a memory and the ticket free.
    pub bed_ticket_taken: bool,
    /// (day-1 05:45 PT, ironspec 06:24 real) the controller line ends the module declares as
    /// redstone-block stand-ins (`# stand-in: x,y,z (… redstone block …)`).
    pub block_standins: Vec<[i32; 3]>,
    /// (ironspec 06:36 real, the oldest-stamp bar's known positive) by day, one lane rider (lane
    /// x -1's first rider found off its pocket) is given a walk target no path reaches: its stamp must
    /// age past 1,000 gt and the bar must FAIL.
    pub stamp_control: bool,
    /// (ironspec 06:31 real, the farm-stop bar's known positive) the planted erasure: the first napper
    /// to reach the bed stop loses its HOME there; the farm-stop bar must FAIL.
    pub home_erase_control: bool,
    /// (ironspec 06:24 platform, G1: the air bar's positive) the first rider to enter water has its
    /// cart held there 340 gt: it must drown and the air-supply bar FAIL.
    pub air_control: bool,
    /// (ironspec 06:33 / 06:35 platform: 9fc3155f's coverage map from the composed run) a file for
    /// every distinct night eye position of a rider (redsim's seated eye, to 0.01) with its ticks
    /// in all, in REST and in PANIC.
    pub rider_trace: Option<std::path::PathBuf>,
    /// (ironspec D2's known positive) RS_ready forced set at the first departure, before its napper
    /// passes R: the next release must fail "no R1 release before the previous napper passes R".
    pub rs_ready_early_control: bool,
    /// (G1 N8) The bystander control's dud: the call asks 99 willing, so it never stamps and the
    /// control must show NOT EXERCISED.
    pub bystander_dud: bool,
    /// (G1 N18) The DAY update check's control: compare at the half-period off-phase; the bar must
    /// catch it at the edges.
    pub day_phase_control: bool,
    /// (ironspec 07:10 real (2), tick-exact 06:09 PT: the reconciliation row) tick-exact's farm-c7
    /// placement, N riders summoned per lane behind each lane's stored H rider: the night held at
    /// 13,800, the record's overrides laid, N rounds of one cart a lane at (x + 0.6, 24, 6.49) 30 gt
    /// apart, 200 gt, the stored queue cart killed, the napper launched at (8.5, 25.0625, 9.5) with
    /// motion (0.2, 0, 0); its t 0 there, the release lever off at t 117, the night skipped to
    /// 24,000 at t 137 with the time running; no controller drives.
    pub c7_placement: Option<usize>,
    /// The record's six stored riders get HOME = the bed too (the design-occupancy variant).
    pub c7_home_stored: bool,
    /// (ironspec 06:38 platform / 7b90c3f1's r4 bench, farmopt 07:03 PT) the lift exit blocked: at
    /// run tick T an empty cart is held still on the exit (-5,23,8) for N gt, then removed (the
    /// stated maximum ~11 gt must pass the air bar; >= 25 gt is the jam positive).
    pub lift_block: Option<(i64, i64)>,
    /// (cc767b78 07:23 PT, the replay's divergence) with --drive-out: the scene's entity eN traced
    /// per tick over run ticks A..=B (its position, motion, rider, its cell's block and the block
    /// under it, after each tick).
    pub trace_entity: Option<(usize, i64, i64)>,
    /// (G1 08:21 PT) late-path pairs to check against the cart trajectories: (draw cell, path cell,
    /// the least and most gt between).
    pub pair_checks: Vec<([i32; 3], [i32; 3], i64, i64)>,
    /// (ironspec 08:26 platform (3)) the chain trace over run ticks FROM..=TO.
    pub chain_trace: Option<(i64, i64)>,
    /// (G1 / ironspec 08:41 platform) the unrouted bar's positive: the first napper reaching its
    /// siding's R after t 1,000 has PRESENT_l reset on arrival (no release can follow).
    pub strand_control: bool,
    /// (9fc3155f / G1 09:05) every cart's cell transitions through the split row, sidings, merges,
    /// S-bends and drops, written to this file.
    pub cart_cells: Option<std::path::PathBuf>,
    /// (G1 X19's control) the first lane rider in REST after t 1,000 gets its stamp aged to 1,100 gt:
    /// the at-entry bar must fail on it alone.
    pub at_entry_control: bool,
    /// (ironspec 08:26 / 08:41 platform) the fail-safe's control: the first napper cart at a siding's
    /// last cell before R after t 1,000 is held 130 gt; it must rejoin a D period late and pass.
    pub late_return_control: bool,
    /// (ironspec 09:44 / 09:48, G1 09:45) the design's declared never-switched Ts (its
    /// `placement-order-ts:` cells): any change of their shape during the run FAILS.
    pub stable_ts: Vec<[i32; 3]>,
    /// (9fc3155f / G1 09:39) --cart-cells' box (default the split row's region y 22..26, z 5..10, x
    /// -6..10).
    pub cart_cells_box: Option<([i32; 3], [i32; 3])>,
    /// (G1 10:24 (c)) the per-lane count bar's positive control: two misroutes into lane -1.
    pub misroute_control: bool,
    /// (G1 10:34) the forced lip row: the first cart on a drop cell after t 1,000 held this many gt.
    pub lip_hold: Option<i64>,
    /// (G1 10:36) the drop-entry envelope: the most off-rail ticks on the drop cell and the largest
    /// lateral displacement of a normal pass (tick-exact's game rows: ~5 gt, 0.198).
    pub lip_envelope: (i64, f64),
    /// (G1 10:34) the siding trace's window.
    pub siding_trace: Option<(i64, i64)>,
    /// (G1 10:47 blocking 4's control) the first napper seated on its siding's R with PRESENT_l set
    /// after t 1,000 is dismounted there: PRESENT_l must stay set.
    pub eject_control: bool,
    /// (day-1 11:40) hold the outbound napper's J's off until (8,25,9), as before the per-J release.
    pub picker_hold_to_leg: bool,
    /// (G1's P1 controls) "rest": a cart pinned at rest in lane -1's column cell until its next pass;
    /// "moving": a cart falling through it at a lane -1 pass's start.
    pub vholds_control: Option<String>,
    /// (cc767b78 11:15) The build's offset from the design frame (x / z whole chunks).
    pub offset: [i32; 3],
    /// (day-1's Delta, ironspec 12:41 (4)) the hold pulse's shift (gt).
    pub hold_pulse_delta: i64,
    /// (ironspec 13:04 (1), cc767b78's switch) a seated rider's doPush onto carts off: a counterfactual.
    pub rider_push_off: bool,
    /// (ironfarm 12:47) Write each lip rest's lane over its 60 gt before into this file.
    pub lip_dump: Option<std::path::PathBuf>,
    /// (G1 12:21) A villager's per-tick state over FROM..TO: (id, from, to).
    pub villager_trace: Option<(u32, i64, i64)>,
    /// (day-1 12:49 / ironspec 12:52) The outbound J release evidence-keyed (tripwires east of each J).
    pub evidence_keyed: bool,
    /// The evidence keying's delay from a press to the J's hold-off end (gt).
    pub evidence_delay: i64,
    /// (ironspec 12:54 (2)) F1's REL gated by J(-1)'s armed latch.
    pub rel_gate_armed: bool,
    /// (farmopt 13:30) the lanes' pocket offsets after REL (None: 0 / 14 / 24).
    pub lane_offsets: Option<[i64; 3]>,
    /// (day-1 13:43) the no-string ring with its clock-locked L's (L_J3(1), L_J7(1), L_J7(2), L_J7(3),
    /// L_J(-1)(3)); None: today's ring.
    pub ring_timed: Option<[i64; 5]>,
    /// (day-1 13:43) the ring's world-open term k.
    pub ring_k: i64,
    /// (day-1's G11 rev 15:42) the lane-7 pick's emulated D(7) after lane 3's real D(3) press.
    pub delta7: i64,
    /// (G1 15:37) ticks at which every cart's position is printed.
    pub positions_at: Vec<i64>,
    /// (G1 16:18 (2)) a fold file and its move whose `# integrity:` line is judged every tick.
    pub integrity_from: Option<(std::path::PathBuf, [i32; 3])>,
    /// (ironspec 16:50 (3)) every villager's VILLAGER_HOSTILES scan phase forced to this value mod 20.
    pub force_hostile_phase: Option<i64>,
    /// (G1 14:11's control) a pick moving nap_lane inside each napper's window.
    pub nap_lane_control: bool,
    /// (G1 14:21's control) the old defect restored: each K write's cell from nap_lane at fire time.
    pub stuck_k_control: bool,
    /// (ironspec 10:51 (g)) the longest designed rest on a station hold H's trapdoor, gt.
    pub h_rest_bound: i64,
    /// (ironspec 10:51 (f)) the siding check's positive: a batch cart planted on lane -1's siding R.
    pub siding_plant_control: bool,
    /// (G1 10:47 blocking 5) the c7 opt-out's per-entity rows (HOME holders under Load -> as loaded),
    /// the last row "... (FAIL)" when it changed nothing; empty in design runs.
    pub c7_port_home: Vec<String>,
    /// (G1 10:47 O12) the `no-home`-tagged villagers in the design (a design run with one FAILS).
    pub no_home_tagged: usize,
    /// (G1 11:19 F4 (c)) a lane-7 napper's cart sent past a switched K(3).
    pub k_misroute_control: bool,
    /// (G1 11:19 F4 (c)) a westbound cart released on the split row.
    pub westbound_control: bool,
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
    /// (mob-AI 14:14) Record the run as a mobai drive v1: the command line for its header (None:
    /// no drive).
    pub drive_rec: Option<String>,
    /// (the brief's watchdog, ironspec 16:10) a cycle starts when none started within W live gt
    /// (0: the golem pulse alone, the old emulation).
    pub watchdog: i64,
    /// (ironspec 16:08's recovery row) every villager's LAST_SLEPT stale at the start.
    pub stale_start: bool,
    /// (divert sampling (i)) the picker reads HELD at the picked cart's row arrival.
    pub held_at_divert: bool,
    /// (ironspec 17:16) the queue's hold term in gt (0: the release as soon as the stop is free).
    pub queue_hold: i64,
    /// (G1's guard) the cells the module declares `emulated:` (a drive may write a stand-in there
    /// whose golem and path properties differ from the built cell's), and the state each names.
    pub emulated: Vec<[i32; 3]>,
    pub emulated_states: Vec<([i32; 3], String)>,
    /// (7b90c3f1 18:06) the module's declared placement-order rail Ts.
    pub ts: Vec<[i32; 3]>,
    /// (ironspec 18:31 (2) / (3)) the rail Ts the module declares `update-only-ts:` (state-stable
    /// for the state they hold: a stand-in's update may reach them).
    pub update_only: Vec<[i32; 3]>,
    /// (ironspec 19:24, the direct summon check's control) the lane x whose pocket rider is
    /// removed at the start.
    pub empty_pocket: Option<i32>,
    /// (ironspec 23:56 PT (4)) the gossip bars' positive control: a gossip attempt logged from lane
    /// x -1's pocket at t 1,000 (the bars must count it).
    pub gossip_control: bool,
    /// (ironspec 02:48 PT) the harm bar's control: a real summon stamping a napper, `late` on its
    /// return across the merge row (the harm bar must FAIL) or `early` at its own launch (it must
    /// PASS).
    pub bystander_control: Option<String>,
    /// (ironspec 22:03 / 22:09 PT) the module's `k-lock: N (why)` and `k-sw: N (why)` lines: the
    /// hold gate's delayed Q and the siding's repeater after D (a napper run refuses a module
    /// without them).
    pub k_lock: Option<i64>,
    pub k_sw: Option<i64>,
    /// (ironspec 21:25 PT) the stops and buffers the module's header declares (`stops: CELLS
    /// (why)`): the only cells a cart may touch.
    pub stops: Vec<[i32; 3]>,
    /// (ironspec 23:16 PT) the module's `# inert:` and `# always-on:` cells: watched through the
    /// run, any state change fails.
    pub inert: Vec<[i32; 3]>,
    pub always_on: Vec<[i32; 3]>,
    /// (ironspec 23:51 PT (2)) the module's `# push-pair: A+B=REASON` declarations: the two cells and
    /// the displacement bound the reason states (its first number, in blocks).
    pub push_pairs: Vec<([i32; 3], [i32; 3], f64)>,
    /// (ironspec 00:33 PT, 58f770cc 00:39) the module's `# a6-pickup: x,y,z=…` line: A6's pickup cell
    /// P and the stranded bound (the number after "wake edge +" in its reason).
    pub a6_pickup: Option<([i32; 3], i64)>,
    /// (G1's [7]) the module declares its (C) way-in composed (`# way-in:`): the fall's exemption off.
    pub way_in_composed: bool,
    /// (G1 on e7dc61d21) the inert watch's positive control: J(-1)'s lever (-1,26,9), toggled by
    /// the run, declared inert; the run must fail.
    pub inert_control: bool,
    /// The declared inert / always-on cells' reasons (for the starting-kind check).
    pub watch_reasons: Vec<([i32; 3], String)>,
    /// (ironspec 20:40 PT) the feed stack's direct bench instead of the run.
    pub stack_bench: bool,
    /// (ironspec 20:34 PT) the queue's dwell cap from the module's `dwell-cap: N (why)` line (day-1's
    /// lane model; 29 on today's way-in, 58 with the water descent); none declared fails the bar.
    pub dwell_cap: Option<i64>,
}

/// The three lanes (x) and their launch offsets after REL (s 12).
const LANES: [i32; 3] = [-1, 3, 7];
// (ironspec 20:07 PT: lane 3's offset 14 is final; day-1 moves lane 3's tap + 2)
const OFFS: [i64; 3] = [0, 14, 24];
/// (ironspec 22:03 / 22:09 PT, the three K names) k_lock: the hold gate's delayed Q (line 3, the
/// waiting RS's Q delayed k_lock gt, sampled with the live Q at H2 = E + 2 = ARM3 + 4); k_sw: the
/// S-bend siding's repeater after D's edge (line 14: R powered at D + k_sw + 8); K(x) are the split's
/// K Ts. k_lock and k_sw come from the module's `k-lock:` / `k-sw:` header lines (no hard-coded
/// value).

/// A rider's place kind by its cell: 0 the pocket (x,0,0), 1 the lane (R_1 (x,17,1) and the slopes
/// (x,18..22,2..6)), 2 the siding (the S-bend's K (x-2,25,9), F (x-2,25,8), R (x-2,24,7)), 3
/// elsewhere.
/// (ironspec's E7) The night a day time's REST belongs to: the evening half (12,000..24,000) is its
/// own day's night, the morning before 12,000 the previous day's.
pub fn night_of(day_time: i64) -> i64 {
    let d = day_time.div_euclid(24_000);
    if day_time.rem_euclid(24_000) >= 12_000 { d } else { d - 1 }
}

/// (G1 N15) GOLEM_DETECTED_RECENTLY's lifetime as the margins count it: redsim's
/// `GolemSensor.MEMORY_TIME_TO_LIVE` (599): a call at T' finds a memory set at T gone when
/// T' - (T + 599) >= 1.
pub const GOLEM_TTL: i64 = redsim::entity::mob::villager::GOLEM_MEMORY_TTL;

/// (G1 N13) ValidateNearbyPoi's predicate: the bed head's centre (x + 0.5, y + 0.5, z + 0.5)
/// strictly within 16 of a villager's feet, in squared distance (`BlockPos.closerToCenterThan`).
pub fn poi_near(bed: [i32; 3], feet: redsim::entity::Vec3) -> bool {
    let (dx, dy, dz) = (feet.x - (bed[0] as f64 + 0.5), feet.y - (bed[1] as f64 + 0.5), feet.z - (bed[2] as f64 + 0.5));
    dx * dx + dy * dy + dz * dz < 256.0
}

/// (G1 N4) The module's daylight detector (-5,15,3): its stored power, an error when the cell holds
/// no daylight detector or its power doesn't parse (one reader for every DAY use).
fn detector_level(w: &redsim::world::World) -> Result<u32, String> {
    let s = w.get(dbp(-5, 15, 3)).to_string();
    if !s.contains("daylight_detector") {
        return Err(format!("the DAY detector cell (-5,15,3) holds {s}, not a daylight detector"));
    }
    s.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).ok_or_else(|| format!("the DAY detector's power doesn't parse: {s}"))
}

/// (G1 07:17 PT; G1 X34: against an INDEPENDENT source) The WRITTEN scene's entity lines, parsed
/// back from the text the replayer reads, against the entities the run spawned as the world holds
/// them: one line per spawned root entity, in order, the same type, position and passengers (their
/// count and types), an error on any difference (an extra or missing line, a dropped cart, a
/// passenger off).
pub fn check_scene_text(spawned: &[(String, [f64; 3], Vec<String>)], text: &str) -> Result<(), String> {
    let lines: Vec<(String, [f64; 3], Vec<String>)> = text
        .lines()
        .filter_map(|l| l.strip_prefix("entity: "))
        .map(|l| {
            let f: Vec<&str> = l.splitn(5, ' ').collect();
            let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
            let nbt = f.get(4).copied().unwrap_or("");
            let passengers: Vec<String> = match nbt.find("Passengers:[") {
                Some(i) => nbt[i..].match_indices("id:\"").map(|(j, _)| nbt[i + j + 4..].split('"').next().unwrap_or("").to_string()).collect(),
                None => Vec::new(),
            };
            (f.first().copied().unwrap_or("").to_string(), [num(1), num(2), num(3)], passengers)
        })
        .collect();
    if lines.len() != spawned.len() {
        return Err(format!("--scene-out: the written scene lists {} entity lines, the run spawned {} entities", lines.len(), spawned.len()));
    }
    for (n, ((ty, p, ps), (sty, sp, sps))) in lines.iter().zip(spawned.iter()).enumerate() {
        let ty_ok = ty == sty || (ty.ends_with("_boat") && sty.ends_with("_boat"));
        let pos_ok = (0..3).all(|k| (p[k] - sp[k]).abs() < 1e-6);
        if !ty_ok || !pos_ok || ps != sps {
            return Err(format!("--scene-out: line e{n} is {ty} at ({},{},{}) with passengers {:?}, the run spawned {sty} at ({:.4},{:.4},{:.4}) with {:?}", p[0], p[1], p[2], ps, sp[0], sp[1], sp[2], sps));
        }
    }
    Ok(())
}

/// (ironspec 01:59 PT, the build guide's start; G1 U13) The module's stored queue cart (the cart at
/// the way-in's queue cell (9, bed y + 13, -4)) dropped from a scene's entity list: run_inner's own
/// drop, shared with its test.
pub fn drop_stored_queue_cart(entities: &mut Vec<(String, [f64; 3], Option<String>)>, bed_y: i32) {
    let q = [9, bed_y + 13, -4];
    entities.retain(|e| !(e.0.contains("minecart") && [e.1[0].floor() as i32, e.1[1].floor() as i32, e.1[2].floor() as i32] == q));
}

/// The run's spawned root entities as the world holds them (for [`check_scene_text`]): each one's
/// type, position and passengers' types.
fn spawned_view(w: &redsim::world::World, spawned: &[u32]) -> Result<Vec<(String, [f64; 3], Vec<String>)>, String> {
    let kind = |e: &redsim::entity::Entity| match &e.kind {
        EntityKind::Minecart(_) => "minecraft:minecart".to_string(),
        EntityKind::Mob(m) => format!("minecraft:{}", m.ty.def().id),
        _ => "minecraft:oak_boat".to_string(),
    };
    spawned
        .iter()
        .enumerate()
        .map(|(n, id)| {
            let k = redsim::entity::index_of(w, *id).ok_or_else(|| format!("--scene-out: e{n} (#{id}) isn't in the world"))?;
            let e = &w.entities.list[k];
            let ps: Vec<String> = e.passengers.iter().filter_map(|p| redsim::entity::index_of(w, *p).map(|q| kind(&w.entities.list[q]))).collect();
            Ok((kind(e), [dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z], ps))
        })
        .collect()
}

/// (G1 07:17 PT) The scene's entity lines as `--scene-out` writes them: every entity of the run's
/// spawn list in order, each preceded by its drive name `# eN`.
pub fn scene_entity_lines(entities: &[(String, [f64; 3], Option<String>)]) -> String {
    entities.iter().enumerate().map(|(n, (ty, p, nbt))| format!("# e{n}\nentity: {ty} {} {} {} {}\n", p[0], p[1], p[2], nbt.as_deref().unwrap_or(""))).collect()
}

/// (G1 / ironspec 08:31-08:33 platform; G1 10:47 blocking 1 / O1) The emulated controller's entity
/// keys across a world open: every id it holds (the held napper, the picked and diverted carts, the
/// nappers' visits, the releases', departures', picks' and diverts' logs, the new nappers, R's
/// detector riders, R's cart and its departures, the golems) follows its entity through the open's
/// id map; `follow` returns Err for a LIVE id the open didn't renew (a dead one keeps its old id,
/// said so). Returns each key's old -> new ids, one line per key.
fn follow_open(ctl: &mut Ctl, nappers: &mut [(u32, usize, i64)], follow: impl Fn(u32) -> Result<u32, String>) -> Result<Vec<String>, String> {
    let mut keys: Vec<String> = Vec::new();
    let pairs = |name: &str, ids: &[u32], new: &[u32]| -> String { format!("{name}: {}", if ids.is_empty() { "none".to_string() } else { ids.iter().zip(new).map(|(a, b)| format!("#{a} -> #{b}")).collect::<Vec<_>>().join(", ") }) };
    if let Some(h) = ctl.held_by {
        let n = follow(h)?;
        keys.push(pairs("the held napper", &[h], &[n]));
        ctl.held_by = Some(n);
    }
    if let Some((c, k)) = ctl.pick_cart {
        let n = follow(c)?;
        keys.push(format!("{} (lane x {})", pairs("the picked cart", &[c], &[n]), LANES[k]));
        ctl.pick_cart = Some((n, k));
    }
    if let Some((c, k)) = ctl.divert {
        let n = follow(c)?;
        keys.push(format!("{} (lane x {})", pairs("the diverted cart", &[c], &[n]), LANES[k]));
        ctl.divert = Some((n, k));
    }
    let old: Vec<u32> = nappers.iter().map(|n| n.0).collect();
    let new: Vec<u32> = old.iter().map(|v| follow(*v)).collect::<Result<_, _>>()?;
    for (n, v) in nappers.iter_mut().zip(&new) {
        n.0 = *v;
    }
    keys.push(pairs("the nappers' visits", &old, &new));
    // (G1 X21) the releases' napper ids and R's detector riders, so D2's own-pass check holds across
    // the open; (G1 10:47 O1) the departures', picks' and diverts' logs, the picks' carts and riders,
    // the new nappers, R's cart and its departures, the golems
    let mut remap_log = |name: &str, log: &mut Vec<(i64, u32)>| -> Result<(), String> {
        let old: Vec<u32> = log.iter().map(|r| r.1).collect();
        let new: Vec<u32> = old.iter().map(|v| follow(*v)).collect::<Result<_, _>>()?;
        for (r, v) in log.iter_mut().zip(&new) {
            r.1 = *v;
        }
        keys.push(pairs(name, &old, &new));
        Ok(())
    };
    remap_log("the releases' nappers", &mut ctl.release_log)?;
    remap_log("the departures", &mut ctl.dep_log)?;
    remap_log("the picks", &mut ctl.pick_log)?;
    remap_log("the diverts", &mut ctl.divert_log)?;
    let mut rd_old: Vec<u32> = Vec::new();
    let mut rd_new: Vec<u32> = Vec::new();
    for r in ctl.rdet_riders.iter_mut() {
        if let Some(v) = r.1 {
            let n = follow(v)?;
            rd_old.push(v);
            rd_new.push(n);
            r.1 = Some(n);
        }
    }
    keys.push(pairs("R's detector riders", &rd_old, &rd_new));
    let mut pk_old: Vec<u32> = Vec::new();
    let mut pk_new: Vec<u32> = Vec::new();
    for p in ctl.pick_carts.iter_mut() {
        for v in [&mut p.1, &mut p.2] {
            let n = follow(*v)?;
            pk_old.push(*v);
            pk_new.push(n);
            *v = n;
        }
    }
    keys.push(pairs("the picks' carts and riders", &pk_old, &pk_new));
    let old: Vec<u32> = ctl.new_nappers.iter().map(|n| n.0).collect();
    let new: Vec<u32> = old.iter().map(|v| follow(*v)).collect::<Result<_, _>>()?;
    for (n, v) in ctl.new_nappers.iter_mut().zip(&new) {
        n.0 = *v;
    }
    keys.push(pairs("the new nappers", &old, &new));
    let mut rc_old: Vec<u32> = Vec::new();
    let mut rc_new: Vec<u32> = Vec::new();
    for slot in ctl.r_cart_id_prev.iter_mut() {
        if let Some((c, r)) = slot {
            let nc = follow(*c)?;
            rc_old.push(*c);
            rc_new.push(nc);
            let nr = match r {
                Some(v) => {
                    let n = follow(*v)?;
                    rc_old.push(*v);
                    rc_new.push(n);
                    Some(n)
                }
                None => None,
            };
            *slot = Some((nc, nr));
        }
    }
    keys.push(pairs("R's carts and riders", &rc_old, &rc_new));
    let mut dp_old: Vec<u32> = Vec::new();
    let mut dp_new: Vec<u32> = Vec::new();
    for d in ctl.r_departures.iter_mut() {
        let nc = follow(d.2)?;
        dp_old.push(d.2);
        dp_new.push(nc);
        d.2 = nc;
        if let Some(v) = d.3 {
            let n = follow(v)?;
            dp_old.push(v);
            dp_new.push(n);
            d.3 = Some(n);
        }
    }
    keys.push(pairs("R's departures", &dp_old, &dp_new));
    let old: Vec<u32> = ctl.golems.iter().copied().collect();
    let new: Vec<u32> = old.iter().map(|v| follow(*v)).collect::<Result<_, _>>()?;
    ctl.golems = new.iter().copied().collect();
    keys.push(pairs("the golems", &old, &new));
    Ok(keys)
}

/// (G1 07:28 PT) The air control's own sample: every drowned rider is the held one or rode a cart
/// stacked below it in its column at the hold's start; a drowning anywhere else is a natural failure
/// beside the control.
pub fn air_control_own(drowned: &[u32], held: u32, stacked: &[u32]) -> bool {
    drowned.contains(&held) && drowned.iter().all(|d| *d == held || stacked.contains(d))
}

/// (G1 07:28 PT, ironspec 07:09 platform) A clearing event: the brain's CANT_REACH_WALK_TARGET_SINCE
/// erased, which only the sleep start, MoveToTargetSink's reached test (the target block within its
/// close-enough Manhattan distance) or the sink's own tryComputePath reaching canReach do; a
/// navigation recompute erases nothing, so while the stamp stays set there's no clearing, whatever the
/// path. `target` is the walk target's block and close-enough distance, `me` the villager's block.
pub fn clearing_source(prev_stamp: Option<i64>, stamp: Option<i64>, napped: bool, target: Option<([i32; 3], i32)>, me: [i32; 3], kind: &str) -> Option<String> {
    if prev_stamp.is_none() || stamp.is_some() {
        return None;
    }
    Some(match target {
        _ if napped => format!("a nap (the sleep start erases it{})", if kind.is_empty() { String::new() } else { format!("; target {kind}") }),
        Some((tb, close)) => {
            let m = (tb[0] - me[0]).abs() + (tb[1] - me[1]).abs() + (tb[2] - me[2]).abs();
            if m <= close {
                format!("its target reached (MoveToTargetSink's reached test: {kind} at Manhattan {m} <= close {close})")
            } else {
                format!("the sink's own tryComputePath reaching canReach ({kind} at Manhattan {m}, close {close})")
            }
        }
        None => "no target on record (a model case to report)".to_string(),
    })
}

/// An activity strip run-length encoded: `P×120 R×3 P×50`.
fn rle(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        let mut n = 1;
        while it.peek() == Some(&c) {
            it.next();
            n += 1;
        }
        out.push(if n == 1 { c.to_string() } else { format!("{c}×{n}") });
    }
    out.join(" ")
}

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
    least_orig_margin: Option<i64>,
    min_rotation: [Option<i64>; 3],
    /// (ironspec 12:48) the blocked callers by their memory's last setter.
    blocked_by: BTreeMap<&'static str, i64>,
    /// Each day's calls (willing callers at a summon tick) and of them the calls with 3 willing
    /// villagers in the box.
    by_day: BTreeMap<i64, (i64, i64)>,
    /// (ironspec 16:08 (3)) by day, the willing villagers counted in each call's box (5: 5 or
    /// more); the first call with 3 (the recovery row).
    counted_hist: BTreeMap<(i64, usize), i64>,
    first_fresh3: Option<i64>,
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
    /// (ironspec 19:07 PT) the bystander stamps on departing riders (out of their pockets), of
    /// them in locked running (the last REL at 5 mod 100), the first rows.
    departing_bystanders: i64,
    departing_locked: i64,
    departing_rows: Vec<String>,
    bystander_margin: Option<i64>,
    bystander_rows: Vec<String>,
}

#[derive(Default)]
struct Ctl {
    golems: std::collections::BTreeSet<u32>,
    busy: bool,
    pending: bool,
    /// (the brief's watchdog, ironspec 16:10) W (0: none), the last cycle's start, the starts it
    /// made and every cycle's REL (the re-phase to the summon ticks).
    watchdog: i64,
    last_start: Option<i64>,
    watchdog_starts: i64,
    /// (ironspec 19:47) B's expiry, the re-arms by golem pulses during B, the starts that found
    /// a cycle still in flight (the liveness restart).
    b_until: i64,
    rearms: i64,
    restarted_busy: i64,
    rels: Vec<i64>,
    /// (ironspec 16:10, divert sampling (i)) HELD read at the picked cart's row arrival, not at the
    /// pick; the picks it turned away there.
    held_at_divert: bool,
    picks_held_at_divert: i64,
    /// (day-1's counter) the daylight detector's power last tick.
    sensor_prev: Option<i64>,
    rel: Option<i64>,
    support_off: [Option<i64>; 3],
    /// (V15, 9fc3155f 15:01: glass under the pockets) each pocket's support cell (x,-1,0) as
    /// built, restored after its release block.
    support_base: [Option<String>; 3],
    /// (M16bw) the release block's cell per lane: the side drive's feed input, or None for the
    /// cell under the pocket rail.
    drive_cell: [Option<[i32; 3]>; 3],
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
    /// (day-1's canonical gates rev 15:37, onechunk 15:37) the (A) stop composed (E's activator at
    /// module (-2,-27,1)): the design controller's stop and F4(a) instead of A6's levers and the
    /// H2-keyed K windows. ARM (-2,-27,0)'s last state and edge; SLEPT (G16, toggled by each
    /// bed-head observer pulse after ARM); SLEPT_ONCE (G17); ARMED70 (G18) at ARM + 70; OPEN (G19)
    /// and the shutter lever's scheduled writes (G20); the one-shot released flag (G21); P_STRING's
    /// last state (G22); IN-STOP_l (G3); the slope taps' off ticks (G4); RETURN_x and its set tick
    /// (G12); the sampled lane's pending K / J writes (G13 / G14: tick, cell, on); the hold's
    /// departure watch (T_d = the napper's first end-of-tick in (9,0,7))
    a_stop: bool,
    /// (day-1's propagation delays rev 15:42) the observer pulses' scheduled effects (tick, SLEPT's
    /// toggle or SLEPT_ONCE's set), OPEN's condition last tick, the release's AND last tick, P_STRING's
    /// last raw press (its ~10-gt release lag), each visit's gap from P_STRING's last release to SO's
    /// set (G1 15:40, >= 2), the taps' scheduled writes
    a_obs_due: Vec<(i64, bool)>,
    a_stale: bool,
    a_stale_log: Vec<String>,
    a_rls_log: Vec<String>,
    /// (G1 17:33) the park rest tick per visit and its margin against PARKED's ARM + 52
    a_park_rest: Option<i64>,
    a_park_log: Vec<String>,
    /// (G1 16:00, night check 4) each tick the bed is occupied: every awake HOME-holder within 16 of
    /// the bed head's centre (tick, villager, distance), bar 0
    exposure_16: Vec<(i64, u32, f64)>,
    /// (G1 16:06 (a)) the least distance of any awake HOME-holder to the occupied bed's head (distance,
    /// tick, villager), bar > 16
    exposure_least: (f64, i64, u32),
    a_pstring_hist: std::collections::VecDeque<bool>,
    a_press_start: Option<i64>,
    a_bed_change_at: Option<i64>,
    tail_worst: [f64; 3],
    rs_reset_due: Option<i64>,
    a_open_cond_prev: bool,
    a_rls_prev: bool,
    a_pstring_last_raw: Option<i64>,
    a_so_gaps: Vec<i64>,
    away_reset_due: [Option<i64>; 3],
    /// (day-1's G8 rev 15:42) RE_ARMED and its setters (evidence events only), D(3)'s presses, and
    /// G11's delta7 (the lane-7 pick's emulated D(7) after lane 3's real D(3) press)
    re_armed: bool,
    re_armed_setters: Vec<String>,
    d3_presses: Vec<i64>,
    /// (ironspec 17:29, day-1 17:22's BATCH_DONE) SEEN(-1) / SEEN(3) / SEEN(7): each lane's D press since REL
    seen: [Option<i64>; 3],
    delta7: i64,
    a_open_at: Option<i64>,
    nap_lane_of: BTreeMap<u32, usize>,
    a_arm_prev: bool,
    a_arm_at: Option<i64>,
    a_slept: bool,
    a_slept_once: bool,
    a_open: bool,
    a_shutter_writes: Vec<(i64, bool)>,
    a_released: bool,
    a_release_off: Option<i64>,
    a_pstring_prev: bool,
    a_lie_at: Option<i64>,
    in_stop: [bool; 3],
    in_stop_rider: [Option<u32>; 3],
    tap_off: Option<i64>,
    return_bits: [Option<i64>; 3],
    f4_writes: Vec<(i64, [i32; 3], bool)>,
    f4_windows: Vec<String>,
    f4_faults: Vec<String>,
    hold_cell_prev: bool,
    string_prev: [bool; 3],
    queue_release_due: Option<i64>,
    a_log: Vec<String>,
    rdet_prev: bool,
    /// (ironspec D2) every rise of R's detector.
    rdet_rises: Vec<i64>,
    /// (G1 X21) each rise with the rider on R's detector then.
    rdet_riders: Vec<(i64, Option<u32>)>,
    /// (b8949c96's way-in, e9f0f4cbf) the queue's gate (day-1's to build, EMULATED): RELEASE =
    /// RS_ready AND the hold (>= 80 gt from the arrival) AND REST AND NOT DAY, a redstone block at
    /// the queue rail's west (8,y+13,-4) until the departure; RS_ready set at power-up and by A6's
    /// R, reset at the departure.
    wayin: bool,
    q_rs_ready: bool,
    /// (G1's [4]) R's detector power over the last 9 ticks: RS_ready's set input is its value 8 gt
    /// ago.
    rdet_hist: std::collections::VecDeque<bool>,
    /// (ironspec 15:23 (3), rider-keyed RS_ready) R's detector held by an empty cart last tick, and
    /// the empty presses counted (none sets RS_ready)
    rdet_empty_prev: bool,
    rdet_empty_presses: u32,
    q_hold_since: Option<i64>,
    /// (ironspec 17:16) the queue's hold term in gt (0: none, the release as soon as the stop is
    /// free); every arrival (the dwell: arrival to release); every PRESENT_l set and every D pass
    /// that consumed one (tick, lane).
    queue_hold: i64,
    /// (ironspec 22:03 / 22:09 PT) the module's k_lock (the hold gate's delayed Q) and k_sw (the
    /// siding's repeater after D), from its `k-lock:` / `k-sw:` lines.
    k_lock: Option<i64>,
    k_sw: Option<i64>,
    /// (G1's note) each siding's expected R (D + k_sw + 8), its last power, and the observed rises
    /// (lane, expected, observed).
    r_expect: [Option<i64>; 3],
    r_prev: [bool; 3],
    r_obs: Vec<(usize, i64, i64)>,
    q_arrivals: Vec<i64>,
    /// (ironspec 23:56 PT (2)) HELD's set (tick, cause), every set and reset with its cause, and the
    /// sets still standing past the bound (tick, cause).
    held_set: Option<(i64, String)>,
    held_log: Vec<String>,
    held_stuck: Vec<(i64, String)>,
    /// (ironspec 00:16 PT) the napper's rest at the seat after the last arrival, and the module's
    /// dwell cap (HELD's bound: rest + the cap + R1's 4 gt).
    q_rest_at: Option<i64>,
    dwell_cap: Option<i64>,
    /// (day-1 01:10 PT, the per-pick trace) REST_l's and DAY's last levels and their edges; every
    /// pick opportunity (tick, C, lane, verdict); the napper holding HELD; each divert (tick, cart).
    rest_prev: Option<bool>,
    day_prev: Option<bool>,
    edge_log: Vec<String>,
    pick_opps: Vec<(i64, i64, usize, String)>,
    held_by: Option<u32>,
    divert_log: Vec<(i64, u32)>,
    /// Each pick made: (tick, cart, villager, lane).
    pick_carts: Vec<(i64, u32, u32, usize)>,
    /// (day-1 01:27 PT, m per visit) each H2 sample that fired a hold pulse (tick, lane), and every
    /// D press per lane (tick, lane).
    h2_log: Vec<(i64, usize, i64)>,
    dpress_log: Vec<(i64, usize)>,
    /// (ironspec 01:59 PT, each visit's decomposition) every lift-exit pass (the waiting RS set).
    lift_exit_log: Vec<i64>,
    /// (ironspec 02:18 PT (2)) DAY at the design's threshold (level >= this, 6) and the stand-in's
    /// validation: the ticks read, the ticks where the threshold's DAY differs from level >= 6, the
    /// detector's updates, the last power and DAY state, the edges.
    day_threshold: u32,
    day_ticks: i64,
    day_mismatch: i64,
    day_updates: i64,
    day_power_prev: Option<u32>,
    day_state_prev: Option<bool>,
    day_edges: Vec<String>,
    /// (G1 N3) the detector cell's sky light (read once) and the first update ticks where the
    /// stored power differs from the analytic detector's.
    day_sky: Option<i32>,
    day_mismatch_rows: Vec<String>,
    /// (G1 N18's control) compare at the half-period off-phase.
    day_phase_control: bool,
    /// (G1 X24) the control's one offset write: done, its tick; and every mismatch's tick.
    day_phase_done: bool,
    day_phase_at: Option<i64>,
    day_mismatch_at: Vec<i64>,
    /// (ironspec's E2) the tick R1 last opened (REST_l, DAY low, RS_ready) while HELD was set.
    r1_open_since: Option<i64>,
    /// (G1's note) a pick decided while another was still pending (an overwrite).
    pick_overwrites: Vec<String>,
    /// (day-1's R1) the line ends held as levels (their built states), the queue rail's power last
    /// tick.
    levels: BTreeMap<[i32; 3], String>,
    q_rail_prev: bool,
    /// (path (a)) the module holds lane 7's braked slope (-4,12,7): it is released at REL + 96
    /// (queued at each start: REL + 96 comes after the next start), its lever off 18 gt later.
    l7_slope: bool,
    /// (G1's [3]) path (a): (5,0,7) a plain rail (exactly), checked against lane 7's braked slope.
    path_a: bool,
    l7_on: Vec<i64>,
    l7_off: Option<i64>,
    /// (ironspec 19:29 (a) / (b)) every queue release, departure and pick: (tick, the napper).
    release_log: Vec<(i64, u32)>,
    /// (ironspec D2's known positive) RS_ready forced set at the first departure.
    rs_early: bool,
    rs_early_done: bool,
    /// (G1 X22) the tick the RS_ready control forced its set.
    rs_early_at: Option<i64>,
    dep_log: Vec<(i64, u32)>,
    pick_log: Vec<(i64, u32)>,
    present_log: Vec<(i64, usize)>,
    dpass_log: Vec<(i64, usize)>,
    q_release_on: bool,
    q_arr_prev: bool,
    q_dep_prev: bool,
    q_released: Vec<i64>,
    /// HELD: the queue occupied (the arrival sets it, the departure resets it).
    q_held: bool,
    /// (day-1's picker) the REL count, the pick waiting for its lane's launch, the picked cart
    /// (its id and lane), the divert in progress (the levers of its J and every J east held off
    /// until it passes (8,25,9)), each lane's away latch, the nappers it made (rider, lane, tick)
    /// for the run to record, the picks its gates held back and the row arrivals DAY turned away.
    rel_count: i64,
    /// (day-1 22:06) the counter's log: `day:C` at each release, and every RISE / EIGHT load with
    /// its day time.
    counter_log: Vec<String>,
    /// The day time at this tick's step (the counter's log reads it at a start).
    day_now: i64,
    /// (the counter's RISE / EIGHT read the detector only after its first own update: the file
    /// stores its power as pasted, and the detector updates at game time % 20 == 0, so the first
    /// 21 ticks only set the baseline)
    sensor_armed: i64,
    /// (G1's [6]) the counter's C derived at the first release; a first release between RISE and
    /// EIGHT (not modelled) fails.
    counter_pending: bool,
    counter_unmodelled: Option<i64>,
    /// (G1's blocker 3, ironspec 07:18 real: the window cites day-1's counter) the day times the
    /// first-release formula holds between: EIGHT (trigger 2 loads C 19 as the detector's level
    /// rises across 8) to RISE (the detector's 0 -> 1 loads C 0), each from redsim's analytic
    /// detector at the module's detector cell on a clear clock.
    counter_window: Option<(i64, i64)>,
    pick: Option<usize>,
    pick_cart: Option<(u32, usize)>,
    divert: Option<(u32, usize)>,
    away: [bool; 3],
    new_nappers: Vec<(u32, usize, i64)>,
    picks_gated: i64,
    /// The gated picks by their gate: HELD, the lane's away latch, a picked cart not yet routed.
    gated_by: [i64; 3],
    /// Picks whose lane's pocket was empty at its launch (no pick made).
    picks_empty: i64,
    picks_day: i64,
    /// The hold (c' as built): the waiting RS set by the lift exit (7,-3,4) (its delayed Q 38 gt
    /// later), reset by the fired pulse; the input (7,-1,8) on at ARM3 + 10 for 12 gt.
    rs_set: Option<i64>,
    hold_on: Option<i64>,
    hold_off: Option<i64>,
    /// The waiting napper's lane (an index into LANES) and its cycle's ARM3.
    nap_lane: usize,
    nap_arm3: Option<i64>,
    /// K(n) set at ARM3 + 93 / 108 / 124 (+ the driver's 4), reset at ARM3 + 125 / 140 / 156 (+ 6),
    /// that reset's edge setting PRESENT_l. (onechunk 13:28: each event carries its own K cell and
    /// lane, keyed at the napper's own H2 sample: a later pick moving nap_lane before the reset turned
    /// the WRONG K off and left the napper's K on (t12's K-bounce nights); queued, so two napppers'
    /// windows never overwrite each other)
    k_events: Vec<(i64, i32, bool)>,
    present_events: Vec<(i64, usize)>,
    present: [bool; 3],
    /// (day-1 08:31) a ridden cart on each lane's siding R last tick, for PRESENT_l's reset by the
    /// departure past R; and every reset.
    r_cart_prev: [bool; 3],
    present_resets: Vec<(i64, usize)>,
    /// (G1 10:34) the ridden cart on each lane's R (cart, rider) last tick, and every ridden departure
    /// from R: (tick, lane, cart, rider, PRESENT_l set at the departure)
    r_cart_id_prev: [Option<(u32, Option<u32>)>; 3],
    r_departures: Vec<(i64, usize, u32, Option<u32>, bool)>,
    /// (G1 10:47 blocking 4) the nappers' ids, set by the run each tick: PRESENT_l resets only on a
    /// departure from R carrying one
    napper_ids: std::collections::BTreeSet<u32>,
    /// (day-1 11:40) the picker's per-J release: which of J(-1) / J(3) / J(7) the outbound napper has
    /// cleared this pass, and whether to hold them all to (8,25,9) as before (--picker-hold-to-leg)
    divert_cleared: [bool; 3],
    picker_hold_to_leg: bool,
    /// (day-1's Delta, ironspec 12:41 (4)) the hold pulse's shift in gt (and the taps keyed off it)
    hold_delta: i64,
    /// (day-1 12:49 / 12:53, ironspec 12:52) the outbound J release evidence-keyed (the tripwires at
    /// (0,26,9) / (4,26,9) / (8,26,9) ANDed with each J's hold-off), its delay, the pending releases,
    /// every press (tick, J, presser cart, the napper's), and the presses by a cart not the napper's
    evidence_keyed: bool,
    evidence_delay: i64,
    ev_release_at: [Option<i64>; 3],
    ev_presses: Vec<(i64, i32, u32, bool)>,
    ev_faults: Vec<String>,
    ev_releases: Vec<(i64, i32, u32)>,
    /// (ironspec 12:54 (2)) REL gated by J(-1)'s armed latch: on, the request held since, and each
    /// held start's (request, delay)
    rel_gate_armed: bool,
    gated_since: Option<i64>,
    rel_delays: Vec<(i64, i64)>,
    /// (dfb89499 / farmopt 13:30) each lane's pocket offset after REL (default OFFS, 0 / 14 / 24)
    offs: [i64; 3],
    /// (day-1 13:43, --ring-timed) the no-string ring: on; its clock-locked L's (L_J3(1), L_J7(1),
    /// L_J7(2), L_J7(3), L_J(-1)(3)) and k; each batch's (REL, pick); the J lever writes pending; the
    /// holds; J(3)'s latch; the holds' resets pending and done; the timed D(7)s; the re-arm events;
    /// its log
    ring_timed: bool,
    ring_l: [i64; 5],
    ring_k: i64,
    rt_batches: Vec<(i64, Option<usize>)>,
    rt_writes: Vec<(i64, i32, bool)>,
    rt_hold3: bool,
    rt_hold7: bool,
    rt_j3_latch: bool,
    rt_resets: Vec<(i64, i32, i64, usize)>,
    rt_reset_log: Vec<(i64, i32, i64, usize)>,
    rt_timed_d7: Vec<i64>,
    rt_rearm_events: Vec<i64>,
    rt_log: Vec<String>,
    /// (G1 10:47 blocking 4's control) the eject-on-R control: armed, and its sample (tick, lane,
    /// the napper, PRESENT_l right after)
    eject_control: bool,
    eject_done: Option<(i64, usize, u32)>,
    /// (G1 / ironspec 08:41) the unrouted bar's positive: armed, and done.
    strand_control: bool,
    strand_done: bool,
    /// (G1 10:24 (c)) the per-lane count bar's positive: the split's J(-1) kept on through this many
    /// of lane -1's branch-detector edges (after the first 10), sending the next arrival into lane -1
    /// too; the edges seen and the misroutes done
    misroute_left: u32,
    b0_edges: u32,
    misroute_at: Vec<i64>,
    /// J(-1) kept from re-arming from ARM3 + 72 to ARM3 + 127 for an eastbound napper (lanes 3, 7),
    /// re-armed at its end.
    j_blocks: Vec<(i64, i64)>,
    /// (G1 14:11, CHANGES 2 on ed4978c0c) each napper's H2 sample taken while an earlier napper's
    /// window was still pending (K events, PRESENT, the hold's pulse, J(-1)'s block): a failure
    overlaps: Vec<String>,
    /// (G1 14:11's control) --nap-lane-control: on; the forced nap_lane moves pending; each napper's
    /// window (its H2 sample, lane, K cell); every K write fired (tick, cell, on)
    nap_lane_control: bool,
    stuck_k_control: bool,
    nap_lane_moves: Vec<(i64, usize)>,
    k_windows: Vec<(i64, usize, i32)>,
    k_fired: Vec<(i64, i32, bool)>,
    /// The siding's release: D's rise at (x,24,8) while PRESENT_l: the input (x-2,23,9) on 4 gt
    /// later for 30 gt.
    siding_on: [Option<i64>; 3],
    siding_off: [Option<i64>; 3],
    /// The siding release's input cell and its block as built (restored at the release's end).
    siding_cell: [Option<([i32; 3], String)>; 3],
    d7_prev: bool,
    branch_prev_nap: [bool; 2],
    /// A ridden cart on the hold last tick (the departure's edge).
    hold_prev: bool,
    /// The napper's arrival on the hold and its first tick at rest there; pulses fired with the
    /// hold empty (bar 0).
    hold_enter: Option<i64>,
    hold_rest: Option<i64>,
    empty_pulses: i64,
    rolling_pulses: i64,
    /// The events, for the napper's lines.
    nap_events: Vec<String>,
}

thread_local! {
    /// (ironspec 21:40 PT) every stand-in placed by `input_on`: its cell, its state, how often.
    static PLACED: std::cell::RefCell<BTreeMap<[i32; 3], (String, i64)>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    /// (mob-AI 14:14) The drive being recorded (`--drive-out`): its t0 and its acts, every block
    /// write an `act t set x y z <state>` in the run's own order within each tick.
    static DRIVE_LOG: std::cell::RefCell<Option<(i64, Vec<String>)>> = const { std::cell::RefCell::new(None) };
    /// (G1's guard) the built cells (the module's states by cell) and the cells the module
    /// declares `emulated:`, while a run drives.
    #[allow(clippy::type_complexity)]
    static BUILT: std::cell::RefCell<Option<(BTreeMap<[i32; 3], String>, std::collections::BTreeSet<[i32; 3]>)>> = const { std::cell::RefCell::new(None) };
    /// (G1 17:50) the cells a drive wrote with a stand-in that only suppresses (what it lost
    /// against the built cell), named in the report.
    static SUPPRESSED: std::cell::RefCell<BTreeMap<[i32; 3], String>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    /// (7b90c3f1 18:06) the module's declared placement-order rail Ts: no stand-in lever's updates
    /// may reach one.
    static TS: std::cell::RefCell<Vec<[i32; 3]>> = const { std::cell::RefCell::new(Vec::new()) };
    /// (day-1 05:45 PT) the controller line ends the module declares as redstone-block stand-ins
    /// (`# stand-in: x,y,z (… redstone block …)`): a block while the line is on, the built air off,
    /// no support cell.
    static BLOCK_STANDINS: std::cell::RefCell<Vec<[i32; 3]>> = const { std::cell::RefCell::new(Vec::new()) };
    /// (G1 / tick-exact, Nov 13) the drive's names for the scene's entities (`eN` a line's cart,
    /// `eN.pK` its passenger), for the recorded state written beside every act.
    static DRIVE_NAMES: std::cell::RefCell<BTreeMap<u32, String>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    /// (ironspec 06:24 real, the 04:07 override rule) every redstone-block stand-in toggle: its
    /// cell, tick, the cell's state before and after.
    static BLOCK_TOGGLES: std::cell::RefCell<Vec<([i32; 3], i64, String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// (tick-exact 06:21 PT) the drive's writes of a cell's existing state, skipped (no write, no act).
    static NOOP_WRITES: std::cell::RefCell<i64> = const { std::cell::RefCell::new(0) };
    /// (G1 X16) the no-op emulated writes' rows (tick, cell, state), the first 40.
    static NOOP_ROWS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// (G1 / tick-exact, Nov 13: the replay's divergence check) redsim's recorded state at a drive act,
/// written as a comment line after it: the carts in the act's column (its cell and the cells above
/// and below) with their riders, and every hold's (x,16,0) and pocket's (x,0,0) cart and rider, by
/// the drive's names.
fn drive_expect(w: &redsim::world::World, p: [i32; 3], t: i64) -> String {
    let name = |id: u32| DRIVE_NAMES.with(|n| n.borrow().get(&id).cloned()).unwrap_or_else(|| format!("#{id}"));
    let carts_at = |c: [i32; 3]| -> Vec<String> {
        w.entities
            .list
            .iter()
            .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == c)
            .map(|e| format!("{}{}", name(e.id), e.passengers.first().map_or(" (empty)".to_string(), |r| format!(" with {}", name(*r)))))
            .collect()
    };
    let at: Vec<String> = (-1..=1).flat_map(|dy| carts_at([p[0], p[1] + dy, p[2]]).into_iter().map(move |s| format!("{s} at y{dy:+}"))).collect();
    let occ = |y: i32| LANES.iter().map(|x| {
        let v = carts_at([*x, y, 0]);
        format!("{x}:{}", if v.is_empty() { "none".to_string() } else { v.join("+") })
    }).collect::<Vec<_>>().join(" ");
    format!("# expect {t} {} {} {}: carts {}; holds {}; pockets {}", p[0], p[1], p[2], if at.is_empty() { "none".to_string() } else { at.join(", ") }, occ(16), occ(0))
}

fn drive_push(line: String) {
    DRIVE_LOG.with(|d| {
        if let Some((_, acts)) = d.borrow_mut().as_mut() {
            acts.push(line);
        }
    });
}

fn set(w: &mut redsim::world::World, p: [i32; 3], state: &str, flags: u32) -> Result<(), String> {
    let b = redsim::block::Block::parse(state).map_err(|e| format!("{state}: {e}"))?;
    // (tick-exact / ironspec, the override rule on the drive: a write of the cell's existing state
    // changes nothing in the game (setBlock returns early with no update), so it's no write and no
    // drive act)
    if w.get(dbp(p[0], p[1], p[2])) == b {
        NOOP_WRITES.with(|n| *n.borrow_mut() += 1);
        // (G1 X16) every no-op emulated write listed (its tick, cell and state)
        NOOP_ROWS.with(|r| {
            let mut r = r.borrow_mut();
            if r.len() < 40 {
                r.push(format!("t {} ({},{},{}) {state}", w.time, p[0], p[1], p[2]));
            }
        });
        return Ok(());
    }
    // (G1's guard, ironspec 17:46 / 17:49; G1 17:50: refuse a difference that ADDS a golem or path
    // option, list one that only suppresses) an emulated write keeps the built cell's golem and
    // path properties, or is a state of the built block itself, or the cell is declared
    // `emulated:` in the module (the statics run with it)
    BUILT.with(|g| -> Result<(), String> {
        let g = g.borrow();
        let Some((built, emulated)) = g.as_ref() else { return Ok(()) };
        let have_s = built.get(&p).cloned().unwrap_or_else(|| "minecraft:air".to_string());
        let id = |s: &str| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        if id(state) == id(&have_s) || emulated.contains(&p) {
            return Ok(());
        }
        let have = redsim::block::Block::parse(&have_s).map_err(|e| format!("{have_s}: {e}"))?;
        // (ironspec 22:37 / 23:13 PT, G1) the stand-in guard is record-match's shared
        // `guard_intrinsics`: golem in both roles (the cell as a floor, and as a golem's spawn space
        // over the built cell below), the full PathType classification, late-path effects and sight,
        // each answered exactly as the built cell answers it; update reach, power reach and survival
        // are this bench's (the T reach and power reach checks of `lever_standin`, the survival watch)
        let cell_at = |q: [i32; 3]| -> Result<redsim::block::Block, String> {
            let s = built.get(&q).cloned().unwrap_or_else(|| "minecraft:air".to_string());
            redsim::block::Block::parse(&s).map_err(|e| format!("{s}: {e}"))
        };
        let (above, below) = (cell_at([p[0], p[1] + 1, p[2]])?, cell_at([p[0], p[1] - 1, p[2]])?);
        let v = crate::recordmatch::guard_intrinsics(&b, &have, &above, &below);
        let fails: Vec<String> = crate::recordmatch::PROPERTIES.iter().zip(v.iter()).filter_map(|(n, x)| match x {
            Some(Err(why)) => Some(format!("{n}: {why}")),
            _ => None,
        }).collect();
        if !fails.is_empty() {
            return Err(format!(
                "the emulated write at ({},{},{}) of {state} answers unlike the built {have_s} (record-match's guard_intrinsics): {}; drive the built input, or declare the cell `emulated:` in the module and run the statics with it",
                p[0],
                p[1],
                p[2],
                fails.join("; ")
            ));
        }
        Ok(())
    })?;
    DRIVE_LOG.with(|d| -> Result<(), String> {
        if let Some((t0, acts)) = d.borrow_mut().as_mut() {
            // (the drive's `set` writes with every update: a write without them has no act)
            if flags != redsim::world::flags::ALL {
                return Err(format!("--drive-out: the write at ({},{},{}) skips the updates, and the drive's `set` has every update", p[0], p[1], p[2]));
            }
            acts.push(format!("act {} set {} {} {} {state}", w.time - *t0, p[0], p[1], p[2]));
            // (G1: redsim's recorded state at the act, before its write)
            acts.push(drive_expect(w, p, w.time - *t0));
        }
        Ok(())
    })?;
    w.set_block(dbp(p[0], p[1], p[2]), b, flags);
    // (G1 16:18 (1), 26.3 LeverBlock.updateNeighbours 133-139: `level.updateNeighborsAt(pos)` and
    // `level.updateNeighborsAt(pos.relative(front))`, front = getConnectedDirection(state).getOpposite())
    // a commanded lever performs a pulled lever's full update: its ATTACHED block's neighbours too
    // (every emulated lever write passes here: the (A) stop's stand-ins, the R line's, the hold input's,
    // the junction levers)
    if state.contains("minecraft:lever[") {
        let att = if state.contains("face=floor") {
            [p[0], p[1] - 1, p[2]]
        } else if state.contains("face=ceiling") {
            [p[0], p[1] + 1, p[2]]
        } else if state.contains("facing=east") {
            [p[0] - 1, p[1], p[2]]
        } else if state.contains("facing=west") {
            [p[0] + 1, p[1], p[2]]
        } else if state.contains("facing=south") {
            [p[0], p[1], p[2] - 1]
        } else {
            [p[0], p[1], p[2] + 1]
        };
        w.update_neighbors_at(dbp(att[0], att[1], att[2]), redsim::block::Kind::Lever);
        LEVER_FULL_UPDATES.with(|n| *n.borrow_mut() += 1);
    }
    Ok(())
}

thread_local! {
    /// (G1 16:18 (1)) every commanded lever write's attached-block update, counted for the gate print
    static LEVER_FULL_UPDATES: std::cell::RefCell<u64> = const { std::cell::RefCell::new(0) };
}

/// (G1 16:18 (2)) A fold's `# integrity:` line, moved: `x,y,z=connections a,b,c+d,e,f` (the rail at the
/// cell must join those two neighbours: its flat shape from their directions) and `holds x,y,z=SHAPE`,
/// entries separated by "; ". Each entry as (cell, the shape it must hold, the entry's text).
fn parse_integrity(text: &str, mv: [i32; 3]) -> Result<Vec<([i32; 3], String, String)>, String> {
    let p3 = |s: &str| -> Result<[i32; 3], String> {
        let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>().map_err(|e| format!("integrity cell {s}: {e}"))).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(format!("integrity cell {s}: not x,y,z"));
        }
        Ok([v[0] + mv[0], v[1] + mv[1], v[2] + mv[2]])
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("# integrity:").or_else(|| line.strip_prefix("integrity:")) else { continue };
        for entry in rest.split("; ") {
            let e = entry.trim();
            if let Some(h) = e.strip_prefix("holds ") {
                let (c, shape) = h.split_once('=').ok_or_else(|| format!("integrity entry {e}: no '='"))?;
                out.push((p3(c)?, shape.trim().to_string(), e.to_string()));
            } else if let Some((c, conn)) = e.split_once("=connections ") {
                let cell = p3(c)?;
                let (a, b) = conn.split_once('+').ok_or_else(|| format!("integrity entry {e}: connections A+B"))?;
                let (a, b) = (p3(a)?, p3(b)?);
                let dir = |n: [i32; 3]| -> Result<&'static str, String> {
                    match (n[0] - cell[0], n[2] - cell[2]) {
                        (0, -1) => Ok("north"),
                        (0, 1) => Ok("south"),
                        (1, 0) => Ok("east"),
                        (-1, 0) => Ok("west"),
                        _ => Err(format!("integrity entry {e}: ({},{},{}) not beside the cell", n[0], n[1], n[2])),
                    }
                };
                let (da, db) = (dir(a)?, dir(b)?);
                let shape = match (da, db) {
                    ("north", "south") | ("south", "north") => "north_south",
                    ("east", "west") | ("west", "east") => "east_west",
                    ("south", "east") | ("east", "south") => "south_east",
                    ("south", "west") | ("west", "south") => "south_west",
                    ("north", "west") | ("west", "north") => "north_west",
                    ("north", "east") | ("east", "north") => "north_east",
                    _ => return Err(format!("integrity entry {e}: {da} + {db} is no rail shape")),
                };
                out.push((cell, shape.to_string(), e.to_string()));
            } else if !e.is_empty() {
                return Err(format!("integrity entry {e}: neither 'holds' nor 'connections'"));
            }
        }
    }
    Ok(out)
}

/// (G1's guard, ironspec 17:49) A controller line end's stand-in matching the built air in every
/// golem and path property: a powered lever on the first sturdy face around the cell (below, above,
/// then the north, south, west and east sides), or None; a face is skipped when the lever's
/// updates (its own neighbours and its support's: vanilla `LeverBlock.updateNeighbours`) would
/// reach a rail T the module declares `placement-order-ts:` (7b90c3f1 18:06: the T-settle rule).
fn lever_standin(w: &redsim::world::World, c: [i32; 3], target: [i32; 3]) -> Option<String> {
    use redsim::behavior::{sturdy_of, Support};
    use redsim::pos::Dir;
    let at = |d: [i32; 3]| w.get(dbp(c[0] + d[0], c[1] + d[1], c[2] + d[2]));
    let ts: Vec<[i32; 3]> = TS.with(|t| t.borrow().clone());
    let n6 = |p: [i32; 3]| [[p[0] + 1, p[1], p[2]], [p[0] - 1, p[1], p[2]], [p[0], p[1] + 1, p[2]], [p[0], p[1] - 1, p[2]], [p[0], p[1], p[2] + 1], [p[0], p[1], p[2] - 1]];
    // (ironspec 18:34 (5), power reach: the lever powers its six neighbours and, strongly, its
    // support, which carries it on only if it conducts; so a usable face has a support that
    // doesn't conduct and no powerable component beside the lever but its target (a repeater or
    // comparator counts only from behind, its input))
    let powerable = |q: [i32; 3]| -> bool {
        let s = w.get(dbp(q[0], q[1], q[2])).to_string();
        let n = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        if n == "repeater" || n == "comparator" {
            // (its input is the cell its facing names)
            let f = s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
            let back = match f {
                "north" => [q[0], q[1], q[2] - 1],
                "south" => [q[0], q[1], q[2] + 1],
                "west" => [q[0] - 1, q[1], q[2]],
                "east" => [q[0] + 1, q[1], q[2]],
                _ => return true,
            };
            return back == c;
        }
        n.contains("rail") || n == "redstone_wire" || n.contains("trapdoor") || n.ends_with("_door") || n.contains("piston") || matches!(n.as_str(), "dispenser" | "dropper" | "note_block" | "redstone_lamp" | "tnt" | "hopper" | "bell" | "crafter") || n.ends_with("fence_gate") || n.ends_with("copper_bulb")
            // (G1's note (b) on 961090aef: an observer facing the stand-in's cell sees it change)
            || (n == "observer" && {
                let f = s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
                let seen = match f {
                    "north" => [q[0], q[1], q[2] - 1],
                    "south" => [q[0], q[1], q[2] + 1],
                    "west" => [q[0] - 1, q[1], q[2]],
                    "east" => [q[0] + 1, q[1], q[2]],
                    "up" => [q[0], q[1] + 1, q[2]],
                    "down" => [q[0], q[1] - 1, q[2]],
                    _ => q,
                };
                seen == c
            })
    };
    // (G1's note (a) on 961090aef: the written-over block's own updates count too: dust swapped
    // for the lever and back updates out to taxicab 2)
    let written_over_dust = w.get(dbp(c[0], c[1], c[2])).to_string().contains("redstone_wire");
    let dust_reach: Vec<[i32; 3]> = if written_over_dust {
        let mut v = Vec::new();
        for dx in -2i32..=2 {
            for dy in -2i32..=2 {
                for dz in -2i32..=2 {
                    if dx.abs() + dy.abs() + dz.abs() <= 2 {
                        v.push([c[0] + dx, c[1] + dy, c[2] + dz]);
                    }
                }
            }
        }
        v
    } else {
        Vec::new()
    };
    let clean = |support: [i32; 3]| {
        let sb = w.get(dbp(support[0], support[1], support[2]));
        !n6(c).iter().chain(n6(support).iter()).chain(dust_reach.iter()).any(|q| ts.contains(q))
            && !redsim::behavior::is_conductor(w, &sb, dbp(support[0], support[1], support[2]))
            && n6(c).iter().all(|q| *q == target || *q == support || !powerable(*q))
    };
    let support = |d: [i32; 3]| [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
    if sturdy_of(&at([0, -1, 0]), Dir::Up, Support::Full) && clean(support([0, -1, 0])) {
        return Some("minecraft:lever[face=floor,facing=north,powered=true]".to_string());
    }
    if sturdy_of(&at([0, 1, 0]), Dir::Down, Support::Full) && clean(support([0, 1, 0])) {
        return Some("minecraft:lever[face=ceiling,facing=north,powered=true]".to_string());
    }
    // (a wall lever facing F hangs on the block behind it, at c - F)
    for (d, face, facing) in [([0, 0, -1], Dir::South, "south"), ([0, 0, 1], Dir::North, "north"), ([-1, 0, 0], Dir::East, "east"), ([1, 0, 0], Dir::West, "west")] {
        if sturdy_of(&at(d), face, Support::Full) && clean(support(d)) {
            return Some(format!("minecraft:lever[face=wall,facing={facing},powered=true]"));
        }
    }
    None
}

/// A controller input driven on: the lever stand-in where one fits, else a redstone block (the
/// guard then wants the cell declared `emulated:`); a lever that doesn't survive its face fails
/// the run (G1 17:50: a popped lever leaves its line unpowered).
fn input_on(w: &mut redsim::world::World, c: [i32; 3], target: [i32; 3]) -> Result<(), String> {
    // (ironspec 18:24 / 22:09 PT: a conforming lever only, never a solid stand-in: a cell with no
    // face the guard passes stops the run; day-1 05:45 PT: a line end the module declares a
    // redstone-block stand-in takes the block, with no support cell)
    let block = BLOCK_STANDINS.with(|b| b.borrow().contains(&c));
    let s = if block {
        "minecraft:redstone_block".to_string()
    } else {
        lever_standin(w, c, target).ok_or_else(|| format!("no conforming lever face for the stand-in at ({},{},{}) (target ({},{},{}))", c[0], c[1], c[2], target[0], target[1], target[2]))?
    };
    let before = w.get(dbp(c[0], c[1], c[2])).to_string();
    set(w, c, &s, redsim::world::flags::ALL)?;
    let now = w.get(dbp(c[0], c[1], c[2])).to_string();
    if !(now.contains("lever") || now.contains("redstone_block")) {
        return Err(format!("the stand-in {s} at ({},{},{}) didn't survive its placement: the cell holds {now}", c[0], c[1], c[2]));
    }
    if block {
        // (ironspec 06:24 real: the override rule on every toggle, its before and after)
        BLOCK_TOGGLES.with(|t| t.borrow_mut().push((c, w.time, before, now.clone())));
    }
    // (ironspec 21:40 PT: each stand-in prints the guard's properties) its state and placements
    PLACED.with(|p| {
        let mut p = p.borrow_mut();
        let e = p.entry(c).or_insert((s.trim_start_matches("minecraft:").to_string(), 0));
        e.1 += 1;
    });
    Ok(())
}

/// A controller input driven off: the stand-in, still there (a lever on its face), back to the
/// built state.
fn input_off(w: &mut redsim::world::World, c: [i32; 3], base: &str) -> Result<(), String> {
    let now = w.get(dbp(c[0], c[1], c[2])).to_string();
    if !(now.contains("lever") || now.contains("redstone_block")) {
        return Err(format!("the stand-in at ({},{},{}) didn't survive its pulse: the cell holds {now}", c[0], c[1], c[2]));
    }
    set(w, c, base, redsim::world::flags::ALL)?;
    if BLOCK_STANDINS.with(|b| b.borrow().contains(&c)) {
        let after = w.get(dbp(c[0], c[1], c[2])).to_string();
        BLOCK_TOGGLES.with(|t| t.borrow_mut().push((c, w.time, now, after)));
    }
    Ok(())
}

fn powered(w: &redsim::world::World, p: [i32; 3]) -> bool {
    w.get(dbp(p[0], p[1], p[2])).to_string().contains("powered=true")
}

impl Ctl {
    /// (ironspec 02:37 PT) every divert with its napper: each divert's cart joined to the pick that
    /// sent it (the latest pick of that cart at or before the divert).
    fn diverts(&self) -> Vec<(i64, u32)> {
        self.divert_log
            .iter()
            .filter_map(|(dt, c)| self.pick_carts.iter().filter(|(pt, pc, _, _)| pc == c && pt <= dt).max_by_key(|x| x.0).map(|x| (*dt, x.2)))
            .collect()
    }

    fn note(&mut self, t: i64, s: String) {
        // (G1's P10) every note kept: the report judges the whole log
        self.log.push(format!("t {t}: {s}"));
    }

    /// One tick's drives, after the world's tick at `now` (`rel` the run's tick).
    fn step(&mut self, w: &mut redsim::world::World, rel: i64) -> Result<(), String> {
        use redsim::world::flags;
        let now = rel;
        self.day_now = w.day_time().rem_euclid(24_000);
        // (ironspec 12:54 (2), day-1's spec: F1's REL AND J(-1) ARMED, "armed" read from J(-1)'s latch,
        // the lever (-1,26,9)) with --rel-gate-armed, a start requested while J(-1) isn't armed waits
        // until it is; the delay per batch printed
        // (day-1's G8 rev 15:42, the (A) stop's design controller) F1: REL = holds full (all three
        // slot-1 strings pressed: a cart on each lane's R_1 (x,17,1)) AND RE_ARMED; otherwise J(-1)'s
        // latch with --rel-gate-armed
        let holds_full = LANES.iter().all(|&x| w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [x, 17, 1]));
        let j_armed = if self.a_stop { holds_full && self.re_armed } else { w.get(dbp(-1, 26, 9)).to_string().contains("powered=true") };
        let rel_gated = self.rel_gate_armed || self.a_stop;
        if rel_gated
            && let Some(since) = self.gated_since
            && j_armed
        {
            self.gated_since = None;
            self.rel_delays.push((since, now - since));
            self.start(now + 2 + 4);
            self.note(now, format!("the REL gate: {}: REL at t {} ({} gt after the request)", if self.a_stop { "holds full AND RE_ARMED" } else { "J(-1) armed" }, now + 6, now - since));
        }
        // the golem pulse: a golem appeared
        let new: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::IronGolem))).map(|e| e.id).filter(|id| !self.golems.contains(id)).collect();
        for id in new {
            self.golems.insert(id);
            // (ironspec 19:47, day-1's accepted chain: B is held 100 gt from each start; a golem
            // pulse during B re-arms B to the pulse + 100 and latches P, which starts the cycle at
            // B's expiry: the next pulse's tick, so the chain locks to the summon period; a pulse
            // with B expired starts at once)
            if now < self.b_until {
                self.b_until = now + 100;
                self.pending = true;
                self.rearms += 1;
                self.note(now, format!("golem {id}: the pulse during B: B re-armed to t {}, P latched", now + 100));
            } else if rel_gated && !j_armed {
                if self.gated_since.is_none() {
                    self.gated_since = Some(now);
                }
                self.note(now, format!("golem {id}: the pulse; REL held by the gate (J(-1) not armed)"));
            } else {
                self.start(now + 2 + 4);
                self.note(now, format!("golem {id}: the pulse; REL at t {}", now + 6));
            }
        }
        // (P at B's expiry starts at once; the W flywheel (W = 100, retriggerable from the last
        // start) starts a cycle when none started within W and B has expired, ties toward the start;
        // either restarts the chain from any state of the cycle in flight: liveness, ironspec 19:29
        // (c))
        if self.pending && now >= self.b_until {
            self.pending = false;
            if self.busy {
                self.restarted_busy += 1;
            }
            if self.rel_gate_armed && !j_armed {
                if self.gated_since.is_none() {
                    self.gated_since = Some(now);
                }
                self.note(now, "B's expiry with P: REL held by the gate (J(-1) not armed)".to_string());
            } else {
                self.start(now + 2 + 4);
                self.note(now, format!("B's expiry with P: REL at t {}", now + 6));
            }
        } else if self.watchdog > 0 && now >= self.b_until && now - self.last_start.unwrap_or(0) >= self.watchdog && self.gated_since.is_none() {
            self.watchdog_starts += 1;
            if self.busy {
                self.restarted_busy += 1;
            }
            if self.rel_gate_armed && !j_armed {
                self.gated_since = Some(now);
                self.note(now, "the flywheel: REL held by the gate (J(-1) not armed)".to_string());
            } else {
                self.start(now + 2 + 4);
                self.note(now, format!("the flywheel: no cycle within {} gt; REL at t {}", self.watchdog, now + 6));
            }
        }
        // (day-1 16:14: the counter's daily re-phase from the daylight detector: C := 0 at RISE
        // (0 -> 1), C := 19 at EIGHT (7 -> 8) when C >= 136 or C <= 15)
        {
            let p: i64 = detector_level(w)? as i64;
            if let Some(prev) = self.sensor_prev
                && now >= self.sensor_armed
            {
                if prev == 0 && p >= 1 {
                    self.counter_log.push(format!("RISE at day {} (C {} := 0)", w.day_time().rem_euclid(24_000), self.rel_count));
                    self.rel_count = 0;
                    self.note(now, "the counter: RISE, C := 0".to_string());
                }
                if prev <= 7 && p >= 8 && (self.rel_count >= 136 || self.rel_count <= 15) {
                    self.counter_log.push(format!("EIGHT at day {} (C {} := 19)", w.day_time().rem_euclid(24_000), self.rel_count));
                    self.rel_count = 19;
                    self.note(now, "the counter: EIGHT, C := 19".to_string());
                }
            }
            self.sensor_prev = Some(p);
        }
        // (path (a), ironspec 19:44: lane 7's braked slope (-4,12,7) released at L7 = REL + 96 for
        // 18 gt (7b90c3f1's lane-7 pulse), by a lever beside it at (-4,12,6) hung from the lift
        // wall's glass (-4,13,6) (lever_standin's ceiling face: no added block, sight unchanged);
        // the controller's line isn't laid; scheduled at the start, since the cycle's own state
        // ends at REL + 82)
        if self.l7_slope {
            if let Some(k) = self.l7_on.iter().position(|t| *t == now) {
                self.l7_on.remove(k);
                self.level(w, [-4, 12, 6], [-4, 12, 7], true)?;
                self.l7_off = Some(now + 18);
                self.note(now, "lane 7's slope released (REL + 96, 18 gt)".to_string());
            }
            if self.l7_off == Some(now) {
                self.level(w, [-4, 12, 6], [-4, 12, 7], false)?;
                self.l7_off = None;
            }
        }
        if let Some(r) = self.rel {
            for k in 0..3 {
                let x = LANES[k];
                if now == r + self.offs[k] {
                    // (day-1's picker: the picked lane's batch cart, the one this launch sends)
                    if self.pick == Some(k) {
                        if let Some(cid) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [x, 0, 0]).map(|e| e.id) {
                            self.pick_cart = Some((cid, k));
                            // (ironspec 19:29 (b): who was picked)
                            if let Some(v) = w.entities.list.iter().find(|e| e.id == cid).and_then(|e| e.passengers.first().copied()) {
                                self.pick_log.push((now, v));
                                self.pick_carts.push((now, cid, v, k));
                            }
                            self.away[k] = true;
                            // (ironspec 00:12 PT, F1 adopted: HELD is set at the queue's arrival
                            // detector and reset at the departure; the pick only logs its tick for
                            // the one-in-flight timing bar)
                            self.note(now, format!("the picker: lane x {x}'s batch cart {cid} picked"));
                        } else {
                            self.picks_empty += 1;
                            self.note(now, format!("the picker: lane x {x}'s pocket empty at its launch, no pick"));
                        }
                        self.pick = None;
                    }
                    // (M16bw's side drive where it is laid: a floor lever in the riser's cell
                    // (x-2,0,0) on its glass, into the repeater (x-1,0,0) and the pocket rail, the
                    // dust restored after (ironspec 18:10: no solid stand-in); else the old block
                    // under the rail)
                    match self.drive_cell[k] {
                        Some(c) => input_on(w, c, [x - 1, 0, 0])?,
                        None => set(w, [x, -1, 0], "minecraft:redstone_block", flags::ALL)?,
                    }
                    self.support_off[k] = Some(now + 16);
                    self.armed[k] = true;
                    // (the cold start: no cart on the pocket's string, the trapdoor opens anyway)
                    if !powered(w, [x, 1, 0]) {
                        self.trap_open[k] = Some(now + 4);
                        self.armed[k] = false;
                    }
                }
                if self.support_off[k] == Some(now) {
                    let base = self.support_base[k].clone().unwrap_or_else(|| "minecraft:stone".to_string());
                    match self.drive_cell[k] {
                        Some(c) => input_off(w, c, &base)?,
                        None => set(w, [x, -1, 0], &base, flags::ALL)?,
                    }
                    self.support_off[k] = None;
                }
                let d = powered(w, [x, 0, 2]);
                if self.armed[k] && d && !self.d_prev[k] && now >= r + self.offs[k] {
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
                if w.get(dbp(x + 1, 17, 1)).to_string().contains("repeater") {
                    input_on(w, [x + 2, 17, 1], [x + 1, 17, 1])?;
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
                input_off(w, [x + 2, 17, 1], "minecraft:air")?;
            }
            self.r_input_off = None;
        }
        if self.r_off == Some(now) {
            for x in LANES {
                if !w.get(dbp(x + 1, 17, 1)).to_string().contains("repeater") {
                    set(w, [x + 1, 17, 1], "minecraft:glass", flags::ALL)?;
                }
            }
            self.r_off = None;
            self.busy = false;
            self.rel = None;
            self.shut_done = [false; 3];
            self.cycles += 1;
            self.note(now, format!("cycle {} done (the R line off)", self.cycles));
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
        // (day-1's picker: while a picked cart passes the row, its own J and every J east of it
        // held off; the split's own writes don't turn them on)
        let held_off = |x: i32, dv: Option<(u32, usize)>| dv.is_some_and(|(_, k)| x >= LANES[k]);
        let dv = self.divert;
        // (day-1 13:43, ironspec 13:37: the no-string ring, --ring-timed) its own latch writes
        if self.ring_timed {
            self.ring_timed_step(w, now, b0, b1, row)?;
        }
        if !self.ring_timed && b0 && !self.branch_prev[0] {
            self.b0_edges += 1;
            if self.misroute_left > 0 && self.b0_edges > 10 {
                // (G1 10:24 (c)) the misroute control: J(-1) left on, J(3) untouched: the next
                // arrival turns into lane -1 as well
                self.misroute_left -= 1;
                self.misroute_at.push(now);
                self.note(now, "the misroute control: J(-1) left on through lane -1's branch edge (the next arrival into lane -1 too)".to_string());
            } else {
                set(w, [-1, 26, 9], &lever(false), flags::ALL)?;
                if !held_off(3, dv) {
                    set(w, [3, 26, 9], &lever(true), flags::ALL)?;
                }
            }
        }
        if !self.ring_timed && b1 && !self.branch_prev[1] {
            set(w, [3, 26, 9], &lever(false), flags::ALL)?;
        }
        // (an eastbound napper's window keeps J(-1) from re-arming)
        let j_blocked = self.j_blocks.iter().any(|(a, b)| now >= *a && now < *b);
        if !self.ring_timed && row && !self.row_prev && !j_blocked && !held_off(-1, dv) {
            set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
        }
        self.branch_prev = [b0, b1];
        self.row_prev = row;
        if let Some((cid, k)) = self.pick_cart {
            match w.entities.list.iter().find(|e| e.id == cid && !e.removed).map(|e| (cell(e.pos), e.passengers.first().copied())) {
                None => {
                    self.pick_cart = None;
                    self.divert = None;
                    self.away[k] = false;
                    self.held_log.push(format!("t {now}: a LOST PICK: the picked cart {cid} is gone before the queue (no arrival, HELD not set)"));
                }
                Some((c, rider)) => {
                    if self.divert.is_none() && c[1] == 25 && c[2] == 9 {
                        // (the row arrival: DAY sampled once, the module's daylight detector at
                        // the design's threshold, level >= `--day-threshold` (6: ironspec 02:18 PT))
                        let power: u32 = detector_level(w)?;
                        if power >= self.day_threshold {
                            self.pick_cart = None;
                            self.away[k] = false;
                            self.picks_day += 1;
                            self.held_log.push(format!("t {now}: a LOST PICK: lane x {}'s pick turned away by DAY at the row (no arrival, HELD not set)", LANES[k]));
                            self.note(now, format!("the picker: lane x {}'s cart {cid} on the row in DAY (power {power}): no divert", LANES[k]));
                        } else if self.held_at_divert && self.q_held {
                            // (divert sampling (i): HELD read here, at the row arrival)
                            self.pick_cart = None;
                            self.away[k] = false;
                            self.picks_held_at_divert += 1;
                            self.held_log.push(format!("t {now}: lane x {}'s pick turned away by HELD at the row", LANES[k]));
                            self.note(now, format!("the picker: lane x {}'s cart {cid} on the row with the queue HELD: no divert", LANES[k]));
                        } else {
                            self.divert = Some((cid, k));
                            self.divert_cleared = [false; 3];
                            self.divert_log.push((now, cid));
                            // (--ring-timed: day-1's HOLD_3 / HOLD_7 do the holding, keyed on REL)
                            if !self.ring_timed {
                                for x in [-1, 3, 7] {
                                    if x >= LANES[k] {
                                        set(w, [x, 26, 9], &lever(false), flags::ALL)?;
                                    }
                                }
                            }
                            self.note(now, format!("the picker: lane x {}'s cart {cid} on the row at {c:?}: J({}) and every J east held off", LANES[k], LANES[k]));
                        }
                    } else if let Some((dcid, dk)) = self.divert
                        && dcid == cid
                        && c[0] < 9
                        && !self.picker_hold_to_leg
                        && !self.evidence_keyed
                        && !self.ring_timed
                    {
                        // (day-1 11:40: the picker emulation's R6 error) each J the outbound napper
                        // clears is released at once, as that J's own D rise would leave it with the
                        // napper standing in for its lane's batch cart: past J(3) (x > 3) with lane -1's
                        // napper, J(3) on for the batch's next cart; past J(7) (x > 7), J(7) on, and with
                        // lane 7's napper J(-1) re-armed; not held to (8,25,9) (--picker-hold-to-leg)
                        if c[0] > 3 && !self.divert_cleared[1] {
                            self.divert_cleared[1] = true;
                            if dk == 0 {
                                set(w, [3, 26, 9], &lever(true), flags::ALL)?;
                                self.note(now, format!("the picker: cart {cid} past J(3): J(3) on for the batch's next cart"));
                            }
                        }
                        if c[0] > 7 && !self.divert_cleared[2] {
                            self.divert_cleared[2] = true;
                            if dk == 2 && !j_blocked {
                                set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
                            }
                            set(w, [7, 26, 9], &lever(true), flags::ALL)?;
                            self.note(now, format!("the picker: cart {cid} past J(7): J(7) on{}", if dk == 2 { ", J(-1) re-armed" } else { "" }));
                        }
                    } else if self.divert.is_some() && c[0] == 9 {
                        // (past (8,25,9) on the leg's curve: the split as if the cart had taken
                        // its lane: lane -1's D rise (J(-1) off, J(3) on), lane 3's (J(3) off),
                        // lane 7's (J(-1) re-armed); J(7) back on; since day-1 11:40 each J was
                        // released as the napper cleared it, unless --picker-hold-to-leg)
                        // (--ring-timed: the ring's own timed writes, nothing restored here)
                        match k {
                            _ if self.ring_timed => {}
                            0 => {
                                if !self.divert_cleared[1] {
                                    set(w, [3, 26, 9], &lever(true), flags::ALL)?;
                                }
                            }
                            1 => {}
                            _ => {
                                if !j_blocked && !self.divert_cleared[2] {
                                    set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
                                }
                            }
                        }
                        if !self.divert_cleared[2] && !self.ring_timed {
                            set(w, [7, 26, 9], &lever(true), flags::ALL)?;
                        }
                        self.divert_cleared = [false; 3];
                        self.divert = None;
                        self.pick_cart = None;
                        if let Some(v) = rider {
                            self.new_nappers.push((v, k, now));
                            self.nap_lane_of.insert(v, k);
                        }
                        self.note(now, format!("the picker: cart {cid} past (8,25,9) onto the leg; the split restored"));
                    }
                }
            }
        }
        // (day-1 12:49 / 12:53, ironspec 12:52 / 12:54: the outbound napper's J release EVIDENCE-KEYED)
        // with --evidence-keyed, each held-off J (x) ends its hold-off at the first press of its tripwire
        // east of it, the string at (x+1,26,9) (its hooks at (x+1,26,8) / (x+1,26,10)) ANDed with the hold-
        // off, + --evidence-delay gt; a press is any rider's box meeting the attached string's shape
        // (y 26 + 1/16 .. 26 + 2.5/16); a press by any cart but the napper's is a fault row (it ends the
        // hold-off as the built AND would); the napper's press is the emulated D(x) for the ring
        if self.evidence_keyed
            && let Some((dcid, dk)) = self.divert
        {
            for (jk, jx) in [(1usize, 3i32), (2, 7)] {
                if jx < LANES[dk] || self.divert_cleared[jk] || self.ev_release_at[jk].is_some() {
                    continue;
                }
                let sx = jx + 1;
                let o = frame_offset();
                let (x0, x1) = ((sx + o[0]) as f64, (sx + 1 + o[0]) as f64);
                let (y0, y1) = ((26 + o[1]) as f64 + 1.0 / 16.0, (26 + o[1]) as f64 + 2.5 / 16.0);
                let (z0, z1) = ((9 + o[2]) as f64, (10 + o[2]) as f64);
                let pressers: Vec<u32> = w
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && matches!(e.kind, EntityKind::Mob(_)) && e.vehicle.is_some())
                    .filter(|e| e.bb.max_x > x0 && e.bb.min_x < x1 && e.bb.max_y > y0 && e.bb.min_y < y1 && e.bb.max_z > z0 && e.bb.min_z < z1)
                    .filter_map(|e| e.vehicle)
                    .collect();
                if pressers.is_empty() {
                    continue;
                }
                self.ev_release_at[jk] = Some(now + self.evidence_delay);
                for p in &pressers {
                    if *p == dcid {
                        self.ev_presses.push((now, jx, *p, true));
                    } else {
                        self.ev_presses.push((now, jx, *p, false));
                        self.ev_faults.push(format!("t {now}: cart #{p} (not the napper's #{dcid}) pressed held-off J({jx})'s tripwire ({sx},26,9)"));
                    }
                }
                self.note(now, format!("the evidence keying: J({jx})'s tripwire ({sx},26,9) pressed by {:?}: its hold-off ends at t {}", pressers, now + self.evidence_delay));
            }
            for (jk, jx) in [(1usize, 3i32), (2, 7)] {
                if self.ev_release_at[jk] == Some(now) {
                    self.ev_release_at[jk] = None;
                    self.divert_cleared[jk] = true;
                    if jk == 1 && dk == 0 {
                        set(w, [3, 26, 9], &lever(true), flags::ALL)?;
                    }
                    if jk == 2 {
                        if dk == 2 && !self.j_blocks.iter().any(|(a, b)| now >= *a && now < *b) {
                            set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
                        }
                        set(w, [7, 26, 9], &lever(true), flags::ALL)?;
                    }
                    self.note(now, format!("the evidence keying: J({jx})'s hold-off ended"));
                    self.ev_releases.push((now, jx, dcid));
                }
            }
        }
        if self.napper {
            self.nap_step(w, now, [b0, b1, row])?;
        }
        Ok(())
    }

    /// (day-1 13:43, ironspec 13:37: the no-string ring, --ring-timed) Each latch write one stage (2 gt)
    /// after its trigger, each J's switch 4 gt after its drive (the lever written then):
    /// - a D(-1) press: J(-1) off, J(3) on (held while HOLD_3); on lane 3's pick, the emulated D(3) at
    ///   the press + 2 jumps J(3) to J(7) at once (J(3) never armed for the napper);
    /// - a D(3) press: J(3) off;
    /// - a D(7) press: the re-arm event E = the press + 2 (J(-1) on), the next batch's hold event;
    /// - at E, the current batch's pick: lane -1: HOLD_3 and HOLD_7 set, the emulated D(-1) (J(-1)
    ///   never armed for the napper, J(3) on but held); lane 3 / 7: HOLD_7 set; the holds' resets at
    ///   REL + L + k (L the slot's clock-locked last tick in that J); lane 7's pick: the emulated D(7)
    ///   TIMED at REL + L_J(-1)(3) + k, its re-arm event (no D(7) press keys it).
    fn ring_timed_step(&mut self, w: &mut redsim::world::World, now: i64, b0: bool, b1: bool, row: bool) -> Result<(), String> {
        use redsim::world::flags;
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        let [lj3_1, lj7_1, lj7_2, lj7_3, ljm1_3] = self.ring_l;
        let k = self.ring_k;
        let cur = self.rt_batches.last().copied();
        if b0 && !self.branch_prev[0] {
            // J(-1) off; J(3) on unless held; lane 3's pick: the emulated D(3) keeps J(3) off (the
            // pressing cart's batch: the one whose REL lies 85..125 gt before the press, slot 1's
            // clock-locked D(-1) press)
            self.rt_writes.push((now + if self.a_stop { 8 } else { 6 }, -1, false));
            let lane3_pick = self.rt_batches.iter().rev().find(|(r, _)| now - r >= 85 && now - r <= 125).is_some_and(|(_, p)| *p == Some(1));
            if lane3_pick {
                self.rt_writes.push((now + if self.a_stop { 8 } else { 6 }, 3, false));
                self.rt_log.push(format!("t {now}: D(-1) pressed: J(-1) off; lane 3's pick: the emulated D(3) at t {} (J(3) never armed)", now + 2));
            } else {
                self.rt_j3_latch = true;
                if !self.rt_hold3 {
                    self.rt_writes.push((now + 6, 3, true));
                }
            }
        }
        if b1 && !self.branch_prev[1] {
            self.rt_j3_latch = false;
            self.rt_writes.push((now + if self.a_stop { 8 } else { 6 }, 3, false));
        }
        let mut events: Vec<i64> = Vec::new();
        let mut sources: Vec<&'static str> = Vec::new();
        if row && !self.row_prev {
            events.push(now + 2);
            sources.push("lane 7's real D(7) + 2");
        }
        // the timed emulated D(7) of a lane-7 pick
        if self.rt_timed_d7.first().is_some_and(|t| *t == now) {
            self.rt_timed_d7.remove(0);
            events.push(now);
            sources.push("the emulated D(7)");
            self.rt_log.push(format!("t {now}: the {} emulated D(7) (lane 7's pick): the re-arm event", if self.a_stop { "D(3)-keyed" } else { "TIMED" }));
        }
        // (day-1's G11 rev 15:42, ironspec 15:37 (2)) the lane-7 pick's emulated D(7) keys on lane 3's
        // REAL D(3) press + delta7 (the closed loop's measured maximum of the napper's last tick in
        // J(-1)'s cell minus D(3)'s press, + 3): with the design controller, here, at D(3)'s rise
        if self.a_stop && b1 && !self.branch_prev[1] {
            self.d3_presses.push(now);
            if let Some((rel, Some(2))) = cur
                && now - rel >= 85
                && now - rel <= 200
            {
                self.rt_timed_d7.push(now + self.delta7);
                self.rt_timed_d7.sort();
                self.rt_log.push(format!("t {now}: D(3) pressed (REL {rel} + {}), lane 7's pick: the emulated D(7) at t {} (delta7 {})", now - rel, now + self.delta7, self.delta7));
            }
        }
        // (day-1's G8 rev 15:42) RE_ARMED: set ONLY by the ring's re-arm events (lane 7's real D(7) + 2,
        // or the lane-7 pick's D(3)-keyed emulated D(7)), reset at REL; its setters logged.
        // (ironspec 17:28 platform, day-1's rev 17:30: the cheaper C) with the design controller G8:
        // RE_ARMED's set = D(7)'s press AND SEEN3; G8a: SEEN3 set by D(3) (real, or G11-emulated), reset
        // at REL; G10: J(-1)'s re-arm the same AND
        if self.a_stop {
            if b1 && !self.branch_prev[1] && self.seen[1].is_none() {
                self.seen[1] = Some(now);
            }
            if !events.is_empty() {
                if let Some(s3) = self.seen[1] {
                    if !self.re_armed {
                        self.re_armed = true;
                        self.re_armed_setters.push(format!("t {now}: D(7) ({}) AND SEEN3 (D(3) t {s3})", sources.join(" / ")));
                    }
                } else {
                    self.re_armed_setters.push(format!("t {now}: D(7) ({}) WITHOUT SEEN3: RE_ARMED not set (G8a)", sources.join(" / ")));
                    events.clear();
                }
            }
        }
        for (e, src) in events.iter().zip(sources.iter()) {
            if !self.a_stop {
                self.re_armed = true;
                self.re_armed_setters.push(format!("t {e}: {src}"));
            }
        }
        for e in events {
            self.rt_rearm_events.push(e);
            if let Some((rel, p)) = cur {
                match p {
                    Some(0) => {
                        // HOLD_3, HOLD_7 set; the emulated D(-1): J(-1) never re-armed for the napper
                        self.rt_hold3 = true;
                        self.rt_hold7 = true;
                        self.rt_j3_latch = true;
                        self.rt_writes.push((e + if self.a_stop { 6 } else { 4 }, 3, false));
                        self.rt_writes.push((e + if self.a_stop { 6 } else { 4 }, 7, false));
                        self.rt_writes.push((e + 6, -1, false));
                        self.rt_resets.push((rel + lj3_1 + k, 3, rel, 0));
                        self.rt_resets.push((rel + lj7_1 + k, 7, rel, 0));
                        self.rt_log.push(format!("t {e}: the re-arm event for REL {rel} (lane -1's pick): HOLD_3 and HOLD_7 set, the emulated D(-1) at t {}", e + 2));
                    }
                    Some(pk) => {
                        self.rt_hold7 = true;
                        self.rt_writes.push((e + 4, -1, true));
                        self.rt_writes.push((e + if self.a_stop { 6 } else { 4 }, 7, false));
                        let l7 = if pk == 1 { lj7_2 } else { lj7_3 };
                        self.rt_resets.push((rel + l7 + k, 7, rel, pk));
                        if pk == 2 && !self.a_stop {
                            self.rt_timed_d7.push(rel + ljm1_3 + k);
                            self.rt_timed_d7.sort();
                        }
                        self.rt_log.push(format!("t {e}: the re-arm event for REL {rel} (lane x {}'s pick): J(-1) armed, HOLD_7 set", LANES[pk]));
                    }
                    None => {
                        self.rt_writes.push((e + 4, -1, true));
                    }
                }
            } else {
                self.rt_writes.push((e + 4, -1, true));
            }
        }
        // the holds' resets: the latch's own state comes back 4 gt after (J(3) on if its latch is
        // set; J(7) always on)
        let due: Vec<(i64, i32, i64, usize)> = self.rt_resets.iter().filter(|r| r.0 == now).copied().collect();
        self.rt_resets.retain(|r| r.0 != now);
        for (t, jx, rel, pk) in due {
            if jx == 3 {
                self.rt_hold3 = false;
                if self.rt_j3_latch {
                    self.rt_writes.push((t + 4, 3, true));
                }
            } else {
                self.rt_hold7 = false;
                self.rt_writes.push((t + 4, 7, true));
            }
            self.rt_reset_log.push((t, jx, rel, pk));
            self.rt_log.push(format!("t {t}: HOLD_{jx} reset (REL {rel}): J({jx}) switches at t {}", t + 4));
        }
        // the writes due now (each J's lever)
        let writes: Vec<(i64, i32, bool)> = self.rt_writes.iter().filter(|x| x.0 == now).copied().collect();
        self.rt_writes.retain(|x| x.0 != now);
        for (_, jx, on) in writes {
            // a held J stays off whatever its latch says
            let held = (jx == 3 && self.rt_hold3) || (jx == 7 && self.rt_hold7);
            set(w, [jx, 26, 9], &lever(on && !held), flags::ALL)?;
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
        // ARM3: the main line's detector risen at or after REL + 24; with path (a) the main line's
        // (5,0,7) is a plain rail and ARM3 := REL + 52 (day-1, ironspec 19:44: the napper's hold rail
        // at L7 - 32 = REL + 64)
        let path_a = self.path_a;
        if path_a {
            if let Some(r) = self.rel
                && now == r + 52
                && self.arm3.is_none()
            {
                self.arm3 = Some(now);
            }
        }
        let det5 = powered(w, [5, 0, 7]);
        if !path_a && det5 && !self.det5_prev && self.arm3.is_none() && self.rel.is_some_and(|r| now >= r + 24) {
            self.arm3 = Some(now);
        } else if !path_a && det5 && !self.det5_prev {
            // (ironspec 11:05) any other rise of (5,0,7) (the napper rolling over it after the hold,
            // or a rise before REL + 24): logged with the ARM3 consumers' states around it; the
            // emulation latches ARM3 once a cycle, so a second rise changes nothing here (the
            // built circuit's consumers: day-1's list)
            let lv = |x: i32| w.get(dbp(x, 26, 9)).to_string().contains("powered=true");
            self.nap_events.push(format!(
                "t {now}: (5,0,7) rose again ({}): the emulation keeps ARM3 latched; the levers J/K (x,26,9) at x -3 {}, -1 {}, 1 {}, 3 {}, 5 {}; the hold input (7,-1,8) {}; a cycle {}",
                self.arm3.map_or_else(|| self.rel.map_or("no cycle".to_string(), |r| format!("REL + {}", now - r)), |a| format!("ARM3 + {}", now - a)),
                lv(-3),
                lv(-1),
                lv(1),
                lv(3),
                lv(5),
                if w.get(dbp(7, -1, 8)).to_string().contains("redstone_block") { "on" } else { "off" },
                if self.busy { "running" } else { "idle" }
            ));
        }
        self.det5_prev = det5;
        // the stop (its cells moved with it: `stop_dy`, the bed head's y + 18; ironspec 12:23 put
        // the stop 9 down)
        let sy = self.stop_dy;
        let by = sy - 18;
        let bed = matches!(w.get(dbp(-3, by, 3)).to_string(), s if s.contains("occupied=true"));
        // (G1 16:00, night check 4; tick-exact 16:03: ValidateNearbyPoi erases HOME of any awake HOME-
        // holder within 16 of the head's centre while the bed is occupied) each occupied tick, every
        // awake HOME-holder within 16 of the bed head's centre (-2.5, by + 0.5625 / 2, 3.5), riding or not
        if bed {
            let head = (-2.5f64, by as f64 + 0.28125, 3.5f64);
            for e in w.entities.list.iter().filter(|e| !e.removed) {
                if let EntityKind::Mob(m) = &e.kind
                    && matches!(m.ty, redsim::entity::mob::types::MobType::Villager)
                    && m.brain.as_deref().is_some_and(|b| b.home.is_some())
                    && m.sleeping_pos.is_none()
                {
                    let p = dpos(e.pos);
                    let d = ((p.x - head.0).powi(2) + (p.y - head.1).powi(2) + (p.z - head.2).powi(2)).sqrt();
                    if d < self.exposure_least.0 || self.exposure_least.0 == 0.0 {
                        self.exposure_least = (d, now, e.id);
                    }
                    if d < 16.0 {
                        self.exposure_16.push((now, e.id, d));
                    }
                }
            }
        }
        // (58f770cc's A6, b36302af8: every controller a floor lever on glass; the hatch's levers
        // (-4,by,2) and (-4,by+1,4) ON in the normal state, OFF at F; the park's levers (1,by+1,-6)
        // and (0,by,-6) OFF (braked), ON at the release, the bed's wake edge + 1 gt (58f770cc 00:39
        // PT); R's detector (4,by-1,4) re-arms both. d907's stop: stone and redstone blocks at its
        // control cells, the release 30 gt after F)
        let a6 = w.get(dbp(1, by + 1, -6)).to_string().contains("lever");
        let set_lever = |w: &mut redsim::world::World, c: [i32; 3], on: bool| -> Result<(), String> {
            let s = w.get(dbp(c[0], c[1], c[2])).to_string();
            if !s.contains("lever") {
                return Err(format!("A6's control cell ({},{},{}) holds {s}, not a lever", c[0], c[1], c[2]));
            }
            set(w, c, &s.replace(if on { "powered=false" } else { "powered=true" }, if on { "powered=true" } else { "powered=false" }), flags::ALL)
        };
        if self.a_stop {
            // (day-1's canonical gates rev 15:37: the (A) stop's shutter and guard, its taps)
            self.a_stop_step(w, now, bed)?;
        }
        if !self.a_stop && bed && !self.bed_prev {
            self.f_at = Some(now + 10);
            self.nap_events.push(format!("t {now}: lie-down (the bed occupied)"));
        }
        if !self.a_stop && a6 && !bed && self.bed_prev {
            // (58f770cc 00:39 PT, ironspec 01:08 PT: today's A6 releases at the wake edge, the
            // bench's release landing 1 gt after the recorded wake; the cart reaches P 36..40 gt
            // later)
            self.release_at = Some(now + 1);
            self.nap_events.push(format!("t {now}: the wake edge (the bed freed): the release at t {}", now + 1));
        }
        self.bed_prev = bed;
        if !self.a_stop && self.f_at == Some(now) {
            if a6 {
                set_lever(w, [-4, by, 2], false)?;
                set_lever(w, [-4, by + 1, 4], false)?;
            } else {
                set(w, [-4, -18 + sy, 2], "minecraft:stone", flags::ALL)?;
                set(w, [-4, -16 + sy, 3], "minecraft:stone", flags::ALL)?;
                self.release_at = Some(now + 30);
            }
            self.f_at = None;
            self.nap_events.push(format!("t {now}: F (the hatch's controls off)"));
        }
        if !self.a_stop && self.release_at == Some(now) {
            if a6 {
                set_lever(w, [1, by + 1, -6], true)?;
                set_lever(w, [0, by, -6], true)?;
            } else {
                set(w, [1, -18 + sy, -5], "minecraft:redstone_block", flags::ALL)?;
                set(w, [0, -19 + sy, -5], "minecraft:redstone_block", flags::ALL)?;
            }
            self.release_at = None;
            self.nap_events.push(format!("t {now}: the stop's release (the park's {} on)", if a6 { "levers" } else { "supports" }));
        }
        let rdet = powered(w, [4, by - 1, 4]);
        if rdet && !self.rdet_prev && !self.a_stop {
            if a6 {
                set_lever(w, [1, by + 1, -6], false)?;
                set_lever(w, [0, by, -6], false)?;
                set_lever(w, [-4, by, 2], true)?;
                set_lever(w, [-4, by + 1, 4], true)?;
                self.f_at = None;
                self.release_at = None;
            } else {
                set(w, [1, -18 + sy, -5], "minecraft:stone", flags::ALL)?;
                set(w, [0, -19 + sy, -5], "minecraft:stone", flags::ALL)?;
                set(w, [-4, -18 + sy, 2], "minecraft:redstone_block", flags::ALL)?;
                set(w, [-4, -16 + sy, 3], "minecraft:redstone_block", flags::ALL)?;
            }
            self.nap_events.push(format!("t {now}: R's detector: the stop re-armed"));
        }
        // (ironspec D2) every rise of R's detector: a napper passing R (the stop single-occupied, it's
        // the latest released napper's)
        if rdet && !self.rdet_prev {
            self.rdet_rises.push(now);
            // (G1 X21) the rise attributed to the rider on R's detector cell (or the cell above the
            // rail's floor), so D2 can require the PREVIOUS napper's own pass
            let who = w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && {
                    // (onechunk 17:24) the detector's sensing box, as the rider-keyed press reads it
                    let p = dpos(e.pos);
                    (p.x - 4.5).abs() < 0.8 && (p.z - 4.5).abs() < 0.8 && p.y >= (by - 1) as f64 - 0.1 && p.y < (by + 1) as f64
                })
                .find_map(|e| e.passengers.first().copied());
            self.rdet_riders.push((now, who));
        }
        self.rdet_prev = rdet;
        // (ironspec 22:40 PT (2), G1's [4]) the latch's set input is R's detector power delayed
        // (day-1 15:26: d_RS = 10 gt, the repeaters 2 + 3) as day-1's line-7 design builds it:
        // RS_ready set while rdet(now - 10) is high (its level, not its rise), reset at the queue's
        // departure; (ironspec 15:23 (3), 12:50 bought) RIDER-KEYED: R's press counts only with a
        // RIDDEN cart on R's detector, an empty cart never sets it
        // (onechunk 16:50) a cart presses the detector when its box meets the sensing box (the cell's
        // 0.2..0.8 in x / z, DetectorRailBlock): its centre within 0.79 of the cell's centre, not
        // necessarily inside the cell (a rider pressing from (3,y,4) was read as an empty press)
        let on_r = |e: &redsim::entity::Entity| -> bool {
            let p = dpos(e.pos);
            (p.x - 4.5).abs() < 0.8 && (p.z - 4.5).abs() < 0.8 && p.y >= (by - 1) as f64 - 0.1 && p.y < (by + 1) as f64
        };
        let ridden_on_r = rdet
            && w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && on_r(e));
        if rdet && !ridden_on_r && !self.rdet_empty_prev {
            self.nap_events.push(format!("t {now}: R's detector pressed by an EMPTY cart: RS_ready not set (rider-keyed)"));
            self.rdet_empty_presses += 1;
        }
        self.rdet_empty_prev = rdet && !ridden_on_r;
        self.rdet_hist.push_back(ridden_on_r);
        while self.rdet_hist.len() > 11 {
            self.rdet_hist.pop_front();
        }
        if self.wayin && self.rdet_hist.len() == 11 && self.rdet_hist[0] {
            if !self.q_rs_ready {
                self.nap_events.push(format!("t {now}: RS_ready set (R's detector pressed by a ridden cart at t {})", now - 10));
            }
            self.q_rs_ready = true;
        }
        // (day-1's G1 with the design controller) RS_ready reset 2 gt after the queue release
        if self.rs_reset_due == Some(now) {
            self.rs_reset_due = None;
            self.q_rs_ready = false;
            self.nap_events.push(format!("t {now}: RS_ready reset (the queue release + 2)"));
        }
        if self.wayin {
            let (q, rel_cell, arr, dep) = ([9, by + 13, -4], [8, by + 13, -4], [9, by + 14, -5], [9, by + 12, -3]);
            let a = powered(w, arr);
            if a && !self.q_arr_prev {
                self.q_hold_since = Some(now);
                // (ironspec 00:12 PT, F1 adopted) HELD set at the queue's arrival detector, reset at
                // the departure; every set and reset logged with its cause
                self.q_held = true;
                self.held_set = Some((now, "the queue's arrival".to_string()));
                self.held_by = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && (cell(e.pos) == arr || cell(e.pos) == q)).and_then(|e| e.passengers.first().copied());
                self.held_log.push(format!("t {now}: HELD set by the queue's arrival"));
                self.q_arrivals.push(now);
                self.nap_events.push(format!("t {now}: the queue's arrival ({},{},{}): HELD, the hold from now; the hatch (6,{},-5) {}", arr[0], arr[1], arr[2], by + 15, if w.get(dbp(6, by + 15, -5)).to_string().contains("open=true") { "open" } else { "closed" }));
            }
            self.q_arr_prev = a;
            // (the napper's rest at the seat, for HELD's bound)
            if self.q_arrivals.last().is_some_and(|ar| self.q_rest_at.is_none_or(|r| r < *ar))
                && w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == q && e.delta_movement.x.abs() + e.delta_movement.z.abs() < 1e-3)
            {
                self.q_rest_at = Some(now);
            }
            // (ironspec's E2, day-1 06:51 PT: HELD stays set while R1 is closed (REST_l off, DAY
            // high or RS_ready false); its stuck bound is R1 OPEN (REST_l, DAY low, the stop free)
            // with HELD still set past the arrival's travel onto the rail and its departure (30 gt;
            // day-1's measured travel 11-12 gt, the departure 17 gt after the arrival): checked
            // below, once REST_l and DAY are read)
            let dp = powered(w, dep);
            if dp && !self.q_dep_prev {
                // (ironspec 19:29 (b): who departed)
                if let Some(v) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == dep).and_then(|e| e.passengers.first().copied()) {
                    self.dep_log.push((now, v));
                }
                if !self.a_stop {
                    self.q_rs_ready = false;
                }
                // first departure, before its napper passes R: the next release must fail the bar
                if self.rs_early && !self.rs_early_done {
                    self.q_rs_ready = true;
                    self.rs_early_done = true;
                    self.rs_early_at = Some(now);
                    self.nap_events.push(format!("t {now}: the RS_ready control: RS_ready forced set at the departure, before its napper passes R"));
                }
                if self.q_held {
                    self.held_log.push(format!("t {now}: HELD reset by the queue's departure{}", self.held_set.as_ref().map_or(String::new(), |(t, c)| format!(" (set t {t} by {c}, {} gt)", now - t))));
                }
                self.q_held = false;
                self.held_set = None;
                self.q_hold_since = None;
                if self.q_release_on {
                    input_off(w, rel_cell, "minecraft:air")?;
                    self.q_release_on = false;
                }
                self.nap_events.push(format!("t {now}: the queue's departure ({},{},{}): RS_ready and HELD reset, the release off", dep[0], dep[1], dep[2]));
            }
            self.q_dep_prev = dp;
            let present = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == q);
            // (ironspec's design-fidelity D1, day-1 06:15 PT: REST_l is the built counter's line,
            // C >= 138 or C < 15 (ctlcounter.rs :696-697), not the clock's 12,000; before the
            // run's first release C is the counter's at this day time)
            let c_now = if self.counter_pending { counter_c(w.day_time().rem_euclid(24_000)) } else { self.rel_count };
            let rest = c_now >= 138 || c_now < 15;
            // (NOT DAY: the module's daylight detector (-5,15,3) below the design's DAY level,
            // `--day-threshold` (6, ironspec 02:18 PT: the real line's L-6 threshold); the detector
            // updates its power every 20 gt, so the level is sampled at its updates)
            // (G1 N4: one reader for every DAY use, an error when the cell isn't a detector)
            // (G1 X24) --day-phase-control now offsets the STORED power the check reads, once: at
            // the first on-phase update after t 1,000 the detector's power is written one off
            // (before -> after printed), and exactly that update must mismatch
            if self.day_phase_control && !self.day_phase_done && now >= 1_000 && w.time % 20 == 0 {
                let pos = dbp(-5, 15, 3);
                let before = w.get(pos).to_string();
                let p0 = detector_level(w)?;
                let p1 = if p0 == 15 { 14 } else { p0 + 1 };
                let after = before.replace(&format!("power={p0}"), &format!("power={p1}"));
                let b = redsim::block::Block::parse(&after).map_err(|e| format!("{after}: {e}"))?;
                w.set_block(pos, b, redsim::world::flags::RAW);
                self.day_phase_done = true;
                self.day_phase_at = Some(now);
                self.nap_events.push(format!("t {now}: the day-phase control: the detector's stored power {p0} -> {p1} ({before} -> {after})"));
            }
            let power = detector_level(w)?;
            let not_day = power < self.day_threshold;
            // (ironspec's E2, day-1 06:51 PT) HELD's stuck bound: R1 open (REST_l, DAY low, RS_ready)
            // for more than 30 gt with HELD still set
            let r1_open = rest && not_day && self.q_rs_ready;
            if r1_open && self.held_set.is_some() {
                let since = *self.r1_open_since.get_or_insert(now);
                if let Some((t, cause)) = self.held_set.clone()
                    && now - since > 30
                    && !self.held_stuck.iter().any(|(st, _)| *st == t)
                {
                    self.held_stuck.push((t, cause.clone()));
                    self.held_log.push(format!("t {now}: HELD still set {} gt after its set at t {t} by {cause}, R1 open (REST_l, DAY low, RS_ready) since t {since}: past the 30-gt travel and departure", now - t));
                }
            } else {
                self.r1_open_since = None;
            }
            // (G1 N3 / N18, ironspec D3: the stand-in's validation, the stored power against redsim's
            // analytic detector at every 20-gt update from the run's first update on (the stored
            // power before it is the save's), zero tolerance; --day-phase-control compares at the
            // half-period off-phase instead, which the bar must catch at the edges)
            {
                let p = power;
                self.day_ticks += 1;
                if w.time % 20 == 0 && now >= 1 {
                    let sky = match self.day_sky {
                        Some(s) => s,
                        None => {
                            let s = redsim::daylight::sky_light(w, dbp(-5, 15, 3)).map_err(|e| format!("the DAY detector's sky light: {e}"))?;
                            self.day_sky = Some(s);
                            s
                        }
                    };
                    let want = redsim::daylight::detector_power(sky, w.sky_darken(), w.day_time(), false) as u32;
                    self.day_updates += 1;
                    if p != want {
                        self.day_mismatch += 1;
                        self.day_mismatch_at.push(now);
                        if self.day_mismatch_rows.len() < 8 {
                            self.day_mismatch_rows.push(format!("t {now} (day time {}): stored {p}, analytic {want}", w.day_time().rem_euclid(24_000)));
                        }
                    }
                }
                self.day_power_prev = Some(p);
                if self.day_state_prev != Some(!not_day) {
                    if self.day_edges.len() < 24 {
                        self.day_edges.push(format!("t {now} (day time {}) DAY {} at power {p}", w.day_time().rem_euclid(24_000), if not_day { "off" } else { "on" }));
                    }
                    self.day_state_prev = Some(!not_day);
                }
            }
            // (day-1's R1, 19:19: the release is the built gate (the comparator (7,y+13,-4)
            // subtracting NOT-ready from N, the repeater (8,y+13,-4) into the queue rail); the
            // emulation drives only its unlaid line ends as levels: NOT-ready at the side dust
            // (7,y+13,-5) (high from the departure's rise until A6's R), REST_l at (6,y+15,-1), DAY at
            // (4,y+15,-2); the release is read off the queue rail's power)
            if w.get(dbp(rel_cell[0], rel_cell[1], rel_cell[2])).to_string().contains("repeater") {
                // (NOT-ready drives the latch's side dust (7,y+13,-5) from a lever beside it at
                // (8,y+13,-5), hung on the glass (9,y+13,-5): a comparator reads a side dust's own
                // power, but a lever standing in the dust's cell only by its direct signal, which a
                // floor lever sends down, not sideways (the gate bench shows it))
                self.level(w, [8, by + 13, -5], [7, by + 13, -5], !self.q_rs_ready)?;
                self.level(w, [6, by + 15, -1], [6, by + 15, -2], rest)?;
                self.level(w, [4, by + 15, -2], [5, by + 15, -2], !not_day)?;
                // (day-1 01:10 PT, the per-pick trace) the night's REST_l and DAY edges
                if self.rest_prev != Some(rest) {
                    self.edge_log.push(format!("t {now}: REST_l {}", if rest { "on" } else { "off" }));
                    self.rest_prev = Some(rest);
                }
                if self.day_prev != Some(!not_day) {
                    self.edge_log.push(format!("t {now}: DAY {}", if not_day { "off" } else { "on" }));
                    self.day_prev = Some(!not_day);
                }
                let qp = powered(w, q);
                // (the release: the queue rail powered while a napper is on it, a cart arriving on
                // an already powered rail included (it rolls straight through))
                let live = qp && present;
                if live && !self.q_rail_prev {
                    self.q_released.push(now);
                    if let Some(v) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == q).and_then(|e| e.passengers.first().copied()) {
                        self.release_log.push((now, v));
                        // (day-1's G3, the built gate too) IN-STOP_l set at a lane-l queue release
                        if self.a_stop
                            && let Some(&k) = self.nap_lane_of.get(&v)
                        {
                            self.in_stop[k] = true;
                            self.in_stop_rider[k] = Some(v);
                            self.nap_events.push(format!("t {now}: IN-STOP set, lane x {} (#{v} released into the stop)", LANES[k]));
                        }
                    }
                    // (day-1's G1 resets RS_ready at the queue release + 2; the record's BUILT gate is the
                    // level N AND RS_ready into the queue rail, so a reset 2 gt after the release cut the
                    // queue rail's boost after 2 gt and the napper's cart crawled off the way-in's rail end
                    // and stopped short of the shaft (t22's smoke, #51 at (9.5,-15,-0.59)): with the built
                    // gate RS_ready resets at the queue's departure, as the built line holds it; onechunk 17:24)
                    self.nap_events.push(format!("t {now}: the queue rail powered with a napper on it, through R1 (the built gate: N and RS_ready); the hatch (6,{},-5) {}", by + 15, if w.get(dbp(6, by + 15, -5)).to_string().contains("open=true") { "open" } else { "closed" }));
                }
                self.q_rail_prev = live;
            } else {
            // (ironspec 17:16: the gate's 80-gt hold term goes; `--queue-hold N` keeps one for a
            // comparison)
            let hold_ok = self.q_hold_since.is_none_or(|s| now >= s + self.queue_hold);
            // (day-1's G2: R1's release path carries the dwell 6, one delay-3 repeater: with the (A)
            // stop's design controller the release lands 6 gt after its AND holds)
            let cond = !self.q_release_on && self.q_rs_ready && present && hold_ok && rest && not_day;
            let fire = if self.a_stop {
                if cond && self.queue_release_due.is_none() {
                    // (day-1's delays rev 15:42: NOT-ready's measured 4 + the dwell's 6)
                    self.queue_release_due = Some(now + 10);
                }
                if self.queue_release_due == Some(now) {
                    self.queue_release_due = None;
                    !self.q_release_on
                } else {
                    false
                }
            } else {
                cond
            };
            if fire {
                input_on(w, rel_cell, q)?;
                self.q_release_on = true;
                self.q_released.push(now);
                // (day-1's G1: RS_ready RESET at the queue release, through a torch)
                if self.a_stop {
                    self.rs_reset_due = Some(now + 2);
                }
                // (ironspec 19:29 (a): who was released)
                if let Some(v) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == q).and_then(|e| e.passengers.first().copied()) {
                    self.release_log.push((now, v));
                    // (day-1's G3) IN-STOP_l set at a lane-l queue release
                    if self.a_stop
                        && let Some(&k) = self.nap_lane_of.get(&v)
                    {
                        self.in_stop[k] = true;
                        self.in_stop_rider[k] = Some(v);
                        self.nap_events.push(format!("t {now}: IN-STOP set, lane x {} (#{v} released into the stop)", LANES[k]));
                    }
                }
                self.nap_events.push(format!("t {now}: the queue's release (RS_ready, the hold {} met, REST, NOT DAY{}): the stand-in on at ({},{},{})", self.queue_hold, if self.a_stop { ", the dwell 6" } else { "" }, rel_cell[0], rel_cell[1], rel_cell[2]));
            }
            }
        }
        // (day-1's canonical gates rev 15:42, G12–G14) F4(a) with the (A) stop's design controller
        if self.a_stop {
            self.f4a_step(w, now)?;
        }
        // (ironspec's hold release (c') as built, 11:49; k_lock 22:03 PT) the lift exit (7,-3,4) as a
        // detector rail sets the waiting RS; its Q delayed k_lock gt; E samples live Q AND delayed Q
        // at its edge (ARM3
        // + 2); locked: the 12-gt pulse fires (the input (7,-1,8) at ARM3 + 10, the rail at ARM3 +
        // 12) and resets the RS (the departing napper's second ARM3 rise fires nothing; an empty
        // hold fires nothing); the fired
        // pulse keys K(x) (on ARM3 + 97 / 112 / 128, off ARM3 + 125 / 140 / 156),
        // PRESENT_l at its tap, and J(-1)'s block ARM3 + 72..127 for an eastbound napper
        // (G1's note: k_lock / k_sw required; unset is an error where used, never 0)
        let k_lock = self.k_lock.ok_or("k_lock unset: a napper run needs the module's `k-lock:` line")?;
        let at_exit = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [7, -3, 4]);
        if at_exit && self.rs_set.is_none() {
            self.rs_set = Some(now);
            self.lift_exit_log.push(now);
            self.nap_events.push(format!("t {now}: the lift exit (7,-3,4): the waiting RS set (its delayed Q at t {}, k_lock {k_lock})", now + k_lock));
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
                self.nap_events.push(format!("t {now}: the napper at rest on the hold; the delayed Q reaches the sampler at t {} ({:+} gt against the rest)", s + k_lock, s + k_lock - now));
            }
        }
        // (ironspec 22:09 PT: H2 samples at the built gate's instant, E + 2 = ARM3 + 4)
        if let Some(a) = self.arm3
            && now == a + 4
            && let Some(s) = self.rs_set
            && now >= s + k_lock
        {
            let k = self.nap_lane;
            // (day-1 01:27 / ironspec 01:26 PT, m and the catch margin per visit) the H2 sample this
            // napper caught and its catch margin: the sample's tick minus Q-up (the waiting RS's set +
            // k_lock), bar >= 2
            self.h2_log.push((now, k, now - (s + k_lock)));
            // (day-1's Delta, ironspec 12:41 (4): --hold-pulse-delta shifts the hold's pulse and every
            // tap keyed off the fired pulse (the K windows, PRESENT, J(-1)'s block) by Delta gt)
            let a = a + self.hold_delta;
            // (G1 14:11, CHANGES 2 on ed4978c0c) a napper's H2 sample while an earlier napper's
            // window is still pending is a FAILURE (F4(a): K windows disjoint, one napper), counted
            // and printed, never an overwrite: a K set / reset or PRESENT still queued, the hold's
            // pulse not yet ended, J(-1)'s block not yet ended, or the K's / J(-1)'s window of the
            // previous napper still open
            {
                let mut pend: Vec<String> = Vec::new();
                if self.k_events.iter().any(|e| e.0 >= now) {
                    pend.push(format!("K events {:?}", self.k_events));
                }
                if !self.present_events.is_empty() {
                    pend.push(format!("PRESENT {:?}", self.present_events));
                }
                if self.hold_off.is_some_and(|t| t >= now) {
                    pend.push(format!("the hold's pulse until t {}", self.hold_off.unwrap_or_default()));
                }
                if self.j_blocks.iter().any(|(_, b)| *b >= now) {
                    pend.push(format!("J(-1)'s block {:?}", self.j_blocks));
                }
                if !pend.is_empty() {
                    self.overlaps.push(format!("t {now}: lane x {}'s napper sampled with the previous napper's window pending ({})", LANES[k], pend.join("; ")));
                }
            }
            // (day-1's G5, the (A) stop's design controller) the hold's pulse gated by "at most one
            // RETURN bit set"; F4(a)'s K / J windows are keyed at T_d (f4a_step), not here
            if self.a_stop && self.return_bits.iter().filter(|b| b.is_some()).count() > 1 {
                self.f4_faults.push(format!("t {now}: the hold's pulse withheld (G5: {} RETURN bits set)", self.return_bits.iter().filter(|b| b.is_some()).count()));
            } else {
                self.hold_on = Some(a + 10);
                self.hold_off = Some(a + 22);
            }
            self.rs_set = None;
            self.nap_arm3 = Some(a);
            // (G1 14:11's control, --nap-lane-control) a pick moving nap_lane inside the window: 30 gt
            // after this sample nap_lane becomes another lane; every K / PRESENT event of this window
            // must still fire on this napper's own cells
            if self.nap_lane_control {
                self.nap_lane_moves.push((now + 30, (k + 1) % 3));
            }
            if !self.a_stop {
            self.k_windows.push((now, k, LANES[k] - 2));
            // (day-1 19:19, K = 4: the K-set + 4 at a + 97 / 112 / 128, the reset at K + 16 =
            // a + 125 / 140 / 156, PRESENT's set edge on its own tap at K + 2 = a + 111 / 126 / 142;
            // ironspec's E5, day-1 06:51 PT: PRESENT_l is a two-torch RS NOR whose output reaches the
            // AND 4 gt after its set edge (measured at all 20 phases), so PRESENT is effective at
            // a + 115 / 130 / 146)
            let kx0 = LANES[k] - 2;
            self.k_events.push((a + [97, 112, 128][k], kx0, true));
            self.k_events.push((a + [125, 140, 156][k], kx0, false));
            self.present_events.push((a + [115, 130, 146][k], k));
            if k > 0 {
                // (G1 14:11: queued, each window its own; an overlap is counted above)
                self.j_blocks.push((a + 72, a + 127));
            }
            }
            if !on_hold {
                self.empty_pulses += 1;
            } else if self.hold_rest.is_none() {
                // (G1, not blocking: a fired pulse that finds the napper still rolling; bar 0 under
                // the settle lock)
                self.rolling_pulses += 1;
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
            input_on(w, [7, -1, 8], [8, -1, 8])?;
            self.hold_on = None;
        }
        if self.hold_off == Some(now) {
            input_off(w, [7, -1, 8], "minecraft:air")?;
            self.hold_off = None;
        }
        // (each K event its own cell, keyed at its napper's H2 sample: the sets, PRESENT, then the
        // resets, in the tick's order as before)
        // (--nap-lane-control: the forced nap_lane moves due now)
        let moves: Vec<(i64, usize)> = self.nap_lane_moves.iter().filter(|m| m.0 == now).copied().collect();
        self.nap_lane_moves.retain(|m| m.0 != now);
        for (_, nk) in moves {
            self.nap_lane = nk;
            self.note(now, format!("the nap-lane control: nap_lane moved to lane x {} inside a window", LANES[nk]));
        }
        let mut due: Vec<(i64, i32, bool)> = self.k_events.iter().filter(|e| e.0 == now).copied().collect();
        self.k_events.retain(|e| e.0 != now);
        // (G1 14:21's control, --stuck-k-control: the old defect restored, each K write's cell taken
        // from nap_lane at the fire tick; with --nap-lane-control it must FAIL the K-write audit)
        if self.stuck_k_control {
            let kx_now = LANES[self.nap_lane] - 2;
            for e in due.iter_mut() {
                e.1 = kx_now;
            }
        }
        for (_, kx, on) in due.iter() {
            self.k_fired.push((now, *kx, *on));
        }
        for (_, kx, _) in due.iter().filter(|e| e.2) {
            set(w, [*kx, 26, 9], &lever(true), flags::ALL)?;
            self.nap_events.push(format!("t {now}: K({kx}) on"));
        }
        let pdue: Vec<(i64, usize)> = self.present_events.iter().filter(|e| e.0 == now).copied().collect();
        self.present_events.retain(|e| e.0 != now);
        for (_, k) in pdue {
            self.present[k] = true;
            self.present_log.push((now, k));
        }
        for (_, kx, _) in due.iter().filter(|e| !e.2) {
            set(w, [*kx, 26, 9], &lever(false), flags::ALL)?;
            self.nap_events.push(format!("t {now}: K({kx}) off"));
        }
        if self.j_blocks.iter().any(|(_, b)| now == *b) {
            self.j_blocks.retain(|(_, b)| now != *b);
            // (re-armed only when no other napper's block is still open)
            if !self.j_blocks.iter().any(|(a, b)| now >= *a && now < *b) {
                set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
            }
            self.nap_events.push(format!("t {now}: J(-1) re-armed (the napper's block ends)"));
        }
        // the siding: D's rise while PRESENT_l (the branch detectors' rises, this tick)
        let prev = [self.branch_prev_nap[0], self.branch_prev_nap[1], self.d7_prev];
        for k in 0..3 {
            // (day-1 01:27 PT, m per visit) every D press on each lane
            if d[k] && !prev[k] {
                self.dpress_log.push((now, k));
            }
            // (day-1's G6, the (A) stop's design controller: REL_l = D_l AND string_l (a ridden cart on
            // R_l (x-2,24,7)), no latch, R = D + 34 through option (A)'s +20 on the feed)
            let string_l = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [LANES[k] - 2, 24, 7]);
            let gate_l = if self.a_stop { string_l } else { self.present[k] };
            if d[k] && !prev[k] && gate_l && self.siding_on[k].is_none() && self.siding_off[k].is_none() {
                // (day-1 19:19; k_sw = 6, ironspec 21:35 / 22:03: SIDING_l from WINDOW_l AND
                // PRESENT_l at D's rise, R powered at D + k_sw + 8 = D + 14, no wait for D off: the
                // input on at that tick less its line's latency, the (a) feed stack's delay-3
                // repeater and the siding repeater 8 gt, the siding repeater alone 2 gt)
                let sx = LANES[k] - 2;
                let stack = w.get(dbp(sx - 1, 22, 9)).to_string().contains("repeater");
                let k_sw = self.k_sw.ok_or("k_sw unset: a napper run needs the module's `k-sw:` line")?;
                let r_at = if self.a_stop { now + 34 } else { now + k_sw + 8 };
                self.siding_on[k] = Some(r_at - if stack { 8 } else { 2 });
                // (G1's note) R's observed rise printed against D + k_sw + 8 (D + 34 with G6)
                self.r_expect[k] = Some(r_at);
                // (day-1 / ironspec 08:28-08:31 platform: PRESENT_l, the built RS NOR latch, is
                // reset ONLY by the departure detector past R, so a napper late to its release
                // pulse is released at the lane's next D press, a lap late: the fail-safe; it was
                // reset here, by the release, which stranded a late napper on R)
                self.dpass_log.push((now, k));
            }
            let sx = LANES[k] - 2;
            // (day-1 08:31) PRESENT_l's reset: the napper's cart leaving the siding's R; (G1 10:47
            // blocking 4 / O5) R's DEPARTURE is the cart that stood on R last tick no longer there (a
            // dismount on R departs nothing), and only one carrying a napper (by id) resets PRESENT_l
            let r_any: Option<(u32, Option<u32>)> = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [sx, 24, 7]).map(|e| (e.id, e.passengers.first().copied()));
            let r_cart = r_any.is_some_and(|(_, r)| r.is_some());
            let departed: Option<(u32, Option<u32>)> = match (self.r_cart_id_prev[k], r_any) {
                (Some((c, _)), Some((c2, _))) if c == c2 => None,
                (Some(prev), _) => Some(prev),
                _ => None,
            };
            // (G1 10:34) every departure from R: the cart, its rider at its last tick on R, and
            // whether PRESENT_l was set
            if let Some((cid, rider)) = departed {
                self.r_departures.push((now, k, cid, rider, self.present[k]));
            }
            // (G1 08:41 / ironspec 08:41 platform: the unrouted bar's positive) --strand-control: the
            // first napper reaching its siding's R after t0 + 1,000 has PRESENT_l reset on arrival, so
            // no release follows: it must strand and fail the bar on its own sample
            if self.strand_control && !self.strand_done && r_cart && !self.r_cart_prev[k] && self.present[k] && now >= 1_000 {
                self.present[k] = false;
                self.strand_done = true;
                self.nap_events.push(format!("t {now}: the strand control: PRESENT_l reset on the napper's arrival at R, lane x {} (no release can follow)", LANES[k]));
            }
            if let Some((cid, rider)) = departed
                && self.present[k]
            {
                if rider.is_some_and(|r| self.napper_ids.contains(&r)) {
                    self.present[k] = false;
                    self.present_resets.push((now, k));
                    self.nap_events.push(format!("t {now}: PRESENT_l reset, lane x {}, by the napper's departure past R (cart #{cid})", LANES[k]));
                } else {
                    self.nap_events.push(format!("t {now}: lane x {}: cart #{cid} left R carrying {}: PRESENT_l kept", LANES[k], rider.map_or("no one".to_string(), |r| format!("#{r}, not a napper"))));
                }
            }
            self.r_cart_id_prev[k] = r_any;
            self.r_cart_prev[k] = r_cart;
            // (G1's note) R's observed rise against D + k_sw + 8
            let r_on = powered(w, [sx, 24, 7]);
            if r_on && !self.r_prev[k]
                && let Some(e) = self.r_expect[k].take()
            {
                self.r_obs.push((k, e, now));
            }
            self.r_prev[k] = r_on;
            if self.siding_on[k] == Some(now) {
                // (the (a) feed stack, day-1's: a repeater west of the siding's slime (sx-1,22,9)
                // takes the release at its input (sx-2,22,9); otherwise the old input (sx,23,9))
                let stack = w.get(dbp(sx - 1, 22, 9)).to_string().contains("repeater");
                let (cell, target) = if stack { ([sx - 2, 22, 9], [sx - 1, 22, 9]) } else { ([sx, 23, 9], [sx, 23, 8]) };
                let base = w.get(dbp(cell[0], cell[1], cell[2])).to_string();
                input_on(w, cell, target)?;
                self.siding_cell[k] = Some((cell, base));
                self.siding_on[k] = None;
                self.siding_off[k] = Some(now + 30);
                self.nap_events.push(format!("t {now}: the siding's release, lane x {} (the input ({},{},{}) on 30 gt)", LANES[k], cell[0], cell[1], cell[2]));
            }
            if self.siding_off[k] == Some(now) {
                if let Some((cell, base)) = self.siding_cell[k].take() {
                    input_off(w, cell, &base)?;
                }
                self.siding_off[k] = None;
            }
        }
        self.branch_prev_nap = [d[0], d[1]];
        self.d7_prev = d[2];
        if self.nap_arm3.is_some() && !self.k_events.iter().any(|e| !e.2) && self.present.iter().all(|p| !p) && self.siding_off.iter().all(|s| s.is_none()) && self.present_events.is_empty() {
            self.nap_arm3 = None;
        }
        Ok(())
    }

    /// A level held by a stand-in at a line end (`level`): on while `want`, the built state
    /// restored when off.
    /// (day-1's canonical gates rev 15:37, https://artifacts.internalmeta.com/a/2002426774049950 ;
    /// onechunk 15:39) the (A) stop composed at loop + (0,-27,-1), module cells: ARM (-2,-27,0), the
    /// bed head (-3,-27,3) and its observer (-3,-28,3), P (-2,-27,3) and P_STRING over it at
    /// (-2,-26,3) (G22), F's output (-5,-26,7) (the shutter's stand-in lever, ON = the window shut),
    /// the park's drive input (3,-27,3) (a wall lever on the conductor (2,-27,3)), the slope tap
    /// (1,-29,2) (a floor lever into DAY-1's repeater (1,-29,3)), R's detector (4,-28,4). G16 SLEPT,
    /// G17 SLEPT_ONCE, G18 ARMED70, G19 OPEN, G20 the shutter / the park's release, G21 the release
    /// (the edge of P_STRING AND SLEPT_ONCE, parity-free, one-shot per visit), G3 IN-STOP_l, G4 the
    /// slope taps (REL + 43 / 41 / 33 AND IN-STOP_l, held 10 gt), G12 RETURN_x's set at the tap
    fn a_stop_step(&mut self, w: &mut redsim::world::World, now: i64, bed: bool) -> Result<(), String> {
        use redsim::world::flags;
        const ARM: [i32; 3] = [-2, -27, 0];
        const F_OUT: [i32; 3] = [-5, -26, 7];
        const PARK_IN: [i32; 3] = [3, -27, 3];
        const TAP: [i32; 3] = [1, -29, 2];
        const RDET: [i32; 3] = [4, -28, 4];
        let lever_to = |w: &mut redsim::world::World, c: [i32; 3], on: bool| -> Result<(), String> {
            let s = w.get(dbp(c[0], c[1], c[2])).to_string();
            if !s.contains("lever") {
                return Err(format!("(A)'s stand-in cell ({},{},{}) holds {s}, not a lever", c[0], c[1], c[2]));
            }
            set(w, c, &s.replace(if on { "powered=false" } else { "powered=true" }, if on { "powered=true" } else { "powered=false" }), flags::ALL)?;
            // (the attached block's neighbours: updated by `set` for every lever write, LeverBlock's
            // updateNeighbours 133-139)
            Ok(())
        };
        // ARM's edge: SLEPT_ONCE reset (G17), the one-shot cleared (G21), ARMED70's hopper started (G18)
        let arm = powered(w, ARM);
        if arm && !self.a_arm_prev {
            self.a_arm_at = Some(now);
            self.a_slept_once = false;
            self.a_released = false;
            self.a_park_rest = None;
            self.a_log.push(format!("t {now}: ARM's edge (-2,-27,0)"));
        }
        // (G1 17:33) the park rest against PARKED's timer (ARM + 52): the tick the empty cart first rests on
        // the park (1,-27,3), its margin printed per visit (a rest after ARM + 52 would be held unreleased)
        if self.a_park_rest.is_none()
            && let Some(a) = self.a_arm_at
            && w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [1, -27, 3] && e.passengers.is_empty() && {
                let v = dpos(e.delta_movement);
                v.x.abs() + v.y.abs() + v.z.abs() < 1e-3
            })
        {
            self.a_park_rest = Some(now);
            self.a_park_log.push(format!("ARM t {a}: the park rest t {now} (ARM + {}), PARKED at ARM + 52: margin {}", now - a, a + 52 - now));
        }
        self.a_arm_prev = arm;
        // the bed-head observer's pulse at each occupancy change (a lie-down, a wake): rises 2 gt after
        // the bed head's change (tick-exact 26.3); SLEPT toggled 4 gt after the pulse (G16: + 6 in all),
        // SLEPT_ONCE set 2 gt after it (G17: + 4), both only after ARM (day-1's propagation delays rev
        // 15:42, DESIGNED)
        if bed != self.bed_prev {
            self.a_bed_change_at = Some(now);
            if self.a_arm_at.is_some() {
                self.a_obs_due.push((now + 6, true));
                self.a_obs_due.push((now + 4, false));
            }
            if bed {
                self.a_lie_at = Some(now);
                self.nap_events.push(format!("t {now}: lie-down (the bed occupied; the observer's pulse at t {}: SLEPT toggles at t {}, SLEPT_ONCE at t {})", now + 2, now + 6, now + 4));
            } else {
                self.nap_events.push(format!("t {now}: the wake edge (the bed freed; the observer's pulse at t {}: SLEPT toggles at t {})", now + 2, now + 6));
            }
        }
        let obs: Vec<(i64, bool)> = self.a_obs_due.iter().filter(|x| x.0 == now).copied().collect();
        self.a_obs_due.retain(|x| x.0 != now);
        for (_, toggle) in obs {
            if toggle {
                self.a_slept = !self.a_slept;
            } else if !self.a_slept_once {
                self.a_slept_once = true;
                // (G1 15:40) a stale P_STRING press from before the lie-down excluded: the gap from
                // P_STRING's last release (its last press + ~10, the string's re-check) to SO's set
                let gap = self.a_pstring_last_raw.map(|t| now - (t + 10));
                self.a_so_gaps.push(gap.unwrap_or(i64::MAX));
            }
        }
        // G22 P_STRING: pressed in the tick a villager on foot's box meets it (its cell (-2,-26,3)),
        // released ~10 gt after it leaves (the string's re-check)
        let p_raw = w.entities.list.iter().any(|e| {
            !e.removed && e.vehicle.is_none() && matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager)) && {
                let p = dpos(e.pos);
                (p.x - (-1.5)).abs() < 0.8 && (p.z - 3.5).abs() < 0.8 && p.y < -25.0 && p.y + 1.95 > -26.0
            }
        });
        if p_raw {
            self.a_pstring_last_raw = Some(now);
        }
        let pstring = p_raw || self.a_pstring_last_raw.is_some_and(|t| now - t <= 10);
        // P_STRING's continuous press: since when it has held without a fall
        if pstring {
            if self.a_press_start.is_none() || !self.a_pstring_prev {
                self.a_press_start = Some(now);
            }
        } else {
            self.a_press_start = None;
        }
        // G18 / G19 (ironspec 15:58 / 16:00): OPEN set = A70 AND SLEPT_ONCE AND NOT P_STRING (the AND 2 +
        // the edge 2 + the latch 2 = 6), reset = RLS OR P_STRING; G20 the shutter = NOT OPEN through a
        // torch, 2 gt
        let armed70 = !self.a_released && self.a_arm_at.is_some_and(|a| now >= a + 70);
        let cond = armed70 && self.a_slept_once && !pstring;
        if cond && !self.a_open_cond_prev && !self.a_open {
            self.a_open = true;
            self.a_open_at = Some(now + 6);
            self.a_shutter_writes.push((now + 8, false));
            self.nap_events.push(format!("t {now}: A70 AND SLEPT_ONCE AND NOT P_STRING: OPEN set at t {}, the window opens at t {} (F)", now + 6, now + 8));
        }
        self.a_open_cond_prev = cond;
        if self.a_open && pstring && !self.a_pstring_prev {
            self.a_open = false;
            self.a_shutter_writes.push((now + 2, true));
            self.nap_events.push(format!("t {now}: P_STRING: OPEN reset, close at t {}", now + 2));
        }
        // G21 (ironspec 16:00 + G1 16:02): the release keys on SUSTAINED PRESENCE: P_STRING continuous
        // from the latest observer pulse L (a bed change + 2) through L + 12, with PARKED (ARM + 52: the
        // worst measured park 46 + the world-open term + 4), by L + 50; one-shot per visit; PARK = RLS + 8;
        // OPEN reset at RLS (close + 2); SLEPT reset
        let parked = self.a_arm_at.is_some_and(|a| now >= a + 52);
        let pulse_l = self.a_bed_change_at.map(|b| b + 2);
        let rls = pstring && parked && pulse_l.is_some_and(|l| self.a_press_start.is_some_and(|p| p <= l) && now >= l + 12 && now <= l + 50);
        if rls && !self.a_rls_prev && !self.a_released {
            self.a_released = true;
            let was_open = self.a_open;
            self.a_open = false;
            self.a_slept = false;
            if was_open {
                self.a_shutter_writes.push((now + 2, true));
            }
            // (ironspec 17:35: PARK = RLS + p, p = 4 PROVISIONAL)
            self.release_at = Some(now + 4);
            self.a_rls_log.push(format!("t {now}: RLS (P_STRING held since t {} through the pulse t {} + 12, PARKED)", self.a_press_start.unwrap_or(now), pulse_l.unwrap_or(now)));
            self.nap_events.push(format!("t {now}: RLS (P_STRING continuous from the pulse t {} through + 12, PARKED){}: the park's release at t {}", pulse_l.unwrap_or(now), if was_open { format!(", OPEN reset, close at t {}", now + 2) } else { String::new() }, now + 4));
        }
        self.a_rls_prev = rls;
        self.a_pstring_prev = pstring;
        let due: Vec<(i64, bool)> = self.a_shutter_writes.iter().filter(|x| x.0 == now).copied().collect();
        self.a_shutter_writes.retain(|x| x.0 != now);
        for (_, on) in due {
            lever_to(w, F_OUT, on)?;
            self.nap_events.push(format!("t {now}: the shutter's stand-in lever (-5,-26,7) {} (the window {})", if on { "ON" } else { "OFF" }, if on { "SHUT: close" } else { "OPEN: F" }));
        }
        if self.release_at == Some(now) {
            lever_to(w, PARK_IN, true)?;
            self.release_at = None;
            self.nap_events.push(format!("t {now}: the stop's release (the park's drive (3,-27,3) on)"));
        }
        // R's rise (the rider-keyed string's edge, G1 / G3): the park's drive off, ARM's latches
        // reset, IN-STOP of the lane whose napper passed reset
        let rdet = powered(w, RDET);
        if rdet && !self.rdet_prev {
            let rider = w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && {
                    let p = dpos(e.pos);
                    (p.x - 4.5).abs() < 0.8 && (p.z - 4.5).abs() < 0.8 && p.y >= -28.1 && p.y < -26.0
                })
                .find_map(|e| e.passengers.first().copied());
            if w.get(dbp(PARK_IN[0], PARK_IN[1], PARK_IN[2])).to_string().contains("powered=true") {
                lever_to(w, PARK_IN, false)?;
            }
            if let Some(r) = rider {
                for k in 0..3 {
                    if self.in_stop[k] && self.in_stop_rider[k].is_none_or(|v| v == r) {
                        self.in_stop[k] = false;
                        self.in_stop_rider[k] = None;
                        self.nap_events.push(format!("t {now}: IN-STOP reset, lane x {} (#{r} passed R)", LANES[k]));
                    }
                }
                self.a_arm_at = None;
                self.a_open = false;
                self.a_slept = false;
                self.nap_events.push(format!("t {now}: R's string (a ridden cart, #{r}): the park's drive off, the stop re-armed"));
            } else {
                self.nap_events.push(format!("t {now}: R's detector pressed by an EMPTY cart: no reset (rider-keyed)"));
            }
        }
        // G4 the slope taps: lane l's train at REL + 43 / 41 / 33 AND IN-STOP_l (the AND's 2 gt: the
        // tap lever written at REL + phase + 2, DAY-1's slope repeater adds its own 2), held 10 gt;
        // G12 RETURN_l set at a tap that LAUNCHES (IN-STOP_l AND the tap, with the napper's cart at rest
        // on the slope hold (1,-28,4) to launch: onechunk 16:49's reading; a latch set at every tap
        // made two lanes' bits overlap while the next napper slept, and G5 withheld the hold's pulse)
        if let Some(rel) = self.rel {
            for k in 0..3 {
                if now == rel + [43, 41, 33][k] + 2 && self.in_stop[k] {
                    lever_to(w, TAP, true)?;
                    self.tap_off = Some(now + 10);
                    let launches = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [1, -28, 4] && !e.passengers.is_empty());
                    if launches && self.return_bits[k].is_none() {
                        self.return_bits[k] = Some(now);
                    }
                    self.nap_events.push(format!("t {now}: the slope tap, lane x {} (REL {rel} + {}){}", LANES[k], now - rel, if launches { ": it launches the napper's cart, RETURN set" } else { ": no cart on the slope hold (no launch, RETURN unchanged)" }));
                }
            }
        }
        if self.tap_off == Some(now) {
            lever_to(w, TAP, false)?;
            self.tap_off = None;
        }
        Ok(())
    }

    /// (day-1's canonical gates rev 15:37, G12–G14; 7b90c3f1 15:35's tau and s from returnbench.rs
    /// 1030bb50c: s = 4, tau_K(-1 / 3 / 7) = 94 / 109 / 125, tau_J(-1) = 102, tau_J(3) = 117) F4(a):
    /// RETURN_x cleared by string_x's rise (own lane) or at T_ret 450 (a counted fault); at T_d (the
    /// napper's first end-of-tick in (9,0,7) after the hold) the RETURN bits sampled: exactly one set,
    /// lane x: K(x) on over [T_d + tau_K - 12, T_d + tau_K + 8] (its lever written at the start + 4 and
    /// the end + 6: day-1's driver latency) and each J west of K(x) held off over [T_d + tau_J - 12,
    /// T_d + tau_J + 8] (written at the start + 6 and the end + 4)
    fn f4a_step(&mut self, w: &mut redsim::world::World, now: i64) -> Result<(), String> {
        use redsim::world::flags;
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        // string_x: a ridden cart on its siding's R (x-2,24,7)
        for k in 0..3 {
            let sx = LANES[k] - 2;
            let s_on = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [sx, 24, 7]);
            if s_on && !self.string_prev[k] && self.return_bits[k].is_some() {
                self.return_bits[k] = None;
                self.nap_events.push(format!("t {now}: RETURN cleared, lane x {} (string_l's rise)", LANES[k]));
            }
            if let Some(t) = self.return_bits[k]
                && now >= t + 450
            {
                self.return_bits[k] = None;
                self.f4_faults.push(format!("t {now}: RETURN timed out, lane x {} (set t {t}, T_ret 450: a counted fault)", LANES[k]));
            }
            // G7: AWAY_l reset at string_l's FALLING edge after the return (the returning napper leaving
            // R_l): the string's ~10-gt release lag + the edge 2 + the latch 2 (day-1's delays rev 15:42)
            if !s_on && self.string_prev[k] && self.away[k] {
                self.away_reset_due[k] = Some(now + 14);
            }
            if self.away_reset_due[k] == Some(now) {
                self.away_reset_due[k] = None;
                self.away[k] = false;
                self.nap_events.push(format!("t {now}: AWAY reset, lane x {} (string_l's fall at t {} + 14: the napper's rejoin)", LANES[k], now - 14));
            }
            self.string_prev[k] = s_on;
        }
        // T_d: the napper's first end-of-tick in (9,0,7) after the hold (9,0,8)
        let in_hold = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [9, 0, 8]);
        let out_n = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && cell(e.pos) == [9, 0, 7]);
        if self.hold_cell_prev && !in_hold && out_n {
            let set: Vec<usize> = (0..3).filter(|&k| self.return_bits[k].is_some()).collect();
            if set.len() == 1 {
                let x = set[0];
                let td = now;
                let tk = [94, 109, 125][x];
                let kc = [LANES[x] - 2, 26, 9];
                self.f4_writes.push((td + tk - 12 + 4, kc, true));
                self.f4_writes.push((td + tk + 8 + 6, kc, false));
                let mut js: Vec<(i64, [i32; 3])> = Vec::new();
                if x >= 1 {
                    js.push((102, [-1, 26, 9]));
                }
                if x == 2 {
                    js.push((117, [3, 26, 9]));
                }
                for (tj, jc) in &js {
                    self.f4_writes.push((td + tj - 12 + 6, *jc, false));
                    self.f4_writes.push((td + tj + 8 + 4, *jc, true));
                }
                self.k_windows.push((now, x, LANES[x] - 2));
                self.f4_windows.push(format!("T_d {td}: lane x {} sampled: K({}) on t {}..{} (writes {} / {}){}", LANES[x], LANES[x] - 2, td + tk - 12, td + tk + 8, td + tk - 8, td + tk + 14, js.iter().map(|(tj, jc)| format!("; J({}) held off t {}..{}", jc[0], td + tj - 12, td + tj + 8)).collect::<String>()));
                self.nap_events.push(format!("t {now}: T_d (the napper off the hold into (9,0,7)): the sampled lane x {}: K({})'s window keyed", LANES[x], LANES[x] - 2));
            } else {
                self.f4_faults.push(format!("t {now}: T_d with {} RETURN bits set ({:?}): no K window (a fault)", set.len(), set.iter().map(|&k| LANES[k]).collect::<Vec<_>>()));
            }
        }
        self.hold_cell_prev = in_hold;
        let due: Vec<(i64, [i32; 3], bool)> = self.f4_writes.iter().filter(|x| x.0 == now).copied().collect();
        self.f4_writes.retain(|x| x.0 != now);
        for (_, c, on) in due {
            set(w, c, &lever(on), flags::ALL)?;
            self.k_fired.push((now, c[0], on));
            self.nap_events.push(format!("t {now}: F4(a)'s write: ({},{},{}) {}", c[0], c[1], c[2], if on { "on" } else { "off" }));
        }
        Ok(())
    }

    fn level(&mut self, w: &mut redsim::world::World, c: [i32; 3], target: [i32; 3], want: bool) -> Result<(), String> {
        let on = self.levels.contains_key(&c);
        if want && !on {
            let base = w.get(dbp(c[0], c[1], c[2])).to_string();
            input_on(w, c, target)?;
            self.levels.insert(c, base);
        } else if !want && on {
            let base = self.levels.remove(&c).unwrap_or_else(|| "minecraft:air".to_string());
            input_off(w, c, &base)?;
        }
        Ok(())
    }

    fn start(&mut self, rel: i64) {
        // (day-1's G8) RE_ARMED reset at REL; (ironspec 17:29) BATCH_DONE's SEEN latches with it
        self.re_armed = false;
        self.seen = [None; 3];
        // (--ring-timed) each batch's REL and its pick (set when the pick is decided)
        self.rt_batches.push((rel, None));
        self.busy = true;
        self.rel = Some(rel);
        // (path (a): lane 7's slope at L7 = REL + 96, queued: it falls after the next start)
        if self.l7_slope {
            self.l7_on.push(rel + 96);
        }
        self.armed = [false; 3];
        self.arm3 = None;
        // (a restart from a stuck cycle begins clean: ironspec 19:29 (c))
        self.shut_done = [false; 3];
        self.last_start = Some(rel - 6);
        // (ironspec 19:47: B held 100 gt from the start)
        self.b_until = rel - 6 + 100;
        self.rels.push(rel);
        // (day-1's picker, 14:46 / 16:14, EMULATED: the counter C steps on each release (C := 0 at
        // the morning RISE, C := 19 at EIGHT when C >= 136 or C <= 15); a pick when C mod 10 is 1 /
        // 4 / 7, lanes A / B / C = x -1 / 3 / 7 (7b90c3f1), gated by HELD and the lane's away
        // latch; DAY is sampled at the picked cart's row arrival; with divert sampling (i), HELD too)
        if self.wayin {
            if self.counter_pending {
                // (G1's [6]) the first release's C from its own day time
                self.counter_pending = false;
                let d = self.day_now;
                let in_window = self.counter_window.is_some_and(|(e8, rise)| {
                    let dd = d.rem_euclid(24_000);
                    dd >= e8 && dd < rise
                });
                if in_window {
                    self.rel_count = counter_c(d.rem_euclid(24_000)) - 1;
                    self.counter_log.push(format!("C {} at the first release (day {d}; day-1's built counter: 141 + ceil((d - 12,540) / 100), inside EIGHT..RISE {:?})", self.rel_count + 1, self.counter_window));
                } else {
                    self.counter_unmodelled = Some(d);
                    self.counter_log.push(format!("the first release at day {d}: outside EIGHT..RISE {:?} (day-1's counter: RISE loads C 0, trigger 2 loads 19), not modelled (FAIL)", self.counter_window));
                }
            }
            self.rel_count += 1;
            self.counter_log.push(format!("{}:{}", self.day_now, self.rel_count));
            let u = self.rel_count.rem_euclid(10);
            // (day-1 22:06: picks only inside the counter's window C 141..221, opening at day time
            // 12,431..12,549 on a normal day and closing near 20,500; no pick after C 221)
            // (day-1 06:07 PT, ctlcounter.rs :156: the window (141, 221), half-open, so picks at C
            // 141..220; ironspec's E1)
            if (u == 1 || u == 4 || u == 7) && (141..=220).contains(&self.rel_count) {
                let k = ((u - 1) / 3) as usize;
                if (self.q_held && !self.held_at_divert) || self.away[k] || self.pick_cart.is_some() {
                    self.picks_gated += 1;
                    let g = if self.q_held && !self.held_at_divert { 0 } else if self.away[k] { 1 } else { 2 };
                    self.gated_by[g] += 1;
                    let gn = ["HELD (a napper in flight)", "the lane's away latch", "a picked cart not yet routed"][g];
                    self.note(rel, format!("the picker: lane x {}'s pick at C {} gated by {gn}", LANES[k], self.rel_count));
                    let verdict = match g {
                        0 => format!("HELD (held by #{})", self.held_by.map_or("?".to_string(), |v| v.to_string())),
                        1 => "the away latch".to_string(),
                        _ => "unrouted".to_string(),
                    };
                    self.pick_opps.push((rel, self.rel_count, k, verdict));
                } else {
                    // (G1's note) no pick may still be pending at a decision: an overwrite fails
                    if let Some(p) = self.pick {
                        self.pick_overwrites.push(format!("t {rel}: lane x {}'s pick decided while lane x {}'s was still pending", LANES[k], LANES[p]));
                    }
                    self.pick = Some(k);
                    if let Some(b) = self.rt_batches.last_mut() {
                        b.1 = Some(k);
                    }
                    self.pick_opps.push((rel, self.rel_count, k, "picked".to_string()));
                }
            }
        }
    }

    /// The idle state at the start (the split's latches): J(-1) on (into lane -1), J(3) off
    /// (through), J(7) on (into lane 7).
    fn init(&mut self, w: &mut redsim::world::World) -> Result<(), String> {
        use redsim::world::flags;
        let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
        self.l7_slope = w.get(dbp(-4, 12, 7)).to_string().contains("powered_rail");
        // (G1's [3]) path (a) exactly: a plain rail at (5,0,7), and lane 7's braked slope laid; the
        // old path: the detector rail at (5,0,7) and no slope; anything else stops the run
        let m = w.get(dbp(5, 0, 7)).to_string();
        self.path_a = if m.starts_with("minecraft:rail[") {
            true
        } else if m.starts_with("minecraft:detector_rail[") {
            false
        } else {
            return Err(format!("(5,0,7) is {m}: neither path (a)'s plain rail nor the old path's detector rail"));
        };
        if self.path_a != self.l7_slope {
            return Err(format!("path (a) {} at (5,0,7) but lane 7's braked slope (-4,12,7) {}: the two checks disagree", self.path_a, if self.l7_slope { "laid" } else { "absent" }));
        }
        // (day-1's canonical gates rev 15:42; 58f770cc 15:28's stand-ins, declared overrides) the (A)
        // stop composed: E's activator at module (-2,-27,1); its three drives laid as the bench lays
        // them where the record keeps them empty: F's output (-5,-26,7) a floor lever ON (the window
        // shut), the park's drive (3,-27,3) a wall lever facing east OFF on the conductor (2,-27,3),
        // the slope tap (1,-29,2) a floor lever OFF into DAY-1's repeater (1,-29,3)
        self.a_stop = w.get(dbp(-2, -27, 1)).to_string().contains("activator_rail");
        if self.a_stop && !self.ring_timed {
            // (day-1's G11, ironspec 15:37) the design controller's outbound is the no-string ring:
            // onechunk's clock-locked L's (t16's passage table) and k 1 (tick-exact 15:31, on the
            // convention, within the module's chunk) unless --ring-timed / --ring-k name others
            self.ring_timed = true;
            self.ring_l = [118, 134, 154, 183, 152];
            self.ring_k = 1;
        }
        // (day-1's G8) RE_ARMED starts set (the first batch's ring at rest is armed)
        self.re_armed = self.a_stop;
        if self.a_stop {
            for (c, s) in [
                ([-5, -26, 7], "minecraft:lever[face=floor,facing=north,powered=true]"),
                ([3, -27, 3], "minecraft:lever[face=wall,facing=east,powered=false]"),
                ([1, -29, 2], "minecraft:lever[face=floor,facing=north,powered=false]"),
            ] {
                let was = w.get(dbp(c[0], c[1], c[2])).to_string();
                if was != "minecraft:air" && !was.contains("lever") {
                    return Err(format!("the (A) stop's stand-in cell ({},{},{}) holds {was}, not air", c[0], c[1], c[2]));
                }
                set(w, c, s, flags::ALL)?;
            }
        }
        for (k, x) in LANES.iter().enumerate() {
            // (M16bw's side drive: a repeater at (x-1,0,0) facing west fed by the riser dust
            // (x-2,0,0), the step (x-3,-1,0) and the feed from the north (x-3,-1,-1) or the west
            // (x-4,-1,0); ironspec 17:49 / 18:06: no solid stand-in in place, so the release is a
            // floor lever in the riser's cell on its glass (x-2,-1,0) for 16 gt, the dust restored
            // after: the repeater's own line side, with the built cell's golem and path properties)
            let at = |c: [i32; 3]| w.get(dbp(c[0], c[1], c[2])).to_string();
            self.drive_cell[k] = if at([x - 1, 0, 0]).contains("repeater") && at([x - 3, -1, 0]).contains("redstone_wire") {
                if !at([x - 2, 0, 0]).contains("redstone_wire") {
                    return Err(format!("M16bw's side drive at lane x {x}: no riser dust at ({},0,0)", x - 2));
                }
                Some([x - 2, 0, 0])
            } else {
                None
            };
            let c = self.drive_cell[k].unwrap_or([*x, -1, 0]);
            self.support_base[k] = Some(at(c));
        }
        for (x, on) in [(-1, true), (3, false), (7, true)] {
            if !w.get(dbp(x, 26, 9)).to_string().contains("lever") {
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
    /// (ironspec 16:08) the stamp's oldest age at any REST tick per position class (lane queue, station
    /// hold / pocket, split row, lift, the queue gate, the way-in / stop, other): (age, tick, cell)
    class_peak: [(i64, i64, [i32; 3]); 7],
    /// (G1 16:37 (2)) the previous tick's cell and the passes through the lift foot (-4,2,7)
    prev_cell: Option<[i32; 3]>,
    foot_passes: u32,
    /// (G1 16:47) VILLAGER_HOSTILES' scan phase mod 20
    hostile_phase: Option<i64>,
    /// (tick-exact 17:03 (iii)) NEAREST_LIVING_ENTITIES' scan phase mod 20 (the sight list's)
    living_phase: Option<i64>,
    calm_rest_ticks: i64,
    over_1000: i64,
    clears: Vec<String>,
    home_lost: Vec<String>,
    home_at_start: Option<[i32; 3]>,
    home_at_dawn: Option<Option<[i32; 3]>>,
    prev_stamp: Option<i64>,
    prev_home: Option<[i32; 3]>,
    prev_target: Option<[f64; 3]>,
    /// (G1 07:28 PT) the previous tick's WALK_TARGET close-enough distance (MoveToTargetSink's
    /// reached test: the target block within it, Manhattan).
    prev_target_close: Option<i32>,
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
    /// (ironspec 12:13) the lane whose pocket, drop, H or slopes the rider last rode (x = the lane,
    /// y 0..22, z 0..6).
    lane: Option<i32>,
    /// (a napper, ironspec 13:44 / 14:41 / 14:47) its stamp's age at its queue arrival, at R and
    /// at its first clear after R; the queue arrival's age; its first call back in its lane.
    nap_stamps: Vec<String>,
    queue_age: Option<Option<i64>>,
    nap_call: Option<String>,
    /// (ironspec 12:48) who set the memory last; a bystander's stamp (tick, position) pending its
    /// next summon.
    gdr_setter: Option<&'static str>,
    bystander_stamp: Option<(i64, [f64; 3], &'static str)>,
    /// (ironspec 20:07 PT) the tick its panic began (PANIC entered), and last tick's panic.
    panic_since: Option<i64>,
    panic_prev: bool,
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
    /// (ironspec 06:24 / 06:31 real, G1: the decisive bar) the REST spell in progress: its start,
    /// its entry row, whether it counts against the bar (a lane rider outside its own trip), whether
    /// it entered at a design position (riding a cart), and the first HOME path seen in it
    /// (complete or partial).
    rest_spell: Option<(i64, String, bool, bool)>,
    rest_path: Option<bool>,
    /// (ironspec 06:36 real, the runtime stamp bar) the oldest CANT_REACH stamp per day (the day
    /// index of the world's day time): its age, tick and cell.
    oldest_by_day: BTreeMap<i64, (i64, i64, [i32; 3])>,
    /// (G1: each napper's excluded trip as numbers) its trips, pick to R: (start, end).
    trips: Vec<(i64, Option<i64>)>,
    in_trip_prev: bool,
    /// (ironspec 07:10 real (2)) its night ticks by activity: PANIC, REST, other.
    night_acts: [i64; 3],
    /// (G1, farmopt 06:12 PT) its activity at each tick of day time 11,990..=12,040 (the schedule
    /// switch): P (PANIC), R (REST), . (other), - (not seen).
    switch_strip: String,
    /// (ironspec 06:55 platform (3)) its first REST tick in that window: the tick, the cell and the
    /// scarer sight then.
    switch_rest_at: Option<String>,
    /// (G1 07:28 PT) that row waits for the clip and the covered / dark call later in the tick.
    switch_pending: bool,
    /// (G1 X40) that REST tick's covered / dark call.
    switch_covered: Option<bool>,
    /// (ironspec 07:30 real (1), G1) every CANT_REACH arming and its clearing: (armed, cleared,
    /// how); the arming in progress.
    arm_open: Option<i64>,
    arm_pairs: Vec<(i64, Option<i64>, String)>,
    /// (ironspec 06:22 platform + G1, the air-supply bar) its least air supply in each lift transit
    /// (a stretch of ticks with its air below the full 300): (from, least, at tick, cell); the
    /// transit in progress.
    air_open: Option<(i64, i32, i64, [i32; 3])>,
    air_transits: Vec<(i64, i32, i64, [i32; 3])>,
    /// (ironspec 06:31 / 06:33 platform) its last REST entry and its last H visit (the tick), and
    /// whether it sat on H last tick: each entry and visit prints what cleared its stamp since the
    /// previous one.
    last_rest_entry: Option<i64>,
    last_h_visit: Option<i64>,
    on_h_prev: bool,
    h_rows: Vec<String>,
    /// (ironspec 06:34 platform, the PANIC -> REST switch) in the REST spell in progress: the tick its
    /// WALK_TARGET first went empty (the sink stopped), the tick HOME was first targeted, and
    /// whether a stamp was armed in it.
    spell_sink_stop: Option<i64>,
    spell_home_at: Option<i64>,
    spell_armed: bool,
    /// (ironspec 06:37 platform, G1) the stamp's age at the spell's first HOME-targeting tick (the
    /// first tick SetWalkTargetFromBlockMemory ran): Some(None) unarmed.
    spell_home_stamp: Option<Option<i64>>,
    /// (ironspec 06:40 platform, G1 1c) each wake: its row, kept open until the napper's next REST
    /// entry (the waker's flee target, the path, the stamp, and whether it cleared by then).
    sleeping_prev: bool,
    wake_open: Option<(i64, String)>,
    wake_rows: Vec<String>,
    /// (ironspec 06:55 platform (2)) the REST spell in progress: its entry cell and whether it began
    /// with a WALK_TARGET held.
    spell_cell: [i32; 3],
    spell_held: bool,
}

/// (ironspec 20:40 PT, the 4 undeclared T sources (2b): the (a) feed stack's dust under lane -1's
/// T) A direct bench row on the module as laid: with the T (-3,25,9)'s lever (-3,26,9) off, then
/// on, the stack's input forced on (a lever at (-5,22,9) driving its repeater (-4,22,9)) and off,
/// 12 gt each; after every step the dust's power and the T's stored shape against its power's
/// settle (bar: the T always in its intended shape).
fn stack_bench(run: &mut Run) -> Result<String, String> {
    use redsim::world::flags;
    let tp = dbp(-3, 25, 9);
    let lever = |on: bool| format!("minecraft:lever[face=ceiling,facing=north,powered={on}]");
    let mut rows = Vec::new();
    let mut bad = 0;
    let look = |run: &Run, what: &str, rows: &mut Vec<String>, bad: &mut i32| {
        let dust = run.world.get(dbp(-3, 23, 9)).to_string();
        let dp = dust.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("-").to_string();
        match redsim::rails::junction_at(&run.world, tp) {
            Some(j) => {
                let want = if j.signal { j.powered } else { j.unpowered };
                if j.stored != want {
                    *bad += 1;
                }
                rows.push(format!("{what}: the dust power {dp}, the T {:?} (signal {}, its settle {:?}: {})", j.stored, j.signal, want, if j.stored == want { "intended" } else { "NOT intended" }));
            }
            None => {
                // (G1's [2]) a missing T is a failure
                *bad += 1;
                rows.push(format!("{what}: no T at (-3,25,9) (FAIL)"));
            }
        }
    };
    let lever_cell = run.world.get(dbp(-3, 26, 9)).to_string();
    if !lever_cell.contains("lever") {
        return Err(format!("--stack-bench: (-3,26,9) holds {lever_cell}, not the T's lever"));
    }
    if !run.world.get(dbp(-4, 22, 9)).to_string().contains("repeater") {
        return Err("--stack-bench: no feed stack repeater at (-4,22,9)".to_string());
    }
    for j in [false, true] {
        set(&mut run.world, [-3, 26, 9], &lever(j), flags::ALL)?;
        for _ in 0..4 {
            run.tick();
        }
        look(run, &format!("the lever {}", if j { "on" } else { "off" }), &mut rows, &mut bad);
        let base = run.world.get(dbp(-5, 22, 9)).to_string();
        input_on(&mut run.world, [-5, 22, 9], [-4, 22, 9])?;
        for _ in 0..12 {
            run.tick();
        }
        look(run, "  the stack's input on", &mut rows, &mut bad);
        input_off(&mut run.world, [-5, 22, 9], &base)?;
        for _ in 0..12 {
            run.tick();
        }
        look(run, "  the stack's input off", &mut rows, &mut bad);
    }
    Ok(format!(
        "the feed stack's bench (ironspec 20:40 PT: the dust (-3,23,9) through both power states with the T (-3,25,9)'s lever in each; bar: the T always in its power's settle): {}; the T not in its intended shape {bad} times ({})\n",
        rows.join("; "),
        if bad == 0 { "PASS" } else { "FAIL" }
    ))
}

/// (day-1's R1, 19:19) The queue gate's direct bench on the module as laid: for each state of its
/// line ends (REST_l, DAY, NOT-ready), the levers set, 10 gt, then every gate cell's state and the
/// queue rail's power (expected: the rail powered only for REST_l, no DAY, no NOT-ready).
/// (ironspec 21:51 PT, the mechanism lines' benches and the emulation's validation rows) Each line
/// whose end f1q2-run drives: its input switched on at a bench tick and off 30 gt later, the ticks
/// until its measured cell powers and unpowers; where the emulation times the input by an assumed
/// latency (the siding's R at D + k_sw + 8: 8 gt through the (a) feed stack, 2 through the siding
/// repeater; the hold pulse's rail 2 gt after its input), the measured latency against it, and the
/// 1-gt-shifted control (the same check against latency + 1 must fail).
fn line_bench(run: &mut Run, by: i32) -> Result<String, String> {
    let mut rows = Vec::new();
    let mut bad = 0;
    let mut lines: Vec<(String, [i32; 3], [i32; 3], [i32; 3], Option<i64>)> = Vec::new();
    for x in LANES {
        let sx = x - 2;
        let stack = run.world.get(dbp(sx - 1, 22, 9)).to_string().contains("repeater");
        let (cell, target, lat) = if stack { ([sx - 2, 22, 9], [sx - 1, 22, 9], 8) } else { ([sx, 23, 9], [sx, 23, 8], 2) };
        lines.push((format!("lane x {x}'s siding feed ({})", if stack { "the (a) feed stack" } else { "the siding repeater" }), cell, target, [sx, 24, 7], Some(lat)));
    }
    lines.push(("the hold pulse (line 3)".to_string(), [7, -1, 8], [8, -1, 8], [9, 0, 8], Some(2)));
    for x in LANES {
        let at = |c: [i32; 3]| run.world.get(dbp(c[0], c[1], c[2])).to_string();
        if at([x - 1, 0, 0]).contains("repeater") {
            lines.push((format!("lane x {x}'s pocket release (M16bw)"), [x - 2, 0, 0], [x - 1, 0, 0], [x, 0, 0], None));
        }
        if at([x + 1, 17, 1]).contains("repeater") {
            lines.push((format!("lane x {x}'s R line"), [x + 2, 17, 1], [x + 1, 17, 1], [x, 17, 1], None));
        }
    }
    lines.push(("R1's gate, REST_l with DAY off and RS_ready".to_string(), [6, by + 15, -1], [6, by + 15, -2], [9, by + 13, -4], None));
    for (name, cell, target, meas, lat) in lines {
        let base = run.world.get(dbp(cell[0], cell[1], cell[2])).to_string();
        input_on(&mut run.world, cell, target)?;
        let (mut rise, mut fall) = (None, None);
        for t in 1..=40i64 {
            run.tick();
            if rise.is_none() && powered(&run.world, meas) {
                rise = Some(t);
            }
            if t == 30 {
                input_off(&mut run.world, cell, &base)?;
            }
            if t > 30 && rise.is_some() && fall.is_none() && !powered(&run.world, meas) {
                fall = Some(t - 30);
            }
        }
        for _ in 0..20 {
            run.tick();
            if rise.is_some() && fall.is_none() && !powered(&run.world, meas) {
                fall = Some(-1);
            }
        }
        let check = match (lat, rise) {
            (Some(l), Some(r)) => {
                let ok = r == l;
                let control_caught = r != l + 1;
                if !ok {
                    bad += 1;
                }
                format!("; the emulation's latency {l}: {} (the 1-gt-shifted control {l} + 1: {})", if ok { "PASS" } else { "FAIL" }, if control_caught { "caught" } else { "NOT caught (FAIL)" })
            }
            (Some(l), None) => {
                bad += 1;
                format!("; the emulation's latency {l}: FAIL (never powered)")
            }
            (None, _) => " (information)".to_string(),
        };
        rows.push(format!(
            "{name}: input ({},{},{}) -> ({},{},{}) powered after {}, unpowered {} after the input off{check}",
            cell[0], cell[1], cell[2], meas[0], meas[1], meas[2],
            rise.map_or("never (in 30 gt)".to_string(), |r| format!("{r} gt")),
            match fall { Some(f) if f >= 0 => format!("{f} gt"), Some(_) => "late (over 10 gt)".to_string(), None => "-".to_string() }
        ));
    }
    Ok(format!("the mechanism lines' benches (ironspec 21:51 PT: each emulated line's built part; the validation rows where the emulation assumes a latency): {}; validation failures {bad} ({})\n", rows.join("; "), if bad == 0 { "PASS" } else { "FAIL" }))
}

fn gate_bench(run: &mut Run, by: i32) -> Result<String, String> {
    let mut rows = Vec::new();
    let mut bad = 0;
    let cells: [([i32; 3], &str); 8] = [
        ([6, by + 15, -2], "N's comparator"),
        ([6, by + 15, -3], "N's dust"),
        ([5, by + 14, -3], "the line's dust"),
        ([5, by + 13, -4], "the line's lower dust"),
        ([6, by + 13, -4], "the line's repeater"),
        ([7, by + 13, -4], "R1's comparator"),
        ([8, by + 13, -4], "R1's repeater"),
        ([9, by + 13, -4], "the queue rail"),
    ];
    for (rest, day, notready) in [(true, false, false), (true, true, false), (true, false, true), (false, false, false)] {
        let ends = [([6, by + 15, -1], [6, by + 15, -2], rest), ([4, by + 15, -2], [5, by + 15, -2], day), ([8, by + 13, -5], [7, by + 13, -5], notready)];
        let mut bases = Vec::new();
        for (c, t, on) in ends {
            let base = run.world.get(dbp(c[0], c[1], c[2])).to_string();
            if on {
                input_on(&mut run.world, c, t)?;
            }
            bases.push((c, base, on));
        }
        for _ in 0..10 {
            run.tick();
        }
        let states: Vec<String> = cells.iter().map(|(c, n)| format!("{n} {}", run.world.get(dbp(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"))).collect();
        let powered = run.world.get(dbp(9, by + 13, -4)).to_string().contains("powered=true");
        let want = rest && !day && !notready;
        if powered != want {
            bad += 1;
        }
        rows.push(format!("REST_l {rest}, DAY {day}, NOT-ready {notready}: the rail {} (want {}): {}", if powered { "powered" } else { "unpowered" }, if want { "powered" } else { "unpowered" }, states.join(", ")));
        for (c, base, on) in bases {
            if on {
                input_off(&mut run.world, c, &base)?;
            }
        }
        for _ in 0..10 {
            run.tick();
        }
    }
    Ok(format!("the queue gate's bench (day-1's R1, 19:19): {}; wrong {bad} ({})\n", rows.join("; "), if bad == 0 { "PASS" } else { "FAIL" }))
}

/// (ironspec 20:10 PT, the 7th property) Whether the segment from `a` to `b` passes through the
/// cell `c`'s unit box (the slab test).
fn seg_hits_cell(a: redsim::entity::Vec3, b: redsim::entity::Vec3, c: [i32; 3]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q, lo) in [(a.x, b.x, c[0] as f64), (a.y, b.y, c[1] as f64), (a.z, b.z, c[2] as f64)] {
        let d = q - p;
        if d.abs() < 1e-12 {
            if p < lo || p > lo + 1.0 {
                return false;
            }
            continue;
        }
        let (u, v) = ((lo - p) / d, (lo + 1.0 - p) / d);
        let (u, v) = if u < v { (u, v) } else { (v, u) };
        t0 = t0.max(u);
        t1 = t1.min(v);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// (ironspec 19:58 PT (2b)) The split's Ts (x,25,9), x -3 / -1 / 1 / 3 / 5 / 7: the ticks each
/// one's stored shape differs from its power's settle, of them with a cart within 1.5; its shape
/// changes, of them to a shape that isn't its power's settle; the last stored shape; the rows.
#[derive(Default)]
struct TSplit {
    stale: [i64; 6],
    stale_cart: [i64; 6],
    changes: [i64; 6],
    wrong_changes: [i64; 6],
    prev: [Option<redsim::block::RailShape>; 6],
    rows: Vec<String>,
    change_rows: Vec<String>,
    /// (ironspec 23:51 PT (2)) each T's lever's last state and flip tick; the shape changes with no
    /// flip of its lever within 2 gt (an update-only source acting on its own).
    lever_prev: [Option<bool>; 6],
    lever_flip: [i64; 6],
    unswitched: [i64; 6],
    unswitched_rows: Vec<String>,
    /// (G1's [2]) the ticks each T wasn't a junction (a missing T fails).
    missing: [i64; 6],
}

/// (ironspec 19:24) The direct summon check's tally; (19:33 PT) every summon tick's class
/// (pending until the tick after S shows whether a golem was born), all and in locked running.
#[derive(Default)]
struct Direct {
    checks: i64,
    misses: [i64; 3],
    rows: Vec<String>,
    pending: Option<(i64, &'static str, bool, usize)>,
    classes: BTreeMap<&'static str, (i64, i64)>,
    /// (ironspec 19:51 / 19:58 PT) the stand's summon ticks: all, locked, with a golem; the ticks
    /// without a roll from the stand, all and locked; the riders covered, blocked, short; per
    /// willing-and-ticking count at the tick's start the ticks and golems; the first rows.
    s_count: i64,
    s_locked: i64,
    s_born: i64,
    stand_misses: i64,
    stand_misses_locked: i64,
    /// (G1 X23) locked misses with a rider event at the emptied lane's pocket (not the empty-pocket
    /// control's doing).
    misses_not_empty: i64,
    /// (ironspec 23:55 PT (2)) the first S without a roll and its pocket riders' LAST_SLEPT expiries.
    first_miss: Option<(i64, String)>,
    covered: i64,
    blocked: i64,
    /// Callers not willing because they hadn't slept within a day (LAST_SLEPT expired, no stamp).
    not_slept: i64,
    short: i64,
    by_ready: BTreeMap<usize, (i64, i64)>,
    skips: i64,
    call_rows: Vec<String>,
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
    // (cc767b78 11:15, G1 10:50: the build-position term) every cell f1q2-run reads or writes is in
    // the DESIGN frame; the world stands at the design + --offset
    let o = frame_offset();
    [p.x.floor() as i32 - o[0], p.y.floor() as i32 - o[1], p.z.floor() as i32 - o[2]]
}

thread_local! {
    /// (cc767b78 11:15) f1q2-run's --offset: the world's build stands at the design frame + this (x / z
    /// whole chunks); every coordinate the controller and the report use stays in the design frame.
    static FRAME_OFFSET: std::cell::Cell<[i32; 3]> = const { std::cell::Cell::new([0, 0, 0]) };
}

/// (cc767b78 11:15) the run's --offset (the world = the design + this).
fn frame_offset() -> [i32; 3] {
    FRAME_OFFSET.with(|c| c.get())
}

/// (cc767b78 11:15) A design-frame cell's world block position (the design + --offset).
fn dbp(x: i32, y: i32, z: i32) -> redsim::pos::BlockPos {
    let o = frame_offset();
    redsim::pos::BlockPos::new(x + o[0], y + o[1], z + o[2])
}

/// (cc767b78 11:15) A world position in the design frame (the world - --offset).
fn dpos(v: redsim::entity::Vec3) -> redsim::entity::Vec3 {
    let o = frame_offset();
    redsim::entity::Vec3::new(v.x - o[0] as f64, v.y - o[1] as f64, v.z - o[2] as f64)
}

/// (cc767b78 11:15) A design-frame position in the world (the design + --offset).
fn wpos(x: f64, y: f64, z: f64) -> redsim::entity::Vec3 {
    let o = frame_offset();
    redsim::entity::Vec3::new(x + o[0] as f64, y + o[1] as f64, z + o[2] as f64)
}

/// One run: the report.
pub fn run(spec: &FarmSpec, o: &Opts) -> Result<String, String> {
    run_driven(spec, o).map(|(r, _)| r)
}

/// One run: the report, and with `drive_rec` the drive's text and the scene's added entity lines
/// (mob-AI 14:14: `mctas mobai fixture` replays them on the bridge and on redsim).
pub fn run_driven(spec: &FarmSpec, o: &Opts) -> Result<(String, Option<(String, String)>), String> {
    // (G1's guard: the built cells and the module's declared emulated cells, for every drive
    // write)
    let built: BTreeMap<[i32; 3], String> = spec.blocks.iter().map(|(p, b)| ([p.x, p.y, p.z], b.to_string())).collect();
    BUILT.with(|g| *g.borrow_mut() = Some((built, o.emulated.iter().chain(o.block_standins.iter()).copied().collect())));
    BLOCK_STANDINS.with(|b| *b.borrow_mut() = o.block_standins.clone());
    SUPPRESSED.with(|s| s.borrow_mut().clear());
    BLOCK_TOGGLES.with(|t| t.borrow_mut().clear());
    NOOP_WRITES.with(|n| *n.borrow_mut() = 0);
    NOOP_ROWS.with(|r| r.borrow_mut().clear());
    let r = run_inner(spec, o);
    BUILT.with(|g| *g.borrow_mut() = None);
    BLOCK_STANDINS.with(|b| b.borrow_mut().clear());
    TS.with(|t| t.borrow_mut().clear());
    // (the recorder off whatever happened, so a later run on this thread records nothing)
    let log = DRIVE_LOG.with(|d| d.borrow_mut().take());
    let (rep, drive_text, scene) = r?;
    Ok((rep, drive_text.map(|(head, t0)| {
        let mut s = head;
        s.push_str(&format!("# t0 {t0}\n"));
        // (onechunk 15:24, cc767b78's replay) the run's own day0 (RunOpts::steady's draw), which the
        // replay's RunOpts must equal: the scene's `# day-time:` is the save's, not the run's
        s.push_str(&format!("# day0 {}\n", RunOpts::steady(o.seed, 0, WalkMode::Random).day0));
        for a in log.map(|l| l.1).unwrap_or_default() {
            s.push_str(&a);
            s.push('\n');
        }
        (s, scene)
    })))
}

fn run_inner(spec: &FarmSpec, o: &Opts) -> Result<(String, Option<(String, i64)>, String), String> {
    let mut sp = spec.clone();
    // (ironspec 08:05 / 08:14 / 08:16 platform, tick-exact's farm-c7 t0 state) the c7 reconciliation's
    // stored riders are HOME-less as tick-exact's game summons them: the CLI loads the record with
    // spec_of's PortHome::Skip in c7 mode (no `port home` memory in any of the record's villagers);
    // the HOME-holder check after the setup stays as the guard
    // (G1 10:47 blocking 5) the c7 opt-out's per-entity effect, PortHome::Load's HOME holders against
    // the loaded spec's (Skip), from the CLI; empty in design runs
    let c7_strip: Vec<String> = o.c7_port_home.clone();
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
        // (ironfarm 11:40: the napper is one of its lane's 8 carts. ironspec 01:59 PT: the bench's
        // start is the build guide's, every napper in its lane and the queue empty, so the
        // module's queued cart is dropped; `--seed-queued-napper` keeps the old seeding, the
        // napper queued at the way-in and its lane one slope cart short)
        let nap_lane = (o.napper && o.seed_queued_napper).then(|| LANES[o.seed.rem_euclid(3) as usize % 3]);
        let mut carts = lane_carts(o.per_lane.saturating_sub(3));
        if let Some(x) = nap_lane
            && let Some(k) = carts.iter().rposition(|c| (c.1[0] - (x as f64 + 0.5)).abs() < 0.01 && c.1[1] > 18.0)
        {
            carts.remove(k);
        }
        if o.napper && !o.seed_queued_napper {
            drop_stored_queue_cart(&mut sp.entities, o.bed[1]);
        }
        sp.entities.extend(carts);
    }
    // (mob-AI 14:14: the drive) the napper must come from b8949c96's queue: the drive spawns
    // nothing later and teleports only at rest
    let drive_on = o.drive_rec.is_some();
    let by = o.bed[1];
    if drive_on {
        if o.official_lane3 || o.neighbour.is_some() || o.control_rider.is_some() || !o.restarts.is_empty() {
            return Err("--drive-out: not with --official-lane3, --neighbour-golems, --control-rider or restarts (their writes are no drive acts)".to_string());
        }
        // (the drive teleports only at rest, and a cart at rest on A6's entry booster doesn't
        // start: the napper comes from b8949c96's queue, released by its gate)
        if o.napper && !sp.blocks.iter().any(|(p, b)| *p == redsim::pos::BlockPos::new(9, by + 13, -4) && b.to_string().contains("powered_rail")) {
            return Err(format!("--drive-out --napper: no way-in queue at (9,{},-4) (b8949c96's e9f0f4cbf): the napper must start from it", by + 13));
        }
    }
    // (tick-exact 07:12 PT: the scene's entity lines are exactly the run's spawn list, each named as
    // the drive names it; the module's own lines the run dropped (the build guide's empty queue) are
    // gone, every seeded cart is there)
    let scene_entities: String = scene_entity_lines(&sp.entities);
    anyhow_free(sp.has_bed(), "the module holds no bed: no villager gets a HOME")?;
    // (cc767b78 09:38 / ironspec 09:33: no bench moves the day after its villagers' brains are built)
    // --official-lane3 builds at 18,000 and holds it; the c7 placement builds at tick-exact's 13,800
    let ro = RunOpts { ai: true, day0: if o.official_lane3 { 18_000 } else if o.c7_placement.is_some() { 13_800 } else { o.day0 }, slept: true, ..RunOpts::steady(o.seed, (o.ticks + 200) as u32, WalkMode::Random) };
    // (the shared bed, as the station bench sets it: the farm built homeless, the bed's POI taken,
    // every villager's HOME the bed and LAST_SLEPT 100 gt back; with the drive, the world started
    // as every bench's drive starts it, `Run::start` with the drive's options, the settle run out
    // and the scene's entity lines spawned in file order, the memories set by the drive's acts)
    let drive_ticks = (o.ticks + 200) as u32;
    // (cc767b78 11:15, G1 10:50: the build-position term) --offset: the world's build stands at the
    // design + the offset (blocks, entities, block entities, the output, the box, the Nether
    // anchor), while every cell the controller and the report use stays in the design frame.
    // (onechunk 15:08: any offset; the column convention, the module anchor (-6,0,-6) at a world chunk
    // corner, is x and z = 6 mod 16, printed by the report; offset 0 puts the anchor at (-6,_,-6), the
    // module straddling the world's chunk lines x = 0 and z = 0)
    if o.offset != [0, 0, 0] && drive_on {
        return Err("--offset with --drive-out: the drive's acts are written in the design frame (not supported)".to_string());
    }
    FRAME_OFFSET.with(|c| c.set(o.offset));
    struct FrameReset;
    impl Drop for FrameReset {
        fn drop(&mut self) {
            FRAME_OFFSET.with(|c| c.set([0, 0, 0]));
        }
    }
    let _frame_reset = FrameReset;
    let sp_world = shift_spec_world(&sp, o.offset);
    let (mut run, mut lines) = if drive_on {
        let dro = RunOpts { ai: true, ..RunOpts::steady(o.seed, drive_ticks, WalkMode::Random) };
        let mut run = Run::start(&sp, &dro)?;
        while run.settle_tick() {}
        let lines = redsim::entity::mob::drive::spawn_lines(&mut run, &sp)?;
        // (G1 07:17 PT: the scene's entity lines are the run's spawn list, asserted at write time
        // by count, type and position)
        // (G1 07:17 PT, X34) the WRITTEN scene text (what the replayer reads) against the world's
        // spawned entities, passengers included, at write time
        check_scene_text(&spawned_view(&run.world, &lines)?, &scene_entity_lines(&sp.entities))?;
        (run, lines)
    } else {
        (Run::new_homeless(&sp_world, &ro)?, Vec::new())
    };
    // (ironspec 13:04 (1), cc767b78's switch) a seated rider's doPush onto carts off, from the first
    // tick: a counterfactual measurement with no game twin
    run.world.mob.rider_cart_push_off = o.rider_push_off;
    // (ironspec 14:57's rule) the natural spawner OFF EXPLICITLY: no bar of f1q2-run counts natural
    // spawns (the only hostiles are the design's own scarers), so its scope never rides a light-read
    // default; the report's first line says so
    let natural_was = run.world.mob.natural.is_some();
    run.world.mob.natural = None;
    let bed = dbp(o.bed[0], o.bed[1], o.bed[2]);
    // (ironspec 08:05 platform (2), cc767b78's check 2) with --c7-placement, the HOME holders after
    // each setup phase, to find the setter of the stored riders' HOME
    let home_holders = |w: &redsim::world::World| -> Vec<u32> { w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.home.is_some()))).map(|e| e.id).collect() };
    let mut c7_phase_seen: Vec<(&'static str, Vec<u32>)> = Vec::new();
    if o.c7_placement.is_some() {
        c7_phase_seen.push(("after Run::start, the settle and the entity spawn", home_holders(&run.world)));
    }
    // (ironspec 07:43 / 07:57 / 08:05 platform, G1 07:50, cc767b78: redsim reads AIR outside the
    // build, so a run is a FLOATING site whose envelope holds the design's own neighbouring parts
    // only when its file composes them: the N = 1 loop part at the gap chunk (1,1), within the idle
    // strolls' 10 / 7 of the riders near the module's +x / +z faces; the BURIED variant waits for
    // 9fc3155f's shared site code (fleecheck::surround_spec), one definition for runs and statics)
    // (G1 X43) the loop part detected by its whole frame and loop, not one block: at least 10
    // obsidian (the portal frame) and 2 droppers (the item loop) beyond x 14, z 16
    let loop_obsidian = sp.blocks.iter().filter(|(p, b)| p.x >= 14 && p.z >= 16 && b.to_string().contains("obsidian")).count();
    let loop_droppers = sp.blocks.iter().filter(|(p, b)| p.x >= 14 && p.z >= 16 && b.to_string().contains("dropper")).count();
    let loop_composed = loop_obsidian >= 10 && loop_droppers >= 2;
    let site_line = format!(
        "the site (ironspec 07:43 / 07:57 platform, G1 07:50): FLOATING (redsim reads air outside the build); {} ({loop_obsidian} obsidian, {loop_droppers} droppers beyond x 14, z 16; G1 X43: the frame needs >= 10, the loop >= 2)",
        if loop_composed { "the N = 1 loop part composed in: \"(floating site: void outside the box, envelope covered)\"" } else { "the N = 1 loop part NOT composed in this file: \"(floating site, envelope partly covered: the loop part at chunk (1,1) missing)\"" }
    );
    let t0 = run.world.time;
    let mut drive_head: Option<String> = None;
    if let Some(rec) = &o.drive_rec {
        DRIVE_LOG.with(|d| *d.borrow_mut() = Some((t0, Vec::new())));
        drive_head = Some(format!(
            "# drive v1: {rec}\n# caption the F1 + Q2 module's day as f1q2-run drives it: the station's controller, the napper hold's release (c'), the A6 stop's levers and the way-in queue's gate written as blocks (EMULATED); {}; every rider's HOME and LAST_SLEPT set by the `home` / `last-slept` acts at t 0 (a memory: no ticket), the bed's ticket {}\n# seed {}\n# days 1\n# ticks {drive_ticks}\n# walk random\n# ai on\n# start Run::start(the scene's spec, RunOpts {{ ai: true, ..RunOpts::steady(seed, ticks, WalkMode::Random) }}), the settle run out, then the scene's entity lines spawned in file order (`drive::spawn_lines`)\n# build: the scene as pasted (no build order)\n# act t: applied when the world's time is t0 + t, before the run's next tick (f1q2-run's writes after its tick, in its order)\n",
            if o.seed_queued_napper { "the napper starts queued on the way-in (--seed-queued-napper)" } else { "the build guide's start: every napper in its lane, the queue empty" },
            if o.bed_ticket_taken { "taken by the `poi-take` act (state (ii))" } else { "free (the design start (i))" },
            o.seed
        ));
        if o.bed_ticket_taken {
            drive_push(format!("act 0 poi-take {} {} {}", bed.x, bed.y, bed.z));
        }
    }
    // (ironspec 06:31 / 06:38 PT: the design start (i) leaves the bed's single ticket FREE, every
    // villager's HOME a memory only; `--bed-ticket-taken` is state (ii), the bench taking it)
    // (G1 / ironspec 06:42 real, the pre-fix ticket gap: redsim builds its ticket table on the first
    // ticket access from the HOME memories then held, counting the bed taken once any HOME names
    // it; the bench builds the table here, before any HOME memory is set, so state (i) keeps the
    // bed's ticket free as the game's loaded HOME does: a declared compensation, harmless once
    // cc767b78's fix lands)
    let free_before_homes = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed);
    if o.bed_ticket_taken {
        redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    }
    // (the drive's names: line N's entity eN, its passenger K eN.pK)
    let mut names: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    DRIVE_NAMES.with(|d| d.borrow_mut().clear());
    for (n, id) in lines.iter().enumerate() {
        if let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) {
            DRIVE_NAMES.with(|d| d.borrow_mut().insert(*id, format!("e{n}")));
            for (k, p) in e.passengers.iter().enumerate() {
                names.insert(*p, (n, k));
                DRIVE_NAMES.with(|d| d.borrow_mut().insert(*p, format!("e{n}.p{k}")));
            }
        }
    }
    // (ironspec 19:24: the direct summon check's control) one pocket's rider removed at the start
    if let Some(x) = o.empty_pocket {
        let ci = run.world.entities.list.iter().position(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [x, 0, 0] && !e.passengers.is_empty()).ok_or_else(|| format!("--empty-pocket {x}: no rider in the pocket ({x},0,0)"))?;
        let v = run.world.entities.list[ci].passengers[0];
        run.world.entities.list[ci].passengers.clear();
        if let Some(vi) = redsim::entity::index_of(&run.world, v) {
            run.world.entities.list[vi].vehicle = None;
            run.world.entities.list[vi].removed = true;
        }
    }
    let mut memory_acts: Vec<((usize, usize), String)> = Vec::new();
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        if e.removed || !matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager)) {
            continue;
        }
        // (the recovery row: every LAST_SLEPT a day and 100 gt old, no villager fresh)
        let slept_back = if o.stale_start { 24_100 } else { 100 };
        // (tick-exact's farm-c7 placement, 06:09 PT: the record's own stored riders carry no HOME
        // unless --c7-home-stored)
        if o.c7_placement.is_some() && !o.c7_home_stored {
            continue;
        }
        if drive_on {
            let (n, k) = *names.get(&e.id).ok_or_else(|| format!("--drive-out: the villager {} rides none of the scene's entity lines", e.id))?;
            memory_acts.push(((n, k), format!("act 0 home e{n}.p{k} {} {} {}", bed.x, bed.y, bed.z)));
            memory_acts.push(((n, k), format!("act 0 last-slept e{n}.p{k} -{slept_back}")));
        }
        let b = redsim::entity::mob::ai::brain::brain_mut(&mut run.world, i);
        b.home = Some(bed);
        b.last_slept = Some(t0 - slept_back);
    }
    memory_acts.sort_by_key(|(nk, _)| *nk);
    for (_, a) in memory_acts {
        drive_push(a);
    }
    if o.c7_placement.is_some() {
        c7_phase_seen.push(("after f1q2-run's HOME loop (it skips every villager in c7 mode)", home_holders(&run.world)));
    }
    // (tick-exact 06:09 PT / ironspec 08:05 platform (2)) the c7 placement's stored riders must start
    // HOME-less, as tick-exact's game setup has them: a HOME found on any after the setup FAILS the
    // run (which rider, the HOME, the setup phase it was first seen at), never cleared silently
    let c7_holders: Vec<String> = if o.c7_placement.is_some() && !o.c7_home_stored {
        run.world
            .entities
            .list
            .iter()
            .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.home.is_some())))
            .map(|e| {
                let h = match &e.kind {
                    EntityKind::Mob(m) => m.brain.as_deref().and_then(|b| b.home),
                    _ => None,
                };
                let first = c7_phase_seen.iter().find(|(_, ids)| ids.contains(&e.id)).map_or("after the HOME loop", |(ph, _)| *ph);
                format!("#{} at ({:.2},{:.2},{:.2}) HOME {:?}, first seen {first}", e.id, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, h.map(|h| [h.x, h.y, h.z]))
            })
            .collect()
    } else {
        Vec::new()
    };
    let c7_free_t0 = if o.c7_placement.is_some() { redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed) } else { None };
    // (ironspec 06:42 real) the START-STATE LINE: the bed's free tickets and the HOME memory
    // holders after the setup, and how the run handles the ticket gap; (farmopt 06:04 PT) the early
    // read is a declared stand-in under the override rule: the free count redsim's own lazy table
    // would read from the memories now (before) against the early-built table's (after); in state
    // (i) a read that changes nothing (cc767b78's fix landed) FAILS, so it is dropped
    let holders_of = |w: &redsim::world::World| -> Vec<u32> {
        w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.home == Some(bed)))).map(|e| e.id).collect()
    };
    let early_table = run.world.mob.ai.poi_tickets.take();
    let lazy_free = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed);
    run.world.mob.ai.poi_tickets = early_table;
    let start_free = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed);
    let start_holders = holders_of(&run.world);
    let start_state_line = format!(
        "the start-state line (ironspec 06:42 real; the surroundings: the report's site line (G1 07:50: a run is a FLOATING site, redsim reading air outside the build; the BURIED variant waits for 9fc3155f's shared site code)): the bed's free tickets {}, HOME memory holders {}; {}; the pre-fix ticket gap: the early ticket read (a declared stand-in, built before any HOME was set: {} free then) against redsim's own lazy table from the memories, {} -> {} free{}{}",
        start_free.map_or("- (no POI at the bed)".to_string(), |f| f.to_string()),
        start_holders.len(),
        if o.bed_ticket_taken { "state (ii), the bench taking the ticket (--bed-ticket-taken)" } else { "the design start (i)" },
        free_before_homes.map_or("-".to_string(), |f| f.to_string()),
        lazy_free.map_or("-".to_string(), |f| f.to_string()),
        start_free.map_or("-".to_string(), |f| f.to_string()),
        if o.bed_ticket_taken { " (state (ii): the stand-in isn't judged)" } else if lazy_free == start_free { " (a no-op: the lazy table already leaves the ticket free (cc767b78's fix?): FAIL, drop the stand-in)" } else { " (it changes the cell: the game's (i))" },
        match (o.bed_ticket_taken, start_free) {
            (false, Some(1)) | (true, Some(0)) => "; the start is the state it names: PASS".to_string(),
            (_, f) => format!("; {} free is not the named state's: FAIL", f.map_or("-".to_string(), |f| f.to_string())),
        }
    );
    let start_ticket = (start_free, start_holders);
    let mut recs: BTreeMap<u32, Rec> = BTreeMap::new();
    // (ironspec 17:10 (3)) per lane cell (x, y, z): panic ticks, stroll attempts, draws with a target,
    // clears in PANIC
    let mut panic_cells: BTreeMap<(i32, i32, i32), [u64; 4]> = BTreeMap::new();
    // (day-1 01:10 PT, the per-pick trace) each napper's milestones of its earlier visits (a pick
    // starts a visit's list afresh)
    let mut past_marks: BTreeMap<u32, Vec<(String, i64)>> = BTreeMap::new();
    let villagers = |w: &redsim::world::World| -> Vec<u32> { w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager))).map(|e| e.id).collect() };
    for (k, id) in villagers(&run.world).into_iter().enumerate() {
        let i = redsim::entity::index_of(&run.world, id).ok_or("a villager vanished")?;
        // (ironspec 16:50 (3)) --force-hostile-phase P: every villager's VILLAGER_HOSTILES scan phase set
        // to P mod 20 (its timeToTick so that now + timeToTick = P mod 20): the forced-phase runs
        if let Some(p) = o.force_hostile_phase {
            let now = run.world.time;
            let ttt = (p - now).rem_euclid(20);
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, i).sensors[5] = if ttt == 0 { 20 } else { ttt };
        }
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
            // (the spec's cells are the design frame's; the world's at + the offset)
            let now = run.world.get(dbp(p.x, p.y, p.z));
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
    let mut ctl = Ctl { napper: o.napper, stop_dy: sy, watchdog: o.watchdog, held_at_divert: o.held_at_divert, queue_hold: o.queue_hold, day_threshold: o.day_threshold, ..Ctl::default() };
    if o.napper && o.drive {
        ctl.k_lock = Some(o.k_lock.ok_or("the module declares no `k-lock: N (why)` line (the hold gate's delayed Q, ironspec 22:09)")?);
        ctl.k_sw = Some(o.k_sw.ok_or("the module declares no `k-sw: N (why)` line (the siding's repeater after D, ironspec 22:03)")?);
        // (ironspec 06:35 platform: the dwell is design, no fixed cap of record unless derived and
        // cited; a declared cap still bounds HELD, and without one HELD's stuck bar isn't judged)
        ctl.dwell_cap = o.dwell_cap;
        ctl.rs_early = o.rs_ready_early_control;
        ctl.day_phase_control = o.day_phase_control;
        ctl.strand_control = o.strand_control;
        ctl.misroute_left = if o.misroute_control { 2 } else { 0 };
        ctl.eject_control = o.eject_control;
        ctl.picker_hold_to_leg = o.picker_hold_to_leg;
        ctl.hold_delta = o.hold_pulse_delta;
        ctl.evidence_keyed = o.evidence_keyed;
        ctl.evidence_delay = o.evidence_delay;
        ctl.rel_gate_armed = o.rel_gate_armed;
        ctl.offs = o.lane_offsets.unwrap_or(OFFS);
        ctl.nap_lane_control = o.nap_lane_control;
        ctl.stuck_k_control = o.stuck_k_control;
        if let Some(l) = o.ring_timed {
            ctl.ring_timed = true;
            ctl.ring_l = l;
            ctl.ring_k = o.ring_k;
        }
    }
    ctl.delta7 = o.delta7;
    // (day-1 06:07 PT, ctlcounter.rs; G1's blocker 3: the counter's state at the first release, a
    // clear clock's: C = 141 + ceil((d - 12,540) / 100) for a first release at day time d between
    // EIGHT (C loads 19) and RISE (C loads 0), each the analytic detector's crossing)
    {
        ctl.counter_pending = true;
        ctl.sensor_armed = 21;
        ctl.counter_window = counter_window(&run.world);
    }
    if o.drive {
        // (ironspec 18:31 (2): a stand-in's update set reaches no rail T, unless the T is declared
        // `update-only-ts:` (state-stable for the state it holds); every T of the build, by
        // redsim's own junction test)
        let ts: Vec<[i32; 3]> = sp
            .blocks
            .iter()
            .filter(|(_, b)| matches!(b, redsim::block::Block::PlainRail { .. }))
            .filter(|(p, _)| redsim::rails::junction_at(&run.world, dbp(p.x, p.y, p.z)).is_some())
            .map(|(p, _)| [p.x, p.y, p.z])
            .filter(|c| !o.update_only.contains(c))
            .chain(o.ts.iter().copied())
            .collect();
        TS.with(|t| *t.borrow_mut() = ts);
        ctl.init(&mut run.world)?;
    }
    // (phase 3a: the nappers seeded, one a night; ironspec 09:59: every villager within 16 of the
    // bed head with its state at each occupied-bed tick; ironspec 10:02: a cart resting on another
    // cart's top is a failure)
    let mut nappers: Vec<(u32, usize, i64)> = Vec::new();
    let mut seeded_night: Option<i64> = None;
    // (b8949c96's way-in, e9f0f4cbf: the napper queued at (9,y+13,-4) from the start, released by
    // its gate; one napper a run until day-1's picker is driven)
    {
        let q_cell = [9, o.bed[1] + 13, -4];
        let w = &run.world;
        ctl.wayin = o.napper && w.get(dbp(q_cell[0], q_cell[1], q_cell[2])).to_string().contains("powered_rail") && {
            // (the release cell: air (a stand-in's) or day-1's R1 repeater)
            let r = w.get(dbp(8, q_cell[1], -4));
            r.is_air() || r.to_string().contains("repeater")
        };
        if ctl.wayin && o.seed_queued_napper {
            ctl.q_rs_ready = true;
            ctl.q_held = true;
            let qv = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == q_cell).and_then(|e| e.passengers.first().copied()).ok_or("the way-in's queue holds no napper")?;
            let k = o.seed.rem_euclid(3) as usize % 3;
            recs.insert(qv, Rec { label: format!("napper0 (id {qv}, lane x {}, queued at the way-in)", LANES[k]), home_at_start: Some([bed.x, bed.y, bed.z]), prev_home: Some([bed.x, bed.y, bed.z]), ..Default::default() });
            nappers.push((qv, k, 0));
            ctl.nap_lane = k;
            ctl.nap_events.push(format!("t 0: napper0 queued at ({},{},{}) (lane x {})", q_cell[0], q_cell[1], q_cell[2], LANES[k]));
        } else if ctl.wayin {
            // (ironspec 01:59 PT: the build guide's start, every napper in its lane: the queue
            // empty, the stop ready, HELD reset)
            ctl.q_rs_ready = true;
            ctl.q_held = false;
            ctl.nap_events.push("t 0: the queue empty, every napper in its lane (the build guide's start)".to_string());
        }
    }
    // (ironspec 18:10 / 18:20; 7b90c3f1 18:06) the stand-ins' supports the module declares
    // `emulated: CELL=STATE (why)` (glass under a floor lever where the built cell is air: the
    // queue release's (8,y+12,-4), the hold input's (7,-2,8)), laid once at the start in the state
    // the header names (G1's note (c)); (ironspec 20:10 / 20:19 PT (2c), the 7th property, SIGHT) a
    // support laid where the build has air is watched against every sight ray of the run: each
    // villager's eye to every living entity's eye within 16 (the nearest-living and hostile
    // sensors' reach), bar 0 crossings
    let mut sight_cells: Vec<[i32; 3]> = Vec::new();
    // (a stand-in with no collision box, a lever, can't cross a sight ray: a mob's sight clips
    // against colliders (`ClipContext.Block.COLLIDER`), so its cell isn't watched)
    let mut sight_skipped: Vec<[i32; 3]> = Vec::new();
    // (ironspec 04:07 PT: a bench override that doesn't change its cell fails the run) each
    // emulated cell's state before and after it's laid
    let mut overrides: Vec<([i32; 3], String, String)> = Vec::new();
    if o.drive {
        for (c, state) in &o.emulated_states {
            let p = dbp(c[0], c[1], c[2]);
            let before = run.world.get(p).to_string();
            if run.world.get(p).is_air() {
                set(&mut run.world, *c, state, redsim::world::flags::ALL)?;
                if redsim::behavior::collision_boxes(&run.world.get(p)).is_empty() {
                    sight_skipped.push(*c);
                } else {
                    sight_cells.push(*c);
                }
            }
            overrides.push((*c, before, run.world.get(p).to_string()));
        }
        // (day-1 05:45 PT, ironspec 06:24 real) a redstone-block stand-in is a solid cell while its
        // line is on: watched like a laid support, its crossings counted only while it holds the
        // block (its known difference)
        for c in &o.block_standins {
            if !sight_cells.contains(c) {
                sight_cells.push(*c);
            }
        }
    }
    let mut sight_hits: Vec<i64> = vec![0; sight_cells.len()];
    let mut sight_rows: Vec<String> = Vec::new();
    let mut sight_classes: BTreeMap<String, i64> = BTreeMap::new();
    // (ironspec 02:37 PT (3)) the detector's falls under each weather, at the run's start
    let day_rows = if o.napper && o.drive { day_weather_rows(&run.world, o.day_threshold) } else { String::new() };
    // (ironspec 21:25 PT) the carts' contacts with blocks: cell -> (ticks, the first tick, block)
    let mut contacts: BTreeMap<[i32; 3], (i64, i64, String)> = BTreeMap::new();
    // (ironspec 20:40 PT) the feed stack's direct bench: instead of the run
    if o.stack_bench {
        let gate = gate_bench(&mut run, o.bed[1])?;
        let lines = line_bench(&mut run, o.bed[1])?;
        return Ok((stack_bench(&mut run)? + &gate + &lines, None, String::new()));
    }
    let mut near = Near::default();
    let mut stale = Stale::default();
    let mut seat_stats: Vec<SeatStat> = vec![SeatStat::default(); o.seats.len()];
    let mut gdr = Gdr::default();
    let mut summons = Summons::default();
    let mut golem_born: BTreeMap<u32, i64> = BTreeMap::new();
    // (farmopt / ironspec 00:20 PT) the panics starting at the queue seat
    let mut seat_panics: Vec<String> = Vec::new();
    // (ironspec 00:22 PT) the ticks a napper at the seat had NEAREST_HOSTILE set, and the first
    let mut seat_hostile_ticks = 0i64;
    let mut seat_hostile_first: Option<String> = None;
    // (farmopt 00:38 PT) each napper's path at the seat, logged at each change
    type SeatPath = (Option<[i32; 3]>, Option<[i32; 3]>);
    let mut seat_paths_last: BTreeMap<u32, SeatPath> = BTreeMap::new();
    let mut seat_paths: Vec<String> = Vec::new();
    // (ironspec 02:18 PT (3a)) each lane's H..V carts at the first release, the least, the most
    // away, the rows with more than one away
    let mut occ_full: Option<[i64; 3]> = None;
    // (G1 N6 / ironspec D4) each lane's carts by id at t0 (the pocket P (x,0,0), H (x,16,0), R_1 and
    // the five slopes), asserted against the design's 8 a lane
    let lane_ids_t0: [Vec<u32>; 3] = [0usize, 1, 2].map(|k| {
        let x = LANES[k];
        run.world
            .entities
            .list
            .iter()
            .filter(|e| {
                let c = cell(e.pos);
                !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && (c == [x, 0, 0] || c == [x, 16, 0] || c == [x, 17, 1] || (0..5).any(|j| c == [x, 18 + j, 2 + j]))
            })
            .map(|e| e.id)
            .collect()
    });
    // (G1 X31) each lane cart's lane at t0, by id (the wrong-lane bar follows it; a world open
    // renews the keys)
    let mut cart_lane0: BTreeMap<u32, usize> = lane_ids_t0.iter().enumerate().flat_map(|(k, ids)| ids.iter().map(move |id| (*id, k))).collect();
    let mut wrong_lane: Vec<(u32, String)> = Vec::new();
    // (G1 X31) carts in a lane that weren't lane carts at t0 (the strays, bar 0)
    let mut stray_carts: Vec<(u32, String)> = Vec::new();
    let mut occ_min = [i64::MAX; 3];
    let mut away_max = [0i64; 3];
    let mut away_rows: Vec<String> = Vec::new();
    // (ironspec 00:33 PT) the stranded-napper bar's state: each napper's sleep last tick, the wakes
    // waiting for a cart at P (villager -> deadline), the wakes seen and the stranded ones
    let mut sleep_prev: BTreeMap<u32, bool> = BTreeMap::new();
    let mut wake_pending: BTreeMap<u32, i64> = BTreeMap::new();
    let (mut wakes_seen, mut stranded_n) = (0i64, 0i64);
    let mut stranded: Vec<String> = Vec::new();
    // (ironspec 23:56 PT (4)) the positive controls exercised in this run
    let mut controls_done: Vec<String> = Vec::new();
    let mut bystander_tries = 0u32;
    // (ironspec 02:48 PT) the napper the bystander control stamped, and the stamp's tick
    let mut bystander_target: Option<(u32, i64)> = None;
    // (ironspec 02:37 PT) every napper's first call back in its lane: (napper, tick, its margin
    // from the memory's latest set (None: never set), the setter)
    let mut nap_margins: Vec<(u32, i64, Option<i64>, &'static str)> = Vec::new();
    // (ironspec 06:31 PT) the riders in REST within 16 of the bed while someone sleeps in it, per
    // night: (villager-ticks, the first rows)
    let mut rest_near_bed: BTreeMap<i64, i64> = BTreeMap::new();
    // (ironspec 06:24 / 06:31 real, G1: the decisive bar) every REST entry (its row, closed with
    // its HOME path and any release), the lane riders' REST ticks and entries at night outside
    // their own trip (pick to R), those at a design position with a partial HOME path, and the
    // night window
    let mut rest_rows: Vec<String> = Vec::new();
    let mut rest_entries_total = 0i64;
    // (9fc3155f's coverage fix) the counted REST entries by cell
    let mut rest_by_cell: BTreeMap<[i32; 3], i64> = BTreeMap::new();
    // (ironspec 06:55 platform (2)) the counted spells by entry cell: their lengths, and how many
    // began with a WALK_TARGET held
    let mut spell_by_cell: BTreeMap<[i32; 3], (Vec<i64>, i64)> = BTreeMap::new();
    let mut lane_rest_ticks = 0i64;
    let mut lane_rest_entries = 0i64;
    let mut lane_rest_partial = 0i64;
    let mut night_window: Option<(i64, i64)> = None;
    // (G1) every CANT_REACH arming (the stamp set): night ones printed with position and scarer
    // sight, day ones counted with each arming's target and position
    let mut armings_night: Vec<String> = Vec::new();
    let mut armings_night_n = 0i64;
    let mut armings_day: Vec<String> = Vec::new();
    let mut armings_day_n = 0i64;
    // (ironspec 06:36 real) the runtime stamp bar: the oldest stamp seen (age, tick, rider, cell)
    // and every tick a stamp stood at 1,000 gt or more
    let mut stamp_max: Option<(i64, i64, String, [i32; 3])> = None;
    let mut stamp_over: Vec<String> = Vec::new();
    let mut stamp_over_n = 0i64;
    // (G1 N17: a control is caught only by its own sample) the riders whose stamps reached 1,000,
    // and the nappers seen without HOME at the stop
    let mut stamp_over_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut stop_homeless_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    // (ironspec 06:37 platform, G1) the at-entry stamp bar: the stamp's age at each spell's first
    // HOME-targeting tick (where SetWalkTargetFromBlockMemory ran), the spells judged, the worst
    let mut at_home_target_n = 0i64;
    let mut at_home_target_max = 0i64;
    let mut at_home_target_over = 0i64;
    // (G1 X19's control) the riders with an over-1,000 HOME-target set, and the control's rider
    let mut at_entry_over_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut at_entry_rider: Option<u32> = None;
    // (G1 10:47 O3) the at-entry control's act (tick, the stamp's age before) and its rider's next
    // judged set (tick, age); each rider's judged HOME-target sets
    let mut at_entry_before: Option<(i64, Option<i64>)> = None;
    let mut at_entry_next: Option<(i64, i64)> = None;
    // (G1's P14) the at-entry control's rider's HOME release after its aging (tick, mechanism)
    let mut at_entry_released: Option<(i64, String)> = None;
    let mut judged_sets: BTreeMap<u32, i64> = BTreeMap::new();
    // (ironspec D2) every napper's R: (id, tick)
    let mut at_r_log: Vec<(u32, i64)> = Vec::new();
    // (ironspec 06:31 real) the farm-stop bar: ticks a napper without HOME stood at the bed stop
    let mut stop_homeless: Vec<String> = Vec::new();
    let mut stop_homeless_n = 0i64;
    let mut home_erased = false;
    // (G1 X8) the napper the farm-stop control erased
    let mut home_erased_id: Option<u32> = None;
    let mut stamp_control_rider: Option<u32> = None;
    // (G1: the air bar's positive) the held rider: (id, from, the cart's held position, the cart, its
    // air at the hold, its least air in the hold)
    let mut air_held: Option<(u32, i64, [f64; 3], u32, i32, i32)> = None;
    // (G1 07:28 PT) the riders stacked below the held cart in its column at the hold's start
    let mut air_stacked: Vec<u32> = Vec::new();
    // (the lift-block control's cart)
    let mut lift_blocker: Option<u32> = None;
    // (ironspec 08:26 / 08:41 platform) the late-return control's held cart (id, from, where) and rider
    let mut late_hold: Option<(u32, i64, [f64; 3])> = None;
    // (G1 10:34) the forced lip hold's cart (id, from, where)
    let mut lip_hold_cart: Option<(u32, i64, [f64; 3])> = None;
    // (G1 10:47 blocking 4's control) PRESENT_l 20 gt after the eject
    let mut eject_present_after: Option<bool> = None;
    // (G1 10:36, the drop-entry envelope) each cart's off-rail ticks on its lane's drop cell (x,24,6)
    // since it arrived there and its largest lateral displacement there (|x - the lane's centre|),
    // and per pass (lane, those ticks, that displacement, a cart at C / D behind it at any of them)
    let mut lip_state: BTreeMap<u32, (i64, f64, bool)> = BTreeMap::new();
    // (ironspec 11:08 (2) / 11:37, G1's P1) the open pass windows, the judged passes per lane, the
    // passes with a hit or a push, and the least separation over the clean passes
    let mut pass_open: BTreeMap<u32, PassWindow> = BTreeMap::new();
    let mut pass_judged = [0usize; 3];
    let mut pass_rows: [Vec<PassRow>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut pass_clean_least = [f64::INFINITY; 3];
    // (G1's P2 / P3) the envelope's passes: (lane, off-rail ticks, lateral deviation, a cart behind,
    // how the window closed, cart, start, the cell it closed in)
    let mut env_passes: Vec<(usize, i64, f64, bool, &'static str, u32, i64, [i32; 3])> = Vec::new();
    // (day-1 / ironspec 11:06) every lip arrival: (lane, tick, cart, a napper's)
    let mut lip_arrivals: Vec<(usize, i64, u32, bool)> = Vec::new();
    // (G1 10:34) the siding trace's rows
    let mut siding_rows: Vec<String> = Vec::new();
    let mut late_rider: Option<u32> = None;
    // (G1 10:47 O4) the late-return hold's lane and phase (0: waiting for a window, 1: a window open,
    // 2: let go after it closed)
    let mut late_lane: Option<usize> = None;
    let mut late_phase: u8 = 0;
    // (the late-return control) whether its hold timed out without a window opening and closing
    let mut late_timed_out = false;
    // (G1 X35) the riders in the lift column at the hold's start
    let mut lift_stack_at_hold: Vec<u32> = Vec::new();
    // (cc767b78) the traced entity's per-tick rows
    let mut trace_rows: Vec<String> = Vec::new();
    // (ironspec 08:26 platform (3)) the chain trace's rows
    let mut chain_rows: Vec<String> = Vec::new();
    // (ironspec 09:06 / 09:33 platform) the t1 activity assert's line
    let mut t1_line = String::from("the t1 activity assert: no tick 1 (the run is shorter): NOT JUDGED: FAIL");
    // (ironspec 10:04 / 10:06) off-rail rests (cart -> (from, cell, ticks)), their rows, the moving
    // off-rail ticks per cell, and the siding's carts by rider kind (count, first)
    let mut off_rest: BTreeMap<u32, (i64, [i32; 3], i64)> = BTreeMap::new();
    // (ironspec 10:51 (c)) each off-rail rest's state at its first tick (the per-stall print)
    let mut off_rest_snap: BTreeMap<u32, String> = BTreeMap::new();
    let mut off_rest_rows: Vec<String> = Vec::new();
    let mut off_moving: BTreeMap<[i32; 3], i64> = BTreeMap::new();
    let mut siding_by: BTreeMap<&'static str, (i64, Option<String>)> = BTreeMap::new();
    // (the 27th) the carts resting on the station's trapdoor holds H, cart-ticks per cell
    let mut trapdoor_rests: BTreeMap<[i32; 3], i64> = BTreeMap::new();
    // (ironspec 10:51 (g)) each rest on an H trapdoor: the open ones (cart -> from, cell, ticks) and
    // the closed (cell, from, ticks, cart)
    let mut h_open: BTreeMap<u32, (i64, [i32; 3], i64)> = BTreeMap::new();
    let mut h_rests: Vec<([i32; 3], i64, i64, u32)> = Vec::new();
    // (ironspec 10:51 (f)) the siding plant's cart and tick
    let mut siding_plant: Option<(u32, i64)> = None;
    // (G1 11:19 F4 (c)) the K-misroute control's act (tick, cart, K(3)'s lever before) and the
    // westbound control's (tick, cart)
    let mut kmis: Option<(i64, u32, String)> = None;
    // (G1's P5) whether the K-misroute control's lever write took
    let mut kmis_ok = false;
    let mut westbound: Option<(i64, u32)> = None;
    // (G1's P1 controls) the V-holds control's cart and tick, whether the rest form is done, and lane
    // -1's judged passes when it began
    let mut vhold: Option<(u32, i64)> = None;
    let mut vhold_done = false;
    let mut vhold_judged0 = 0usize;
    // (ironspec 10:51 (f)) every batch cart seen on a siding cell (the plant control's own sample)
    let mut siding_batch_ids: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    // (ironspec 09:44 / 09:48, G1 09:45) every plain-rail T at t0 (3+ rail neighbours) and its shape,
    // and every change
    let mut t_shapes: BTreeMap<[i32; 3], String> = spec
        .blocks
        .iter()
        .filter(|(p, _)| redsim::rails::junction_at(&run.world, dbp(p.x, p.y, p.z)).is_some())
        .map(|(p, b)| ([p.x, p.y, p.z], format!("{:?}", redsim::rails::shape_of(b))))
        .collect();
    let mut t_changes: Vec<(i64, [i32; 3], String, String)> = Vec::new();
    // (7b90c3f1 11:01) every T's shape at t0 (the shape at a tick: t0's plus the changes before it),
    // and the split levers (x,26,9)'s states, each change (tick, on)
    let t_shapes0: BTreeMap<[i32; 3], String> = t_shapes.clone();
    let mut lever_hist: BTreeMap<i32, Vec<(i64, bool)>> = BTreeMap::new();
    // (ironspec 11:33, F4(a)'s bars) (i) a K switched while another lane's napper is on the split row,
    // (ii) a K armed with its R occupied, (iii) more than one napper between the stop's exit and the
    // K's: each one's rows, and the open episodes
    let mut f4_rows: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut f4_open = [false; 3];
    let mut f4_between_open = false;
    // (ironspec 11:29 / 11:33) an unridden cart at rest on a siding's R: per lane (cart, since, flagged)
    let mut r_unridden: [Option<(u32, i64, bool)>; 3] = [None, None, None];
    let mut r_unridden_rows: Vec<String> = Vec::new();
    // (G1 10:24 (a)) the per-lane count: per lane a histogram (count -> ticks), the max and its first
    // tick, and the first tick at 9 or more
    let mut lane_count_hist: [BTreeMap<usize, i64>; 3] = [BTreeMap::new(), BTreeMap::new(), BTreeMap::new()];
    let mut lane_count_max: [(usize, i64); 3] = [(0, 0); 3];
    let mut lane_count_nine: [Option<i64>; 3] = [None; 3];
    // (G1 10:47 blocking 3) the ticks with a cart at rest past V (V full), and the first
    let mut past_v_ticks = [0i64; 3];
    let mut past_v_first: [Option<String>; 3] = [None, None, None];
    // (G1 10:24, the cascade's seed) per lane, a cart off its rail in the drop region (the drop cell
    // (x,24,6) or the column (x,23,6)) while another cart is at C or D: the open episode (from, the
    // cart, the one behind) and every episode (lane, from, length, cart, behind)
    let mut lip_seed_open: [Option<(i64, u32, u32)>; 3] = [None; 3];
    let mut lip_seeds: Vec<(usize, i64, i64, u32, u32)> = Vec::new();
    // (G1 07:59 PT) the lift column around the lift-block control
    let mut lift_rows: Vec<String> = Vec::new();
    // (ironspec 08:13 / 08:14 platform, G1 08:12 PT) each lane's most carts (count, tick, tail) and
    // farthest-back tail; the carts standing still on the exit booster / exit row / split row
    let mut lane_max: [(i64, i64, Option<[i32; 3]>); 3] = [(0, 0, None); 3];
    let mut lane_tail_max: [Option<[i32; 3]>; 3] = [None; 3];
    let mut still_prev: BTreeMap<u32, bool> = BTreeMap::new();
    let mut stopped_rows: Vec<(u32, String)> = Vec::new();
    let mut stopped_ticks: i64 = 0;
    // (G1 08:21 PT) every cart's cell entries (tick, cell), for --pair-check
    let mut cart_entries: BTreeMap<u32, Vec<(i64, [i32; 3])>> = BTreeMap::new();
    // (G1 10:14) the arrivals at each lane's C (x,24,7): (tick, horizontal speed, ridden, cart), and
    // each cart's previous cell
    let mut c_arrivals: [Vec<(i64, f64, String, u32, f64, f64, [i32; 3])>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut c_prev_cell: BTreeMap<u32, [i32; 3]> = BTreeMap::new();
    // (ironspec 10:29 (3)) per lane: the drop passes, the C deflections (C west into c3 by a cart not
    // a napper's) and the off-rail rests in the drop region
    let mut drop_passes = [0i64; 3];
    // (ironspec 11:09 (1): how a lane reaches a 9th cart) the conservation ledger per lane: every cart
    // entering its queue (dropped in from the lip) and leaving its pocket P, (tick, cart, +1 / -1, what)
    let mut ledger: [Vec<(i64, u32, i64, String)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    // (G1's P18) the ledger's base (the region's count at the first tick) and the region's physical
    // count every 500 gt
    let mut ledger_base: [Option<i64>; 3] = [None; 3];
    let mut region_marks: [Vec<(i64, i64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    // (ironfarm 12:47, --lip-dump) per lane the last 60 gt of its carts, and the dumps taken at rests
    let mut lip_ring: [std::collections::VecDeque<String>; 3] = [std::collections::VecDeque::new(), std::collections::VecDeque::new(), std::collections::VecDeque::new()];
    let mut lip_dumps: Vec<String> = Vec::new();
    // (ironfarm 13:36) the lip classifier per lane (its lip in the world frame), its arrivals, and each
    // off-rail rest's (tick, lane, cart) for the class of the arrival before it
    let fo0 = frame_offset();
    let mut lip_lanes: [crate::lipclass::LipLane; 3] = LANES.map(|x| crate::lipclass::LipLane::new(x + fo0[0], 24 + fo0[1], 7 + fo0[2], None));
    let mut lipc_arrivals: [Vec<crate::lipclass::LipArrival>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut off_rest_events: Vec<(i64, usize, u32)> = Vec::new();
    // (G1 12:21) --villager-trace's rows
    let mut vtrace_rows: Vec<String> = Vec::new();
    // (onechunk 11:16) every napper's cart dropping into a lane not its own, with its split turns
    let mut napper_wrong_lane: Vec<String> = Vec::new();
    // (G1's P16) every napper's cart dropping into any lane (the misroute bar's denominator)
    let mut napper_drops = 0usize;
    // (day-1 11:36) every cart's entry into a split junction's cell: (tick, x, class, napper's lane)
    let mut junction_passes: Vec<(i64, [i32; 3], &'static str, Option<usize>, f64)> = Vec::new();
    // (ironspec 12:58) every entry into the shared lift's mouth: (tick, cart, class)
    let mut lift_entries: Vec<(i64, u32, &'static str)> = Vec::new();
    // (dfb89499 14:19) every entry into the column's y-12 cell (-5,12,7): (tick, cart, class, pass or
    // lane 7's side entry)
    let mut y12_entries: Vec<(i64, u32, &'static str, &'static str)> = Vec::new();
    // (G1 15:37) --positions-at: every cart's position at those ticks
    let mut positions_log: Vec<String> = Vec::new();
    // (G1 16:18 (2)) the fold's `# integrity:` line from --integrity-from FILE@dx,dy,dz: (cell, the shape
    // it must hold, the entry), and every tick a declared cell held another shape
    let mut integrity_bad: Vec<String> = Vec::new();
    let integrity: Vec<([i32; 3], String, String)> = match &o.integrity_from {
        None => Vec::new(),
        Some((path, mv)) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("--integrity-from {}: {e}", path.display()))?;
            parse_integrity(&text, *mv)?
        }
    };
    // (ironspec 15:17) each lipclass lip arrival's returning napper (its rider; None: a batch cart),
    // pushed with lipc_arrivals; each K window's napper (ctl.k_windows' twin: the napper's ridden cart
    // on the hold (9,0,8) at the H2 sample that keyed it)
    let mut lipc_who: [Vec<Option<u32>>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut kwin_napper: Vec<Option<u32>> = Vec::new();
    // (dfb89499 15:11, ironspec 15:23 (5) for 7b90c3f1) the station's touching contacts, the cart-cart
    // contacts and the napper's seat on the hold, sampled every tick
    let mut touch = TouchSampler::default();
    // (ironspec 11:48 / 11:50) per lane each returning napper's stored speed entering its own K (speed,
    // tick, cart), and each napper cart's first rest on its siding's R (z in the design frame, tick,
    // cart); the open R visits (cart -> resting already recorded)
    let mut k_speeds: [Vec<(f64, i64, u32)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut r_rests: [Vec<(f64, i64, u32)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut r_rest_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    // (ironspec 11:23, R4) the westbound carts on the split row: the open episodes and each one's row
    let mut westbound_open: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut westbound_rows: Vec<String> = Vec::new();
    let mut c_deflections: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut lip_rests = [0i64; 3];
    // (ironspec 09:21-09:25 platform) every mob-on-cart push: tick, cart, its lane at t0, the pusher's
    // activity, |impulse|, its component along the cart's motion
    let mut pushes: Vec<(i64, u32, Option<usize>, String, f64, f64)> = Vec::new();
    redsim::entity::minecart::log_cart_pushes(&mut run.world);
    redsim::entity::minecart::log_cart_cart_pushes(&mut run.world);
    // (ironspec 09:28) this tick's pushes as the trace's +push fields, and their (pusher, pushed) ids
    let mut tick_pushes: Vec<String> = Vec::new();
    let mut tick_push_ids: Vec<(u32, u32)> = Vec::new();
    // (G1 11:04) the pushes of the last 40 gt: (tick, pusher, pushed cart, the pusher's vehicle and
    // that vehicle's cell)
    let mut push_hist: Vec<(i64, u32, u32, Option<(u32, Option<[i32; 3]>)>)> = Vec::new();
    // (ironspec 08:23 platform (2), G1 08:22 PT) the HOME losses by the release branch, and all
    let mut release_branches: i64 = 0;
    let mut home_lost_total: i64 = 0;
    // (G1 / ironspec 08:41 platform) the unrouted bar's row count (the strand control's own sample)
    let mut unrouted_total: usize = 0;
    // (ironspec 08:26 / 08:41 platform) the late rejoins: a napper on its siding's R that missed a
    // release (back in its lane more than a D period + its 30-gt window after reaching R): napper, R,
    // back
    let mut late_rejoin: Vec<(u32, i64, i64)> = Vec::new();
    // (ironspec 06:31 / 06:42 real) the ticket lines: the bed's free tickets and the HOME memory
    // holders at the start and at every change
    let mut ticket_prev: Option<(Option<u32>, Vec<u32>)> = Some(start_ticket);
    let mut ticket_rows: Vec<String> = Vec::new();
    // (ironfarm / G1, farmopt 06:10 PT) each lane's queue occupancy per tick (carts on H (x,16,0),
    // R_1 (x,17,1) and the five slopes (x,18..22,2..6)), V (the slope top (x,22,6)) flagged
    let mut occ_hist: [BTreeMap<i64, i64>; 3] = Default::default();
    let mut v_ticks = [0i64; 3];
    let mut v_rows: Vec<String> = Vec::new();
    // (ironspec 06:11 platform) any V occupancy at night: its own row per stretch (the lane, the
    // occupant, its activity at every tick until it leaves V or dawn, its stamp and HOME then)
    let mut v_night: [Option<(i64, u32, String, Option<i64>, bool)>; 3] = Default::default();
    let mut v_night_rows: Vec<String> = Vec::new();
    // (ironspec 06:33 platform) every distinct night eye position of a rider: [ticks, REST, PANIC]
    let mut eye_hist: BTreeMap<[i64; 3], [i64; 3]> = BTreeMap::new();
    let mut rest_near_rows: Vec<String> = Vec::new();
    let mut bed_ever_occupied = false;
    // (ironspec D5(a)) REST reads of a rider with no exact brain state (counted, never "not REST")
    let mut brainless_reads = 0i64;
    let mut golem_birth_at: BTreeMap<u32, ([f64; 3], String)> = BTreeMap::new();
    let mut golem_summon: BTreeMap<u32, (i64, u32, u32, Option<[i32; 3]>, Option<[f64; 3]>)> = BTreeMap::new();
    #[allow(clippy::type_complexity)]
    let mut golem_tries: Vec<(i64, u32, u32, Option<u32>, u8, [i32; 3], u8)> = Vec::new();
    // (ironspec 22:54 PT) each gossip attempt's caller activity, by (tick, villager)
    let mut gossip_activity: BTreeMap<(i64, u32), String> = BTreeMap::new();
    // (ironspec 22:56 / 22:58 PT, the loud bars) every cart's cell last tick; the park's state at
    // each arrival (tick, braked, rider); riderless carts past A6's R (tick, cart)
    let mut cart_prev_cell: BTreeMap<u32, [i32; 3]> = BTreeMap::new();
    let mut park_arrivals: Vec<(i64, bool, Option<u32>)> = Vec::new();
    let mut riderless_r: Vec<(i64, u32)> = Vec::new();
    // (ironspec 23:44 / 23:47 PT, B4) carts other than the picked napper's on the leg's cells
    let mut leg_wrong: Vec<(i64, u32, Option<u32>)> = Vec::new();
    // (G1 / ironspec 19:51) the calls short of willing: (tick, caller, willing, threshold, from)
    // (G1 / ironspec 19:51, 19:58) the summon-tick events (threshold 3) in their order: (tick, kind
    // 0 a roll / 1 the caller not willing / 2 short of willing, caller, its block, a golem, willing,
    // the spot tries)
    #[allow(clippy::type_complexity)]
    let mut s_events: Vec<(i64, u8, u32, [i32; 3], bool, u32, u8)> = Vec::new();
    // (ironspec 19:24) the direct summon check: the summon ticks checked, each pocket's misses, the
    // first rows
    let mut direct = Direct::default();
    // (ironspec 19:58 PT (2b)) the split's Ts watched each tick
    let mut tsplit = TSplit::default();
    // (ironspec 00:27 PT) every fluid cell's tick scheduled at the start (a dry cell that a fluid's
    // own tick would fill, like C7's leak, shows in the run), the count printed
    let (mut fluid_water, mut fluid_lava) = (0usize, 0usize);
    {
        let w = &mut run.world;
        let (mn, sz) = (w.min, w.size);
        for x in mn.x..mn.x + sz[0] {
            for y in mn.y..mn.y + sz[1] {
                for z in mn.z..mn.z + sz[2] {
                    // (the world's own bounds: a world position, not a design cell)
                    let p = redsim::pos::BlockPos::new(x, y, z);
                    let fs = redsim::fluid::fluid_state(w, p);
                    if fs.fluid != redsim::fluid::Fluid::Empty {
                        let d = redsim::fluid::tick_delay(w, fs.fluid);
                        w.schedule_fluid_tick(p, fs.fluid, d);
                        if fs.fluid.is_water() {
                            fluid_water += 1;
                        } else {
                            fluid_lava += 1;
                        }
                    }
                }
            }
        }
    }
    // (ironspec 23:16 PT) the declared inert / always-on cells and their states at the start; (G1)
    // each one's starting block printed and checked against the kind its reason names (a torch,
    // a button, a lever, a redstone block, a repeater, a comparator, an observer, a daylight
    // detector; else not air); `--inert-control` adds J(-1)'s lever (-1,26,9), which the run
    // toggles, as inert: the run must fail
    let mut inert_cells = o.inert.clone();
    if o.inert_control {
        inert_cells.push([-1, 26, 9]);
    }
    let mut watched: Vec<([i32; 3], &'static str, String)> = inert_cells
        .iter()
        .map(|c| (*c, "inert"))
        .chain(o.always_on.iter().map(|c| (*c, "always-on")))
        .map(|(c, k)| (c, k, run.world.get(dbp(c[0], c[1], c[2])).to_string()))
        .collect();
    let mut watched_start: Vec<String> = Vec::new();
    let mut watched_kind_bad: Vec<String> = Vec::new();
    for (c, k, st) in &watched {
        let reason = o.watch_reasons.iter().find(|(rc, _)| rc == c).map(|(_, r)| r.to_lowercase()).unwrap_or_default();
        let want = [("torch", "torch"), ("button", "button"), ("lever", "lever"), ("redstone block", "redstone_block"), ("repeater", "repeater"), ("comparator", "comparator"), ("observer", "observer"), ("daylight", "daylight_detector")]
            .iter()
            .find(|(word, _)| reason.contains(word))
            .map(|(_, b)| *b);
        let ok = match want {
            Some(b) => st.contains(b),
            None => !st.contains("minecraft:air") && !st.is_empty(),
        };
        watched_start.push(format!("({},{},{}) {k} {}", c[0], c[1], c[2], st.trim_start_matches("minecraft:")));
        if !ok {
            watched_kind_bad.push(format!("({},{},{}) {k} is {} (declared: {})", c[0], c[1], c[2], st.trim_start_matches("minecraft:"), want.unwrap_or("not air")));
        }
    }
    let mut watched_changes: Vec<String> = Vec::new();
    let mut watched_n = 0usize;
    // (ironspec 23:51 PT (2)) the push pairs' resting carts: cart -> (cell, rest position, bound)
    let mut pp_rest: BTreeMap<u32, ([i32; 3], redsim::entity::Vec3, f64)> = BTreeMap::new();
    let (mut pp_rests, mut pp_fails) = (0i64, 0i64);
    let mut pp_fail: Vec<String> = Vec::new();
    let mut ev_seen = 0usize;
    let mut renew = Renewals::default();
    let mut willing_at: (i64, std::collections::BTreeSet<u32>) = (-1, std::collections::BTreeSet::new());
    let mut callers_now: Vec<(u32, i64)> = Vec::new();
    let mut ball_exits: Vec<(String, i64)> = Vec::new();
    // (ironspec 19:29 (a) / (b)) every napper's 16-ball exit after R (tick, villager), and the stop's
    // track holding two napper carts at once (the first tick, the riders; the ticks)
    let mut exit_log: Vec<(i64, u32)> = Vec::new();
    let mut stop_two: Option<(i64, Vec<u32>)> = None;
    let mut stop_two_ticks = 0i64;
    // (ironspec 19:31 PT (5)) each napper's entry at A6's entry booster: (tick, villager, speed)
    let mut a6_entries: Vec<(i64, u32, f64)> = Vec::new();
    let mut nights: BTreeMap<i64, std::collections::BTreeSet<u32>> = BTreeMap::new();
    let day_start = run.world.day_time();
    let mut nap_times: BTreeMap<u32, Vec<i64>> = BTreeMap::new();
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
        if o.napper && !ctl.wayin {
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
        // (tick-exact 06:09 PT, the reconciliation row: its farm-c7 placement)
        if let Some(n) = o.c7_placement {
            let c0 = 20 + 30 * n as i64 + 200;
            let w = &mut run.world;
            if rel == 0 {
                // (its overrides, each laid and checked changed: REST_l on, DAY off, NOT-ready on, on
                // record7f8's glass supports)
                for (c, st) in [
                    ([6, -13, -1], "minecraft:glass"),
                    ([6, -12, -1], "minecraft:lever[face=floor,facing=north,powered=true]"),
                    ([4, -13, -2], "minecraft:glass"),
                    ([4, -12, -2], "minecraft:lever[face=floor,facing=north,powered=false]"),
                    ([8, -14, -5], "minecraft:lever[face=wall,facing=west,powered=true]"),
                ] {
                    let p = dbp(c[0], c[1], c[2]);
                    let before = w.get(p).to_string();
                    w.set_block(p, redsim::block::Block::parse(st).map_err(|e| format!("{st}: {e}"))?, redsim::world::flags::ALL);
                    let after = w.get(p).to_string();
                    log.push(format!("c7 override ({},{},{}): {before} -> {after}{}", c[0], c[1], c[2], if before == after { " (NO CHANGE: FAIL)" } else { "" }));
                }
                // (time set 13800, advance_time false: the night held; the run was built at 13,800, so
                // this write moves nothing: asserted)
                let before = w.day_time();
                w.mob.day_offset = 13_800 - w.time;
                w.mob.advance_time = false;
                log.push(format!("c7 t {}: the night held at day time {} (it read {before} before the hold{})", rel - c0, w.day_time(), if before == 13_800 { "" } else { ": the day MOVED after the brains were built: FAIL" }));
            }
            if rel >= 20 && rel < 20 + 30 * n as i64 && (rel - 20) % 30 == 0 {
                // (a round: one cart a lane at (x + 0.6, 24.0, 6.49), a villager with HOME riding it)
                let round = (rel - 20) / 30;
                for x in LANES {
                    let cart = redsim::entity::spawn(w, "minecraft:minecart", [x as f64 + 0.6, 24.0, 6.49], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
                    let ci = redsim::entity::index_of(w, cart).ok_or("a c7 cart vanished")?;
                    let vid = *w.entities.list[ci].passengers.first().ok_or("a c7 cart has no rider")?;
                    let vi = redsim::entity::index_of(w, vid).ok_or("a c7 rider vanished")?;
                    redsim::entity::position_rider(w, ci, vi);
                    redsim::entity::mob::ai::brain::brain_mut(w, vi).home = Some(bed);
                    recs.insert(vid, Rec { label: format!("c7r{}x{x} (id {vid}, round {round}, lane x {x})", round + 1), home_at_start: Some([bed.x, bed.y, bed.z]), prev_home: Some([bed.x, bed.y, bed.z]), ..Default::default() });
                }
            }
            if rel == c0 {
                // (the stored queue cart and its rider killed, then the napper launched)
                let q = [9, o.bed[1] + 13, -4];
                let stored: Vec<(u32, Option<u32>)> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == q).map(|e| (e.id, e.passengers.first().copied())).collect();
                for (cart, rider) in stored {
                    if let Some(v) = rider {
                        redsim::entity::kill(w, v)?;
                        recs.remove(&v);
                    }
                    redsim::entity::kill(w, cart)?;
                }
                let cart = redsim::entity::spawn(w, "minecraft:minecart", [8.5, 25.0625, 9.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
                let ci = redsim::entity::index_of(w, cart).ok_or("the c7 napper's cart vanished")?;
                w.entities.list[ci].delta_movement = redsim::entity::Vec3::new(0.2, 0.0, 0.0);
                let vid = *w.entities.list[ci].passengers.first().ok_or("the c7 napper's cart has no rider")?;
                let vi = redsim::entity::index_of(w, vid).ok_or("the c7 napper vanished")?;
                redsim::entity::position_rider(w, ci, vi);
                redsim::entity::mob::ai::brain::brain_mut(w, vi).home = Some(bed);
                recs.insert(vid, Rec { label: format!("c7napper (id {vid}, launched at (8.5,25.0625,9.5))"), home_at_start: Some([bed.x, bed.y, bed.z]), prev_home: Some([bed.x, bed.y, bed.z]), ..Default::default() });
                log.push(format!("c7 t 0 = run t {rel}: the stored queue cart killed, the napper #{vid} launched"));
            }
            if rel == c0 + 117 {
                let p = dbp(8, -14, -5);
                w.set_block(p, redsim::block::Block::parse("minecraft:lever[face=wall,facing=west,powered=false]").map_err(|e| e.to_string())?, redsim::world::flags::ALL);
                log.push("c7 t 117: the release lever (8,-14,-5) off".to_string());
            }
            if rel == c0 + 137 {
                // (the night skip: time set 24000, advance_time true)
                w.mob.day_offset = 24_000 - w.time;
                w.mob.advance_time = true;
                log.push(format!("c7 t 137: the night skipped to day time {}, the time running", w.day_time()));
            }
        }
        // (ironspec 08:26 / 08:41 platform: the fail-safe's control, the lap-late rejoin) the first
        // ridden cart reaching a siding's last cell before its R, (sx,25,8), after t 1,000 is held
        // there 130 gt (more than a D period and its 30-gt release window): it must miss that release,
        // wait on R under the built latch, leave at the lane's next D press and pass the unrouted bar
        // (G1 11:19 F4 (c), the napper path's forced positives) --k-misroute-control: the first lane-7
        // napper's cart reaching (0,25,9) eastbound after t 1,000 finds K(3)'s lever (1,26,9) switched
        // on for 20 gt; --westbound-control: the first ridden cart on (2,25,9) eastbound after t 1,000
        // has its motion reversed (a westbound cart on the split row). With F4 both leave every lane
        // count unchanged.
        if o.k_misroute_control && kmis.is_none() && rel >= 1_000 {
            let w = &mut run.world;
            let hit = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [0, 25, 9] && e.delta_movement.x > 0.0 && e.passengers.first().is_some_and(|r| nappers.iter().rev().find(|(v, _, _)| v == r).is_some_and(|(_, nk, _)| *nk == 2))).map(|e| e.id);
            if let Some(id) = hit {
                let s0 = w.get(dbp(1, 26, 9)).to_string();
                // (G1's P5) act only on K(3)'s lever OFF (never a silent no-op), read the write back
                if s0.contains("lever") && s0.contains("powered=false") {
                    set(w, [1, 26, 9], &s0.replace("powered=false", "powered=true"), redsim::world::flags::ALL)?;
                    let s1 = w.get(dbp(1, 26, 9)).to_string();
                    controls_done.push(format!("the K-misroute control at t {rel}: lane 7's napper cart #{id} at (0,25,9) eastbound; K(3)'s lever (1,26,9) {s0} -> {s1}{}", if s1.contains("powered=true") { "" } else { " (the write did nothing: FAIL)" }));
                    kmis_ok = s1.contains("powered=true");
                    kmis = Some((rel, id, s0));
                }
            }
        }
        if let Some((t, _, s0)) = kmis.as_ref()
            && rel == t + 20
        {
            // (G1's P5) restored only if K(3)'s lever still holds the control's write: a controller
            // write since is kept
            let now_s = run.world.get(dbp(1, 26, 9)).to_string();
            if now_s == s0.replace("powered=false", "powered=true") {
                set(&mut run.world, [1, 26, 9], s0, redsim::world::flags::ALL)?;
                controls_done.push(format!("the K-misroute control at t {rel}: K(3)'s lever restored to {s0}"));
            } else {
                controls_done.push(format!("the K-misroute control at t {rel}: K(3)'s lever now {now_s} (the controller wrote it): not restored"));
            }
        }
        if o.westbound_control && westbound.is_none() && rel >= 1_000 {
            let w = &mut run.world;
            if let Some(i) = w.entities.list.iter().position(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [2, 25, 9] && e.delta_movement.x > 0.05 && !e.passengers.is_empty()) {
                let d = w.entities.list[i].delta_movement;
                w.entities.list[i].delta_movement = redsim::entity::Vec3::new(-d.x, d.y, d.z);
                let id = w.entities.list[i].id;
                westbound = Some((rel, id));
                controls_done.push(format!("the westbound control at t {rel}: cart #{id} on (2,25,9) reversed to {:.3} b/t west", d.x));
            }
        }
        // (G1 10:34, the forced-lip row) --lip-hold N: the first cart reaching a lane's drop cell
        // (x,24,6) after t 1,000 is held there N gt (its position pinned, its motion zeroed), then let go
        // (ironspec 10:51 (f)) --siding-plant-control: at t 1,000 a ridden lane cart whose rider isn't a
        // napper is planted at rest on lane -1's siding R (-3,24,7)
        // (G1's P1 controls) --vholds-control rest: from t 1,000 a ridden lane-3 batch cart is pinned at
        // rest in lane -1's column cell (-1,23,6) until lane -1's next pass window closes; moving: at
        // the first lane -1 pass window opening after t 1,000, a ridden lane-3 batch cart is put falling
        // through (-1,23,6); either must be CAUGHT by the lip-pass bar on its own sample
        if let Some(mode) = o.vholds_control.as_deref() {
            let w = &mut run.world;
            if vhold.is_none() && rel >= 1_000 && (mode == "rest" || pass_open.values().any(|pw| pw.lane == 0 && rel - pw.start <= 1)) {
                let pick = w.entities.list.iter().enumerate().find(|(_, e)| {
                    let c = cell(e.pos);
                    !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && c[0] == 3 && (17..=21).contains(&c[1]) && e.passengers.first().is_some_and(|r| !nappers.iter().any(|(v, _, _)| v == r))
                }).map(|(i, e)| (i, e.id));
                if let Some((i, id)) = pick {
                    if mode == "rest" {
                        w.entities.set_pos(i, wpos(-0.5, 23.0, 6.5));
                        w.entities.list[i].delta_movement = redsim::entity::Vec3::ZERO;
                    } else {
                        w.entities.set_pos(i, wpos(-0.5, 23.8, 6.5));
                        w.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, -0.3, 0.0);
                    }
                    vhold = Some((id, rel));
                    controls_done.push(format!("the V-holds control ({mode}) at t {rel}: batch cart #{id} put {} lane -1's column cell (-1,23,6)", if mode == "rest" { "at rest in" } else { "falling through" }));
                }
            }
            // the rest form stays pinned until a lane -1 pass window opened after it has closed
            if mode == "rest"
                && let Some((id, t)) = vhold
                && !vhold_done
            {
                if pass_judged[0] > vhold_judged0 && !pass_open.values().any(|pw| pw.lane == 0) {
                    vhold_done = true;
                } else if let Some(i) = redsim::entity::index_of(w, id) {
                    w.entities.set_pos(i, wpos(-0.5, 23.0, 6.5));
                    w.entities.list[i].delta_movement = redsim::entity::Vec3::ZERO;
                }
                if rel == t {
                    vhold_judged0 = pass_judged[0];
                }
            }
        }
        if o.siding_plant_control && siding_plant.is_none() && rel >= 1_000 {
            let w = &mut run.world;
            let pick = w.entities.list.iter().enumerate().find(|(_, e)| {
                let c = cell(e.pos);
                !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && c[0] == 3 && (17..=22).contains(&c[1]) && e.passengers.first().is_some_and(|r| !nappers.iter().any(|(v, _, _)| v == r))
            }).map(|(i, e)| (i, e.id));
            if let Some((i, id)) = pick {
                w.entities.set_pos(i, wpos(-2.5, 24.0625, 7.5));
                w.entities.list[i].delta_movement = redsim::entity::Vec3::ZERO;
                siding_plant = Some((id, rel));
                controls_done.push(format!("the siding plant at t {rel}: batch cart #{id} planted at rest on lane -1's siding R (-3,24,7)"));
            }
        }
        if let Some(n) = o.lip_hold {
            let w = &mut run.world;
            if lip_hold_cart.is_none() && rel >= 1_000 {
                if let Some(e) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && LANES.iter().any(|x| cell(e.pos) == [*x, 24, 6])) {
                    lip_hold_cart = Some((e.id, rel, [dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z]));
                    controls_done.push(format!("the lip hold at t {rel}: cart #{} held {n} gt on the drop cell ({},24,6)", e.id, cell(e.pos)[0]));
                }
            }
            if let Some((cart, t, p)) = lip_hold_cart
                && rel < t + n
                && let Some(ci) = redsim::entity::index_of(w, cart)
            {
                w.entities.set_pos(ci, wpos(p[0], p[1], p[2]));
                w.entities.list[ci].delta_movement = redsim::entity::Vec3::ZERO;
            }
        }
        if o.late_return_control {
            // (G1 10:47 O4: deterministic, not by phase chance; no uncited length) the napper's cart is
            // held at its siding's last cell before R until a release window of its lane has OPENED
            // AND CLOSED since the hold began (it misses exactly that release), then let go
            let w = &mut run.world;
            if late_hold.is_none() && rel >= 1_000 {
                if let Some(e) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r)) && LANES.iter().any(|x| cell(e.pos) == [x - 2, 25, 8])) {
                    late_hold = Some((e.id, rel, [dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z]));
                    late_lane = LANES.iter().position(|x| cell(e.pos)[0] == *x - 2);
                    late_rider = e.passengers.first().copied();
                    late_phase = 0;
                    controls_done.push(format!("the late-return control at t {rel}: napper cart #{} (lane x {}) held at its siding's last cell before R until one of its lane's release windows opens and closes", e.id, late_lane.map_or(0, |k| LANES[k])));
                }
            }
            if let (Some((cart, t_hold, p)), Some(k)) = (late_hold, late_lane)
                && late_phase < 2
                && let Some(ci) = redsim::entity::index_of(w, cart)
            {
                let open = ctl.siding_on[k].is_some() || ctl.siding_off[k].is_some();
                if late_phase == 0 && open {
                    late_phase = 1;
                    controls_done.push(format!("the late-return control at t {rel}: lane x {}'s release window opened with the napper cart held", LANES[k]));
                } else if late_phase == 1 && !open {
                    late_phase = 2;
                    controls_done.push(format!("the late-return control at t {rel}: lane x {}'s release window closed: the napper cart let go", LANES[k]));
                } else if rel - t_hold > 300 {
                    // (never a hold for good: no window opened and closed within three D periods)
                    late_phase = 2;
                    late_timed_out = true;
                    controls_done.push(format!("the late-return control at t {rel}: no release window of lane x {} opened and closed within 300 gt of the hold (phase {}): the napper cart let go (the control can't bite)", LANES[k], if open { 1 } else { 0 }));
                }
                if late_phase < 2 {
                    w.entities.set_pos(ci, wpos(p[0], p[1], p[2]));
                    w.entities.list[ci].delta_movement = redsim::entity::Vec3::ZERO;
                }
            }
        }
        // (ironspec 06:38 platform, 7b90c3f1's r4 bench: the lift exit blocked) an empty cart held
        // still on the exit (-5,23,8) from T for N gt
        if let Some((tb, nb)) = o.lift_block {
            let w = &mut run.world;
            if rel == tb {
                let cart = redsim::entity::spawn(w, "minecraft:minecart", [-4.5, 23.0625, 8.5], None)?;
                lift_blocker = Some(cart);
                // (G1 X35) the riders in the lift column at the hold's start: the jam positive's own
                // sample
                lift_stack_at_hold = w
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && {
                        let c = cell(e.pos);
                        c[0] == -5 && (6..=8).contains(&c[2]) && c[1] >= 2
                    })
                    .flat_map(|e| e.passengers.iter().copied())
                    .collect();
                log.push(format!("t {rel}: the lift-block control: an empty cart #{cart} held on the exit (-5,23,8) for {nb} gt"));
                controls_done.push(format!("the lift-block control at t {rel}: the exit (-5,23,8) held {nb} gt"));
            }
            if let Some(cart) = lift_blocker
                && let Some(ci) = redsim::entity::index_of(w, cart)
            {
                if rel < tb + nb {
                    w.entities.set_pos(ci, wpos(-4.5, 23.0625, 8.5));
                    w.entities.list[ci].delta_movement = redsim::entity::Vec3::ZERO;
                } else {
                    redsim::entity::kill(w, cart)?;
                    lift_blocker = None;
                    log.push(format!("t {rel}: the lift-block control: the exit freed"));
                }
            }
            // (G1 07:59 PT: the bench difference named) every cart in the lift column and its top
            // (x -5, z 6..8, y >= 2), from 5 gt before the hold to 150 gt after its start: id,
            // position, motion, its rider
            if rel >= tb - 5 && rel <= tb + 150 {
                let carts: Vec<String> = w
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && Some(e.id) != lift_blocker && {
                        let c = cell(e.pos);
                        c[0] == -5 && (6..=8).contains(&c[2]) && c[1] >= 2
                    })
                    .map(|e| format!("#{} ({:.3},{:.3},{:.3}) v({:.3},{:.3},{:.3}){}", e.id, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.passengers.is_empty() { " empty" } else { "" }))
                    .collect();
                lift_rows.push(format!("t {rel}: {}", if carts.is_empty() { "none".to_string() } else { carts.join(", ") }));
            }
        }
        if o.restarts.contains(&rel) {
            // (G1 10:47 blocking 1) the ids live before the open: one of them the open doesn't renew
            // is an error, never kept silently
            let pre_live: std::collections::BTreeSet<u32> = run.world.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
            let m = crate::liveness::restart(&mut run.world, o.restart)?;
            let fol = |id: u32| -> Result<u32, String> { open_follow(id, &m, &pre_live).map_err(|e| format!("t {rel}: {e}")) };
            let mut open_log: Vec<String> = Vec::new();
            let pairs = |name: &str, old: &[u32], new: &[u32]| -> String { format!("{name}: {}", if old.is_empty() { "none".to_string() } else { old.iter().zip(new).map(|(a, b)| format!("#{a} -> #{b}")).collect::<Vec<_>>().join(", ") }) };
            let old = std::mem::take(&mut recs);
            let (mut ro, mut rn) = (Vec::new(), Vec::new());
            for (id, mut r) in old {
                // (G1 on f1507f5dc: a reload drops CANT_REACH_WALK_TARGET_SINCE, no codec: the
                // next tick's missing stamp is booked as "a reload", not as a clear)
                r.reloaded = true;
                let n = fol(id)?;
                ro.push(id);
                rn.push(n);
                recs.insert(n, r);
            }
            open_log.push(pairs("the villagers' records", &ro, &rn));
            // (G1 / ironspec 08:31-08:33 platform: the emulation must not be keyed on an id a world
            // open renews) the controller's keys and the nappers' visit list follow their entities
            // across the open; each key's old and new id logged
            let keys = follow_open(&mut ctl, &mut nappers, fol)?;
            // (G1 X10 / X35 / X39; G1 10:47 O1) every other id the run keys on follows the open too,
            // each key's old -> new logged
            macro_rules! remap_opt {
                ($name:expr, $v:expr) => {
                    if let Some(id) = $v {
                        let n = fol(id)?;
                        open_log.push(pairs($name, &[id], &[n]));
                        $v = Some(n);
                    }
                };
            }
            remap_opt!("the lift blocker", lift_blocker);
            remap_opt!("the late-return rider", late_rider);
            remap_opt!("the stamp control's rider", stamp_control_rider);
            remap_opt!("the at-entry control's rider", at_entry_rider);
            if let Some((c, t, p)) = late_hold {
                let n = fol(c)?;
                open_log.push(pairs("the late-return cart", &[c], &[n]));
                late_hold = Some((n, t, p));
            }
            if let Some((c, t, p)) = lip_hold_cart {
                let n = fol(c)?;
                open_log.push(pairs("the lip hold's cart", &[c], &[n]));
                lip_hold_cart = Some((n, t, p));
            }
            if let Some(h) = air_held.as_mut() {
                let (a, b) = (fol(h.0)?, fol(h.3)?);
                open_log.push(pairs("the air control's rider and cart", &[h.0, h.3], &[a, b]));
                h.0 = a;
                h.3 = b;
            }
            if let Some((v, at)) = bystander_target {
                let n = fol(v)?;
                open_log.push(pairs("the bystander target", &[v], &[n]));
                bystander_target = Some((n, at));
            }
            macro_rules! remap_vec {
                ($name:expr, $v:expr) => {{
                    let old: Vec<u32> = $v.iter().copied().collect();
                    let new: Vec<u32> = old.iter().map(|x| fol(*x)).collect::<Result<_, _>>()?;
                    open_log.push(pairs($name, &old, &new));
                    $v = new.into_iter().collect();
                }};
            }
            remap_vec!("the air control's stack", air_stacked);
            remap_vec!("the scene's spawn list", lines);
            remap_vec!("the stop's HOME-less ids", stop_homeless_ids);
            remap_vec!("the at-entry bar's over-age ids", at_entry_over_ids);
            remap_vec!("the lift-block stack", lift_stack_at_hold);
            macro_rules! remap_keys {
                ($name:expr, $m:expr) => {{
                    let old = std::mem::take(&mut $m);
                    let (mut a, mut b) = (Vec::new(), Vec::new());
                    for (id, v) in old {
                        let n = fol(id)?;
                        a.push(id);
                        b.push(n);
                        $m.insert(n, v);
                    }
                    open_log.push(pairs($name, &a, &b));
                }};
            }
            remap_keys!("the lane carts' t0 lanes", cart_lane0);
            remap_keys!("the visits' marks", past_marks);
            remap_keys!("the carts' cell entries", cart_entries);
            remap_keys!("the carts' still flags", still_prev);
            remap_keys!("the carts' previous cells", c_prev_cell);
            remap_keys!("the off-rail rests", off_rest);
            remap_keys!("the off-rail rests' first-tick prints", off_rest_snap);
            remap_keys!("the H rests open", h_open);
            remap_vec!("the siding's batch carts", siding_batch_ids);
            remap_vec!("the westbound episodes open", westbound_open);
            // (G1's P8) the 29th-31st's id-keyed state follows the open too
            remap_keys!("the judged HOME-target sets", judged_sets);
            remap_keys!("the nap times", nap_times);
            {
                let (mut a, mut b) = (Vec::new(), Vec::new());
                for set in nights.values_mut() {
                    let old = std::mem::take(set);
                    for id in old {
                        let n = fol(id)?;
                        a.push(id);
                        b.push(n);
                        set.insert(n);
                    }
                }
                open_log.push(pairs("the nights' nappers", &a, &b));
            }
            remap_opt!("the farm-stop control's erased napper", home_erased_id);
            if let Some((t, c, s0)) = kmis.take() {
                let n = fol(c)?;
                open_log.push(pairs("the K-misroute control's cart", &[c], &[n]));
                kmis = Some((t, n, s0));
            }
            if let Some((t, c)) = westbound {
                let n = fol(c)?;
                open_log.push(pairs("the westbound control's cart", &[c], &[n]));
                westbound = Some((t, n));
            }
            if let Some((c, t)) = vhold {
                let n = fol(c)?;
                open_log.push(pairs("the V-holds control's cart", &[c], &[n]));
                vhold = Some((n, t));
            }
            if let Some((t, k, r)) = ctl.eject_done {
                let n = fol(r)?;
                open_log.push(pairs("the eject control's napper", &[r], &[n]));
                ctl.eject_done = Some((t, k, n));
            }
            {
                // the tuple lists keyed by an id in one field; (G1's P9) a history entry naming an id
                // the open didn't renew (dead before it) is dropped from the rolling push window, and
                // kept as it was in printed rows, never aliased into a judged key
                let lenient = |id: u32| -> u32 { m.get(&id).copied().unwrap_or(id) };
                let (mut a, mut b) = (Vec::new(), Vec::new());
                push_hist.retain(|p| m.contains_key(&p.1) && m.contains_key(&p.2));
                for p in push_hist.iter_mut() {
                    let (m1, c1) = (lenient(p.1), lenient(p.2));
                    a.extend([p.1, p.2]);
                    b.extend([m1, c1]);
                    p.1 = m1;
                    p.2 = c1;
                    if let Some((v, cc)) = p.3 {
                        p.3 = Some((lenient(v), cc));
                    }
                }
                for lane in c_arrivals.iter_mut() {
                    for x in lane.iter_mut() {
                        let n = lenient(x.3);
                        a.push(x.3);
                        b.push(n);
                        x.3 = n;
                    }
                }
                for x in lip_arrivals.iter_mut() {
                    let n = lenient(x.2);
                    a.push(x.2);
                    b.push(n);
                    x.2 = n;
                }
                for x in h_rests.iter_mut() {
                    let n = lenient(x.3);
                    a.push(x.3);
                    b.push(n);
                    x.3 = n;
                }
                for x in env_passes.iter_mut() {
                    let n = lenient(x.5);
                    a.push(x.5);
                    b.push(n);
                    x.5 = n;
                }
                for lane in k_speeds.iter_mut().chain(r_rests.iter_mut()) {
                    for x in lane.iter_mut() {
                        let n = lenient(x.2);
                        a.push(x.2);
                        b.push(n);
                        x.2 = n;
                    }
                }
                for lane in pass_rows.iter_mut() {
                    for r in lane.iter_mut() {
                        let n = lenient(r.arrival);
                        a.push(r.arrival);
                        b.push(n);
                        r.arrival = n;
                        for h in r.hits.iter_mut() {
                            h.2 = lenient(h.2);
                        }
                    }
                }
                for slot in r_unridden.iter_mut() {
                    if let Some((c, t, f)) = *slot {
                        let n = fol(c)?;
                        a.push(c);
                        b.push(n);
                        *slot = Some((n, t, f));
                    }
                }
                open_log.push(pairs("the pushes, C arrivals, lip arrivals, H rests, envelope passes, K speeds, R rests, lip-pass rows and unridden R carts", &a, &b));
            }
            remap_keys!("the open pass windows", pass_open);
            remap_vec!("the R visits' rests seen", r_rest_seen);
            if let Some((c, t)) = siding_plant {
                let n = fol(c)?;
                open_log.push(pairs("the siding plant's cart", &[c], &[n]));
                siding_plant = Some((n, t));
            }
            remap_keys!("the drop-cell passes", lip_state);
            {
                // (ironspec 15:17 / dfb89499 15:11 / ironspec 15:23 (5)) the touch prints' carts: the
                // sampler's open state, the K windows' nappers and the lip arrivals' nappers (a dead id
                // keeps its id)
                let lenient = |id: u32| -> u32 { m.get(&id).copied().unwrap_or(id) };
                let (a, b) = touch.remap(&lenient);
                open_log.push(pairs("the touch sampler's carts", &a, &b));
                let old: Vec<u32> = kwin_napper.iter().chain(lipc_who.iter().flatten()).flatten().copied().collect();
                for v in kwin_napper.iter_mut().chain(lipc_who.iter_mut().flatten()).flatten() {
                    *v = lenient(*v);
                }
                let new: Vec<u32> = kwin_napper.iter().chain(lipc_who.iter().flatten()).flatten().copied().collect();
                open_log.push(pairs("the K windows' and the lip arrivals' nappers", &old, &new));
            }
            {
                let old: Vec<u32> = at_r_log.iter().map(|r| r.0).collect();
                let new: Vec<u32> = old.iter().map(|x| fol(*x)).collect::<Result<_, _>>()?;
                for (r, n) in at_r_log.iter_mut().zip(&new) {
                    r.0 = *n;
                }
                open_log.push(pairs("the arrivals at R", &old, &new));
            }
            for (k, slot) in lip_seed_open.iter_mut().enumerate() {
                if let Some((t0, a, b)) = *slot {
                    let (na, nb) = (fol(a)?, fol(b)?);
                    open_log.push(pairs(&format!("lane x {}'s open cascade seed", LANES[k]), &[a, b], &[na, nb]));
                    *slot = Some((t0, na, nb));
                }
            }
            {
                let mut names: Vec<(u32, String)> = Vec::new();
                DRIVE_NAMES.with(|d| names = std::mem::take(&mut *d.borrow_mut()).into_iter().collect());
                let old: Vec<u32> = names.iter().map(|(id, _)| *id).collect();
                let new: Vec<u32> = old.iter().map(|x| fol(*x)).collect::<Result<_, _>>()?;
                open_log.push(pairs("the drive's names", &old, &new));
                DRIVE_NAMES.with(|d| *d.borrow_mut() = new.iter().copied().zip(names.into_iter().map(|(_, n)| n)).collect());
            }
            log.push(format!("t {rel}: the emulation's keys across the open: {}", keys.join("; ")));
            log.push(format!("t {rel}: the run's other keys across the open: {}", open_log.join("; ")));
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
            // (ironspec 19:24: the direct summon check) at each summon tick S, every pocket's rider
            // seated in its pocket, panicking, LAST_SLEPT within 24,000, no GOLEM_DETECTED_RECENTLY,
            // with 3 willing villagers in its 10-box (itself included); a pocket with none is a miss
            for (k, x) in LANES.iter().enumerate() {
                let riders: Vec<usize> = villagers.iter().copied().filter(|&i| cell(w.entities.list[i].pos) == [*x, 0, 0] && w.entities.list[i].vehicle.is_some()).collect();
                let mut why = "no rider seated in the pocket".to_string();
                let mut ok = false;
                for &i in &riders {
                    if !brain_of(i).is_some_and(|b| b.scared()) {
                        why = "its rider calm".to_string();
                        continue;
                    }
                    if !slept(i) {
                        why = "its rider's LAST_SLEPT stale".to_string();
                        continue;
                    }
                    if gdr(i) {
                        why = "its rider's GOLEM_DETECTED_RECENTLY set".to_string();
                        continue;
                    }
                    let bb = w.entities.list[i].bb.inflate(10.0, 10.0, 10.0);
                    let n = villagers.iter().filter(|&&j| bb.intersects(w.entities.list[j].bb) && slept(j) && !gdr(j)).count();
                    if n < 3 {
                        why = format!("{n} willing in its box");
                        continue;
                    }
                    ok = true;
                    break;
                }
                direct.checks += 1;
                if !ok {
                    direct.misses[k] += 1;
                    if direct.rows.len() < 12 {
                        direct.rows.push(format!("S t {}: lane x {x}: {why}", t - t0));
                    }
                }
            }
            // (ironspec 19:33 PT: the summon tick's class, decided after S by whether a golem was
            // born: the callers ready (>= 3 willing, free), blocked by their own memory, short of
            // willing)
            let (mut s_ready, mut s_blocked, mut s_few) = (0usize, 0usize, 0usize);
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
                    s_blocked += 1;
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
                // (ironspec 19:40 PT: the margin from the stamp's original tick, the caller's own
                // summon or a bystander stamp, bar >= 23; from the latest set, renewals included, bar
                // >= 3)
                if let Some(s) = recs.get(&w.entities.list[i].id).and_then(|r| r.last_stamp) {
                    let margin = t - (s + redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                    summons.least_orig_margin = Some(summons.least_orig_margin.map_or(margin, |m: i64| m.min(margin)));
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
                if counted >= 3 {
                    s_ready += 1;
                } else {
                    s_few += 1;
                }
                summons.rejected += rejected.len() as i64;
                // (the day's calls and the calls with 3 willing in the box: a summon expected)
                let day_k = w.day_time().div_euclid(24_000);
                let e = summons.by_day.entry(day_k).or_insert((0, 0));
                e.0 += 1;
                if counted >= 3 {
                    e.1 += 1;
                    // (ironspec 16:10, the recovery row: the first batch of three fresh villagers
                    // at the stand)
                    if summons.first_fresh3.is_none() {
                        summons.first_fresh3 = Some(t);
                    }
                }
                *summons.counted_hist.entry((day_k, counted.min(5))).or_insert(0) += 1;
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
            // (ironspec 19:33 PT) the class pending the golem's birth at S
            let locked = ctl.rels.last().is_some_and(|rl| (rl + t0).rem_euclid(100) == 5);
            let class = if s_ready > 0 {
                "spot miss"
            } else if s_blocked > 0 {
                "caller blocked by its own memory"
            } else if s_few > 0 {
                "fewer than 3 willing"
            } else {
                "no panicking caller"
            };
            // (ironspec 19:58 / 20:07 PT, tick-exact's official row) the pocket riders willing at
            // the tick's start (LAST_SLEPT fresh, no GOLEM_DETECTED_RECENTLY) and panicking: the
            // trigger ticks through s..s + 60 of its panic start s and misses S = s + 61 + 62k (a
            // "skip", not a block); the rest count as willing and ticking
            let (mut ticking, mut skip) = (0usize, 0usize);
            for &i in &villagers {
                let c = cell(w.entities.list[i].pos);
                if !(c[1] == 0 && c[2] == 0 && LANES.contains(&c[0])) || !brain_of(i).is_some_and(|b| b.scared()) || !slept(i) || gdr(i) {
                    continue;
                }
                let since = recs.get(&w.entities.list[i].id).and_then(|r| r.panic_since);
                if since.is_some_and(|p| t - p >= 61 && (t - p - 61).rem_euclid(62) == 0) {
                    skip += 1;
                } else {
                    ticking += 1;
                }
            }
            direct.skips += skip as i64;
            direct.pending = Some((t, class, locked, ticking));
        }
        // (ironspec 12:48) each caller's pending bystander stamp against its summon tick
        for (id, t) in callers_now.drain(..) {
            // (ironspec 14:41) a napper's first call back in its lane: its memory's last setting
            // against it (margin >= 20)
            if nappers.iter().any(|n| n.0 == id)
                && let Some(r) = recs.get_mut(&id)
                && r.nap_call.is_none()
                && r.milestones.iter().any(|(w, _)| w == "back in its lane")
            {
                r.nap_call = Some(match r.gdr_set {
                    Some(s) => {
                        // (ironspec 05:10 real (3), G1 N7: ONE verdict, the margin >= 3 from the latest
                        // set, whatever its source; the stamp's-original-tick margin (the old
                        // "bar >= 23" of ironspec 19:40 PT, superseded) prints as information)
                        let margin = t - (s + GOLEM_TTL);
                        let orig = r.last_stamp.map(|o| t - (o + GOLEM_TTL));
                        let pass = margin >= 3;
                        // (ironspec 02:37 PT: the harm bar reads every napper's first call back)
                        nap_margins.push((id, t - t0, Some(margin), r.gdr_setter.unwrap_or("?")));
                        format!(
                            "t {}, its memory last set t {} by {}: margin {margin:+} gt from the latest set (bar >= 3){} ({})",
                            t - t0,
                            s - t0,
                            r.gdr_setter.unwrap_or("?"),
                            orig.map_or(String::new(), |m| format!(", {m:+} gt from the stamp's original tick (information)")),
                            if pass { "PASS" } else { "FAIL" }
                        )
                    }
                    None => {
                        nap_margins.push((id, t - t0, None, "never set"));
                        format!("t {}, its memory never set: free", t - t0)
                    }
                });
            }
            if let Some(r) = recs.get_mut(&id)
                && let Some((s, p, trip)) = r.bystander_stamp.take()
            {
                let margin = t - (s + redsim::entity::mob::villager::GOLEM_MEMORY_TTL);
                renew.bystander_margin = Some(renew.bystander_margin.map_or(margin, |m: i64| m.min(margin)));
                if renew.bystander_rows.len() < 24 {
                    renew.bystander_rows.push(format!("{} stamped as a bystander at t {} at ({:.1},{:.1},{:.1}) ({trip}), its next summon t {}: {margin:+} gt past the expiry", r.label.split(' ').next().unwrap_or(""), s - t0, p[0], p[1], p[2], t - t0));
                }
            }
        }
        run.tick();
        if o.drive {
            // (G1 10:47 blocking 4) the nappers' ids for PRESENT_l's reset by id
            ctl.napper_ids = nappers.iter().map(|(v, _, _)| *v).collect();
            // (G1 10:47 blocking 4's control) --eject-control: the first napper seated on its siding's
            // R with PRESENT_l set after t 1,000 is dismounted there; PRESENT_l must stay set
            if ctl.eject_control && ctl.eject_done.is_none() && rel >= 1_000 {
                let w = &mut run.world;
                let hit = (0..3).find_map(|k| {
                    if !ctl.present[k] {
                        return None;
                    }
                    let sx = LANES[k] - 2;
                    w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [sx, 24, 7]).and_then(|e| e.passengers.first().copied()).filter(|r| ctl.napper_ids.contains(r)).map(|r| (k, r))
                });
                if let Some((k, r)) = hit
                    && let Some(i) = redsim::entity::index_of(w, r)
                {
                    redsim::entity::stop_riding(w, i);
                    ctl.eject_done = Some((rel, k, r));
                    controls_done.push(format!("the eject control at t {rel}: napper #{r} dismounted on lane x {}'s R with PRESENT_l set", LANES[k]));
                }
            }
            ctl.step(&mut run.world, rel)?;
            // (ironspec 15:17) each new K window's napper: the napper's ridden cart on the hold (9,0,8)
            // at the H2 sample that keyed it (ctl.k_windows' twin)
            while kwin_napper.len() < ctl.k_windows.len() {
                let napper = run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [9, 0, 8]).find_map(|e| e.passengers.first().copied().filter(|r| nappers.iter().any(|(v, _, _)| v == r)));
                kwin_napper.push(napper);
            }
            if let Some((t, k, _)) = ctl.eject_done
                && rel == t + 20
            {
                eject_present_after = Some(ctl.present[k]);
            }
            // (day-1's picker: the carts it sent down the leg are nappers now)
            for (vid, k, at) in std::mem::take(&mut ctl.new_nappers) {
                let n = nappers.len();
                if let Some(r) = recs.get_mut(&vid) {
                    r.label = format!("napper{n} (id {vid}, lane x {}, picked at t {at})", LANES[k]);
                    // (the per-pick trace reads every visit: the last visit's milestones kept)
                    past_marks.entry(vid).or_default().append(&mut r.milestones);
                    r.route.clear();
                    r.ball_exit_after_r = None;
                    // (the last trip's first call back kept with its stamps)
                    if let Some(c) = r.nap_call.take() {
                        r.nap_stamps.push(format!("its first call back after that trip {c}"));
                    }
                }
                nappers.push((vid, k, at));
            }
        }
        let w = &mut run.world;
        let now = w.time;
        let day = w.day_time();
        // (ironspec 06:31 / 06:42 real) the ticket lines: the bed's free tickets and the HOME
        // memory holders at every change
        if o.napper {
            for (k, x) in LANES.iter().enumerate() {
                let mut n = 0i64;
                let mut at_v = false;
                let mut v_rider: Option<u32> = None;
                for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                    let c = cell(e.pos);
                    if c == [*x, 16, 0] || c == [*x, 17, 1] || (0..5).any(|j| c == [*x, 18 + j, 2 + j]) {
                        n += 1;
                        if c == [*x, 22, 6] {
                            at_v = true;
                            v_rider = e.passengers.first().copied();
                        }
                    }
                }
                // (V at night: the occupant's activity per tick, its stamp and HOME)
                let night_now = (12_000..24_000).contains(&day.rem_euclid(24_000));
                let v_now = if night_now && at_v { v_rider } else { None };
                let state_of = |w: &redsim::world::World, v: u32| -> (char, Option<i64>, bool) {
                    let Some(j) = redsim::entity::index_of(w, v) else { return ('-', None, false) };
                    let b = redsim::entity::mob::ai::brain::brain(w, j);
                    let ex = b.ex.as_deref();
                    let a = ex.map_or('?', |x| match x.active {
                        redsim::entity::mob::ai::brain::Act::Panic => 'P',
                        redsim::entity::mob::ai::brain::Act::Rest => 'R',
                        _ => '.',
                    });
                    (a, ex.and_then(|x| x.mem.cant_reach_since).map(|s| now - s), b.home.is_some())
                };
                let open_v = v_night[k].take();
                match (open_v, v_now) {
                    (Some(mut s), Some(v)) if s.1 == v => {
                        let (a, st, h) = state_of(w, v);
                        s.2.push(a);
                        s.3 = st;
                        s.4 = h;
                        v_night[k] = Some(s);
                    }
                    (open_v, v_now) => {
                        if let Some(s) = open_v
                            && v_night_rows.len() < 12
                        {
                            v_night_rows.push(format!("lane x {x} from t {}: #{} {} (stamp then {}, HOME {})", s.0, s.1, rle(&s.2), s.3.map_or("unarmed".to_string(), |a| format!("{a} gt")), if s.4 { "kept" } else { "LOST" }));
                        }
                        if let Some(v) = v_now {
                            let (a, st, h) = state_of(w, v);
                            v_night[k] = Some((now - t0, v, a.to_string(), st, h));
                        }
                    }
                }
                *occ_hist[k].entry(n).or_default() += 1;
                if at_v {
                    v_ticks[k] += 1;
                    if v_rows.len() < 12 && (v_rows.is_empty() || !v_rows.last().is_some_and(|l| l.starts_with(&format!("lane x {x} ")))) {
                        v_rows.push(format!("lane x {x} t {} ({n} in the lane)", now - t0));
                    }
                }
            }
            let free = redsim::entity::mob::ai::poi::free_tickets(w, bed);
            let holders: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.home == Some(bed)))).map(|e| e.id).collect();
            // (ironspec 06:18 platform) the bed's state at every dusk (a free bed makes no village
            // centre, a held ticket does)
            if day.rem_euclid(24_000) == 12_000 && ticket_rows.len() < 24 {
                ticket_rows.push(format!("dusk t {}: free {}, holders {} (state {})", now - t0, free.map_or("-".to_string(), |f| f.to_string()), holders.len(), if free == Some(0) { "(ii), the ticket held" } else { "(i), the ticket free" }));
            }
            if let Some((pf, ph)) = &ticket_prev
                && (*pf != free || *ph != holders)
            {
                if ticket_rows.len() < 24 {
                    let lost: Vec<String> = ph.iter().filter(|v| !holders.contains(v)).map(|v| format!("#{v}")).collect();
                    let gained: Vec<String> = holders.iter().filter(|v| !ph.contains(v)).map(|v| format!("#{v}")).collect();
                    ticket_rows.push(format!(
                        "t {}: free {} -> {}, holders {} -> {}{}{}",
                        now - t0,
                        pf.map_or("-".to_string(), |f| f.to_string()),
                        free.map_or("-".to_string(), |f| f.to_string()),
                        ph.len(),
                        holders.len(),
                        if lost.is_empty() { String::new() } else { format!(", lost {}", lost.join(" ")) },
                        if gained.is_empty() { String::new() } else { format!(", gained {}", gained.join(" ")) }
                    ));
                }
                ticket_prev = Some((free, holders));
            }
        }
        let dawn = day < prev_day || (prev_day < 23_999 && day >= 23_999);
        prev_day = day;
        // (ironspec 23:56 PT (4) / 00:32 PT rule (A) refined, the bars' positive controls)
        // `--gossip-control`: a gossip attempt (path 5, 5 willing) logged from lane x -1's pocket at
        // t 1,000, in the event shape redsim's gossip path emits (cited: redsim's
        // `a_gossip_call_logs_its_threshold_and_caller` test), which the gossip bars must count;
        // `--bystander-control`: a REAL summon (`spawn_golem_if_needed`, threshold 3) called from a
        // pocket rider at the first ticks from t 1,000 with a rider out of its pocket inside the
        // caller's 10-box, until a golem is born: the summon stamps every villager in its 10-box, the
        // rider out of its pocket included, which the departing-bystander bar must count
        if o.gossip_control && now - t0 == 1_000 {
            let v = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [-1, 0, 0]).and_then(|e| e.passengers.first().copied());
            if let Some(villager) = v {
                w.mob.events.push(redsim::entity::mob::MobEvent::GolemTry { t: now, villager, willing: 5, golem: None, at: None, needed: 5, from: [-1, 0, 0], tries: Some(10) });
                controls_done.push(format!("the gossip control at t 1000 (villager {villager})"));
            }
        }
        if let Some(variant) = o.bystander_control.as_deref()
            && now - t0 >= 1_000
            && !controls_done.iter().any(|c| c.starts_with("the bystander control"))
            && bystander_tries < 400
        {
            // (ironspec 02:48 PT: the harm bar's control) a real summon stamping a napper: `late`, the
            // napper back in its own pocket before its first own-pocket S (the stamp 25..35 gt
            // before that S: the harm bar must FAIL); `early`, the same stamp planted at an S tick
            // on the napper's outbound trip (its picked cart before the divert, or its visit before
            // the queue), where a pocket rider's 10-box reaches it, before the whole round trip (the
            // summon's other stamps are the ones that S lays anyway; the harm bar must PASS)
            // (ironspec D7: the S phase from the run's ACTUAL S ticks; G1 X25: the target's own pocket's,
            // in own_to_s below)
            // (G1 N8: the early target is the napper of the current pick only; X26: the divert
            // fallback only for a divert at or after the latest pick whose napper isn't back in its
            // lane)
            let latest_pick = ctl.pick_log.last().map_or(i64::MIN, |p| p.0);
            let picked_v: Option<u32> = ctl.pick_cart.and_then(|(pc, _)| w.entities.list.iter().find(|e| e.id == pc).and_then(|e| e.passengers.first().copied())).or_else(|| {
                ctl.diverts().last().filter(|d| d.0 >= latest_pick && !recs.get(&d.1).is_some_and(|r| r.milestones.iter().any(|(m, _)| m == "back in its lane"))).map(|d| d.1)
            });
            // (G1 X25) the late variant's S phase is the TARGET's own pocket's latest try (none:
            // no target, the control not exercised)
            let own_to_s = |x: i32| -> Option<i64> {
                w.mob.events.iter().rev().find_map(|e| match e {
                    redsim::entity::mob::MobEvent::GolemTry { t, from, .. } if *from == [x, 0, 0] => Some(t.rem_euclid(100)),
                    _ => None,
                }).map(|m| (m - now).rem_euclid(100))
            };
            let target = w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty())
                .find(|e| {
                    let c = cell(e.pos);
                    let Some(v) = e.passengers.first() else { return false };
                    match variant {
                        "late" => {
                            // (the stamp 25..35 gt before the napper's next own-pocket S)
                            c[1] == 0
                                && c[2] == 0
                                && LANES.contains(&c[0])
                                && own_to_s(c[0]).is_some_and(|ts| (25..=35).contains(&ts))
                                && nappers.iter().any(|n| n.0 == *v)
                                && recs.get(v).is_some_and(|r| r.milestones.iter().any(|(m, _)| m == "back in its lane") && r.nap_call.is_none())
                        }
                        _ => {
                            // (the current pick's napper, outbound: before its queue arrival,
                            // anywhere a pocket rider's 10-box reaches; the call's willing pocket
                            // riders are its summoners, so their stamps are a summon's own)
                            picked_v == Some(*v) && recs.get(v).is_some_and(|r| !r.milestones.iter().any(|(m, _)| m == "at the queue's arrival"))
                        }
                    }
                })
                .and_then(|e| e.passengers.first().copied().map(|v| (v, e.pos)));
            // (the summon's 10-box: the caller's box inflated by 10 against the target's box, each
            // 0.6 wide and 1.95 tall)
            let caller = target.and_then(|(tv, tp)| {
                w.entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && !e.passengers.is_empty() && e.passengers.first() != Some(&tv))
                    .find(|e| {
                        let c = cell(e.pos);
                        let cp = w.entities.list.iter().find(|p| Some(&p.id) == e.passengers.first()).map_or(e.pos, |p| p.pos);
                        let tq = w.entities.list.iter().find(|p| p.id == tv).map_or(tp, |p| p.pos);
                        c[1] == 0 && c[2] == 0 && LANES.contains(&c[0]) && (cp.x - tq.x).abs() <= 10.6 && (cp.z - tq.z).abs() <= 10.6 && tq.y + 1.95 >= cp.y - 10.0 && tq.y <= cp.y + 1.95 + 10.0
                    })
                    .and_then(|e| e.passengers.first().copied())
            });
            if let Some((tv, tp)) = target
                && let Some(v) = caller
                && let Some(k) = redsim::entity::index_of(w, v)
            {
                bystander_tries += 1;
                let before = w.mob.events.len();
                // (the call's own willing villagers, `wantsToSpawnGolem` in the caller's 10-box, are
                // its summoners: their stamps are a summon's, not a bystander's; a stamped napper
                // that isn't willing is the bystander)
                let cp = dpos(w.entities.list[k].pos);
                let willing: std::collections::BTreeSet<u32> = (0..w.entities.list.len())
                    .filter(|&j| {
                        let e = &w.entities.list[j];
                        !e.removed
                            && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager)
                            && (dpos(e.pos).x - cp.x).abs() <= 10.6
                            && (dpos(e.pos).z - cp.z).abs() <= 10.6
                            && dpos(e.pos).y + 1.95 >= cp.y - 10.0
                            && dpos(e.pos).y <= cp.y + 1.95 + 10.0
                            && redsim::entity::mob::ai::brain::brain(w, j).wants_golem(now)
                    })
                    .map(|j| w.entities.list[j].id)
                    .collect();
                // (G1 N8) the target's GOLEM_DETECTED_RECENTLY before and after the call; the dud
                // control (--bystander-dud) asks 99 willing, so no golem comes and the control must
                // show NOT EXERCISED
                let ttl_of = |w: &redsim::world::World| redsim::entity::index_of(w, tv).and_then(|j| redsim::entity::mob::ai::brain::brain(w, j).golem_detected);
                let ttl_before = ttl_of(w);
                redsim::entity::mob::spawn::spawn_golem_if_needed(w, k, if o.bystander_dud { 99 } else { 3 });
                let ttl_after = ttl_of(w);
                let born_at = w.mob.events[before..].iter().find_map(|e| match e {
                    redsim::entity::mob::MobEvent::GolemTry { golem: Some(_), t, .. } => Some(*t),
                    _ => None,
                });
                let born = born_at.is_some() && ttl_after != ttl_before;
                if let Some(bt) = born_at {
                    willing_at = (bt, willing);
                }
                if born {
                    bystander_target = Some((tv, now - t0));
                    controls_done.push(format!(
                        "the bystander control ({variant}) at t {}: a real summon from #{v} stamped napper #{tv} at ({:.1},{:.1},{:.1}), {} (its GOLEM_DETECTED_RECENTLY {} -> {}; after {bystander_tries} calls; the harm bar must {})",
                        now - t0,
                        tp.x,
                        tp.y,
                        tp.z,
                        if variant == "late" { "back in its own pocket, 25..35 gt before its next S" } else { "on its outbound trip (the current pick)" },
                        ttl_before.map_or("unset".to_string(), |t| t.to_string()),
                        ttl_after.map_or("unset".to_string(), |t| t.to_string()),
                        if variant == "late" { "catch it" } else { "hold" }
                    ));
                }
            }
        }
        // (ironspec 09:59; G1 N13: the bed's own OCCUPIED property) the bed occupied this tick
        let bed_occupied = w.get(dbp(o.bed[0], o.bed[1], o.bed[2])).to_string().contains("occupied=true");
        // (ironspec 06:31 / 06:38 PT: ValidateNearbyPoi's 16-block exposure) while someone sleeps in
        // the bed, every other villager in REST within 16 of it, per night
        if o.napper && bed_occupied {
            bed_ever_occupied = true;
            // (ValidateNearbyPoi's predicate: the bed head's centre within 16 of the villager's feet,
            // `BlockPos.closerToCenterThan(position, 16)`, strict)
            let bc = redsim::entity::Vec3::new(o.bed[0] as f64 + 0.5, o.bed[1] as f64 + 0.5, o.bed[2] as f64 + 0.5);
            for j in 0..w.entities.list.len() {
                let e = &w.entities.list[j];
                let EntityKind::Mob(m) = &e.kind else { continue };
                if e.removed || m.ty != redsim::entity::mob::types::MobType::Villager || m.sleeping_pos.is_some() {
                    continue;
                }
                let d2 = (dpos(e.pos).x - bc.x).powi(2) + (dpos(e.pos).y - bc.y).powi(2) + (dpos(e.pos).z - bc.z).powi(2);
                let d = d2.sqrt();
                let rest = match redsim::entity::mob::ai::brain::brain(w, j).ex.as_deref() {
                    Some(x) => x.active == redsim::entity::mob::ai::brain::Act::Rest,
                    None => {
                        brainless_reads += 1;
                        false
                    }
                };
                if d2 < 256.0 && rest {
                    *rest_near_bed.entry(w.day_time().div_euclid(24_000)).or_default() += 1;
                    if rest_near_rows.len() < 12 {
                        rest_near_rows.push(format!("t {} {} at ({:.1},{:.1},{:.1}), {:.1} from the bed", now - t0, recs.get(&e.id).map_or(format!("#{}", e.id), |r| r.label.split(' ').next().unwrap_or("").to_string()), dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, d));
                    }
                }
            }
        }
        // (ironspec 19:29 (a)) the stop's track (its box: x -6..4, z -6..5, the bed's y - 2 .. + 5)
        // holding carts of two napper riders at once
        {
            let (lo, hi) = ([-6, o.bed[1] - 2, -6], [4, o.bed[1] + 5, 5]);
            let mut riders: Vec<u32> = w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
                .filter(|e| {
                    let c = cell(e.pos);
                    (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k])
                })
                .filter_map(|e| e.passengers.first().copied())
                .filter(|v| nappers.iter().any(|n| n.0 == *v))
                .collect();
            // (and a napper asleep in the bed, out of its cart)
            for e in w.entities.list.iter() {
                if !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.sleeping_pos.is_some()) && nappers.iter().any(|n| n.0 == e.id) && !riders.contains(&e.id) {
                    riders.push(e.id);
                }
            }
            // (G1 08:28 / ironspec 08:28 platform (3), ONE IN FLIGHT: two nappers in the stop's volume
            // in the same tick, riding or ON FOOT, e.g. one ejected and walking the stop floor)
            for e in w.entities.list.iter() {
                if e.removed || e.vehicle.is_some() || !matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) || !nappers.iter().any(|n| n.0 == e.id) || riders.contains(&e.id) {
                    continue;
                }
                let c = cell(e.pos);
                if (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]) {
                    riders.push(e.id);
                }
            }
            if riders.len() >= 2 {
                stop_two_ticks += 1;
                if stop_two.is_none() {
                    stop_two = Some((now - t0, riders));
                }
            }
            // (ironspec 19:31 PT (5)) each napper's entry speed at A6's entry booster (3,y+2,-5):
            // an entry outside [0.11, 0.21] is (C)'s failure, printed in its own class
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [3, o.bed[1] + 2, -5]) {
                if let Some(v) = e.passengers.first().copied()
                    && nappers.iter().any(|n| n.0 == v)
                    && let Some(r) = recs.get_mut(&v)
                    && !r.milestones.iter().any(|(m, _)| m == "at A6's entry")
                {
                    r.milestones.push(("at A6's entry".to_string(), now - t0));
                    let sp = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
                    a6_entries.push((now - t0, v, sp));
                }
            }
            // (ironspec 22:56 / 22:58 PT, the loud bars) A6's park, the slope (1,y+1,-5) / (0,y,-5)
            // the activator's cart rolls onto: braked (both unpowered) at every arrival; and no
            // riderless cart past A6's R detector (4,y-1,4)
            let by = o.bed[1];
            let (park_in, act_cell, r_cell) = ([1, by + 1, -5], [2, by + 2, -5], [4, by - 1, 4]);
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                let c = cell(e.pos);
                let prev = cart_prev_cell.get(&e.id).copied();
                if c == park_in && prev == Some(act_cell) {
                    let braked = !powered(w, park_in) && !powered(w, [0, by, -5]);
                    park_arrivals.push((now - t0, braked, e.passengers.first().copied()));
                }
                if c == r_cell && prev != Some(r_cell) && e.passengers.is_empty() {
                    riderless_r.push((now - t0, e.id));
                }
                // (ironspec 23:44 / 23:47 PT, B4) only the picked napper's cart rides the leg's
                // cells (8,25,9) / (9,25,8): any other cart there fails the run
                if (c == [8, 25, 9] || c == [9, 25, 8]) && prev != Some(c) {
                    let rider = e.passengers.first().copied();
                    let picked = ctl.pick_log.last().map(|p| p.1);
                    if rider.is_none() || rider != picked {
                        leg_wrong.push((now - t0, e.id, rider));
                    }
                }
                cart_prev_cell.insert(e.id, c);
            }
        }
        // (ironspec 11:02) the zombies where they sit this tick (a boated zombie seated: the boat's
        // y + its passenger attachment + the zombie's riding offset)
        // (cc767b78 11:15) in the design frame, as every position f1q2-run compares
        let zombies_now: Vec<redsim::entity::Vec3> = crate::fleecheck::zombies_seated(w, spec).into_iter().map(dpos).collect();
        // (ironspec 12:23) every iron golem's first tick seen (its birth); where, and the nearest
        // villager then (its likely summoner)
        // (farmopt 17:23) each summon that spawned a golem (redsim's GolemTry): its tick, the
        // summoner and where it stood, the willing count, the golem's cell
        while ev_seen < w.mob.events.len() {
            if let redsim::entity::mob::MobEvent::GolemCall { t, villager, willing, needed, from, caller_willing } = &w.mob.events[ev_seen]
                && *needed == 3
            {
                // (G1 / ironspec 19:51, 19:58) a trigger's call without a roll; a caller not willing
                // because it hasn't slept within a day (LAST_SLEPT expired, no golem stamp) is its
                // own class (3), apart from a golem stamp (1)
                let kind = if *caller_willing {
                    2
                } else {
                    let b = redsim::entity::index_of(w, *villager).map(|k| redsim::entity::mob::ai::brain::brain(w, k));
                    let stamp = b.is_some_and(|b| b.golem_detected.is_some_and(|ttl| ttl >= 1));
                    let slept = b.is_some_and(|b| b.last_slept.is_some_and(|s| *t - s < redsim::entity::mob::villager::SLEPT_WITHIN));
                    if !stamp && !slept { 3 } else { 1 }
                };
                s_events.push((*t, kind, *villager, *from, false, *willing, 0));
            }
            if let redsim::entity::mob::MobEvent::GolemTry { t, villager, willing, golem, at, needed, from, tries } = &w.mob.events[ev_seen] {
                // (ironspec 19:20 / 19:34: every attempt with its path, the caller's block position
                // and willing count; 19:51: the spot tries used, 0 when the call logged none)
                golem_tries.push((*t, *villager, *willing, *golem, *needed, *from, tries.unwrap_or(0)));
                if *needed == 5 {
                    // (ironspec 22:54 PT) each gossip candidate's activity (REST / PANIC riders
                    // can't take part: TradeWithVillager runs only in MEET / IDLE)
                    let act = redsim::entity::index_of(w, *villager)
                        .and_then(|k| redsim::entity::mob::ai::brain::brain(w, k).ex.as_deref().map(|x| format!("{:?}", x.active)))
                        .unwrap_or_else(|| "-".to_string());
                    gossip_activity.insert((*t, *villager), act);
                }
                if *needed == 3 {
                    s_events.push((*t, 0, *villager, *from, golem.is_some(), *willing, tries.unwrap_or(0)));
                }
                if let Some(g) = golem {
                    let vpos = redsim::entity::index_of(w, *villager).map(|k| {
                        let p = dpos(w.entities.list[k].pos);
                        [p.x, p.y, p.z]
                    });
                    golem_summon.insert(*g, (*t, *villager, *willing, *at, vpos));
                }
            }
            ev_seen += 1;
        }
        for e in w.entities.list.iter() {
            if !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem) && !golem_born.contains_key(&e.id) {
                golem_born.insert(e.id, now);
                let near = w
                    .entities
                    .list
                    .iter()
                    .filter(|v| !v.removed && matches!(&v.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager)))
                    .map(|v| (((dpos(v.pos).x - dpos(e.pos).x).powi(2) + (dpos(v.pos).y - dpos(e.pos).y).powi(2) + (dpos(v.pos).z - dpos(e.pos).z).powi(2)).sqrt(), v.id, [dpos(v.pos).x, dpos(v.pos).y, dpos(v.pos).z]))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                golem_birth_at.insert(e.id, ([dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z], near.map(|(d, id, p)| format!("villager {id} at ({:.2},{:.2},{:.2}), {d:.2} away", p[0], p[1], p[2])).unwrap_or_default()));
            }
        }
        // (ironspec 19:33 PT) the summon tick's class, once its tick ran: a golem born at S, else
        // the class decided before it
        if let Some((s, class, locked, ready_n)) = direct.pending
            && now > s
        {
            let born = golem_tries.iter().any(|g| g.0 == s && g.3.is_some());
            let c = if born { "golem born" } else { class };
            let e = direct.classes.entry(c).or_insert((0, 0));
            e.0 += 1;
            if locked {
                e.1 += 1;
            }
            // (ironspec 19:58 PT: the summon bar per copy's stand, its three pockets together) the
            // trigger events at S from the pockets, in their order: the rolls up to the first golem;
            // a caller not willing after this S's golem is COVERED (the success already happened),
            // before it (an older stamp: a neighbour's golem, gossip, an earlier S, a sensor
            // renewal) BLOCKED; a call short of willing; bar: >= 1 roll (>= 3 willing) from the
            // stand at every due S in locked running; the golem rate by the willing-and-ticking
            // count at the tick's start against 1 - 0.209^n
            let pocket = |b: [i32; 3]| b[1] == 0 && b[2] == 0 && LANES.contains(&b[0]);
            let (mut rolls, mut covered, mut blocked, mut short, mut golem_seen) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), false);
            let mut not_slept: Vec<String> = Vec::new();
            for ev in s_events.iter().filter(|x| x.0 == s && pocket(x.3)) {
                match ev.1 {
                    0 if !golem_seen => {
                        let tn = if ev.6 == 0 { "? (none logged)".to_string() } else { ev.6.to_string() };
                        rolls.push(format!("#{} at x {} {} willing: {}", ev.2, ev.3[0], ev.5, if ev.4 { format!("a golem after {tn} tries") } else { format!("spot miss ({tn} tries)") }));
                        golem_seen |= ev.4;
                    }
                    0 => rolls.push(format!("#{} at x {} after the golem (?)", ev.2, ev.3[0])),
                    1 if golem_seen => covered.push(format!("#{} at x {}", ev.2, ev.3[0])),
                    1 => blocked.push(format!("#{} at x {}", ev.2, ev.3[0])),
                    3 => not_slept.push(format!("#{} at x {}", ev.2, ev.3[0])),
                    _ => short.push(format!("#{} at x {} ({} willing)", ev.2, ev.3[0], ev.5)),
                }
            }
            direct.not_slept += not_slept.len() as i64;
            direct.s_count += 1;
            direct.s_born += i64::from(golem_seen);
            if locked {
                direct.s_locked += 1;
            }
            if rolls.is_empty() {
                direct.stand_misses += 1;
                if locked {
                    direct.stand_misses_locked += 1;
                    // (G1 X23: the empty-pocket control's own sample) a locked miss with a rider
                    // event at the emptied lane's pocket isn't the emptied pocket's doing
                    if let Some(x) = o.empty_pocket
                        && s_events.iter().any(|e| e.0 == s && pocket(e.3) && e.3[0] == x)
                    {
                        direct.misses_not_empty += 1;
                    }
                }
                // (ironspec 23:55 PT (2), the sleep-expiry signature's control) the first S without
                // a roll, with each pocket rider's LAST_SLEPT expiry (its last sleep + a day)
                if direct.first_miss.is_none() {
                    let riders: Vec<String> = LANES
                        .iter()
                        .filter_map(|x| w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [*x, 0, 0]).and_then(|e| e.passengers.first().copied()))
                        .map(|v| {
                            let exp = redsim::entity::index_of(w, v).and_then(|k| redsim::entity::mob::ai::brain::brain(w, k).last_slept).map(|s| s + redsim::entity::mob::villager::SLEPT_WITHIN - t0);
                            format!("#{v} LAST_SLEPT expiring t {}", exp.map_or("- (never slept)".to_string(), |e| e.to_string()))
                        })
                        .collect();
                    direct.first_miss = Some((s - t0, riders.join(", ")));
                }
            }
            direct.covered += covered.len() as i64;
            direct.blocked += blocked.len() as i64;
            direct.short += short.len() as i64;
            let r = direct.by_ready.entry(ready_n.min(3)).or_insert((0, 0));
            r.0 += 1;
            r.1 += i64::from(golem_seen);
            if direct.call_rows.len() < 16 && (direct.call_rows.len() < 8 || rolls.is_empty() || !blocked.is_empty()) {
                direct.call_rows.push(format!(
                    "S t {}{}: willing and ticking at the start {ready_n}; rolls [{}]; covered [{}]; blocked [{}]; short [{}]; {}",
                    s - t0,
                    if locked { " (locked)" } else { "" },
                    rolls.join("; "),
                    covered.join("; "),
                    blocked.join("; "),
                    short.join("; "),
                    if golem_seen { "a golem" } else { "no golem" }
                ));
            }
            direct.pending = None;
        }
        // (ironspec 11:29) the neighbour's pad golems this tick (EMULATED: ours moved by the
        // neighbour's offset)
        let neighbour_golems: Vec<redsim::entity::Vec3> = match o.neighbour {
            Some(d) => w
                .entities
                .list
                .iter()
                .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem))
                .map(|e| redsim::entity::Vec3::new(dpos(e.pos).x + d[0] as f64, dpos(e.pos).y + d[1] as f64, dpos(e.pos).z + d[2] as f64))
                .collect(),
            None => Vec::new(),
        };
        if bed_occupied {
            near.occupied_ticks += 1;
        }
        // (ironspec 21:25 PT) each cart's contacts with blocks: a tick its move was stopped
        // sideways or from above, and the blocks (not rails) whose collision boxes its box, grown
        // 0.02, meets; a contact cell the header doesn't declare as a stop or buffer (`stops:`)
        // fails the row
        for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
            if !(e.horizontal_collision || (e.vertical_collision && !e.vertical_collision_below)) {
                continue;
            }
            let bb = e.bb.inflate(0.02, 0.02, 0.02);
            let (x0, x1, y0, y1, z0, z1) = (bb.min_x.floor() as i32, bb.max_x.floor() as i32, bb.min_y.floor() as i32, bb.max_y.floor() as i32, bb.min_z.floor() as i32, bb.max_z.floor() as i32);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        // (the entity's own box: world positions)
                        let b = w.get(redsim::pos::BlockPos::new(x, y, z));
                        let name = b.to_string();
                        if name.contains("rail") {
                            continue;
                        }
                        let hit = redsim::behavior::collision_boxes(&b).iter().any(|q| {
                            let a = redsim::entity::Aabb::new(x as f64 + q[0] as f64 / 16.0, y as f64 + q[2] as f64 / 16.0, z as f64 + q[4] as f64 / 16.0, x as f64 + q[1] as f64 / 16.0, y as f64 + q[3] as f64 / 16.0, z as f64 + q[5] as f64 / 16.0);
                            // (the box's floor contact with the block it rides isn't a contact)
                            a.intersects(bb) && !(a.max_y <= e.bb.min_y + 1e-6)
                        });
                        if hit {
                            // (farmopt 02:38 PT, ironfarm: the first contact's cart, its position
                            // and its motion, printed with the cell)
                            let first = format!("cart {} at ({:.3},{:.3},{:.3}) moving ({:.3},{:.3},{:.3})", e.id, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z);
                            let fo = frame_offset();
                            let k = contacts.entry([x - fo[0], y - fo[1], z - fo[2]]).or_insert((0, now - t0, format!("{} [{first}]", name.trim_start_matches("minecraft:").split('[').next().unwrap_or(""))));
                            k.0 += 1;
                        }
                    }
                }
            }
        }
        // (ironspec 20:10 / 20:19 PT (2c), the 7th property) every villager eye to every living
        // entity's eye within 16, against each emulated support laid where the build has air
        if !sight_cells.is_empty() {
            let living: Vec<(usize, redsim::entity::Vec3, bool)> = w
                .entities
                .list
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.removed && matches!(&e.kind, EntityKind::Mob(_)))
                .map(|(j, e)| {
                    let villager = matches!(&e.kind, EntityKind::Mob(m) if matches!(m.ty, redsim::entity::mob::types::MobType::Villager));
                    (j, if villager { redsim::entity::mob::villager::eye_pos(w, j) } else { redsim::entity::mob::ai::eye_pos(w, j) }, villager)
                })
                .collect();
            for (a, ea, va) in &living {
                if !va {
                    continue;
                }
                for (b, eb, _) in &living {
                    if a == b || (ea.x - eb.x).abs() > 16.0 || (ea.y - eb.y).abs() > 16.0 || (ea.z - eb.z).abs() > 16.0 {
                        continue;
                    }
                    for (k, c) in sight_cells.iter().enumerate() {
                        // (a watched cell crosses a ray only while it holds a collider: a toggled
                        // stand-in back to air is the build's own air)
                        if redsim::behavior::collision_boxes(&w.get(dbp(c[0], c[1], c[2]))).is_empty() {
                            continue;
                        }
                        if seg_hits_cell(*ea, *eb, *c) {
                            sight_hits[k] += 1;
                            // (ironspec 02:18 PT: the ray classes) each entity by its label or
                            // mob type, the crossings counted per cell and class
                            let what = |i: usize| -> String {
                                let e = &w.entities.list[i];
                                match &e.kind {
                                    EntityKind::Mob(m) => recs.get(&e.id).map_or(format!("{:?}", m.ty), |r| r.label.split(' ').next().unwrap_or("villager").to_string()),
                                    _ => "entity".to_string(),
                                }
                            };
                            let class = |s: String| s.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string();
                            *sight_classes.entry(format!("({},{},{}) {} -> {}", c[0], c[1], c[2], class(what(*a)), class(what(*b)))).or_default() += 1;
                            if sight_rows.len() < 12 {
                                let (pa, pb) = (dpos(w.entities.list[*a].pos), dpos(w.entities.list[*b].pos));
                                sight_rows.push(format!("t {}: ({},{},{}) on the ray from {} #{} at ({:.1},{:.1},{:.1}) to {} #{} at ({:.1},{:.1},{:.1})", now - t0, c[0], c[1], c[2], what(*a), w.entities.list[*a].id, pa.x, pa.y, pa.z, what(*b), w.entities.list[*b].id, pb.x, pb.y, pb.z));
                            }
                        }
                    }
                }
            }
        }
        // (ironspec 23:16 PT) every declared `# inert:` / `# always-on:` cell watched through the
        // run: any change of its state fails, with the tick and the states
        for (c, kind, last) in watched.iter_mut() {
            let now_state = w.get(dbp(c[0], c[1], c[2])).to_string();
            if now_state != *last {
                if watched_changes.len() < 40 {
                    watched_changes.push(format!("t {}: ({},{},{}) {kind} {} -> {}", now - t0, c[0], c[1], c[2], last, now_state));
                }
                watched_n += 1;
                *last = now_state;
            }
        }
        // (ironspec 23:51 PT (2)) every declared push pair checked against the run: a cart resting
        // in either cell (speed under 1e-3, its rail unpowered) is never displaced beyond the bound
        // the declaration states; a failure prints the tick, the cell, the displacement and the
        // nearest other cart (what acted)
        if !o.push_pairs.is_empty() {
            for (a, b, bound) in &o.push_pairs {
                for c in [*a, *b] {
                    for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == c) {
                        let sp = (e.delta_movement.x.powi(2) + e.delta_movement.y.powi(2) + e.delta_movement.z.powi(2)).sqrt();
                        if !pp_rest.contains_key(&e.id) && sp < 1e-3 && !powered(w, c) {
                            pp_rest.insert(e.id, (c, dpos(e.pos), *bound));
                            pp_rests += 1;
                        }
                    }
                }
            }
            let tracked: Vec<u32> = pp_rest.keys().copied().collect();
            for id in tracked {
                let (c, p0, bound) = pp_rest[&id];
                let Some(e) = w.entities.list.iter().find(|e| e.id == id && !e.removed) else {
                    pp_rest.remove(&id);
                    continue;
                };
                if powered(w, c) {
                    pp_rest.remove(&id);
                    continue;
                }
                let d = ((dpos(e.pos).x - p0.x).powi(2) + (dpos(e.pos).y - p0.y).powi(2) + (dpos(e.pos).z - p0.z).powi(2)).sqrt();
                if d > bound + 1e-6 {
                    let near = w
                        .entities
                        .list
                        .iter()
                        .filter(|o2| !o2.removed && o2.id != id && matches!(o2.kind, EntityKind::Minecart(_)))
                        .map(|o2| (((dpos(o2.pos).x - dpos(e.pos).x).powi(2) + (dpos(o2.pos).y - dpos(e.pos).y).powi(2) + (dpos(o2.pos).z - dpos(e.pos).z).powi(2)).sqrt(), o2.id))
                        .min_by(|x, y| x.0.total_cmp(&y.0));
                    if pp_fail.len() < 12 {
                        pp_fail.push(format!("t {}: cart {id} resting at ({},{},{}) displaced {d:.4} (bound {bound}){}", now - t0, c[0], c[1], c[2], near.map_or(String::new(), |(dd, oid)| format!(", the nearest cart {oid} at {dd:.2}"))));
                    }
                    pp_fails += 1;
                    pp_rest.remove(&id);
                } else if cell(e.pos) != c {
                    pp_rest.remove(&id);
                }
            }
        }
        // (ironspec 19:58 PT (2b)) the split's Ts (x,25,9): each tick a T's stored shape against the
        // settle of its power (a stale shape: a change of its power that no update reached, or an
        // update from another source); the ticks a cart is within 1.5 of a stale T; each shape change
        // with its lever, its branch detector (x,24,8) and the (a) feed stack's dust (-3,23,9)
        for (k, x) in [-3, -1, 1, 3, 5, 7].iter().enumerate() {
            let p = dbp(*x, 25, 9);
            let Some(j) = redsim::rails::junction_at(w, p) else {
                // (G1's [2]) a missing T fails its bars, never skipped
                tsplit.missing[k] += 1;
                continue;
            };
            let want = if j.signal { j.powered } else { j.unpowered };
            if j.stored != want {
                tsplit.stale[k] += 1;
                let near_cart = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && ((dpos(e.pos).x - (*x as f64 + 0.5)).powi(2) + (dpos(e.pos).y - 25.0).powi(2) + (dpos(e.pos).z - 9.5).powi(2)).sqrt() < 1.5);
                if near_cart {
                    tsplit.stale_cart[k] += 1;
                    if tsplit.rows.len() < 12 {
                        tsplit.rows.push(format!("t {}: ({x},25,9) stored {:?}, its power's settle {:?}, a cart within 1.5", now - t0, j.stored, want));
                    }
                }
            }
            // (ironspec 23:51 PT (2)) a T beside a declared update-only source changes shape only
            // when its own switch flips it: the lever (x,26,9)'s last flip
            let lever_on = w.get(dbp(*x, 26, 9)).to_string().contains("powered=true");
            if tsplit.lever_prev[k].is_some_and(|p| p != lever_on) {
                tsplit.lever_flip[k] = now - t0;
            }
            tsplit.lever_prev[k] = Some(lever_on);
            if tsplit.prev[k].is_some_and(|s| s != j.stored) {
                tsplit.changes[k] += 1;
                if now - t0 - tsplit.lever_flip[k] > 2 {
                    tsplit.unswitched[k] += 1;
                    if tsplit.unswitched_rows.len() < 12 {
                        let d = w.get(dbp(*x, 24, 8)).to_string();
                        let dust = w.get(dbp(-3, 23, 9)).to_string();
                        tsplit.unswitched_rows.push(format!("t {}: ({x},25,9) {:?} -> {:?} with its lever's last flip at t {} (what was acting: (x,24,8) {}; the stack's dust (-3,23,9) {})", now - t0, tsplit.prev[k].unwrap(), j.stored, tsplit.lever_flip[k], d.trim_start_matches("minecraft:"), dust.trim_start_matches("minecraft:")));
                    }
                }
                let on = |c: [i32; 3]| w.get(dbp(c[0], c[1], c[2])).to_string().contains("powered=true");
                let dust = w.get(dbp(-3, 23, 9)).to_string();
                let dust_power = dust.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("-").to_string();
                if tsplit.change_rows.len() < 40 {
                    tsplit.change_rows.push(format!("t {} ({x},25,9) {:?} -> {:?} (signal {}, lever {}, detector (x,24,8) {}, the stack's dust power {dust_power})", now - t0, tsplit.prev[k].unwrap(), j.stored, j.signal, on([*x, 26, 9]), on([*x, 24, 8])));
                }
                if j.stored != want {
                    tsplit.wrong_changes[k] += 1;
                }
            }
            tsplit.prev[k] = Some(j.stored);
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
            // (cc767b78 11:15) the villager's position in the DESIGN frame (its world position is
            // w.entities.list[i].pos); world queries convert back with wpos
            let pos = dpos(w.entities.list[i].pos);
            // (G1's P14 / the at-entry control) a stamp a control wrote this tick: the next tick's
            // pre-tick stamp is the written one, not the one read before the write
            let mut forced_stamp: Option<i64> = None;
            let (home, activity_rest, panic, sleeping, stamp, target, last_slept, reach, sink_cd, path_target, path_end, hostile, target_kind, activity, target_key, path_from, target_close) = {
                use redsim::entity::mob::ai::brain::Act;
                let b = redsim::entity::mob::ai::brain::brain(w, i);
                let ex = b.ex.as_deref();
                // (the exact brain's own activity: REST, PANIC; the legacy fields aren't kept with
                // the mob AI on)
                let rest = match ex {
                    Some(x) => x.active == Act::Rest,
                    // (ironspec D5(a): a rider with no exact brain state can't be read as "not REST")
                    None => {
                        brainless_reads += 1;
                        matches!(b.activity, redsim::entity::mob::villager::Activity::Rest)
                    }
                };
                let panic = ex.map_or(b.panic_running, |x| x.active == Act::Panic);
                let sleeping = matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.sleeping_pos.is_some());
                let target = ex.and_then(|x| x.mem.walk_target.as_ref()).and_then(|t| match t.target {
                    // (cc767b78 11:15: every position in the design frame)
                    redsim::entity::mob::ai::brain::Tracker::Block(p) => {
                        let o = frame_offset();
                        Some([(p.x - o[0]) as f64 + 0.5, (p.y - o[1]) as f64, (p.z - o[2]) as f64 + 0.5])
                    }
                    redsim::entity::mob::ai::brain::Tracker::Point(v) => {
                        let q = dpos(v);
                        Some([q.x, q.y, q.z])
                    }
                    redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => redsim::entity::index_of(w, id).map(|k| {
                        let q = dpos(w.entities.list[k].pos);
                        [q.x, q.y, q.z]
                    }),
                });
                // (G1 07:28 PT) the walk target's close-enough distance, for MoveToTargetSink's
                // reached test
                let target_close = ex.and_then(|x| x.mem.walk_target.as_ref()).map(|t| t.close);
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
                let fo = frame_offset();
                let path_target = ex.and_then(|x| x.mem.path.as_ref()).map(|p| [p.target.x - fo[0], p.target.y - fo[1], p.target.z - fo[2]]);
                let path_end = ex.and_then(|x| x.mem.path.as_ref()).and_then(|p| p.nodes.last().map(|n| [n.x - fo[0], n.y - fo[1], n.z - fo[2]]));
                // (the path's first node and length: where it was computed from)
                let path_from = ex.and_then(|x| x.mem.path.as_ref()).map(|p| (p.nodes.first().map(|n| [n.x - fo[0], n.y - fo[1], n.z - fo[2]]), p.nodes.len()));
                let hostile = b.nearest_hostile;
                // (MoveToTargetSink's remainingCooldown)
                let sink_cd = ex.and_then(|x| x.nodes.iter().find_map(|n| match &n.kind {
                    redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { remaining_cooldown, .. } => Some(*remaining_cooldown),
                    _ => None,
                }));
                (b.home.map(|h| [h.x - fo[0], h.y - fo[1], h.z - fo[2]]), rest, panic, sleeping, ex.and_then(|x| x.mem.cant_reach_since), target, b.last_slept, reach, sink_cd, path_target, path_end, hostile, target_kind, activity, target_key, path_from, target_close)
            };
            // (G1 12:21, #43's late lie-down) --villager-trace ID,FROM,TO: the villager's own state per
            // tick (position, vehicle, activity, sleeping, its walk target, the path, the stamp, the
            // nearest hostile, the sink's cooldown)
            if let Some((tid, tf, tt)) = o.villager_trace
                && w.entities.list[i].id == tid
                && rel >= tf
                && rel <= tt
            {
                let veh = w.entities.list[i].vehicle.map_or("on foot".to_string(), |vid| redsim::entity::index_of(w, vid).map_or(format!("in #{vid}"), |vi| {
                    let c = cell(w.entities.list[vi].pos);
                    format!("in cart #{vid} at ({},{},{})", c[0], c[1], c[2])
                }));
                vtrace_rows.push(format!(
                    "t {rel}: ({:.3},{:.3},{:.3}) {veh} {activity}{}{} target {} {} close {} reach {} path end {} stamp {} hostile {} sink cd {}",
                    pos.x,
                    pos.y,
                    pos.z,
                    if panic { " PANIC" } else { "" },
                    if sleeping { " SLEEPING" } else { "" },
                    target.map_or("none".to_string(), |t| format!("({:.2},{:.2},{:.2})", t[0], t[1], t[2])),
                    target_kind.clone().unwrap_or_default(),
                    target_close.map_or("-".to_string(), |c| c.to_string()),
                    reach.map_or("-".to_string(), |r| r.to_string()),
                    path_end.map_or("-".to_string(), |p| format!("({},{},{})", p[0], p[1], p[2])),
                    stamp.map_or("none".to_string(), |s| format!("{} gt old", now - s)),
                    hostile.map_or("none".to_string(), |h| format!("#{h}")),
                    sink_cd.map_or("-".to_string(), |c| c.to_string())
                ));
            }
            let r = recs.get_mut(&id).unwrap();
            let c = cell(w.entities.list[i].pos);
            // (ironspec 06:24 / 06:31 / 06:36 real, G1) the rider-state bars: REST entries, the
            // stamps, the farm stop
            {
                let dt = day.rem_euclid(24_000);
                let night = (12_000..24_000).contains(&dt);
                let riding = w.entities.list[i].vehicle.is_some();
                // (a napper's own trip: from its pick until R; outside it every rider is a lane rider)
                let in_trip = nappers.iter().any(|n| n.0 == id) && !r.milestones.is_empty() && !r.milestones.iter().any(|(m, _)| m == "at R");
                if in_trip && !r.in_trip_prev {
                    r.trips.push((now - t0, None));
                } else if !in_trip && r.in_trip_prev
                    && let Some(tr) = r.trips.last_mut()
                {
                    tr.1 = Some(now - t0);
                }
                r.in_trip_prev = in_trip;
                let sight = |w: &redsim::world::World| -> String {
                    hostile.and_then(|h| redsim::entity::index_of(w, h)).map_or("no scarer in sight (NEAREST_HOSTILE empty)".to_string(), |k| {
                        let e = &w.entities.list[k];
                        let d = ((dpos(e.pos).x - pos.x).powi(2) + (dpos(e.pos).y - pos.y).powi(2) + (dpos(e.pos).z - pos.z).powi(2)).sqrt();
                        format!("NEAREST_HOSTILE {} at ({:.1},{:.1},{:.1}), {d:.1} away", match &e.kind { EntityKind::Mob(m) => format!("{:?}", m.ty), _ => "?".to_string() }, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z)
                    })
                };
                let who = r.label.split(' ').next().unwrap_or("").to_string();
                let tgt = |t: Option<[f64; 3]>| t.map_or("no walk target".to_string(), |t| format!("walk target ({:.1},{:.1},{:.1})", t[0], t[1], t[2]));
                if let Some(s) = stamp {
                    let age = now - s;
                    let e = r.oldest_by_day.entry(day.div_euclid(24_000)).or_insert((age, now - t0, c));
                    if age > e.0 {
                        *e = (age, now - t0, c);
                    }
                    if stamp_max.as_ref().is_none_or(|m| age > m.0) {
                        stamp_max = Some((age, now - t0, who.clone(), c));
                    }
                    if age >= 1000 {
                        stamp_over_n += 1;
                        stamp_over_ids.insert(id);
                        if stamp_over.len() < 8 {
                            stamp_over.push(format!("t {} {who} at {:?}: {age} gt ({}, {})", now - t0, c, if activity_rest { "REST" } else if panic { "PANIC" } else { "neither" }, tgt(target)));
                        }
                    }
                    if r.prev_stamp.is_none() {
                        // (an arming: the stamp set this tick)
                        r.arm_open = Some(now - t0);
                        if night {
                            armings_night_n += 1;
                            if armings_night.len() < 24 {
                                armings_night.push(format!("t {} {who} at ({:.1},{:.1},{:.1}){}, {}, {}", now - t0, pos.x, pos.y, pos.z, if riding { " riding" } else { "" }, sight(w), tgt(target)));
                            }
                        } else {
                            armings_day_n += 1;
                            if armings_day.len() < 24 {
                                armings_day.push(format!("t {} {who} at ({:.1},{:.1},{:.1}){}, {}", now - t0, pos.x, pos.y, pos.z, if riding { " riding" } else { "" }, tgt(target)));
                            }
                        }
                    }
                }
                if night {
                    night_window = Some(night_window.map_or((now - t0, now - t0), |(a, _)| (a, now - t0)));
                    r.night_acts[if panic { 0 } else if activity_rest { 1 } else { 2 }] += 1;
                    if o.rider_trace.is_some() {
                        let eye = redsim::entity::mob::villager::eye_pos(w, i);
                        let k = [(eye.x * 100.0).round() as i64, (eye.y * 100.0).round() as i64, (eye.z * 100.0).round() as i64];
                        let e = eye_hist.entry(k).or_default();
                        e[0] += 1;
                        e[1] += activity_rest as i64;
                        e[2] += panic as i64;
                    }
                }
                // (G1, farmopt 06:12 PT: a covered rider enters REST for not even one tick at the
                // schedule switch) its activity at each tick of day time 11,990..=12,040
                if (11_990..=12_040).contains(&dt) {
                    let k = (dt - 11_990) as usize;
                    while r.switch_strip.len() < k {
                        r.switch_strip.push('-');
                    }
                    if r.switch_strip.len() == k {
                        r.switch_strip.push(if panic { 'P' } else if activity_rest && in_trip { 'r' } else if activity_rest { 'R' } else { '.' });
                        if activity_rest && !in_trip && r.switch_rest_at.is_none() {
                            r.switch_rest_at = Some(format!("day time {dt} at ({:.1},{:.1},{:.1}) {:?}{}, {}", pos.x, pos.y, pos.z, c, if riding { " riding" } else { "" }, sight(w)));
                            r.switch_pending = true;
                        }
                    }
                }
                // (ironspec 06:22 platform + G1, the air-supply bar) each lift transit's least air
                let air = match &w.entities.list[i].kind {
                    EntityKind::Mob(m) => m.air,
                    _ => 300,
                };
                if air < 300 {
                    match r.air_open.as_mut() {
                        Some(o) => {
                            if air < o.1 {
                                o.1 = air;
                                o.2 = now - t0;
                                o.3 = c;
                            }
                        }
                        None => r.air_open = Some((now - t0, air, now - t0, c)),
                    }
                } else if let Some(o) = r.air_open.take() {
                    r.air_transits.push(o);
                }
                // (ironspec 06:24 platform, G1: the air bar's positive) the first rider to enter
                // water has its cart held where it is for 340 gt (its air runs out at 300)
                if o.air_control && riding {
                    if air_held.is_none() && air < 300 {
                        let cv = w.entities.list[i].vehicle.unwrap_or(0);
                        if let Some(ci) = redsim::entity::index_of(w, cv) {
                            let cp = w.entities.list[ci].pos;
                            air_held = Some((id, now - t0, [dpos(cp).x, dpos(cp).y, dpos(cp).z], cv, air, air));
                            // (G1 07:28 PT) the hold's footprint by entity id and column geometry:
                            // the riders of the carts in the held cart's column below it, from the
                            // hold's start
                            let col = cell(cp);
                            air_stacked = w
                                .entities
                                .list
                                .iter()
                                .filter(|e| !e.removed && e.id != cv && matches!(e.kind, EntityKind::Minecart(_)) && {
                                    let ec = cell(e.pos);
                                    ec[0] == col[0] && ec[2] == col[2] && dpos(e.pos).y < cp.y
                                })
                                .flat_map(|e| e.passengers.iter().copied())
                                .collect();
                            controls_done.push(format!("the air control at t {}: {who}'s cart held at ({:.2},{:.2},{:.2}) for 340 gt from its first tick in water (air {air}) (the air-supply bar must catch it)", now - t0, cp.x, cp.y, cp.z));
                        }
                    }
                    if let Some(h) = air_held.as_mut()
                        && h.0 == id
                        && now - t0 - h.1 < 340
                        && let Some(ci) = redsim::entity::index_of(w, h.3)
                    {
                        w.entities.set_pos(ci, wpos(h.2[0], h.2[1], h.2[2]));
                        w.entities.list[ci].delta_movement = redsim::entity::Vec3::ZERO;
                        h.5 = h.5.min(air);
                    }
                }
                // (ironspec 07:10 real (2)) the nearest hostile mob within 16 of the rider's eye, its
                // distance and the eye-to-eye clip (a collider between them hides it)
                let nearest = |w: &redsim::world::World| -> String {
                    let eye = redsim::entity::mob::villager::eye_pos(w, i);
                    let mut best: Option<(f64, usize)> = None;
                    for (j, e) in w.entities.list.iter().enumerate() {
                        let EntityKind::Mob(m) = &e.kind else { continue };
                        let ty = format!("{:?}", m.ty);
                        if e.removed || !["Zombie", "Husk", "Drowned", "ZombieVillager", "Pillager", "Vindicator", "Evoker", "Illusioner", "Ravager", "Vex"].contains(&ty.as_str()) {
                            continue;
                        }
                        let d = ((dpos(e.pos).x - pos.x).powi(2) + (dpos(e.pos).y - pos.y).powi(2) + (dpos(e.pos).z - pos.z).powi(2)).sqrt();
                        if d <= 16.0 && best.is_none_or(|b| d < b.0) {
                            best = Some((d, j));
                        }
                    }
                    best.map_or("no hostile mob within 16".to_string(), |(d, j)| {
                        let e = &w.entities.list[j];
                        let seye = redsim::entity::mob::ai::eye_pos(w, j);
                        let clear = redsim::entity::mob::los::has_line_of_sight(w, eye, seye);
                        // (G1 06:33 PT: the rider's eye, the scarer's eye, the distance and the
                        // clip's blocking cell, like for like with 9fc3155f's map)
                        let block = if clear { String::new() } else { redsim::entity::mob::los::first_hit(w, eye, seye).map_or(String::new(), |b| { let o = frame_offset(); format!(" at ({},{},{}) {}", b.x - o[0], b.y - o[1], b.z - o[2], w.get(b).to_string().trim_start_matches("minecraft:")) }) };
                        format!(
                            "the nearest hostile {} at ({:.1},{:.1},{:.1}), {d:.2} away; the rider's eye ({:.2},{:.2},{:.2}), its eye ({:.2},{:.2},{:.2}); the eye-to-eye clip {}{block}",
                            match &e.kind { EntityKind::Mob(m) => format!("{:?}", m.ty), _ => "?".to_string() },
                            dpos(e.pos).x,
                            dpos(e.pos).y,
                            dpos(e.pos).z,
                            eye.x,
                            eye.y,
                            eye.z,
                            seye.x,
                            seye.y,
                            seye.z,
                            if clear { "clear" } else { "blocked" }
                        )
                    })
                };
                // (ironspec 06:31 platform) each night H visit (a rider seated on H (x,16,0)): the
                // stamp's age at entry and what cleared it since the previous H visit
                let on_h = riding && LANES.iter().any(|x| c == [*x, 16, 0]);
                if on_h && !r.on_h_prev && night {
                    let since = r.last_h_visit;
                    let cleared = r.arm_pairs.iter().rev().find(|(_, cl, _)| cl.is_some_and(|cl| since.is_none_or(|s| cl > s)));
                    let row = format!(
                        "t {} at {:?}: stamp {}; {}",
                        now - t0,
                        c,
                        stamp.map_or("unarmed".to_string(), |s| format!("{} gt", now - s)),
                        match (cleared, stamp) {
                            (Some((a, Some(cl), how)), _) => format!("cleared since the last H visit at t {cl} (armed t {a}): {how}"),
                            (_, Some(s)) => format!("NOT CLEARED since {} (armed t {})", since.map_or("the run's start".to_string(), |s| format!("the H visit at t {s}")), s - t0),
                            _ => "no stamp since the last H visit".to_string(),
                        }
                    );
                    if r.h_rows.len() < 16 {
                        r.h_rows.push(row);
                    }
                    r.last_h_visit = Some(now - t0);
                }
                r.on_h_prev = on_h;
                // (G1 07:28 PT, ironspec 07:27 platform) each switch-window REST rider: the scarer's
                // eye-to-eye clip, covered or dark (a hostile of the sensor's range for its type, in
                // sight), and why its PANIC ended or never started
                if r.switch_pending {
                    r.switch_pending = false;
                    let eye = redsim::entity::mob::villager::eye_pos(w, i);
                    // (G1 X40: VillagerHostilesSensor's own test, distanceToSqr <= range squared, its
                    // whole table, Zoglin included)
                    let covered = w.entities.list.iter().enumerate().any(|(j, e)| {
                        let EntityKind::Mob(m) = &e.kind else { return false };
                        let range = match format!("{:?}", m.ty).as_str() {
                            "Zombie" | "Husk" | "Drowned" | "ZombieVillager" | "Vex" => 8.0,
                            "Vindicator" | "Zoglin" => 10.0,
                            "Evoker" | "Illusioner" | "Ravager" => 12.0,
                            "Pillager" => 15.0,
                            _ => return false,
                        };
                        let d2 = (dpos(e.pos).x - pos.x).powi(2) + (dpos(e.pos).y - pos.y).powi(2) + (dpos(e.pos).z - pos.z).powi(2);
                        !e.removed && d2 <= range * range && redsim::entity::mob::los::has_line_of_sight(w, eye, redsim::entity::mob::ai::eye_pos(w, j))
                    });
                    r.switch_covered = Some(covered);
                    let had_panic = r.switch_strip.contains('P');
                    let why = if had_panic { "its PANIC ended in the window (VillagerCalmDown: no hostile or hurt memory left), then REST" } else { "no PANIC in the window before it (no hostile in its memory at the 12,000 schedule update: IDLE -> REST)" };
                    if let Some(row) = r.switch_rest_at.as_mut() {
                        row.push_str(&format!("; {}; {why}; {}", nearest(w), if covered { "COVERED (a hostile in its sensor range and in sight): G1's one-tick question" } else { "DARK (no hostile in sensor range and sight): the dark-spell class, judged by the at-entry bar" }));
                    }
                }
                // (ironspec 06:40 platform, G1 1c) every wake: the flee target the waker set, its path
                // (brain: the rider's own path memory), the stamp; closed at the next REST entry
                if r.sleeping_prev && !sleeping {
                    r.wake_open = Some((
                        now - t0,
                        format!(
                            "woke t {} at ({:.1},{:.1},{:.1}): {}, the path then {}, stamp {}",
                            now - t0,
                            pos.x,
                            pos.y,
                            pos.z,
                            target_kind.as_ref().map_or("no walk target".to_string(), |k| format!("{k} {}", tgt(target))),
                            path_from.map_or("none".to_string(), |(n0, len)| format!("brain: from {} ({len} nodes, canReach {})", n0.map_or("-".to_string(), |n| format!("({},{},{})", n[0], n[1], n[2])), reach.map_or("?", |r| if r { "true" } else { "false" }))),
                            stamp.map_or("unarmed".to_string(), |s| format!("{} gt", now - s))
                        ),
                    ));
                }
                r.sleeping_prev = sleeping;
                if activity_rest && r.rest_spell.is_none()
                    && let Some((wt, row)) = r.wake_open.take()
                {
                    let armed: Vec<&(i64, Option<i64>, String)> = r.arm_pairs.iter().filter(|(a, _, _)| *a >= wt).collect();
                    let open_armed = r.arm_open.is_some_and(|a| a >= wt);
                    if r.wake_rows.len() < 12 {
                        r.wake_rows.push(format!(
                            "{row}; until its next REST entry at t {}: {}",
                            now - t0,
                            if armed.is_empty() && !open_armed {
                                "no stamp armed".to_string()
                            } else {
                                format!("{}{}", armed.iter().map(|(a, c, how)| format!("armed t {a}, cleared t {}: {how}", c.map_or("-".to_string(), |c| c.to_string()))).collect::<Vec<_>>().join(", "), if open_armed { format!("; armed t {} NOT CLEARED by the entry", r.arm_open.unwrap_or(0)) } else { String::new() })
                            }
                        ));
                    }
                }
                if activity_rest {
                    if r.rest_spell.is_none() {
                        rest_entries_total += 1;
                        let counted = !in_trip && !sleeping;
                        // (ironspec 06:33 platform (2)) what cleared the stamp since the previous
                        // REST entry: the last clearing after it, or "not cleared"
                        let since = r.last_rest_entry;
                        let cleared = r.arm_pairs.iter().rev().find(|(_, c, _)| c.is_some_and(|c| since.is_none_or(|s| c > s)));
                        let cleared_s = match (cleared, stamp) {
                            (Some((a, Some(c), how)), _) => format!("cleared since the last entry: t {c} (armed t {a}): {how}"),
                            (_, Some(s)) => format!("NOT CLEARED since {} (armed t {})", since.map_or("the run's start".to_string(), |s| format!("the entry at t {s}")), s - t0),
                            _ => "no stamp since the last entry".to_string(),
                        };
                        r.last_rest_entry = Some(now - t0);
                        let row = format!(
                            "t {} {who} entered REST at ({:.1},{:.1},{:.1}){}{}, on_ground {}, WALK_TARGET at entry {}, the path then {}, {}; {}, stamp {} ({cleared_s}), HOME {}",
                            now - t0,
                            pos.x,
                            pos.y,
                            pos.z,
                            if riding { " riding (a design position)" } else { "" },
                            if in_trip { " (in its own trip: excluded)" } else { "" },
                            w.entities.list[i].on_ground,
                            target_kind.as_ref().map_or("none".to_string(), |k| format!("{k} {}", tgt(target))),
                            path_from.map_or("none".to_string(), |(n0, len)| format!("from {} ({len} nodes, {})", n0.map_or("-".to_string(), |n| format!("({},{},{})", n[0], n[1], n[2])), reach.map_or("?", |r| if r { "complete" } else { "partial" }))),
                            sight(w),
                            nearest(w),
                            stamp.map_or("unarmed".to_string(), |s| format!("{} gt", now - s)),
                            home.map_or("none".to_string(), |h| format!("({},{},{})", h[0], h[1], h[2]))
                        );
                        if counted {
                            lane_rest_entries += 1;
                            *rest_by_cell.entry(c).or_default() += 1;
                        }
                        r.rest_spell = Some((now - t0, row, counted, riding));
                        r.rest_path = None;
                        r.spell_sink_stop = None;
                        r.spell_home_at = None;
                        r.spell_armed = false;
                        r.spell_home_stamp = None;
                        r.spell_cell = c;
                        r.spell_held = target.is_some();
                    }
                    if r.rest_path.is_none() && home.is_some() && path_target == home {
                        r.rest_path = reach;
                    }
                    // (ironspec 06:34 platform) the sink's stop, HOME targeted, a stamp armed
                    if r.spell_sink_stop.is_none() && target.is_none() {
                        r.spell_sink_stop = Some(now - t0);
                    }
                    // (G1 X19) EVERY HOME-target set in the spell is judged, not only the first: each
                    // tick WALK_TARGET turns to HOME (it wasn't HOME the tick before), on the
                    // pre-tick stamp; the release branch (HOME erased) is judged by the HOME-loss
                    // mechanism count beside the verdict
                    let target_home = matches!((home, target), (Some(h), Some(t)) if [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32] == h);
                    let prev_home_target = matches!((r.prev_home, r.prev_target), (Some(h), Some(t)) if [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32] == h);
                    if target_home && !prev_home_target {
                        if r.spell_home_at.is_none() {
                            r.spell_home_at = Some(now - t0);
                        }
                        // (ironspec 08:13 / 08:14 platform, tick-exact's citation) the age the
                        // behaviour saw: SetWalkTargetFromBlockMemory reads the PRE-tick stamp (the
                        // memory before this tick's brain step), never the post-tick one
                        let age = r.prev_stamp.map(|s| now - s);
                        if r.spell_home_stamp.is_none() {
                            r.spell_home_stamp = Some(age);
                        }
                        if let Some(a) = age {
                            at_home_target_max = at_home_target_max.max(a);
                            if a >= 1000 {
                                at_home_target_over += 1;
                                at_entry_over_ids.insert(id);
                            }
                            // (G1 10:47 O3) the at-entry control's rider's next judged set after the aging
                            if at_entry_rider == Some(id) && at_entry_next.is_none() {
                                at_entry_next = Some((now - t0, a));
                            }
                        }
                        at_home_target_n += 1;
                        *judged_sets.entry(id).or_default() += 1;
                    }
                    // (G1 X19's control; G1 10:47 O3) --at-entry-control: the first lane rider (not a
                    // napper, riding) in REST after t 1,000 that has had a judged HOME-target set gets
                    // its stamp aged to 1,100 gt (under the release branch's 1,200, so no release
                    // follows) before its next set; its before stamp printed; the at-entry bar must fail
                    // on that rider alone
                    if o.at_entry_control && at_entry_rider.is_none() && now - t0 >= 1_000 && riding && !nappers.iter().any(|n| n.0 == id) && !target_home && judged_sets.get(&id).is_some_and(|n| *n >= 1) {
                        if let Some(ex) = redsim::entity::mob::ai::brain::brain_mut(w, i).ex.as_deref_mut() {
                            let before = ex.mem.cant_reach_since;
                            ex.mem.cant_reach_since = Some(now - 1_100);
                            forced_stamp = Some(now - 1_100);
                            at_entry_rider = Some(id);
                            at_entry_before = Some((now - t0, before.map(|s| now - s)));
                            controls_done.push(format!("the at-entry control at t {}: {who}'s stamp aged from {} to 1,100 gt in REST (under 1,200: no release; the at-entry bar must catch its next HOME-target set)", now - t0, before.map_or("unarmed".to_string(), |s| format!("{} gt", now - s))));
                        }
                    }
                    if stamp.is_some() && r.prev_stamp.is_none() {
                        r.spell_armed = true;
                    }
                    if r.rest_spell.as_ref().is_some_and(|s| s.2) {
                        lane_rest_ticks += 1;
                    }
                } else if let Some((st, row, counted, at_design)) = r.rest_spell.take() {
                    let path = r.rest_path.take();
                    if counted && at_design && path == Some(false) {
                        lane_rest_partial += 1;
                    }
                    if counted {
                        let e = spell_by_cell.entry(r.spell_cell).or_default();
                        e.0.push(now - t0 - st);
                        e.1 += r.spell_held as i64;
                    }
                    if rest_rows.len() < 40 {
                        rest_rows.push(format!(
                            "{row}; the sink stopped (WALK_TARGET empty) {}, HOME targeted {}{}, a stamp armed in the spell: {}; its HOME path {}; left REST at t {} ({} gt){}",
                            r.spell_sink_stop.map_or("never in the spell".to_string(), |t| format!("at t {t}")),
                            r.spell_home_at.map_or("never in the spell".to_string(), |t| format!("at t {t}")),
                            r.spell_home_stamp.map_or(String::new(), |a| format!(" (the stamp then {})", a.map_or("unarmed".to_string(), |a| format!("{a} gt")))),
                            if r.spell_armed { "yes" } else { "no" },
                            path.map_or("never computed".to_string(), |p| if p { "complete".to_string() } else { "PARTIAL".to_string() }),
                            now - t0,
                            now - t0 - st,
                            r.home_lost.last().map_or(String::new(), |l| format!("; HOME released {l}"))
                        ));
                    }
                }
                // (ironspec 06:31 real, the farm-stop bar) a napper without HOME at the bed stop
                let (lo, hi) = ([-6, o.bed[1] - 2, -6], [4, o.bed[1] + 5, 5]);
                let at_stop = (0..3).all(|a| c[a] >= lo[a] && c[a] <= hi[a]);
                if at_stop && nappers.iter().any(|n| n.0 == id) {
                    if o.home_erase_control && !home_erased && home.is_some() {
                        // (the known positive: the planted erasure)
                        redsim::entity::mob::ai::brain::brain_mut(w, i).home = None;
                        home_erased = true;
                        home_erased_id = Some(id);
                        controls_done.push(format!("the farm-stop control at t {}: {who}'s HOME erased at the bed stop {:?} (the farm-stop bar must catch it)", now - t0, c));
                    }
                    if home.is_none() {
                        stop_homeless_n += 1;
                        stop_homeless_ids.insert(id);
                        if stop_homeless.len() < 8 {
                            stop_homeless.push(format!("t {} {who} at {:?}", now - t0, c));
                        }
                    }
                }
                // (ironspec 06:36 real, the stamp bar's known positive) by day, one lane x -1 rider
                // given a walk target no path reaches
                if o.stamp_control && !night && !in_trip && riding && stamp_control_rider.is_none_or(|v| v == id) && (stamp_control_rider.is_some() || (c[0] == -1 && (16..=22).contains(&c[1]) && (1..=6).contains(&c[2]) && !nappers.iter().any(|n| n.0 == id))) {
                    if stamp_control_rider.is_none() {
                        stamp_control_rider = Some(id);
                        controls_done.push(format!("the stamp control at t {}: {who} at {:?} given the walk target (9,-28,9), which no path from the lane reaches, every day tick (the stamp bar must catch it)", now - t0, c));
                    }
                    if let Some(ex) = redsim::entity::mob::ai::brain::brain_mut(w, i).ex.as_deref_mut() {
                        ex.mem.walk_target = Some(redsim::entity::mob::ai::brain::WalkTarget { target: redsim::entity::mob::ai::brain::Tracker::Block(dbp(9, -28, 9)), speed: 0.5, close: 0 });
                    }
                }
            }
            // (ironspec 20:07 PT: the panic's start, for the trigger-skip window [S - 60, S])
            if panic && !r.panic_prev {
                r.panic_since = Some(now);
                // (farmopt / ironspec 00:20 PT, the seat's sight) a panic starting at the queue
                // seat, with NEAREST_HOSTILE and the hatch's state
                if o.napper && c == [9, o.bed[1] + 13, -4] && seat_panics.len() < 40 {
                    let hatch = w.get(dbp(6, o.bed[1] + 15, -5)).to_string();
                    let who = hostile.and_then(|h| redsim::entity::index_of(w, h)).map(|k| {
                        let e = &w.entities.list[k];
                        format!("{:?} at ({:.2},{:.2},{:.2})", match &e.kind { EntityKind::Mob(m) => format!("{:?}", m.ty), _ => "?".to_string() }, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z)
                    });
                    seat_panics.push(format!("t {}: {} panicked at the queue seat, NEAREST_HOSTILE {}, the hatch {}", now - t0, r.label.split(' ').next().unwrap_or(""), who.unwrap_or_else(|| "none".to_string()), if hatch.contains("open=true") { "open" } else { "closed" }));
                }
            }
            r.panic_prev = panic;
            // (ironspec 00:22 PT) the seat's NEAREST_HOSTILE: the ticks a napper at the queue seat
            // has one set (a bar once the hatch input N OR HELD is in; information until then)
            if o.napper && c == [9, o.bed[1] + 13, -4] && hostile.is_some() {
                seat_hostile_ticks += 1;
                if seat_hostile_first.is_none() {
                    seat_hostile_first = Some(format!("t {} {}", now - t0, r.label.split(' ').next().unwrap_or("")));
                }
            }
            // (ironspec 00:33 PT, 58f770cc 00:39) the STRANDED-NAPPER bar: a napper waking in A6 needs
            // a cart at the pickup cell P by its wake edge + the declared bound (`# a6-pickup:`)
            if o.napper
                && let Some((p_cell, bound)) = o.a6_pickup
            {
                let was = sleep_prev.insert(id, sleeping).unwrap_or(false);
                if was && !sleeping && (o.bed[1] - 2..=o.bed[1] + 3).contains(&c[1]) {
                    wake_pending.insert(id, now + bound);
                    wakes_seen += 1;
                    // (ironspec 01:59 PT, each visit's decomposition)
                    r.milestones.push(("woke in A6".to_string(), now - t0));
                }
                if let Some(deadline) = wake_pending.get(&id).copied() {
                    let cart_at_p = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == p_cell);
                    if cart_at_p {
                        r.milestones.push(("a cart at P after the wake".to_string(), now - t0));
                        wake_pending.remove(&id);
                    } else if now > deadline {
                        wake_pending.remove(&id);
                        if stranded.len() < 12 {
                            stranded.push(format!("t {}: {} woke at t {} with no cart at P ({},{},{}) by the bound (+{bound})", now - t0, r.label.split(' ').next().unwrap_or(""), deadline - bound - t0, p_cell[0], p_cell[1], p_cell[2]));
                        }
                        stranded_n += 1;
                    }
                }
            }
            // (farmopt 00:38 PT, b8949c96's probe) each napper's path at the seat, every time it
            // changes (a load or a recompute): its target, its end, its length, complete or partial
            if o.napper && c == [9, o.bed[1] + 13, -4] {
                let now_path = (path_target, path_end);
                if seat_paths_last.get(&id) != Some(&now_path) {
                    seat_paths_last.insert(id, now_path);
                    if seat_paths.len() < 60 {
                        let f = |p: Option<[i32; 3]>| p.map_or("-".to_string(), |p| format!("({},{},{})", p[0], p[1], p[2]));
                        let complete = path_target.is_some() && path_target == path_end;
                        seat_paths.push(format!("t {} {}: to {} ending {} ({} nodes, {})", now - t0, r.label.split(' ').next().unwrap_or(""), f(path_target), f(path_end), path_from.map_or(0, |(_, n)| n), if path_target.is_none() { "no path".to_string() } else { format!("brain: the rider's own path memory, canReach {}, end {} target", reach.map_or("?", |r| if r { "true" } else { "false" }), if complete { "==" } else { "!=" }) }));
                    }
                }
            }
            // (the rider's lane: the lane whose pocket, drop, H or slopes it rides)
            if LANES.contains(&c[0]) && (0..=22).contains(&c[1]) && (0..=6).contains(&c[2]) {
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
            // (+1.62) to the zombie's (+1.74)); only the stop's own waker is left out (G1's CHANGES
            // 2): every other zombie counts everywhere, the stop's cells included
            {
                // (the eye in the world, for the world's line of sight)
                let eye = wpos(pos.x, pos.y + 1.62, pos.z);
                // (G1's CHANGES 2: only the waker is left out, the stop's own zombie, inside the
                // stop's box: every other zombie counts everywhere, the stop's cells included)
                let waker = |z: &redsim::entity::Vec3| z.x >= -6.0 && z.x < 5.0 && z.y >= (-21 + sy) as f64 && z.y < (-13 + sy) as f64 && z.z >= -6.0 && z.z < 7.0;
                let mut seen8 = false;
                let mut hidden605 = false;
                for z in zombies_now.iter().filter(|z| !waker(z)) {
                    let d = ((pos.x - z.x).powi(2) + (pos.y - z.y).powi(2) + (pos.z - z.z).powi(2)).sqrt();
                    if d >= 8.0 {
                        continue;
                    }
                    let sees = redsim::entity::mob::los::has_line_of_sight(w, eye, wpos(z.x, z.y + 1.74, z.z));
                    if sees {
                        seen8 = true;
                    } else if d < 6.05 {
                        hidden605 = true;
                    }
                }
                if seen8 {
                    r.last_seen8 = Some(now);
                }
                if hidden605 && !r.prev_hidden605 {
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
                r.prev_hidden605 = hidden605;
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
                            // (ironspec 04:02 PT: every stamp print names its trip: a napper
                            // outbound before the queue, in the stop, or on its return after the
                            // climb; a lane rider; ironspec D6: from the CURRENT visit only, a
                            // napper back in its lane being a lane rider again)
                            let on_trip = nappers.iter().any(|n| n.0 == id_me) && !r.milestones.is_empty() && !r.milestones.iter().any(|(m, _)| m == "back in its lane");
                            let trip = if on_trip {
                                if r.milestones.iter().any(|(m, _)| m == "in the napper lift" || m == "on the hold (9,0,8)") {
                                    "return, after the climb"
                                } else if r.milestones.iter().any(|(m, _)| m == "at the queue's arrival") {
                                    "in the stop"
                                } else {
                                    "outbound, before the queue"
                                }
                            } else {
                                "a lane rider"
                            };
                            r.bystander_stamp = Some((now, [pos.x, pos.y, pos.z], trip));
                            // (ironspec 19:07 PT: a bystander stamp on a departing rider, out of
                            // its pocket, is the T + 80 rule's failure; 0 in locked running)
                            let c = cell(w.entities.list[i].pos);
                            if !(c[1] == 0 && c[2] == 0 && LANES.contains(&c[0])) {
                                let locked = ctl.rels.last().is_some_and(|rl| (rl + t0).rem_euclid(100) == 5);
                                renew.departing_bystanders += 1;
                                if locked {
                                    renew.departing_locked += 1;
                                }
                                if renew.departing_rows.len() < 12 {
                                    // (farmopt 02:21 PT: which summoner's golem stamped it)
                                    let by = golem_summon.values().find(|g| g.0 == now).map_or(String::new(), |g| format!(", stamped by #{}'s summon{}", g.1, g.4.map_or(String::new(), |p| format!(" at ({:.1},{:.1},{:.1})", p[0], p[1], p[2]))));
                                    renew.departing_rows.push(format!("t {} {} at ({:.1},{:.1},{:.1}) ({trip}){}{by}", now - t0, r.label.split(' ').next().unwrap_or(""), pos.x, pos.y, pos.z, if locked { " (locked)" } else { "" }));
                                }
                            }
                        }
                    } else {
                        renew.sensor += 1;
                        r.gdr_setter = Some("the GolemSensor");
                        let src = w
                            .entities
                            .list
                            .iter()
                            .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::IronGolem))
                            .filter(|e| (dpos(e.pos).x - pos.x).abs() <= 17.0 && dpos(e.pos).y <= pos.y + 17.95 && dpos(e.pos).y + 2.7 >= pos.y - 16.0 && (dpos(e.pos).z - pos.z).abs() <= 17.0)
                            .min_by(|a, b| dpos(a.pos).distance_to_sqr(pos).total_cmp(&dpos(b.pos).distance_to_sqr(pos)))
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
                        ball_exits.push((format!("{} at t {}", r.label.split(' ').next().unwrap_or(""), now - t0), now - t0 - rt));
                        exit_log.push((now - t0, id));
                    }
                }
            }
            // (ironspec 09:59; E10 / G1 N13: ValidateNearbyPoi's strict squared test) within 16 of
            // the bed head
            if poi_near(o.bed, pos) {
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
                    if !r.milestones.iter().any(|(w, _)| w == "asleep in the bed") {
                        nap_times.entry(id).or_default().push(now - t0);
                        // (ironspec 15:43: naps per full night, bar 24; E7 / G1: keyed by the
                        // LIE-DOWN event, to the night whose REST window it falls in, so a nap
                        // spanning 24,000 counts once)
                        nights.entry(night_of(w.day_time())).or_default().insert(id);
                    }
                    mark(r, "asleep in the bed");
                }
                if c == [4, -19 + sy, 4] {
                    if !r.milestones.iter().any(|(w, _)| w == "at R") {
                        // (ironspec 13:44) its stamp's age at R
                        r.nap_stamps.push(format!("at R {}", stamp.map_or("unarmed".to_string(), |s| format!("{} gt", now - s))));
                        // (ironspec D2) R's tick, for "no R1 release before the previous napper passes R"
                        at_r_log.push((id, now - t0));
                    }
                    mark(r, "at R");
                }
                if c == [9, sy - 4, -5] && !r.milestones.iter().any(|(w, _)| w == "at the queue's arrival") {
                    // (ironspec 14:47) its stamp's age at its queue arrival (the way-in's landing
                    // detector)
                    let age = stamp.map(|s| now - s);
                    r.nap_stamps.push(format!("at the queue's arrival {}", age.map_or("unarmed".to_string(), |a| format!("{a} gt"))));
                    r.queue_age = Some(age);
                    mark(r, "at the queue's arrival");
                }
                // (ironspec 04:56 PT: the calm -> release distribution) on its way to the queue, the
                // first tick it isn't panicking and the first tick its CANT_REACH stamp is armed
                // (the leg path's calm)
                if !r.milestones.iter().any(|(w, _)| w == "at the queue's arrival") {
                    if !panic {
                        mark(r, "calm before the queue");
                    }
                    if stamp.is_some() {
                        mark(r, "CANT_REACH armed before the queue");
                    }
                }
                if c[0] == 6 && (-19 + sy..=-4).contains(&c[1]) && c[2] == 4 {
                    mark(r, "in the napper lift");
                }
                if c == [9, 0, 8] {
                    // (the hold's K follows the napper on it)
                    ctl.nap_lane = k;
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
                    // (day-1's picker: the lane's away latch resets on its napper's return; with the
                    // (A) stop's design controller G7 resets it at string_l's fall instead)
                    if !ctl.a_stop {
                        ctl.away[k] = false;
                    }
                }
                // (a returning napper on the row past its own K, before its lane: a wrong lane;
                // a picked cart on its way to the leg passes its K by design)
                if c[2] == 9 && c[1] == 25 && c[0] != lx - 2 && c[0] > lx - 2 && r.milestones.iter().any(|(w, _)| w == "on its K") && !r.milestones.iter().any(|(w, _)| w == "back in its lane") {
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
                r.prev_target_close = target_close;
                r.prev_key = target_key;
                continue;
            }
            if activity_rest && r.dusk.is_none() {
                r.dusk = Some(stamp.map(|s| now - s));
            }
            // (ironspec 16:08, G1 16:05) the longest continuous pathless dwell at night by position class:
            // the stamp's age at every REST tick (it runs while no complete path clears it), per class
            if activity_rest && let Some(s) = stamp {
                let age = now - s;
                let cls = if LANES.contains(&c[0]) && (17..=23).contains(&c[1]) && (1..=6).contains(&c[2]) {
                    0
                } else if LANES.contains(&c[0]) && (c[1] == 16 || c[1] == 0) && c[2] <= 1 {
                    1
                } else if (24..=26).contains(&c[1]) && (7..=10).contains(&c[2]) {
                    2
                } else if (-6..=-4).contains(&c[0]) {
                    3
                } else if c[0] == 9 && (-15..=-13).contains(&c[1]) && (-5..=-3).contains(&c[2]) {
                    4
                } else if c[1] <= -16 {
                    5
                } else {
                    6
                };
                if age > r.class_peak[cls].0 {
                    r.class_peak[cls] = (age, now - t0, c);
                }
            }
            // (G1 16:47, tick-exact 16:50: the hostile sensor's phase mod 20 is fixed per villager in normal
            // operation) VILLAGER_HOSTILES' scan phase (its timeToTick, SENSORS index 5)
            if r.hostile_phase.is_none() {
                let b = redsim::entity::mob::ai::brain::brain(w, i);
                r.hostile_phase = Some((now + b.sensors[5]).rem_euclid(20));
                // (tick-exact 17:03 (iii)) the sight list's own scan phase: NEAREST_LIVING_ENTITIES (index 0)
                r.living_phase = Some((now + b.sensors[0]).rem_euclid(20));
            }
            // (ironspec 17:10 (3)) per lane cell: the panic ticks, the stroll attempts (PANIC with WALK_TARGET
            // absent: one draw per tick), the draws that set a target, and the clears in PANIC
            if panic && LANES.contains(&c[0]) {
                let e = panic_cells.entry((c[0], c[1], c[2])).or_insert([0; 4]);
                e[0] += 1;
                if target.is_none() {
                    e[1] += 1;
                }
                if target.is_some() && r.prev_target.is_none() {
                    e[2] += 1;
                }
                if r.prev_stamp.is_some() && stamp.is_none() {
                    e[3] += 1;
                }
            }
            // (G1 16:37 (2)) the lift foot (-4,2,7): each pass of a rider through the cell counted
            if c == [-4, 2, 7] && r.prev_cell != Some(c) {
                r.foot_passes += 1;
            }
            r.prev_cell = Some(c);
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
                    if let Some(a) = r.arm_open.take() {
                        r.arm_pairs.push((a, Some(now - t0), "wiped by the world open (unsaved)".to_string()));
                    }
                }
                r.reloaded = false;
            } else if let (Some(ps), None) = (r.prev_stamp, stamp) {
                // (ironspec 13:44) a napper's first clear after R: the stamp's age then
                if r.milestones.iter().any(|(w, _)| w == "at R") && !r.nap_stamps.iter().any(|s| s.starts_with("at its first clear after R")) && nappers.iter().any(|n| n.0 == id) {
                    r.nap_stamps.push(format!("at its first clear after R (t {}) {} gt", now - t0, now - ps));
                }
                let napped = sleeping || last_slept == Some(now);
                // (ironspec 16:50 (3), 7b90c3f1 / G1 16:51) a panic clear's trigger: NEAREST_HOSTILE (its id and
                // distance) or HURT_BY (its age), so the lane's panic basis can be classified
                let trigger = {
                    let b = redsim::entity::mob::ai::brain::brain(w, i);
                    match (b.nearest_hostile, b.hurt_by) {
                        (Some(h), _) => {
                            let info = w.entities.list.iter().find(|e| e.id == h).map(|e| {
                                let (p, q) = (dpos(e.pos), dpos(w.entities.list[i].pos));
                                let d = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
                                let ty = match &e.kind {
                                    EntityKind::Mob(m) => format!("{:?}", m.ty).to_lowercase(),
                                    _ => "?".to_string(),
                                };
                                format!(" ({ty} at ({:.1},{:.1},{:.1}), {d:.1} away)", p.x, p.y, p.z)
                            });
                            format!(" [panic trigger: NEAREST_HOSTILE #{h}{}]", info.unwrap_or_default())
                        }
                        (None, Some(t)) => format!(" [panic trigger: HURT_BY {} gt ago]", now - t),
                        _ => " [panic trigger: none held]".to_string(),
                    }
                };
                let what = match r.prev_target {
                    Some(t) => {
                        let tc = [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32];
                        let kind = if r.prev_home.is_some_and(|h| h == tc) { "the bed" } else if panic { "a flee point" } else { "a stroll point" };
                        clearing_source(r.prev_stamp, stamp, napped, Some((tc, r.prev_target_close.unwrap_or(0))), c, kind).unwrap_or_default()
                    }
                    None => clearing_source(r.prev_stamp, stamp, napped, None, c, "").unwrap_or_default(),
                };
                r.clears.push(format!("t {} at {:?}: {what}{}", now - t0, c, if panic { trigger.as_str() } else { "" }));
                if let Some(a) = r.arm_open.take() {
                    let place = ["the pocket", "the lane", "the siding", "elsewhere"][place_of(c)];
                    r.arm_pairs.push((a, Some(now - t0), format!("{what} at {c:?} ({place})")));
                }
                if !activity_rest {
                    r.day_clears[place_of(c)] += 1;
                }
                // (the clears by place and cause, and the stretch since the last clear)
                let cause = if what.starts_with("a nap") { 0 } else if what.starts_with("its target reached") { 1 } else if what.starts_with("the sink's own tryComputePath") { 2 } else { 3 };
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
                // (ironspec 08:23 platform (2), G1 08:22 PT) each HOME loss's mechanism: the
                // release branch (SetWalkTargetFromBlockMemory with the pre-tick stamp over 1,200)
                // or ValidateNearbyPoi (the bed OCCUPIED, the feet within 16 of its head)
                let release = r.prev_stamp.is_some_and(|s| now - s > 1200);
                // (G1 10:47 O6) the farm-stop control's planted erasure is its own sample: neither a
                // release-branch loss nor a HOME loss of the run
                let planted = home_erased_id == Some(id) && home_erased;
                let mech = if release {
                    if !planted {
                        release_branches += 1;
                    }
                    format!("the release branch (pre-tick stamp {} gt > 1,200)", r.prev_stamp.map_or(0, |s| now - s))
                } else if bed_occupied && poi_near([o.bed[0], o.bed[1], o.bed[2]], pos) {
                    "ValidateNearbyPoi (the bed occupied, the feet within 16 of its head)".to_string()
                } else {
                    format!("unclassified (pre-tick stamp {}, the bed {})", r.prev_stamp.map_or("unarmed".to_string(), |s| format!("{} gt", now - s)), if bed_occupied { "occupied" } else { "free" })
                };
                // (G1's P14) the at-entry control's rider: a release after its aging is recorded (the
                // control claims none)
                if at_entry_rider == Some(id) && at_entry_released.is_none() {
                    at_entry_released = Some((now - t0, mech.clone()));
                }
                r.home_lost.push(format!("t {} at {:?} (day time {day}): {mech}{}", now - t0, c, if planted { " (the farm-stop control's planted erasure: its own sample)" } else { "" }));
                if !planted {
                    home_lost_total += 1;
                }
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
                                let q = dpos(w.entities.list[hi].pos);
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
                        .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && (dpos(e.pos).x - lane_x).abs() < 1.5 && dpos(e.pos).y > 15.0 && dpos(e.pos).y < 27.0)
                        .map(|e| format!("{}({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}){}", if Some(e.id) == my_cart { "ITS CART " } else { "" }, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.passengers.is_empty() { " empty" } else { "" }))
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
            r.prev_stamp = forced_stamp.or(stamp);
            r.prev_home = home;
            r.prev_target = target;
            r.prev_target_close = target_close;
            r.prev_key = target_key;
        }
        // (ironfarm 11:40) the traced cart's cells, and each lane's H..V occupancy at every release
        // (ironspec 02:18 PT (3a)) each lane's away count at every release: its nappers picked and
        // not yet back in their lane (the away latch's count); the lane's H..V carts at the first
        // release (the build guide's start: every lane full) give the free caller's margin at the
        // lane's least occupancy, the full count less the most away
        if o.napper && ctl.rel == Some(rel) {
            let occ: [i64; 3] = [0usize, 1, 2].map(|k| {
                let x = LANES[k];
                w.entities
                    .list
                    .iter()
                    .filter(|e| {
                        let c = cell(e.pos);
                        !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && c[0] == x && (16..=22).contains(&c[1]) && (0..=6).contains(&c[2])
                    })
                    .count() as i64
            });
            let full = *occ_full.get_or_insert(occ);
            let mut last_lane: BTreeMap<u32, usize> = BTreeMap::new();
            for (v, k, _) in &nappers {
                last_lane.insert(*v, *k);
            }
            // (G1 X3 / B8 U14) away from the pick itself until "back in its lane": a napper picked with
            // no milestone yet (panicking, no stamp) counts as away too
            let away: [i64; 3] = [0usize, 1, 2].map(|k| last_lane.iter().filter(|(v, kk)| **kk == k && recs.get(v).is_some_and(|r| !r.milestones.iter().any(|(m, _)| m == "back in its lane"))).count() as i64);
            for k in 0..3 {
                occ_min[k] = occ_min[k].min(full[k] - away[k]);
                away_max[k] = away_max[k].max(away[k]);
                if away[k] > 1 && away_rows.len() < 12 {
                    away_rows.push(format!("t {rel}: lane x {} {} away", LANES[k], away[k]));
                }
            }
        }
        // (tick-exact / ironspec 09:21-09:25 platform, day-1's push plug-in) every mob-on-cart push this
        // tick (redsim's push log, a diagnostic with no model state): the pushed cart, its lane at t0,
        // the pusher's activity, the impulse and its component along the cart's motion
        tick_pushes.clear();
        tick_push_ids.clear();
        push_hist.retain(|p| rel - p.0 <= 40);
        for p in redsim::entity::minecart::take_cart_pushes(w) {
            tick_pushes.push(format!("+push mob {} -> cart {} ({:+.4},{:+.4}) at d {:.3}", DRIVE_NAMES.with(|d| d.borrow().get(&p.mob).cloned()).unwrap_or_else(|| format!("#{}", p.mob)), DRIVE_NAMES.with(|d| d.borrow().get(&p.cart).cloned()).unwrap_or_else(|| format!("#{}", p.cart)), p.push[0], p.push[1], p.dist));
            tick_push_ids.push((p.mob, p.cart));
            // (G1 11:04) the recent pushes (tick, pusher, pushed cart, the pusher's vehicle and cell
            // then), for the per-stall print
            let veh = redsim::entity::index_of(w, p.mob).and_then(|i| w.entities.list[i].vehicle).map(|vid| (vid, redsim::entity::index_of(w, vid).map(|j| cell(w.entities.list[j].pos))));
            push_hist.push((rel, p.mob, p.cart, veh));
            let along = redsim::entity::index_of(w, p.cart).map_or(0.0, |k| {
                let v = w.entities.list[k].delta_movement;
                let n = (v.x * v.x + v.z * v.z).sqrt();
                if n > 1e-9 { (p.push[0] * v.x + p.push[1] * v.z) / n } else { 0.0 }
            });
            let act = redsim::entity::index_of(w, p.mob).map_or("gone".to_string(), |k| match &w.entities.list[k].kind {
                EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager => {
                    let b = redsim::entity::mob::ai::brain::brain(w, k);
                    format!("{}{}", b.ex.as_deref().map_or("-".to_string(), |x| format!("{:?}", x.active)), if w.entities.list[k].vehicle.is_some() { " seated" } else { " on foot" })
                }
                EntityKind::Mob(m) => format!("{:?}", m.ty),
                _ => "?".to_string(),
            });
            pushes.push((rel, p.cart, cart_lane0.get(&p.cart).copied(), act, (p.push[0] * p.push[0] + p.push[1] * p.push[1]).sqrt(), along));
        }
        // (ironspec 09:28) the cart-on-cart pushes too, for the trace's +push fields
        for p in redsim::entity::minecart::take_cart_cart_pushes(w) {
            tick_pushes.push(format!("+push cart {} -> cart {} {} (dot {:.3}, d {:.3})", DRIVE_NAMES.with(|d| d.borrow().get(&p.other).cloned()).unwrap_or_else(|| format!("#{}", p.other)), DRIVE_NAMES.with(|d| d.borrow().get(&p.this).cloned()).unwrap_or_else(|| format!("#{}", p.this)), p.what, p.dot, p.dist));
            tick_push_ids.push((p.other, p.this));
        }
        // (G1 08:21 PT) every cart's cell entries, for --pair-check (a cart in the draw cell, then the
        // path cell, within the pair's interval); since the 27th always (the per-stall print reads
        // each cart's junction from them)
        {
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                let c = cell(e.pos);
                let v = cart_entries.entry(e.id).or_default();
                if v.last().map(|(_, l)| *l) != Some(c) {
                    v.push((rel, c));
                }
            }
            // (7b90c3f1 11:01) the split levers (x,26,9)'s states, each change
            for x in [-3, -1, 1, 3, 5, 7] {
                let on = matches!(w.get(dbp(x, 26, 9)), redsim::block::Block::Lever { powered: true, .. });
                let h = lever_hist.entry(x).or_default();
                let rose = on && h.last().map(|(_, s)| *s) == Some(false);
                if h.last().map(|(_, s)| *s) != Some(on) {
                    h.push((rel, on));
                }
                // (ironspec 11:33, F4(a)'s bars) at a K(l) lever (l-2,26,9)'s rising edge (K arming), (ii)
                // its R (l-2,24,7) must be empty
                if let Some(k) = LANES.iter().position(|l| l - 2 == x)
                    && rose
                    && let Some(rc) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [x, 24, 7])
                {
                    f4_rows[1].push(format!("t {rel}: K({}) armed with cart #{} on its R ({},24,7)", LANES[k], rc.id, x));
                }
            }
            // (ironspec 11:33, F4(a)'s bars) (i) K(l) switched (its lever on) while another lane's napper
            // is on the split row, each episode's first tick; (iii) at most one napper between the
            // stop's exit (its A6 R passed this visit) and its K
            for (k, l) in LANES.iter().enumerate() {
                let kx = l - 2;
                let on = lever_hist.get(&kx).and_then(|h| h.last()).is_some_and(|(_, s)| *s);
                let foreign = if on {
                    w.entities.list.iter().find(|e| {
                        let c = cell(e.pos);
                        !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && c[1] == 25 && c[2] == 9 && e.passengers.first().is_some_and(|r| nappers.iter().rev().find(|(v, _, _)| v == r).is_some_and(|(_, nk, _)| *nk != k))
                    }).map(|e| (e.id, cell(e.pos)))
                } else {
                    None
                };
                match (foreign, f4_open[k]) {
                    (Some((id, c)), false) => {
                        f4_open[k] = true;
                        f4_rows[0].push(format!("t {rel}: K({l}) switched with another lane's napper cart #{id} on the split row at ({},{},{})", c[0], c[1], c[2]));
                    }
                    (None, true) => f4_open[k] = false,
                    _ => {}
                }
            }
            let between: Vec<u32> = nappers
                .iter()
                .filter(|(v, _, t)| {
                    let to = ctl.pick_carts.iter().filter(|(pt, _, pv, _)| pv == v && pt > t).map(|x| x.0).min().unwrap_or(i64::MAX);
                    let ms = |what: &str| recs.get(v).and_then(|r| r.milestones.iter().find(|(m, mt)| m == what && *mt >= *t && *mt < to).map(|(_, mt)| *mt));
                    ms("at R").is_some() && ms("on its K").is_none()
                })
                .map(|(v, _, _)| *v)
                .collect();
            if between.len() > 1 && !f4_between_open {
                f4_between_open = true;
                f4_rows[2].push(format!("t {rel}: {} nappers between the stop's exit and their K's ({})", between.len(), between.iter().map(|v| format!("#{v}")).collect::<Vec<_>>().join(", ")));
            } else if between.len() <= 1 {
                f4_between_open = false;
            }
            // (ironspec 11:29 / 11:33) an unridden cart at rest on a siding's R for 20 gt or more: "siding
            // blocked by an unridden cart", each episode
            // (ironspec 11:48 / 11:50) a cart that left every R starts a new visit
            r_rest_seen.retain(|id| w.entities.list.iter().any(|e| e.id == *id && !e.removed && LANES.iter().any(|l| cell(e.pos) == [l - 2, 24, 7])));
            for (k, l) in LANES.iter().enumerate() {
                // (ironspec 11:48 / 11:50) each napper cart's first rest on its siding's R: its z (the
                // presence string's bar: z >= 7.100)
                for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [l - 2, 24, 7]) {
                    let d = e.delta_movement;
                    if d.x * d.x + d.y * d.y + d.z * d.z < 0.003 * 0.003
                        && e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r))
                        && r_rest_seen.insert(e.id)
                    {
                        r_rests[k].push((dpos(e.pos).z, rel, e.id));
                    }
                }
                let empty_rest = w.entities.list.iter().find(|e| {
                    let d = e.delta_movement;
                    !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [l - 2, 24, 7] && e.passengers.is_empty() && d.x * d.x + d.y * d.y + d.z * d.z < 0.003 * 0.003
                }).map(|e| e.id);
                match (empty_rest, r_unridden[k]) {
                    (Some(id), Some((oid, since, flagged))) if id == oid => {
                        if !flagged && rel - since >= 20 {
                            r_unridden[k] = Some((id, since, true));
                            r_unridden_rows.push(format!("t {since}: lane x {l}'s R ({},24,7) blocked by unridden cart #{id} (20 gt at rest by t {rel})", l - 2));
                        }
                    }
                    (Some(id), _) => r_unridden[k] = Some((id, rel, false)),
                    (None, _) => r_unridden[k] = None,
                }
            }
        }
        // (G1 16:18 (2), ironspec 16:19 (b)) the fold's `# integrity:` declarations, read from the fold
        // file at its move (--integrity-from): each declared cell's shape judged every tick
        for (cell_i, want, name) in integrity.iter() {
            let have = w.get(dbp(cell_i[0], cell_i[1], cell_i[2])).to_string();
            if !have.contains(&format!("shape={want}")) {
                integrity_bad.push(format!("t {rel}: ({},{},{}) {name}: wanted shape={want}, holds {have}", cell_i[0], cell_i[1], cell_i[2]));
            }
        }
        // (G1 10:14: the drop-lip disagreement) every cart's arrival at a lane's C (x,24,7): its tick,
        // its horizontal speed, its z phase (the position's fraction) and its rider's activity
        for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
            let c = cell(e.pos);
            // (ironspec 10:29 (3): the denominator) a drop pass: the drop cell (x,24,6) into the column
            // (x,23,6); a C deflection: C (x,24,7) west into c3 (x-1,24,7) by any cart but a napper's
            if o.positions_at.contains(&rel) {
                let q = dpos(e.pos);
                positions_log.push(format!(
                    "t {rel}: cart #{} at ({:.6},{:.6},{:.6}) motion ({:.5},{:.5},{:.5}) rider {}",
                    e.id,
                    q.x,
                    q.y,
                    q.z,
                    e.delta_movement.x,
                    e.delta_movement.y,
                    e.delta_movement.z,
                    e.passengers.first().map_or("none".to_string(), |r| format!("#{r}"))
                ));
            }
            // (G1 15:55) the queue tail's worst z on each lane's slope (x, 17..22, 1..6) over the night,
            // against V (x,22,6)'s edge
            if let Some(k) = LANES.iter().position(|&x| c[0] == x) && (17..=22).contains(&c[1]) && (1..=6).contains(&c[2]) {
                let q = dpos(e.pos);
                if q.z > ctl.tail_worst[k] {
                    ctl.tail_worst[k] = q.z;
                }
            }
            if let Some(p) = c_prev_cell.get(&e.id).copied() {
                // (ironspec 12:58) the shared lift's mouth: a cart entering the lift column (-5,y,7)
                // (y 1..3) from outside it, with its class (a returning napper, else batch)
                if c[0] == -5 && c[2] == 7 && (1..=3).contains(&c[1]) && !(p[0] == -5 && p[2] == 7) {
                    let napper = e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r) && recs.get(r).is_some_and(|rr| rr.milestones.iter().any(|(m, _)| m == "at R") && !rr.milestones.iter().any(|(m, _)| m == "back in its lane")));
                    lift_entries.push((rel, e.id, if napper { "napper" } else { "batch" }));
                }
                // (dfb89499 14:19, ironspec 14:13 (3)(a)) the column's y-12 cell (-5,12,7): a cart
                // entering it from below (a pass: rising in the column) or from the braked slope
                // (-4,12,7) (lane 7's side entry), with its class
                if c == [-5, 12, 7] && p != c {
                    let napper = e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r) && recs.get(r).is_some_and(|rr| rr.milestones.iter().any(|(m, _)| m == "at R") && !rr.milestones.iter().any(|(m, _)| m == "back in its lane")));
                    let how = if p == [-4, 12, 7] { "entry" } else { "pass" };
                    y12_entries.push((rel, e.id, if napper { "napper" } else { "batch" }, how));
                }
                // (day-1 11:36 / ironspec 11:40 (3)) every cart's entry into a split junction's cell
                // (J (x,25,9) or K (x-2,25,9)), a lane's D (x,24,8) or the row's end (8,25,9): its tick,
                // class (a batch cart, the outbound napper (the picker's diverted cart), a returning
                // napper) and (ironspec 11:55 (2), checklist 1c) its STORED speed (deltaMovement)
                let tracked = (c[1] == 25 && c[2] == 9 && [-3, -1, 1, 3, 5, 7, 8].contains(&c[0])) || (c[1] == 24 && c[2] == 8 && LANES.contains(&c[0]));
                if tracked && p != c {
                    // (day-1 11:36) a napper's cart is the OUTBOUND napper while the picker diverts it,
                    // a RETURNING napper from its stop's R ("at R") until "back in its lane" on its
                    // current visit, and otherwise rides as a batch cart (a former napper back in its
                    // lane is batch until picked again)
                    let class = match e.passengers.first() {
                        Some(r) if nappers.iter().any(|(v, _, _)| v == r) => {
                            let ms = recs.get(r).map(|rr| (rr.milestones.iter().any(|(m, _)| m == "at R"), rr.milestones.iter().any(|(m, _)| m == "back in its lane")));
                            if ctl.divert.is_some_and(|(dc, _)| dc == e.id) {
                                "outbound napper"
                            } else if ms.is_some_and(|(at_r, back)| at_r && !back) {
                                "returning napper"
                            } else {
                                "batch"
                            }
                        }
                        Some(_) => "batch",
                        None => "empty",
                    };
                    let lane = e.passengers.first().and_then(|r| nappers.iter().rev().find(|(v, _, _)| v == r).map(|(_, k, _)| *k));
                    let d = e.delta_movement;
                    let stored = (d.x * d.x + d.z * d.z).sqrt();
                    junction_passes.push((rel, c, class, lane, stored));
                    // (ironspec 11:48 / 11:50, G1 11:50) a returning napper's STORED speed (its
                    // deltaMovement, which may exceed its per-tick move) entering its own K
                    if class == "returning napper"
                        && let Some(k) = lane
                        && LANES[k] - 2 == c[0]
                    {
                        let d = e.delta_movement;
                        k_speeds[k].push(((d.x * d.x + d.z * d.z).sqrt(), rel, e.id));
                    }
                }
                // (ironspec 11:23 / G1 11:19 F4 (b), R4) a westbound cart on the split row (x,25,9):
                // each episode's first tick, bar 0
                if c[1] == 25 && c[2] == 9 && e.delta_movement.x < -0.01 && !(p[1] == 25 && p[2] == 9 && westbound_open.contains(&e.id)) {
                    westbound_open.insert(e.id);
                    westbound_rows.push(format!("t {rel} cart #{} at ({},{},{}) moving west {:.3} b/t{}", e.id, c[0], c[1], c[2], e.delta_movement.x, e.passengers.first().map_or(" (empty)".to_string(), |r| if nappers.iter().any(|(v, _, _)| v == r) { format!(" (napper #{r})") } else { format!(" (batch #{r})") })));
                } else if !(c[1] == 25 && c[2] == 9 && e.delta_movement.x < -0.01) {
                    westbound_open.remove(&e.id);
                }
                for (k, x) in LANES.iter().enumerate() {
                    // (ironspec 11:09 (1) / G1's P18, the conservation ledger on the one lane region) a
                    // cart entering the region (+1) or leaving it (-1), with where from / to
                    let (was, is) = (in_lane_region(p, *x), in_lane_region(c, *x));
                    if !was && is {
                        ledger[k].push((rel, e.id, 1, format!("entered at ({},{},{}) from ({},{},{})", c[0], c[1], c[2], p[0], p[1], p[2])));
                    } else if was && !is {
                        ledger[k].push((rel, e.id, -1, format!("left from ({},{},{}) to ({},{},{})", p[0], p[1], p[2], c[0], c[1], c[2])));
                    }
                    if p == [*x, 24, 6] && c == [*x, 23, 6] {
                        drop_passes[k] += 1;
                        // (onechunk 11:16) a napper's cart dropping into a lane not its own: a misroute,
                        // with its split turns; (G1's P16) every napper drop counted, the denominator
                        if let Some(r) = e.passengers.first()
                            && nappers.iter().any(|(v, _, _)| v == r)
                        {
                            napper_drops += 1;
                        }
                        if let Some(r) = e.passengers.first()
                            && let Some((_, nk, _)) = nappers.iter().rev().find(|(v, _, _)| v == r)
                            && *nk != k
                        {
                            napper_wrong_lane.push(format!("t {rel}: napper #{r} of lane x {} (cart #{}) dropped into lane x {x}; its split turns: {}", LANES[*nk], e.id, split_turns(e.id, rel, &cart_entries, &t_changes, &t_shapes0, &lever_hist)));
                        }
                    }
                    if p == [*x, 24, 7] && c == [x - 1, 24, 7] && !e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r)) {
                        c_deflections[k].push(format!("t {rel} cart #{}", e.id));
                    }
                }
            }
            if let Some(k) = LANES.iter().position(|x| c == [*x, 24, 7])
                && c_prev_cell.get(&e.id) != Some(&c)
            {
                let v = e.delta_movement;
                let act = e.passengers.first().and_then(|p| redsim::entity::index_of(w, *p)).map_or("empty".to_string(), |i| {
                    let r = &w.entities.list[i];
                    format!(
                        "{} (riding, its box {:+.3},{:+.3},{:+.3} from the cart)",
                        redsim::entity::mob::ai::brain::brain(w, i).ex.as_deref().map_or("-".to_string(), |x| format!("{:?}", x.active)),
                        dpos(r.pos).x - dpos(e.pos).x,
                        dpos(r.pos).y - dpos(e.pos).y,
                        dpos(r.pos).z - dpos(e.pos).z
                    )
                });
                c_arrivals[k].push((rel, (v.x * v.x + v.z * v.z).sqrt(), act, e.id, dpos(e.pos).z - dpos(e.pos).z.floor(), dpos(e.pos).y, c_prev_cell.get(&e.id).copied().unwrap_or(c)));
                // (ironspec 11:08 (2) / 11:37, G1's P1) the arrival's pass window opens at its entry
                // into C
                if !pass_open.contains_key(&e.id) {
                    let kind = match e.passengers.first() {
                        None => "an empty cart".to_string(),
                        Some(r) if nappers.iter().any(|(v, _, _)| v == r) => format!("napper #{r}'s cart"),
                        Some(r) => format!("batch #{r}'s cart"),
                    };
                    pass_open.insert(e.id, PassWindow { lane: k, start: rel, kind, least: f64::INFINITY, hits: Vec::new(), seen: std::collections::BTreeSet::new(), pushed: Vec::new(), off_rail: 0, max_dx: 0.0, behind: false });
                }
            }
            c_prev_cell.insert(e.id, c);
        }
        // (ironspec 11:08 (2) / 11:37, G1's P1) every open pass window, every tick: any OTHER cart's box
        // or its rider's box intersecting the arriving cart's box inflated by the largest horizontal
        // in-tick step (0.015, tick-exact 11:33) at rest or moving, and every push on the arrival; the
        // window closes when the arrival's box falls below y 23.0 (into the column) or after 60 gt
        // (the game envelope's 12 off-rail gt plus the pass's own ~15, with margin), or when it leaves
        // the lane's C / lip / column cells another way
        {
            let mut closed: Vec<(u32, &'static str, [i32; 3])> = Vec::new();
            for (cid, pw) in pass_open.iter_mut() {
                let Some(ci) = redsim::entity::index_of(w, *cid) else {
                    closed.push((*cid, "removed", [0, 0, 0]));
                    continue;
                };
                let me = &w.entities.list[ci];
                let x = LANES[pw.lane];
                let c = cell(me.pos);
                let gone = !(c[0] == x && ((c[1] == 24 && (c[2] == 6 || c[2] == 7)) || (c[1] == 23 && c[2] == 6)));
                let fell = me.bb.min_y < 23.0;
                if fell || rel - pw.start > 60 || gone {
                    closed.push((*cid, if fell { "into the column" } else if gone { "out of the lane's C / lip / column another way" } else { "still on the lip after 60 gt" }, c));
                    continue;
                }
                // (G1's P3) tick-exact's quantities over the pass: the off-rail ticks, the lateral
                // deviation from the lane's centre line, a cart behind at C / D while on the lip
                let on_rail = redsim::rails::is_rail(&w.get(dbp(c[0], c[1], c[2]))) || redsim::rails::is_rail(&w.get(dbp(c[0], c[1] - 1, c[2])));
                if !on_rail {
                    pw.off_rail += 1;
                }
                pw.max_dx = pw.max_dx.max((dpos(me.pos).x - (x as f64 + 0.5)).abs());
                if c == [x, 24, 6] && w.entities.list.iter().any(|f| !f.removed && f.id != *cid && matches!(f.kind, EntityKind::Minecart(_)) && [[x, 24, 7], [x, 24, 8]].contains(&cell(f.pos))) {
                    pw.behind = true;
                }
                let my_rider = me.passengers.first().copied();
                let mb = me.bb.inflate(0.015, 0.0, 0.015);
                for o in w.entities.list.iter().filter(|o| !o.removed && o.id != *cid && Some(o.id) != my_rider) {
                    let cart = matches!(o.kind, EntityKind::Minecart(_));
                    let rider_of = o.vehicle.filter(|v| *v != *cid);
                    if !cart && rider_of.is_none() {
                        continue;
                    }
                    // the separation between the boxes (negative: an overlap)
                    let gap = [mb.min_x - o.bb.max_x, o.bb.min_x - mb.max_x, mb.min_y - o.bb.max_y, o.bb.min_y - mb.max_y, mb.min_z - o.bb.max_z, o.bb.min_z - mb.max_z].into_iter().fold(f64::NEG_INFINITY, f64::max);
                    pw.least = pw.least.min(gap);
                    if mb.intersects(o.bb) && pw.seen.insert(o.id) {
                        let d = o.delta_movement;
                        let still = d.x * d.x + d.y * d.y + d.z * d.z < 0.003 * 0.003;
                        let oc = cell(o.pos);
                        pw.hits.push((still, format!("t {rel}: {} #{} at ({},{},{}) {}", if cart { "cart" } else { "rider" }, o.id, oc[0], oc[1], oc[2], if still { "at rest" } else { "moving" }), o.id));
                    }
                }
                for p in push_hist.iter().filter(|p| p.0 == rel && p.2 == *cid) {
                    pw.pushed.push(format!("t {rel} by #{}", p.1));
                }
                // (ironspec 12:08 V-EMPTY's 4th sub-bar, G1 12:20) V empty of any cart, moving or not,
                // during the pass window: any other cart in V (x,22,6) or the column (x,23,6) is a hit
                // (a rollback included), once per cart
                for f in w.entities.list.iter().filter(|f| !f.removed && f.id != *cid && matches!(f.kind, EntityKind::Minecart(_)) && (cell(f.pos) == [x, 22, 6] || cell(f.pos) == [x, 23, 6])) {
                    if pw.seen.insert(f.id) {
                        let d = f.delta_movement;
                        let still = d.x * d.x + d.y * d.y + d.z * d.z < 0.003 * 0.003;
                        let fc = cell(f.pos);
                        pw.hits.push((still, format!("t {rel}: cart #{} in V's volume at ({},{},{}) {} (V not empty)", f.id, fc[0], fc[1], fc[2], if still { "at rest" } else { "moving" }), f.id));
                    }
                }
            }
            for (cid, why, at) in closed {
                if let Some(pw) = pass_open.remove(&cid) {
                    pass_judged[pw.lane] += 1;
                    // (G1's P2 / P3) the envelope's pass: its off-rail ticks, lateral deviation, a cart
                    // behind, and how the window closed (a pass ends in the column; any other close is
                    // an escape)
                    env_passes.push((pw.lane, pw.off_rail, pw.max_dx, pw.behind, why, cid, pw.start, at));
                    if !pw.hits.is_empty() || !pw.pushed.is_empty() {
                        pass_rows[pw.lane].push(PassRow { arrival: cid, start: pw.start, kind: pw.kind, least: pw.least, hits: pw.hits, pushed: pw.pushed });
                    } else {
                        pass_clean_least[pw.lane] = pass_clean_least[pw.lane].min(pw.least);
                    }
                }
            }
        }
        // (ironspec 08:13 / 08:14 platform (2), G1 08:12 PT) the cart-conservation check: each lane's
        // carts on its lane (H, R_1 and the slopes) with the farthest-back one, and any cart standing
        // still (motion under 0.001 two ticks running) on the exit booster, the exit row or the
        // split / merge row (z 8..9, y 23..26, x -5..9): bar never
        {
            for (k, x) in LANES.iter().enumerate() {
                let mut n = 0i64;
                let mut tail: Option<[i32; 3]> = None;
                for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                    let c = cell(e.pos);
                    // (G1 10:08 / ironspec 10:06: a cart counts as in its lane only on a rail or falling
                    // in the drop column; an off-rail rest is its own state, failing on its own bar)
                    let on_rail = redsim::rails::is_rail(&w.get(dbp(c[0], c[1], c[2]))) || redsim::rails::is_rail(&w.get(dbp(c[0], c[1] - 1, c[2])));
                    let v = e.delta_movement;
                    if !on_rail && v.x * v.x + v.y * v.y + v.z * v.z < 0.003 * 0.003 {
                        continue;
                    }
                    if c[0] == *x && (16..=22).contains(&c[1]) && (0..=6).contains(&c[2]) {
                        n += 1;
                        if tail.is_none_or(|t| (c[2], c[1]) > (t[2], t[1])) {
                            tail = Some(c);
                        }
                        // (G1 X31 / ironspec 08:13: the lane carts by id) a cart in a lane other than
                        // its lane at t0 (information: the split's J latches send a batch's 1st / 2nd
                        // / 3rd arrival to lanes -1 / 3 / 7 by arrival order, not by origin); a cart
                        // that wasn't a lane cart at t0 in a lane is a stray (bar 0)
                        if let Some(k0) = cart_lane0.get(&e.id).copied()
                            && k0 != k
                            && !wrong_lane.iter().any(|(id, _)| *id == e.id)
                        {
                            wrong_lane.push((e.id, format!("t {rel}: cart #{} (lane x {} at t0) in lane x {} at ({},{},{})", e.id, LANES[k0], x, c[0], c[1], c[2])));
                        }
                        if !cart_lane0.contains_key(&e.id) && !stray_carts.iter().any(|(id, _)| *id == e.id) {
                            stray_carts.push((e.id, format!("t {rel}: cart #{} (not a lane cart at t0) in lane x {x} at ({},{},{})", e.id, c[0], c[1], c[2])));
                        }
                    }
                }
                if n > lane_max[k].0 {
                    lane_max[k] = (n, rel, tail);
                }
                // (G1 10:24 (a) / 10:47 blocking 3: the per-lane COUNT over D4's region) every cart, on
                // a rail or not, in the lane's region (G1's P18: one region for the count, the ledger
                // and the per-stall print: in_lane_region), every tick; a 9th FAILS
                let count = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && in_lane_region(cell(e.pos), *x)).count();
                if ledger_base[k].is_none() {
                    ledger_base[k] = Some(count as i64);
                }
                if rel % 500 == 0 {
                    region_marks[k].push((rel, count as i64));
                }
                // (ironfarm 12:47, --lip-dump) the lane's carts this tick (the region + the lip, C, D):
                // each centre, stored motion and its rider's box, in a 60-gt ring per lane
                // (ironfarm 13:36, mobfarm::lipclass: the one lip-arrival classifier, fed every tick
                // with the lane's carts in the world frame, its lane's lip given in the world frame)
                {
                    let ids: Vec<u32> = w.entities.list.iter().filter(|f| !f.removed && matches!(f.kind, EntityKind::Minecart(_)) && in_lane_region(cell(f.pos), *x)).map(|f| f.id).collect();
                    for a in lip_lanes[k].tick(rel, &crate::lipclass::observe(w, &ids)) {
                        // (ironspec 15:17) the arrival's returning napper (its rider; None: a batch
                        // cart), as the lift's mouth classes it
                        lipc_who[k].push(redsim::entity::index_of(w, a.cart).and_then(|i| w.entities.list[i].passengers.first().copied()).filter(|r| nappers.iter().any(|(v, _, _)| v == r) && recs.get(r).is_some_and(|rr| rr.milestones.iter().any(|(m, _)| m == "at R") && !rr.milestones.iter().any(|(m, _)| m == "back in its lane"))));
                        lipc_arrivals[k].push(a);
                    }
                }
                if o.lip_dump.is_some() {
                    let rows: Vec<String> = w
                        .entities
                        .list
                        .iter()
                        .filter(|f| !f.removed && matches!(f.kind, EntityKind::Minecart(_)) && in_lane_region(cell(f.pos), *x))
                        .map(|f| {
                            let p = dpos(f.pos);
                            let d = f.delta_movement;
                            let rider = f.passengers.first().and_then(|r| redsim::entity::index_of(w, *r)).map_or("empty".to_string(), |ri| {
                                let b = w.entities.list[ri].bb;
                                let lo = dpos(redsim::entity::Vec3::new(b.min_x, b.min_y, b.min_z));
                                let hi = dpos(redsim::entity::Vec3::new(b.max_x, b.max_y, b.max_z));
                                format!("rider #{} box [{:.3},{:.3},{:.3}]..[{:.3},{:.3},{:.3}]", w.entities.list[ri].id, lo.x, lo.y, lo.z, hi.x, hi.y, hi.z)
                            });
                            format!("#{} ({:.4},{:.4},{:.4}) v({:.4},{:.4},{:.4}) {rider}", f.id, p.x, p.y, p.z, d.x, d.y, d.z)
                        })
                        .collect();
                    let ring = &mut lip_ring[k];
                    ring.push_back(format!("t {rel}: {}", rows.join("; ")));
                    while ring.len() > 60 {
                        ring.pop_front();
                    }
                }
                *lane_count_hist[k].entry(count).or_default() += 1;
                if count > lane_count_max[k].0 {
                    lane_count_max[k] = (count, rel);
                }
                if count >= 9 && lane_count_nine[k].is_none() {
                    lane_count_nine[k] = Some(rel);
                }
                // (G1 10:47 blocking 3: the tail never past V) V = (x,22,6), the lane's top slope cell
                // (the queue's 8th place: tick-exact's farm-c7 t0 seats the 8th rider there, ironfarm's
                // lanes end there): a cart at rest in the drop column (x,23..24,6) or at C while V
                // holds a cart is a queue backed past V, FAIL
                let v_full = w.entities.list.iter().any(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == [*x, 22, 6]);
                if v_full
                    && let Some(e) = w.entities.list.iter().find(|e| {
                        let c = cell(e.pos);
                        let d = e.delta_movement;
                        !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && (c == [*x, 23, 6] || c == [*x, 24, 6] || c == [*x, 24, 7] || c == [*x, 24, 8]) && d.x * d.x + d.y * d.y + d.z * d.z < 0.003 * 0.003
                    })
                {
                    let c = cell(e.pos);
                    past_v_ticks[k] += 1;
                    if past_v_first[k].is_none() {
                        past_v_first[k] = Some(format!("t {rel} cart #{} at rest at ({},{},{}) with V full", e.id, c[0], c[1], c[2]));
                    }
                }
                if let Some(t) = tail
                    && lane_tail_max[k].is_none_or(|m: [i32; 3]| (t[2], t[1]) > (m[2], m[1]))
                {
                    lane_tail_max[k] = Some(t);
                }
            }
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && Some(e.id) != lift_blocker) {
                let c = cell(e.pos);
                let watched = (8..=9).contains(&c[2]) && (23..=26).contains(&c[1]) && (-5..=9).contains(&c[0]);
                let still = e.delta_movement.x.abs() < 0.001 && e.delta_movement.y.abs() < 0.001 && e.delta_movement.z.abs() < 0.001;
                let was = still_prev.get(&e.id).copied().unwrap_or(false);
                if watched && still && was && stopped_rows.len() < 40 && !stopped_rows.last().is_some_and(|(id, _)| *id == e.id) {
                    stopped_rows.push((e.id, format!("t {rel}: cart #{} still at ({:.3},{:.3},{:.3}) {:?}{}", e.id, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, c, if e.passengers.is_empty() { " empty" } else { "" })));
                }
                if watched && still && was {
                    stopped_ticks += 1;
                }
                still_prev.insert(e.id, watched && still);
            }
        }
        // (dfb89499 15:11, ironspec 15:23 (5) for 7b90c3f1) the station's touching contacts, the
        // cart-cart contacts and the napper's seat on the hold, every tick
        touch.tick(w, rel, &ctl.rt_batches, &nappers);
        // (ironspec 10:04 / 10:06, G1 10:04 (d): conservation extends to every drop lip) every cart off
        // the rails (no rail in its cell or the cell below, the game's own test) each tick: at rest (its
        // motion under 0.003 a tick) 10 gt or more is a rest (bar 0, any cell); off the rails while
        // moving (the drop columns' falls) counted per cell as information; and the cars on a siding's
        // cells (x-2,25,9), (x-2,25,8), (x-2,24,7) by rider: a batch villager there FAILS
        {
            let w = &*w;
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                let c = cell(e.pos);
                // (the siding's carts by rider, every cart on or off a rail: the 25th's check ran only
                // for carts off a rail, so it never saw one; fixed in the 27th)
                if LANES.iter().any(|x| [[x - 2, 25, 9], [x - 2, 25, 8], [x - 2, 24, 7]].contains(&c)) {
                    let kind = match e.passengers.first() {
                        None => "empty",
                        Some(p) if nappers.iter().any(|(v, _, _)| v == p) => "napper",
                        Some(_) => "batch villager",
                    };
                    let k = siding_by.entry(kind).or_insert((0, None));
                    k.0 += 1;
                    if k.1.is_none() {
                        k.1 = Some(format!("t {rel} cart #{} at ({},{},{})", e.id, c[0], c[1], c[2]));
                    }
                    if kind == "batch villager" {
                        siding_batch_ids.insert(e.id);
                    }
                }
                let on = redsim::rails::is_rail(&w.get(dbp(c[0], c[1], c[2]))) || redsim::rails::is_rail(&w.get(dbp(c[0], c[1] - 1, c[2])));
                // (the station's holds H (x,16,0): a cart rests on the closed trapdoor there by design,
                // off any rail; its rests are listed apart and bounded; G1's P4: only the three hold
                // cells, a trapdoor anywhere else is no hold)
                let held = LANES.iter().any(|x| c == [*x, 16, 0]) && matches!(w.get(dbp(c[0], c[1], c[2])), redsim::block::Block::Trapdoor { .. });
                let v = e.delta_movement;
                let resting = v.x * v.x + v.y * v.y + v.z * v.z < 0.003 * 0.003;
                if on || held {
                    let snap = off_rest_snap.remove(&e.id);
                    if let Some((t0, c0, n)) = off_rest.remove(&e.id)
                        && n >= 10
                    {
                        off_rest_rows.push(format!("cart #{} at ({},{},{}) from t {t0}, {n} gt{}", e.id, c0[0], c0[1], c0[2], snap.map_or(String::new(), |s| format!(" {s}"))));
                        if let Some(k) = LANES.iter().position(|x| c0[0] == *x && c0[2] == 6 && (23..=24).contains(&c0[1])) {
                            lip_rests[k] += 1;
                        }
                    }
                    if held && resting {
                        *trapdoor_rests.entry(c).or_default() += 1;
                        let ent = h_open.entry(e.id).or_insert((rel, c, 0));
                        ent.2 += 1;
                    } else if let Some((t0, hc, n)) = h_open.remove(&e.id) {
                        h_rests.push((hc, t0, n, e.id));
                    }
                    continue;
                }
                if resting {
                    if !off_rest.contains_key(&e.id) {
                        // (ironspec 10:51 (c) / tick-exact 10:55: the per-stall print, at the rest's
                        // first tick) for a rest in or beside a lane's drop region: the carts in its
                        // drop column (x,23..24,6), at V (x,22,6) and below (x,21,5), each with its
                        // rider's activity and box top against the lip's floor (y 24.0), and the
                        // latest C arrival before it (the leader's gap)
                        if let Some(x) = LANES.iter().copied().find(|x| (c[0] - x).abs() <= 1 && (21..=25).contains(&c[1]) && (4..=8).contains(&c[2])) {
                            let k = LANES.iter().position(|v| *v == x).unwrap_or(0);
                            let near: Vec<String> = w
                                .entities
                                .list
                                .iter()
                                .filter(|f| !f.removed && f.id != e.id && matches!(f.kind, EntityKind::Minecart(_)) && [[x, 24, 6], [x, 23, 6], [x, 22, 6], [x, 21, 5], [x, 24, 7], [x, 24, 8]].contains(&cell(f.pos)))
                                .map(|f| {
                                    let fc = cell(f.pos);
                                    let r = f.passengers.first().and_then(|p| redsim::entity::index_of(w, *p)).map_or("empty".to_string(), |i| {
                                        let rv = &w.entities.list[i];
                                        let act = redsim::entity::mob::ai::brain::brain(w, i).ex.as_deref().map_or("-".to_string(), |a| format!("{:?}", a.active));
                                        // (a villager's box: 1.95 tall, or its seated height)
                                        format!("#{} {act}, its box top y {:.3} ({:+.3} vs the lip's floor 24.0)", rv.id, dpos(rv.pos).y + 1.95, dpos(rv.pos).y + 1.95 - 24.0)
                                    });
                                    format!("cart #{} at ({},{},{}) [{:.3},{:.3},{:.3}] rider {r}", f.id, fc[0], fc[1], fc[2], dpos(f.pos).x, dpos(f.pos).y, dpos(f.pos).z)
                                })
                                .collect();
                            let leader = c_arrivals[k].iter().rev().find(|a| a.3 != e.id).map(|a| format!("the latest other C arrival t {} (#{}, {} gt before)", a.0, a.3, rel - a.0)).unwrap_or_else(|| "no earlier C arrival".to_string());
                            let me = e.passengers.first().map_or("empty".to_string(), |p| format!("rider #{p}"));
                            // (ironspec 10:56 / 7b90c3f1 11:01, the root question) the lane's count and
                            // every slot's occupant with its identity (napper / batch, its t0 lane); the
                            // last C arrivals with where each entered C from (D: through J; c3: the
                            // napper's S-bend) and the split junction each took (J or K at (j,25,9)),
                            // that T's shape and its lever's state then, and the clearance to the
                            // cart through it before; the lane's last bottom release (a cart leaving H)
                            let ident = |id: u32| -> String {
                                let rider = w.entities.list.iter().find(|f| f.id == id).and_then(|f| f.passengers.first().copied());
                                let who = match rider {
                                    None => "empty".to_string(),
                                    Some(r) => match nappers.iter().rev().find(|(v, _, _)| *v == r) {
                                        Some((_, nk, _)) => format!("napper #{r} of lane x {}", LANES[*nk]),
                                        None => format!("batch #{r}"),
                                    },
                                };
                                format!("#{id} ({who}; t0 lane {})", cart_lane0.get(&id).map_or("-".to_string(), |l| LANES[*l].to_string()))
                            };
                            let slots = [("P", [x, 0, 0]), ("H", [x, 16, 0]), ("R_1", [x, 17, 1]), ("S1", [x, 18, 2]), ("S2", [x, 19, 3]), ("S3", [x, 20, 4]), ("S4", [x, 21, 5]), ("V", [x, 22, 6]), ("column", [x, 23, 6]), ("lip", [x, 24, 6]), ("C", [x, 24, 7]), ("D", [x, 24, 8])];
                            let occ: Vec<String> = slots.iter().map(|(n, sc)| {
                                let ids: Vec<String> = w.entities.list.iter().filter(|f| !f.removed && matches!(f.kind, EntityKind::Minecart(_)) && cell(f.pos) == *sc).map(|f| ident(f.id)).collect();
                                format!("{n} {}", if ids.is_empty() { "-".to_string() } else { ids.join("+") })
                            }).collect();
                            // (G1's P18) the lane's carts on the one region (the slots list names where)
                            let count = w.entities.list.iter().filter(|f| !f.removed && matches!(f.kind, EntityKind::Minecart(_)) && in_lane_region(cell(f.pos), x)).count();
                            let junction = |id: u32| -> String { split_turns(id, rel, &cart_entries, &t_changes, &t_shapes0, &lever_hist) };
                            let mine_c = c_arrivals[k].iter().rev().find(|a| a.3 == e.id).map(|a| format!("entered C at t {} from ({},{},{})", a.0, a.6[0], a.6[1], a.6[2]));
                            let last2: Vec<String> = c_arrivals[k].iter().rev().filter(|a| a.3 != e.id).take(2).map(|a| format!("{} at C t {} from ({},{},{}), split {}", ident(a.3), a.0, a.6[0], a.6[1], a.6[2], junction(a.3))).collect();
                            let last_h = h_rests.iter().filter(|r| r.0 == [x, 16, 0]).map(|r| r.1 + r.2).filter(|t| *t <= rel).max();
                            // (G1 11:04) the pushes on this cart in the last 40 gt: each pusher, its
                            // vehicle and that vehicle's cell (the game's configuration: a ridden cart in
                            // the drop column below the lip, its rider pushing the lip cart)
                            let pushes: Vec<String> = push_hist.iter().filter(|p| p.2 == e.id).map(|(t, m, _, veh)| {
                                let act = redsim::entity::index_of(w, *m).map_or("-".to_string(), |i| redsim::entity::mob::ai::brain::brain(w, i).ex.as_deref().map_or("-".to_string(), |a| format!("{:?}", a.active)));
                                // (G1 11:18) the pusher's PANIC trigger now: its nearest hostile (id, kind,
                                // distance) and its HURT_BY source
                                let trigger = redsim::entity::index_of(w, *m).map_or("-".to_string(), |i| {
                                    let b = redsim::entity::mob::ai::brain::brain(w, i);
                                    let me_pos = w.entities.list[i].pos;
                                    let host = b.nearest_hostile.and_then(|h| redsim::entity::index_of(w, h)).map_or("no nearest hostile".to_string(), |j| {
                                        let hp = w.entities.list[j].pos;
                                        let d = ((hp.x - me_pos.x).powi(2) + (hp.y - me_pos.y).powi(2) + (hp.z - me_pos.z).powi(2)).sqrt();
                                        format!("nearest hostile #{} {:?} at {d:.2}", w.entities.list[j].id, w.entities.list[j].kind)
                                    });
                                    let hurt = b.ex.as_deref().and_then(|x| x.mem.hurt_by.as_ref()).map_or("no HURT_BY".to_string(), |(dmg, by)| format!("HURT_BY {dmg:?}{}", by.map_or(String::new(), |a| format!(" from #{a}"))));
                                    format!("{host}; {hurt}")
                                });
                                format!("t {t} by #{m} ({act}; {trigger}) {}", veh.map_or("on foot".to_string(), |(v, vc)| format!("riding cart #{v} at {}", vc.map_or("?".to_string(), |c| format!("({},{},{})", c[0], c[1], c[2])))))
                            }).collect();
                            // (ironfarm 12:47, --lip-dump) the stalled lane's last 60 gt, dumped once per rest
                            off_rest_events.push((rel, k, e.id));
                            if o.lip_dump.is_some() {
                                lip_dumps.push(format!("== cart #{} at rest off the rails in lane x {x}'s drop region from t {rel}: the lane's carts over the 60 gt before (centre, stored motion, rider box; design frame)\n{}", e.id, lip_ring[k].iter().cloned().collect::<Vec<_>>().join("\n")));
                            }
                            off_rest_snap.insert(e.id, format!(
                                "[at its first tick: cart {} {me} at [{:.3},{:.3},{:.3}] in lane x {x}'s drop region ({}; split {}); the pushes on it in the last 40 gt: {}; the lane's carts {count}: {}; around it: {}; {leader}; the last two other C arrivals: {}; the lane's last bottom release (a cart leaving H) {}]",
                                ident(e.id),
                                dpos(e.pos).x,
                                dpos(e.pos).y,
                                dpos(e.pos).z,
                                mine_c.unwrap_or_else(|| "no C entry seen".to_string()),
                                junction(e.id),
                                if pushes.is_empty() { "none".to_string() } else { pushes.join(", ") },
                                occ.join(", "),
                                if near.is_empty() { "none".to_string() } else { near.join("; ") },
                                if last2.is_empty() { "none".to_string() } else { last2.join(" | ") },
                                last_h.map_or("none seen".to_string(), |t| format!("t {t} ({} gt before)", rel - t))
                            ));
                        }
                    }
                    let ent = off_rest.entry(e.id).or_insert((rel, c, 0));
                    ent.2 += 1;
                } else {
                    let snap = off_rest_snap.remove(&e.id);
                    if let Some((t0, c0, n)) = off_rest.remove(&e.id)
                        && n >= 10
                    {
                        off_rest_rows.push(format!("cart #{} at ({},{},{}) from t {t0}, {n} gt{}", e.id, c0[0], c0[1], c0[2], snap.map_or(String::new(), |s| format!(" {s}"))));
                        if let Some(k) = LANES.iter().position(|x| c0[0] == *x && c0[2] == 6 && (23..=24).contains(&c0[1])) {
                            lip_rests[k] += 1;
                        }
                    }
                    *off_moving.entry(c).or_default() += 1;
                }
            }
            // (G1 10:36, the drop-entry envelope) per cart on a lane's drop cell (x,24,6): its off-rail
            // ticks there and its largest lateral displacement; at the pass into the column its row
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
                let c = cell(e.pos);
                if let Some(k) = LANES.iter().position(|x| c == [*x, 24, 6]) {
                    let behind = w.entities.list.iter().any(|f| !f.removed && f.id != e.id && matches!(f.kind, EntityKind::Minecart(_)) && [[LANES[k], 24, 7], [LANES[k], 24, 8]].contains(&cell(f.pos)));
                    // (ironspec 11:01 / 11:02, the new bar) a cart arriving at the lip while V (x,22,6)
                    // holds a cart, bar 0
                    if !lip_state.contains_key(&e.id) {
                        // (day-1 / ironspec 11:06: the pair gaps and the loop transit) every lip arrival
                        lip_arrivals.push((k, rel, e.id, e.passengers.first().is_some_and(|r| nappers.iter().any(|(v, _, _)| v == r))));
                    }
                    let st = lip_state.entry(e.id).or_insert((0, 0.0, false));
                    st.0 += 1;
                    st.1 = st.1.max((dpos(e.pos).x - (LANES[k] as f64 + 0.5)).abs());
                    st.2 |= behind;
                } else {
                    // (G1's P2: the entry is dropped whichever way the cart leaves the drop cell; the
                    // envelope itself is judged on the pass windows)
                    lip_state.remove(&e.id);
                }
            }
            // (G1 10:34) --siding-trace FROM,TO: per tick, each lane's PRESENT_l, the carts on its
            // siding's cells (K, the drop to R, R, the S-bend) with their riders, and every napper's place
            if let Some((a, b)) = o.siding_trace
                && rel >= a
                && rel <= b
            {
                let mut row = format!("t {rel}:");
                for (k, x) in LANES.iter().enumerate() {
                    let cells = [[x - 2, 25, 9], [x - 2, 25, 8], [x - 2, 24, 7], [x - 2, 24, 6], [x - 1, 24, 6], [x - 1, 24, 7], [*x, 24, 7], [*x, 24, 6]];
                    let on: Vec<String> = w
                        .entities
                        .list
                        .iter()
                        .filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cells.contains(&cell(e.pos)))
                        .map(|e| {
                            let c = cell(e.pos);
                            let r = match e.passengers.first() {
                                None => "empty".to_string(),
                                Some(p) if nappers.iter().any(|(v, _, _)| v == p) => format!("napper #{p}"),
                                Some(p) => format!("batch #{p}"),
                            };
                            format!("#{} ({},{},{}) {r}", e.id, c[0], c[1], c[2])
                        })
                        .collect();
                    row.push_str(&format!(" lane x {x}: PRESENT {} siding [{}];", if ctl.present[k] { 1 } else { 0 }, on.join(", ")));
                }
                let np: Vec<String> = nappers.iter().filter_map(|(v, _, _)| redsim::entity::index_of(w, *v).map(|i| {
                    let p = dpos(w.entities.list[i].pos);
                    format!("#{v} ({:.2},{:.2},{:.2})", p.x, p.y, p.z)
                })).collect();
                row.push_str(&format!(" nappers {}", np.join(" ")));
                siding_rows.push(row);
            }
            // (G1 10:24, the cascade's seed) per lane: a cart off its rail in the drop region while
            // another cart is at C or D, one episode per contiguous run
            for (k, x) in LANES.iter().enumerate() {
                let at = |cells: &[[i32; 3]], need_off: bool| -> Option<u32> {
                    w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).find_map(|e| {
                        let c = cell(e.pos);
                        if !cells.contains(&c) {
                            return None;
                        }
                        let on = redsim::rails::is_rail(&w.get(dbp(c[0], c[1], c[2]))) || redsim::rails::is_rail(&w.get(dbp(c[0], c[1] - 1, c[2])));
                        (!need_off || !on).then_some(e.id)
                    })
                };
                let lip = at(&[[*x, 24, 6], [*x, 23, 6]], true);
                let behind = at(&[[*x, 24, 7], [*x, 24, 8]], false);
                match (lip, behind, lip_seed_open[k]) {
                    (Some(a), Some(b), None) => lip_seed_open[k] = Some((rel, a, b)),
                    (Some(_), Some(_), Some(_)) => {}
                    (_, _, Some((t0, a, b))) => {
                        lip_seeds.push((k, t0, rel - t0, a, b));
                        lip_seed_open[k] = None;
                    }
                    _ => {}
                }
            }
            // (ironspec 09:44 / 09:48, G1 09:45 (c): the T shapes per tick) every plain-rail T's shape;
            // a change of a declared never-switched T (the design's placement-order-ts) FAILS
            for (p, prev) in t_shapes.iter_mut() {
                let b = w.get(dbp(p[0], p[1], p[2]));
                let now = if matches!(b, redsim::block::Block::PlainRail { .. }) { format!("{:?}", redsim::rails::shape_of(&b)) } else { format!("not a plain rail ({b:?})") };
                if now != *prev {
                    t_changes.push((rel, *p, prev.clone(), now.clone()));
                    *prev = now;
                }
            }
        }
        // (G1 / ironspec 09:06 / 09:33 platform: the t1 activity assert) at the run's first tick every
        // villager's activity and the day time it reads: by day IDLE (or PANIC), at night REST (or
        // PANIC); any other is a bench that moved the day after building the brains
        if rel == 1 {
            let dt = w.day_time().rem_euclid(24_000);
            let night = (12_000..24_000).contains(&dt);
            let mut by: BTreeMap<String, usize> = BTreeMap::new();
            let mut off: Vec<String> = Vec::new();
            for (i, e) in w.entities.list.iter().enumerate() {
                if e.removed || !matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) {
                    continue;
                }
                let act = redsim::entity::mob::ai::brain::brain(w, i).ex.as_deref().map_or("-".to_string(), |x| format!("{:?}", x.active));
                let ok = act == "Panic" || if night { act == "Rest" } else { act == "Idle" };
                if !ok && off.len() < 8 {
                    off.push(format!("#{} {act}", e.id));
                }
                *by.entry(act).or_default() += 1;
            }
            t1_line = format!(
                "the t1 activity assert (ironspec 09:06 / 09:33 platform: the day time each villager's brain sees at the run's first tick): day time {dt} ({}); {}; {} off ({})",
                if night { "night: REST expected" } else { "day: IDLE expected" },
                by.iter().map(|(a, n)| format!("{a} {n}")).collect::<Vec<_>>().join(", "),
                off.len(),
                if off.is_empty() { "PASS".to_string() } else { format!("FAIL: {}", off.join(", ")) }
            );
        }
        // (ironspec 08:26 platform (3)) the chain trace: per tick in FROM..=TO, every napper (a villager
        // that was ever picked): its position, its cart or none, its activity, HOME, WALK_TARGET and
        // stamp; the bed's occupant; RS_ready; each lane's siding R occupancy
        if let Some((a, b)) = o.chain_trace
            && rel >= a
            && rel <= b
        {
            let mut row: Vec<String> = Vec::new();
            let mut seen: Vec<u32> = Vec::new();
            for (vid, _, _) in &nappers {
                if seen.contains(vid) {
                    continue;
                }
                seen.push(*vid);
                let Some(i) = redsim::entity::index_of(w, *vid) else { continue };
                let e = &w.entities.list[i];
                let b = redsim::entity::mob::ai::brain::brain(w, i);
                let ex = b.ex.as_deref();
                let act = ex.map_or("-".to_string(), |x| format!("{:?}", x.active));
                let wt = ex.and_then(|x| x.mem.walk_target.as_ref()).map(|t| match t.target {
                    redsim::entity::mob::ai::brain::Tracker::Block(p) => {
                        let o = frame_offset();
                        format!("({},{},{})", p.x - o[0], p.y - o[1], p.z - o[2])
                    }
                    redsim::entity::mob::ai::brain::Tracker::Point(v) => {
                        let q = dpos(v);
                        format!("({:.1},{:.1},{:.1})", q.x, q.y, q.z)
                    }
                    redsim::entity::mob::ai::brain::Tracker::Entity { id, .. } => format!("#{id}"),
                });
                let sleeping = matches!(&e.kind, EntityKind::Mob(m) if m.sleeping_pos.is_some());
                row.push(format!(
                    "{} ({:.2},{:.2},{:.2}) {} {}{} HOME {} WT {} stamp {}",
                    recs.get(vid).map_or(format!("#{vid}"), |r| r.label.split(' ').next().unwrap_or("").to_string()),
                    dpos(e.pos).x,
                    dpos(e.pos).y,
                    dpos(e.pos).z,
                    e.vehicle.map_or("on foot".to_string(), |v| format!("in cart #{v}")),
                    act,
                    if sleeping { " ASLEEP" } else { "" },
                    if b.home.is_some() { "yes" } else { "NONE" },
                    wt.unwrap_or_else(|| "-".to_string()),
                    ex.and_then(|x| x.mem.cant_reach_since).map_or("-".to_string(), |s| format!("{} gt", now - s))
                ));
            }
            let occupant = w.entities.list.iter().find(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.sleeping_pos.is_some())).map(|e| recs.get(&e.id).map_or(format!("#{}", e.id), |r| r.label.split(' ').next().unwrap_or("").to_string()));
            let sidings: Vec<String> = LANES
                .iter()
                .map(|x| {
                    let r = [x - 2, 24, 7];
                    let n = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && cell(e.pos) == r).count();
                    format!("R({}) {n}", x)
                })
                .collect();
            chain_rows.push(format!(
                "t {rel}: bed {}; RS_ready {}; PRESENT {}; D presses this tick {}; {}; {}",
                occupant.unwrap_or_else(|| "free".to_string()),
                ctl.q_rs_ready,
                LANES.iter().zip(ctl.present.iter()).map(|(x, p)| format!("{x}:{}", if *p { "set" } else { "-" })).collect::<Vec<_>>().join(" "),
                {
                    let d: Vec<String> = ctl.dpress_log.iter().filter(|(t, _)| *t == now).map(|(_, k)| LANES[*k].to_string()).collect();
                    if d.is_empty() { "none".to_string() } else { d.join(",") }
                },
                sidings.join(" "),
                row.join(" | ")
            ));
        }
        // (cc767b78 07:23 PT) the traced entity eN, after each tick
        if let Some((n, a, b)) = o.trace_entity
            && rel >= a
            && rel <= b
            && let Some(id) = lines.get(n).copied()
        {
            let row = match redsim::entity::index_of(w, id) {
                Some(k) => {
                    let e = &w.entities.list[k];
                    let c = cell(e.pos);
                    let below = [c[0], c[1] - 1, c[2]];
                    format!(
                        "t {rel}: pos ({:.4},{:.4},{:.4}) motion ({:.4},{:.4},{:.4}) rider {} on_ground {}; its cell {:?} {}, under it {}",
                        dpos(e.pos).x,
                        dpos(e.pos).y,
                        dpos(e.pos).z,
                        e.delta_movement.x,
                        e.delta_movement.y,
                        e.delta_movement.z,
                        e.passengers.first().map_or("none".to_string(), |p| DRIVE_NAMES.with(|d| d.borrow().get(p).cloned()).unwrap_or_else(|| format!("#{p}"))),
                        e.on_ground,
                        c,
                        w.get(dbp(c[0], c[1], c[2])).to_string().trim_start_matches("minecraft:"),
                        w.get(dbp(below[0], below[1], below[2])).to_string().trim_start_matches("minecraft:")
                    )
                }
                None => format!("t {rel}: e{n} (#{id}) is gone"),
            };
            // (ironspec 09:28) this tick's pushes involving the traced entity (as pusher or pushed, or
            // through its rider), tick-exact's +push fields
            let riders: Vec<u32> = redsim::entity::index_of(w, id).map_or(Vec::new(), |k| w.entities.list[k].passengers.clone());
            let mine: Vec<String> = tick_push_ids
                .iter()
                .zip(tick_pushes.iter())
                .filter(|((a, b), _)| *a == id || *b == id || riders.contains(a))
                .map(|(_, s)| s.clone())
                .collect();
            let row = if mine.is_empty() { row } else { format!("{row}; {}", mine.join("; ")) };
            trace_rows.push(row);
        }
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
                let r_on = w.get(dbp(x - 2, 24, 7)).to_string().contains("powered=true");
                let near: Vec<String> = w
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed && e.id != cart.id && matches!(e.kind, EntityKind::Minecart(_)) && e.pos.distance_to_sqr(cart.pos) <= 2.25)
                    .map(|e| format!("cart {} at ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}){}", e.id, dpos(e.pos).x, dpos(e.pos).y, dpos(e.pos).z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.passengers.is_empty() { " empty" } else { "" }))
                    .collect();
                nap_trace_rows.push(format!(
                    "t {rel}: napper cart {} at ({:.4},{:.4},{:.4}) v ({:.4},{:.4},{:.4}); R ({},24,7) {}; within 1.5: {}",
                    cart.id,
                    dpos(cart.pos).x,
                    dpos(cart.pos).y,
                    dpos(cart.pos).z,
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
            // (the golems born off the pad (feet above y -4), with the nearest villager at the birth)
            let off = off_pad_rows(w, &golem_born, &golem_birth_at, &golem_summon, t0);
            return Err(format!("a model error at t {rel}: {}{}", w.errors.join("; "), if off.is_empty() { String::new() } else { format!("; the golems born off the pad: {}", off.join("; ")) }));
        }
    }
    let mut s = format!(
        "f1q2-run ({}; REAL brains, the mob AI on): {} villagers, {} gt from day time {} (seed {}){}{}\n",
        if o.drive { "phase 2: the station, the R line and the split's latches driven, every drive EMULATED (day-1's v6 at s 12; the cycle's start per day-1's chain-bench 804fce5c9, G1 APPROVE, and its 16:14 statement: the pad's golem pulse (REL = the pulse + 6) or a retriggerable watchdog W gt from the last start (its cells not laid yet, box step 4); the counter C stepping on each release with its RISE / EIGHT re-phase; picks at C mod 10 = 1 / 4 / 7)" } else { "phase 1: every cart at rest, no drive" },
        recs.len(),
        o.ticks,
        o.day0,
        o.seed,
        if o.restarts.is_empty() { String::new() } else { format!(", {} restarts (order {})", o.restarts.len(), o.restart.label()) },
        control_id.map(|_| format!(", the positive control on v{}", o.control_rider.unwrap_or(0))).unwrap_or_default()
    );
    // (ironspec 14:57's rule) the spawning scope, explicit
    s.push_str(&format!(
        "the build position (onechunk 15:08, ironspec 15:05 (e)): offset ({},{},{}); the loop origin at world ({},{},{}); the module anchor (-6,0,-6) at world ({},{},{}): {}\n",
        o.offset[0],
        o.offset[1],
        o.offset[2],
        o.offset[0],
        o.offset[1],
        o.offset[2],
        o.offset[0] - 6,
        o.offset[1],
        o.offset[2] - 6,
        if (o.offset[0] - 6).rem_euclid(16) == 0 && (o.offset[2] - 6).rem_euclid(16) == 0 { "a world chunk corner (the column convention: the module in one chunk)" } else { "NOT a world chunk corner (off the column convention: the module straddles chunk lines)" }
    ));
    s.push_str(&format!(
        "the spawning scope (ironspec 14:57): natural spawning OFF, set explicitly (no bar here counts a natural spawn; the hostiles are the design's own scarers){}\n",
        if natural_was { " (the run's options had a spawner on: turned off)" } else { "" }
    ));
    s.push_str(&format!(
        "the world as built: {} rails whose shape differs from the file (0 required: the file pasted unsettled){}\n",
        rail_mismatch.len(),
        rail_mismatch.iter().take(6).map(|m| format!("; {m}")).collect::<String>()
    ));
    // (ironspec 07:43 platform, G1 07:50) the site the run stands in
    s.push_str(&format!("{site_line}\n"));
    // (G1 10:47 O12) a design run with a `no-home`-tagged villager loses that HOME on purpose only in
    // the c7 fidelity row: anywhere else it FAILS, never silently
    if o.c7_placement.is_none() {
        s.push_str(&format!(
            "the no-home-tagged villagers in the design (G1 10:47 O12: `Tags:[\"no-home\"]` skips `port home`; allowed only in the c7 fidelity row): {} ({})\n",
            o.no_home_tagged,
            if o.no_home_tagged == 0 { "PASS" } else { "FAIL" }
        ));
    }
    // (tick-exact 06:09 PT, ironspec 07:57 platform (4) / (5)) the c7 placement's start: a FIDELITY
    // row only (no controller, static queues, no rotation: not the design's operation)
    if o.c7_placement.is_some() {
        s.push_str(&format!(
            "the c7 placement (tick-exact's farm-c7 reconciliation row, FIDELITY ONLY; AcquirePoi fidelity pending: HOME-less villagers near the bed; ticket state uncertain: pre-fix redsim): the stored riders' HOME {} at t 0; the record loaded with spec_of's PortHome::{} (ironspec 08:16: the per-villager / whole-file `port home` opt-out){}; the HOME holders after each setup phase: {}; the bed's free tickets at t 0 {} (tick-exact's game: free); the stored riders holding HOME after the setup: {} ({})\n",
            if o.c7_home_stored { "the bed (--c7-home-stored)" } else { "none, as tick-exact's game setup" },
            if o.c7_home_stored { "Load" } else { "Skip" },
            if c7_strip.is_empty() { String::new() } else { format!("; the opt-out per entity (G1 10:47 blocking 5: HOME holders under Load -> as loaded; a no-op FAILS): {}", c7_strip.join("; ")) },
            c7_phase_seen.iter().map(|(ph, ids)| format!("{ph} {}", ids.len())).collect::<Vec<_>>().join(", "),
            c7_free_t0.map_or("unreadable".to_string(), |n| n.to_string()),
            if c7_holders.is_empty() { "none".to_string() } else { c7_holders.join("; ") },
            if o.c7_home_stored { "not judged: --c7-home-stored" } else if c7_holders.is_empty() { "PASS" } else { "FAIL: the setter unknown (cc767b78 pins it)" }
        ));
    }
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
    // (ironspec 19:57 PT (2a)) the way-in's airborne second fall, until b8949c96's descent (C) is
    // laid: a stamp peak or a HOME loss in its cells (x 4..9, y <= -13, z <= 0: the queue, the fall,
    // the roll to A6's entry) is printed in its own class, "the way-in's fall (C pending)", not
    // against the bars
    // (G1's [7]) the way-in's fall exempted from the bars only while (C)'s descent isn't laid: a
    // module declaring its way-in (`# way-in:`, C7 composed) puts the fall back under the bars
    let in_fall = |c: [i32; 3]| (4..=9).contains(&c[0]) && c[1] <= -13 && c[2] <= 0 && o.napper && !o.way_in_composed;
    let lost_at = |s: &str| -> Option<[i32; 3]> {
        let a = s.split(" at [").nth(1)?.split(']').next()?;
        let v: Vec<i32> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    let worst = recs.values().filter_map(|r| r.peak.filter(|p| !in_fall(p.2)).map(|p| p.0)).max().unwrap_or(0);
    let fall_peaks: Vec<String> = recs.values().filter_map(|r| r.peak.filter(|p| in_fall(p.2)).map(|p| format!("{} {} (t {} at {:?})", r.label.split(' ').next().unwrap_or(""), p.0, p.1, p.2))).collect();
    // (G1 10:47 O6) the farm-stop control's planted erasure is its own sample, not a HOME loss
    let lost: usize = recs.iter().filter(|(id, r)| Some(**id) != home_erased_id && r.home_lost.iter().any(|h| lost_at(h).is_none_or(|c| !in_fall(c)))).count();
    let fall_lost: Vec<String> = recs.values().flat_map(|r| r.home_lost.iter().filter(|h| lost_at(h).is_some_and(in_fall)).map(|h| format!("{} {h}", r.label.split(' ').next().unwrap_or("")))).collect();
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
            "clears at {name}, all riders (G1 07:28 PT: the sources that erase the stamp; no navigation recompute counts): a nap (the sleep start) {} ({:.2} a lap), the target reached (MoveToTargetSink's Manhattan test) {} ({:.2}), the sink's own tryComputePath {} ({:.2}), no target on record {}\n",
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
        "the bystander stamps (ironspec 02:37 PT, restated as the harm: every stamp printed with its trip, its stamping summoner and the stamped rider's margin at its next own-pocket summon; the bar is every napper's first call back in its lane, its memory, whatever its source, cleared with the free caller's margin >= 3; the summon check's 'other stamp' blocks stay the gate): {} on riders out of their pockets ({} in locked running){}; the margins at the next own-pocket summon: {}; {} stamps with no next summon inside the run; the nappers' first calls back: {}; the least {} ({})\n",
        renew.departing_bystanders,
        renew.departing_locked,
        if renew.departing_rows.is_empty() { String::new() } else { format!(": {}", renew.departing_rows.join("; ")) },
        if renew.bystander_rows.is_empty() { "none".to_string() } else { renew.bystander_rows.join("; ") },
        recs.values().filter(|r| r.bystander_stamp.is_some()).count(),
        if nap_margins.is_empty() {
            "none".to_string()
        } else {
            nap_margins
                .iter()
                .map(|(v, t, m, by)| format!("{} t {t} {}", recs.get(v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string()), m.map_or("free (never set)".to_string(), |m| format!("{m:+} gt (set by {by})"))))
                .collect::<Vec<_>>()
                .join(", ")
        },
        nap_margins.iter().filter_map(|x| x.2).min().map_or("-".to_string(), |m| format!("{m:+} gt (scope, G1 N7: {} of {} visits judged; a visit whose call back falls past the run's end isn't judged)", nap_margins.len(), nappers.len())),
        if nap_margins.is_empty() {
            "NOT JUDGED (0 nappers called back inside the run): FAIL"
        } else if nap_margins.iter().any(|x| x.2.is_some_and(|m| m < 3)) {
            "FAIL"
        } else {
            "PASS"
        }
    ));
    // (ironspec 02:37 PT, farmopt 02:42; G1 N9 / ironspec 07:18 real: by the SUMMONER's cell against
    // each declared place's cell set, not by height) a pocket rider (x,0,0), a hold rider (x,16,0:
    // can't reach the pad, expected 0), any undeclared place (bar 0, each with the golem's spawn
    // cell); 0 summons isn't judged
    {
        let (mut pocket, mut hold, mut other) = (0usize, 0usize, Vec::new());
        for (g, (t, v, _, at, p)) in &golem_summon {
            // (the summoner's own cell: its position at the call; `at` is the golem's spawn cell)
            let c = p.map(|p| [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
            let _ = at;
            match c {
                Some(c) if LANES.iter().any(|x| c == [*x, 0, 0]) => pocket += 1,
                Some(c) if LANES.iter().any(|x| c == [*x, 16, 0]) => hold += 1,
                _ => other.push(format!(
                    "t {} #{v} at {}, the golem #{g} spawned at {}",
                    t - t0,
                    c.map_or("?".to_string(), |c| format!("({},{},{})", c[0], c[1], c[2])),
                    golem_birth_at.get(g).map_or("?".to_string(), |(bp, _)| format!("({:.2},{:.2},{:.2})", bp[0], bp[1], bp[2]))
                )),
            }
        }
        s.push_str(&format!(
            "the successful summons by the summoner's place (ironspec 02:37 PT / 07:18 real, G1 N9: the declared places are the pockets (x,0,0); hold riders (x,16,0) never stamp; an undeclared place fails): {} golems, by pocket riders {pocket}, by hold riders {hold} (expected 0), from undeclared places {}{} ({})\n",
            golem_summon.len(),
            other.len(),
            if other.is_empty() { String::new() } else { format!(": {}", other.join(", ")) },
            if golem_summon.is_empty() { "NOT JUDGED (0 summons): FAIL" } else if hold == 0 && other.is_empty() { "PASS" } else { "FAIL" }
        ));
    }
    s.push_str(&format!(
        "the summons from the pockets (farmopt 12:10, ironspec 12:13: the stand is the pocket; at each summon tick T every scared, slept villager in a pocket cell (x,0,0) calls, the villagers in its box inflated by 10 counted or rejected; a memory present at T: a ttl of 1 or more before the tick): {} calls, {} callers blocked by their own GOLEM_DETECTED_RECENTLY (by setter: {}), {} villagers in the boxes rejected for it (not callers: informational), {} summons blocked by those rejections (participations blocked: callers blocked + summons blocked, bar 0: {}); the least free caller's margin (T - (last set + 599)) {}; each lane's least rotation at a summon tick (the riders whose last lane is it, a napper out while away; bar >= 7): {}; the most nappers away from one lane at once: {}{}\n",
        summons.attempts,
        summons.callers_blocked,
        if summons.blocked_by.is_empty() { "none".to_string() } else { summons.blocked_by.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ") },
        summons.rejected,
        summons.blocked,
        if summons.blocked + summons.callers_blocked == 0 { "PASS" } else { "FAIL" },
        // (ironspec's E9 / G1 N7: ONE harm verdict, the harm bar's; these margins print as information)
        summons.least_caller_margin.map_or("none seen".to_string(), |m| format!("{m} gt (from the latest set; information: the harm bar judges each napper's first call back)")) + &summons.least_orig_margin.map_or(String::new(), |m| format!("; from the stamp's original tick {m} gt (information: the old >= 23 of ironspec 19:40 PT is superseded)")),
        LANES.iter().zip(summons.min_rotation.iter()).map(|(x, m)| format!("lane {x} {}", m.map_or("-".to_string(), |v| v.to_string()))).collect::<Vec<_>>().join(", "),
        usize::from(!nappers.is_empty()),
        if summons.rows.is_empty() { String::new() } else { format!(": {}", summons.rows.join("; ")) }
    ));
    // (ironspec 16:08 (3)) by day: the calls, the calls with 3 willing in the box, the golems, and
    // the willing villagers counted per call
    {
        let mut golems_by_day: BTreeMap<i64, i64> = BTreeMap::new();
        for b in golem_born.values() {
            *golems_by_day.entry((day_start + b - t0).div_euclid(24_000)).or_insert(0) += 1;
        }
        let rows: Vec<String> = summons
            .by_day
            .iter()
            .map(|(d, (calls, three))| {
                let hist: Vec<String> = (0..=5).map(|n| format!("{n}{}: {}", if n == 5 { "+" } else { "" }, summons.counted_hist.get(&(*d, n)).copied().unwrap_or(0))).collect();
                format!("day {d}: {calls} calls, {three} with 3 willing in the box, {} golems; the willing per call {}", golems_by_day.get(d).copied().unwrap_or(0), hist.join(" "))
            })
            .collect();
        s.push_str(&format!("the summons by day (ironspec 16:08 (3), the willing: LAST_SLEPT within 24,000, no GOLEM_DETECTED_RECENTLY): {}\n", rows.join("; ")));
        // (every golem is born on the pad: feet at y -6; one born higher is a summon spawning off it)
        let off = off_pad_rows(&run.world, &golem_born, &golem_birth_at, &golem_summon, t0);
        s.push_str(&format!("the golems born off the pad (feet above y -4; bar 0): {}{}\n", off.len(), if off.is_empty() { " (PASS)".to_string() } else { format!(" (FAIL): {}", off.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }));
        // (ironspec 19:24) the direct summon check
        let missed: i64 = direct.misses.iter().sum();
        // (ironspec 19:33 PT) each summon tick's class; in locked running only a golem born or a
        // spot miss; the spot-miss rate beside the pad's expected (40-44 of 289 reachable spots per
        // caller, 10 tries: (1 - 42/289)^10, about 21%)
        let class_rows: Vec<String> = ["golem born", "spot miss", "caller blocked by its own memory", "fewer than 3 willing", "no panicking caller"].iter().map(|c| {
            let (n, l) = direct.classes.get(c).copied().unwrap_or((0, 0));
            format!("{c} {n} ({l} locked)")
        }).collect();
        let locked_bad: i64 = ["caller blocked by its own memory", "fewer than 3 willing", "no panicking caller"].iter().map(|c| direct.classes.get(c).map_or(0, |x| x.1)).sum();
        let (born_n, miss_n) = (direct.classes.get("golem born").map_or(0, |x| x.0), direct.classes.get("spot miss").map_or(0, |x| x.0));
        s.push_str(&format!(
            "the summon ticks by class (ironspec 19:33 PT; information since 19:51, the bar is the calls below): {}; in locked running only a golem born or a spot miss ({}); the spot-miss rate {} (the pad's expected about 21%: 40-44 of 289 spots, 10 tries)\n",
            class_rows.join(", "),
            if locked_bad == 0 { "none other" } else { "others seen" },
            if born_n + miss_n > 0 { format!("{:.1}% of {}", 100.0 * miss_n as f64 / (born_n + miss_n) as f64, born_n + miss_n) } else { "-".to_string() }
        ));
        // (ironspec 19:58 PT, G1) the direct summon check per copy's stand (its three pockets
        // together): bar >= 1 roll (>= 3 willing) from the stand at every due S in locked running;
        // the riders covered by this S's own golem apart, a stamp from any other source a block, a
        // trigger skip (panic begun in [S - 60, S]) a skip; the golem rate by the willing-and-ticking
        // count at the tick's start against 1 - 0.209^k (about 99 / 95.6 / 79 % for 3 / 2 / 1)
        let ready_rows: Vec<String> = direct.by_ready.iter().map(|(k, (n, g))| {
            let exp = 1.0 - 0.209f64.powi(*k as i32);
            format!("k {k}: {g} golems at {n} S ({:.1}%, expected {:.1}%)", if *n > 0 { 100.0 * *g as f64 / *n as f64 } else { 0.0 }, 100.0 * exp)
        }).collect();
        s.push_str(&format!(
            "the direct summon check (ironspec 19:58 PT: per copy's stand, its three pockets together; every trigger call at a due S from the pockets: the rolls up to the first golem, the riders covered by this S's golem, blocked by an older stamp, not slept within a day (LAST_SLEPT expired, no stamp), short of willing): {} due S ({} locked), golems at {}; S without a roll from the stand {} ({} locked: bar 0 in locked running: {}); riders covered {}, blocked {}, not slept within a day {}, short of willing {}, trigger skips (S = s + 61 (mod 62) for the rider's panic start s; tick-exact's row) {}; the golem rate by the willing-and-ticking count at the tick's start: {}{}{}\n",
            direct.s_count,
            direct.s_locked,
            direct.s_born,
            direct.stand_misses,
            direct.stand_misses_locked,
            if direct.s_locked == 0 { "NOT JUDGED (0 locked S): FAIL" } else if direct.stand_misses_locked == 0 { "PASS" } else { "FAIL" },
            direct.covered,
            direct.blocked,
            direct.not_slept,
            direct.short,
            direct.skips,
            if ready_rows.is_empty() { "-".to_string() } else { ready_rows.join(", ") },
            format!(
                "{}{}",
                o.empty_pocket.map_or(String::new(), |x| format!(" (the control: lane x {x}'s pocket rider removed at the start)")),
                direct.first_miss.as_ref().map_or(String::new(), |(t, r)| format!("; the first S without a roll t {t} (its pocket riders: {r})"))
            ),
            if direct.call_rows.is_empty() { String::new() } else { format!(": {}", direct.call_rows.join("; ")) }
        ));
        s.push_str(&format!(
            "  the riders' states at S (ironspec 19:24, information since 19:51: a seated, panicking, slept rider without GOLEM_DETECTED_RECENTLY and 3 willing in its 10-box; the panic trigger can skip at start + 61): {} pocket-ticks over {} summon ticks, without one lane x -1 {} / 3 {} / 7 {}{}\n",
            direct.checks,
            direct.checks / 3,
            direct.misses[0],
            direct.misses[1],
            direct.misses[2],
            if direct.rows.is_empty() { String::new() } else { format!(": {}", direct.rows.join("; ")) }
        ));
        let _ = missed;
        // (ironspec 19:58 PT (2b)) the split's Ts
        // (ironspec 21:25 PT) the carts' contacts with blocks
        {
            let placed: Vec<String> = PLACED.with(|p| p.borrow().iter().map(|(c, (st, n))| format!("({},{},{}) {st} x{n}", c[0], c[1], c[2])).collect());
            s.push_str(&format!(
                "the stand-ins placed (ironspec 21:40 / 23:13 PT; each emulated write passed record-match's guard_intrinsics (golem as a floor and as a spawn space, the full PathType, late-path, sight, each as the built cell) and this bench's T reach, power reach and survival checks; a refusal stops the run): {}\n",
                if placed.is_empty() { "none".to_string() } else { placed.join("; ") }
            ));
            let undeclared: Vec<String> = contacts.iter().filter(|(c, _)| !o.stops.contains(c)).map(|(c, (n, t, b))| format!("({},{},{}) {b} {n} ticks from t {t}", c[0], c[1], c[2])).collect();
            let declared: Vec<String> = contacts.iter().filter(|(c, _)| o.stops.contains(c)).map(|(c, (n, _, b))| format!("({},{},{}) {b} {n}", c[0], c[1], c[2])).collect();
            s.push_str(&format!(
                "the carts' contacts with blocks (ironspec 21:25 / 22:05 PT: a cart's move stopped sideways or from above, the blocks its box meets; each must be a stop (`stops:`) or a contact (`# contacts: CELLS=REASON (evidence row)`) the header declares): {} cells, undeclared {} ({}){}{}\n",
                contacts.len(),
                undeclared.len(),
                if undeclared.is_empty() { "PASS" } else { "FAIL" },
                if undeclared.is_empty() { String::new() } else { format!(": {}", undeclared.iter().take(120).cloned().collect::<Vec<_>>().join("; ")) },
                if declared.is_empty() { String::new() } else { format!("; declared: {}", declared.join("; ")) }
            ));
        }
        // (ironspec 04:07 PT) every bench override with its cell's state before and after; one that
        // doesn't change its cell fails the run
        let noops: Vec<&([i32; 3], String, String)> = overrides.iter().filter(|(_, b, a)| b == a).collect();
        s.push_str(&format!(
            "the bench overrides (ironspec 04:07 PT: each emulated cell's state before and after it's laid; an override that doesn't change its cell fails the run): {} overrides: {}; no-ops {} ({})\n",
            overrides.len(),
            if overrides.is_empty() { "none".to_string() } else { overrides.iter().map(|(c, b, a)| format!("({},{},{}) {} -> {}", c[0], c[1], c[2], b.trim_start_matches("minecraft:"), a.trim_start_matches("minecraft:"))).collect::<Vec<_>>().join(", ") },
            noops.len(),
            if noops.is_empty() { "PASS".to_string() } else { format!("FAIL: {}", noops.iter().map(|(c, b, _)| format!("({},{},{}) already {}", c[0], c[1], c[2], b.trim_start_matches("minecraft:"))).collect::<Vec<_>>().join(", ")) }
        ));
        s.push_str(&format!(
            "the stand-ins' 7th property, SIGHT (ironspec 20:10 / 20:19 PT (2c): each emulated support laid where the build has air against every villager's eye to every living entity's eye within 16, each tick; bar 0 crossings): {}{}{}\n",
            if sight_cells.is_empty() { "no support laid on air".to_string() } else { sight_cells.iter().zip(sight_hits.iter()).map(|(c, n)| format!("({},{},{}) {n} ({})", c[0], c[1], c[2], if *n == 0 { "PASS" } else { "FAIL" })).collect::<Vec<_>>().join(", ") },
            if sight_skipped.is_empty() { String::new() } else { format!("; not watched, no collision box (a sight ray clips only against colliders): {}", sight_skipped.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ")) },
            if sight_rows.is_empty() { String::new() } else { format!(": {}; the crossings by class: {}", sight_rows.join("; "), sight_classes.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")) }
        ));
        let t_missing: i64 = tsplit.missing.iter().sum();
        s.push_str(&format!(
            "the split's Ts (ironspec 19:58 PT (2b): each tick a T's stored shape against its power's settle; the source re-shaping a T must leave it in its intended shape): {}; ticks a T wasn't a junction {t_missing} (bar 0: {}); the shape changes to a shape other than its power's settle {} (bar 0: {}); stale with a cart within 1.5 {} (bar 0: {}){}{}\n",
            [-3, -1, 1, 3, 5, 7].iter().enumerate().map(|(k, x)| format!("({x},25,9) {} changes, {} stale ticks", tsplit.changes[k], tsplit.stale[k])).collect::<Vec<_>>().join(", "),
            if t_missing == 0 { "PASS" } else { "FAIL" },
            tsplit.wrong_changes.iter().sum::<i64>(),
            if tsplit.wrong_changes.iter().sum::<i64>() == 0 { "PASS" } else { "FAIL" },
            tsplit.stale_cart.iter().sum::<i64>(),
            if tsplit.stale_cart.iter().sum::<i64>() == 0 { "PASS" } else { "FAIL" },
            if tsplit.rows.is_empty() { String::new() } else { format!(": {}", tsplit.rows.join("; ")) },
            if tsplit.change_rows.is_empty() { String::new() } else { format!("; the changes: {}", tsplit.change_rows.join("; ")) }
        ));
        // (ironspec 23:51 PT (2)) the update-only declarations checked against the run
        let unsw: i64 = tsplit.unswitched.iter().sum();
        s.push_str(&format!(
            "the declared push pairs (ironspec 23:51 PT: a resting cart never displaced beyond its declaration's bound): {} pairs, {pp_rests} rests watched, {pp_fails} displaced beyond the bound{} (bar 0: {})\n",
            o.push_pairs.len(),
            if pp_fail.is_empty() { String::new() } else { format!(": {}", pp_fail.join("; ")) },
            if pp_fails == 0 { "PASS" } else { "FAIL" }
        ));
        s.push_str(&format!(
            "the split's Ts against their declared update-only sources (ironspec 23:51 PT: a T changes shape only when its own lever flips it, within 2 gt): {unsw} changes without their lever{} (bar 0: {})\n",
            if tsplit.unswitched_rows.is_empty() { String::new() } else { format!(": {}", tsplit.unswitched_rows.join("; ")) },
            if unsw == 0 { "PASS" } else { "FAIL" }
        ));
        // (ironspec 23:16 PT) the declared inert / always-on cells, checked against the run
        s.push_str(&format!("every fluid cell's tick scheduled at the start (ironspec 00:27 PT: a dry cell a fluid's own tick fills shows in the run): {} cells (water {fluid_water}, lava {fluid_lava})\n", fluid_water + fluid_lava));
        s.push_str(&format!(
            "the declared inert / always-on cells (ironspec 23:16 PT: watched through the run, any state change fails; G1: each starting block against its declared kind): {} inert{}, {} always-on; starting blocks: {}; kind mismatches {}{}; changes {watched_n}{} (bar 0: {})\n",
            inert_cells.len(),
            if o.inert_control { " (with the control (-1,26,9) toggled)" } else { "" },
            o.always_on.len(),
            if watched_start.is_empty() { "none".to_string() } else { watched_start.join(", ") },
            watched_kind_bad.len(),
            if watched_kind_bad.is_empty() { String::new() } else { format!(" ({})", watched_kind_bad.join("; ")) },
            if watched_changes.is_empty() { String::new() } else { format!(": {}", watched_changes.join("; ")) },
            if watched_n == 0 && watched_kind_bad.is_empty() { "PASS" } else { "FAIL" }
        ));
        // (ironspec 19:20 / 19:34, G1: the summon paths) every golem's birth on the panic schedule
        // (game time = 0 mod 100), path 3 (the panic trigger) or 5 (gossip, any tick, 5 willing);
        // every gossip attempt (path 5: its willing count >= 5 by construction) by the caller's
        // block position: at a pad-reachable one (y <= 0: the pad's feet at y -6 within the
        // summon's y +-6) it is a design defect even when the roll fails
        {
            let born: Vec<&(i64, u32, u32, Option<u32>, u8, [i32; 3], u8)> = golem_tries.iter().filter(|x| x.3.is_some()).collect();
            let on_schedule = born.iter().filter(|x| x.0.rem_euclid(100) == 0).count();
            let p5_born = born.iter().filter(|x| x.4 == 5).count();
            let gossip: Vec<&(i64, u32, u32, Option<u32>, u8, [i32; 3], u8)> = golem_tries.iter().filter(|x| x.4 == 5).collect();
            let reachable: Vec<&&(i64, u32, u32, Option<u32>, u8, [i32; 3], u8)> = gossip.iter().filter(|x| x.5[1] <= 0).collect();
            let name = |v: u32| recs.get(&v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string());
            let rows: Vec<String> = gossip.iter().take(12).map(|x| format!("t {} {} at ({},{},{}) with {} willing, activity {}, {}", x.0 - t0, name(x.1), x.5[0], x.5[1], x.5[2], x.2, gossip_activity.get(&(x.0, x.1)).map_or("-", |s| s.as_str()), if x.3.is_some() { "a golem" } else { "no spot" })).collect();
            s.push_str(&format!(
                "the summon paths (ironspec 19:20 / 19:34): {} golems born, {on_schedule} on the panic schedule (bar: all: {}), path 5 (gossip) {p5_born} (bar 0: {}); gossip attempts {} ({} with a golem), at a pad-reachable caller position (block y <= 0) {} (bar 0: {}){}\n",
                born.len(),
                if born.is_empty() { "NOT JUDGED (0 golems born): FAIL" } else if on_schedule == born.len() { "PASS" } else { "FAIL" },
                if p5_born == 0 { "PASS" } else { "FAIL" },
                gossip.len(),
                gossip.iter().filter(|x| x.3.is_some()).count(),
                reachable.len(),
                if reachable.is_empty() { "PASS" } else { "FAIL" },
                if rows.is_empty() { String::new() } else { format!(": {}", rows.join("; ")) }
            ));
        }
    }
    // (ironspec 23:56 PT (4)) the positive controls this run exercised (each must make its bar fail)
    // (G1's P21: acts read back and printed) every control's actions, whichever controls ran (the
    // gossip / bystander controls print the same line with their target's margin below)
    if !(o.gossip_control || o.bystander_control.is_some()) {
        s.push_str(&format!(
            "the positive controls' actions (each judged on its own bar in the controls line at the end): {}\n",
            if controls_done.is_empty() { "none acted".to_string() } else { controls_done.join("; ") }
        ));
    }
    if o.gossip_control || o.bystander_control.is_some() {
        // (ironspec 02:48 PT) the bystander control's stamped napper: its margin at its next
        // own-pocket summon, from the harm bar's rows
        let target_margin = bystander_target.map(|(tv, at)| {
            let label = recs.get(&tv).map_or(format!("#{tv}"), |r| r.label.split(' ').next().unwrap_or("").to_string());
            let call = nap_margins.iter().find(|(v, t, _, _)| *v == tv && *t >= at);
            format!(
                "; the stamped napper {label}'s first call back after the stamp: {}",
                call.map_or("none inside the run (the control isn't judged)".to_string(), |(_, t, m, by)| format!("t {t}, margin {} ({})", m.map_or("free".to_string(), |m| format!("{m:+} gt, set by {by}")), if m.is_none_or(|m| m >= 3) { ">= 3" } else { "< 3" }))
            )
        });
        s.push_str(&format!(
            "the positive controls' actions (ironspec 23:56 PT; each judged on its own bar in the controls line at the end): {}{}\n",
            if controls_done.is_empty() { "none acted".to_string() } else { controls_done.join("; ") },
            target_margin.unwrap_or_default()
        ));
    }
    // (ironspec 16:10) the chain: every cycle's REL and how it re-locks to the summon ticks (game
    // time mod 100) once golems come back; the watchdog's starts
    {
        let phase = |r: i64| (r + t0).rem_euclid(100);
        let fg = golem_born.values().min().map(|b| b - t0);
        let before: Vec<String> = ctl.rels.iter().filter(|r| fg.is_none_or(|g| **r < g)).take(8).map(|r| format!("t {r} (≡{})", phase(*r))).collect();
        let after: Vec<String> = ctl.rels.iter().filter(|r| fg.is_some_and(|g| **r >= g)).take(8).map(|r| format!("t {r} (≡{})", phase(*r))).collect();
        // (G1 17:50: the least REL interval the chain produces, cc767b78's plan rests on it)
        let least_gap = ctl.rels.windows(2).map(|p| (p[1] - p[0], p[1])).min_by_key(|g| g.0);
        // (ironspec 18:40) the cycles off the golem-locked phase (REL at game time = 5 mod 100), by
        // day
        let mut off_by_day: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
        for r in &ctl.rels {
            let e = off_by_day.entry((day_start + r).div_euclid(24_000)).or_insert((0, 0));
            e.0 += 1;
            if phase(*r) != 5 {
                e.1 += 1;
            }
        }
        s.push_str(&format!(
            "the cycles off the golem-locked phase (REL = 5 mod 100; ironspec 18:40), by day: {}\n",
            if off_by_day.is_empty() { "no cycle".to_string() } else { off_by_day.iter().map(|(d, (n, off))| format!("day {d}: {off} of {n}")).collect::<Vec<_>>().join("; ") }
        ));
        // (ironspec 19:29 (c), liveness: the longest gap between two RELs, and the starts that found
        // a cycle still in flight)
        let longest_gap = ctl.rels.windows(2).map(|p| (p[1] - p[0], p[1])).max_by_key(|g| g.0);
        s.push_str(&format!(
            "the chain (day-1's accepted chain, ironspec 19:47: B 100 gt from each start, a golem pulse during B re-arms it to the pulse + 100 with P, the flywheel W {} gt): {} cycles, {} started by the flywheel, {} re-arms; the least REL interval {} (bar >= 100: {}); the longest {} (liveness: <= W + 100: {}); restarts of a cycle still in flight {}; the first call with 3 willing at the stand {}; the first golem {}; RELs before it {}; after it {}\n",
            ctl.watchdog,
            ctl.rels.len(),
            ctl.watchdog_starts,
            ctl.rearms,
            least_gap.map_or("-".to_string(), |(g, t)| format!("{g} gt (ending t {t})")),
            if least_gap.is_none_or(|(g, _)| g >= 100) { "PASS" } else { "FAIL" },
            longest_gap.map_or("-".to_string(), |(g, t)| format!("{g} gt (ending t {t})")),
            if longest_gap.is_none_or(|(g, _)| g <= ctl.watchdog.max(100) + 100) { "PASS" } else { "FAIL" },
            ctl.restarted_busy,
            summons.first_fresh3.map_or("never".to_string(), |t| format!("t {}", t - t0)),
            fg.map_or("never".to_string(), |g| format!("t {g}{}", summons.first_fresh3.map_or(String::new(), |t| format!(" ({} gt after it)", g - (t - t0))))),
            if before.is_empty() { "none".to_string() } else { before.join(", ") },
            if after.is_empty() { "none".to_string() } else { after.join(", ") }
        ));
        // (ironspec 23:13 PT) every emulated write answers as the built cell under record-match's
        // guard_intrinsics (the old suppress-only leniency is gone: a refusal stops the run)
        let sup = SUPPRESSED.with(|x| x.borrow().clone());
        s.push_str(&format!(
            "the drive's stand-ins under record-match's guard_intrinsics (each as the built cell, or a refusal stops the run){}; the cells the module declares emulated: {}\n",
            if sup.is_empty() { String::new() } else { format!(" (listed apart: {})", sup.iter().map(|(c, w)| format!("({},{},{}) {w}", c[0], c[1], c[2])).collect::<Vec<_>>().join("; ")) },
            if o.emulated.is_empty() { "none".to_string() } else { o.emulated.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" ") }
        ));
    }
    // (ironspec 11:29: out of the bed's 16-ball within 152 gt of reaching R) every trip
    if !ball_exits.is_empty() {
        let worst = ball_exits.iter().map(|(_, g)| *g).max().unwrap_or(0);
        // (ironspec 19:07 PT: the 152 bound is retired, printed as information; (a1) is the bar,
        // measured directly with its margin: each lie-down against the previous napper's 16-ball
        // exit)
        s.push_str(&format!(
            "the departing nappers' 16-ball exit after R (information: the 152 bound retired 19:07 PT, (a1) is the bar): {} trips, the worst {worst} gt\n",
            ball_exits.len()
        ));
        let mut lie: Vec<(i64, u32)> = nap_times.iter().flat_map(|(v, ts)| ts.iter().map(move |t| (*t, *v))).collect();
        lie.sort();
        let mut least: Option<(i64, String)> = None;
        for p in lie.windows(2) {
            let ((l0, v0), (l1, v1)) = (p[0], p[1]);
            let e = exit_log.iter().find(|(t, v)| *v == v0 && *t >= l0).map(|(t, _)| *t);
            let m = match e {
                Some(e) => l1 - e,
                None => i64::MIN / 4,
            };
            let name = |v: u32| recs.get(&v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string());
            if least.as_ref().is_none_or(|(lm, _)| m < *lm) {
                least = Some((m, format!("{} lay down t {l1}, {} (asleep t {l0}) left the 16-ball {}", name(v1), name(v0), e.map_or("never before it".to_string(), |e| format!("t {e}")))));
            }
        }
        s.push_str(&format!(
            "  (a1) its margin (ironspec 19:07 PT): each lie-down against the previous napper's 16-ball exit: {} pairs, the least {} (bar > 0: {})\n",
            lie.len().saturating_sub(1),
            least.as_ref().map_or("- (fewer than two naps)".to_string(), |(m, why)| if *m < -1_000_000 { format!("never ({why})") } else { format!("{m:+} gt ({why})") }),
            match least.as_ref() {
                None => "NOT JUDGED (0 samples): FAIL",
                Some((m, _)) if *m > 0 => "PASS",
                Some(_) => "FAIL",
            }
        ));
    } else if o.napper {
        // (ironspec 23:56 PT (3), G1's [1]: no vacuous pass) a napper run with no trip out of the ball
        s.push_str("  (a1) its margin (ironspec 19:07 PT): 0 trips out of the bed's 16-ball: NOT JUDGED (0 samples): FAIL\n");
    }
    // (a trip that reached R and never left the ball: its own row, information only: the 152-gt bound
    // retired 19:07 PT, ironspec's E11)
    for (vid, _, _) in nappers.iter().filter(|(v, _, _)| recs.get(v).is_some_and(|r| r.ball_exit_after_r.is_none())) {
        if let Some(r) = recs.get(vid)
            && let Some(at_r) = r.milestones.iter().find(|(w, _)| w == "at R").map(|(_, t)| *t)
        {
            s.push_str(&format!(
                "the departing napper {} (information: the 152-gt bound retired 19:07 PT; (a1) is the bar): at R t {at_r}, out of the ball {}\n",
                r.label.split(' ').next().unwrap_or(""),
                r.ball_exit_after_r.map_or("not by the run's end".to_string(), |g| format!("{g} gt later"))
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
        "the staleness bar (ironspec 11:02, G1: NEAREST_HOSTILE up to ~39 gt stale; every entry into a position within 6.05 of a zombie hidden from it at least 40 gt after the rider's last position within 8 of a zombie in its sight; only the stop's own waker left out, every other zombie counted everywhere): {} entries, the least gap {}, {} under 40 gt ({}){}\n",
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
        "villagers within 16 of the bed head ({},{},{}): {} villager-ticks over {} cells; the bed occupied {} ticks; (a1) calm (not panicking) REST ticks within 16 while occupied: {} (the bar: 0: {}){}\n",
        o.bed[0],
        o.bed[1],
        o.bed[2],
        near.ticks,
        near.cells.len(),
        near.occupied_ticks,
        near.calm_rest_occupied,
        if near.calm_rest_occupied == 0 { "PASS" } else { "FAIL" },
        if near.calm_rest_rows.is_empty() { String::new() } else { format!(": {}", near.calm_rest_rows.join("; ")) }
    ));
    if !near.cells.is_empty() {
        let mut cs: Vec<(&[i32; 3], &i64)> = near.cells.iter().collect();
        cs.sort_by_key(|x| -x.1);
        s.push_str(&format!("  the cells (villager-ticks): {}\n", cs.iter().take(40).map(|(c, n)| format!("{c:?} {n}")).collect::<Vec<_>>().join(", ")));
    }
    // (tick-exact 06:09 PT, the reconciliation row) the c7 placement prints the rider-state section
    // too, its napper rows empty
    if o.napper || o.c7_placement.is_some() {
        // (one row a napper villager, its last trip's milestones and every trip's stamps; the trips
        // counted)
        let mut seen_v: Vec<u32> = Vec::new();
        let trips = |v: u32| nappers.iter().filter(|n| n.0 == v).count();
        s.push_str(&format!("phase 3a, the nappers (EMULATED drives: day-1's events 10:00, 58f770cc's stop, b8949c96's way-in gate, day-1's picker): {} trips by {} villagers\n", nappers.len(), nappers.iter().map(|n| n.0).collect::<std::collections::BTreeSet<_>>().len()));
        for (vid, k, at) in nappers.iter().rev() {
            if seen_v.contains(vid) {
                continue;
            }
            seen_v.push(*vid);
            if trips(*vid) > 1 {
                s.push_str(&format!("  ({} trips; the last below)\n", trips(*vid)));
            }
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
                    "  {} (seeded t {at}, lane x {}): {}; slept {}; HOME at the end {}; its route from K: {}: {verdict}; its stamp: {}; its first call back in its lane: {}\n",
                    r.label.split(' ').next().unwrap_or(""),
                    LANES[*k],
                    if r.milestones.is_empty() { "no milestone".to_string() } else { r.milestones.iter().map(|(w, t)| format!("{w} t {t}")).collect::<Vec<_>>().join(", ") },
                    r.naps,
                    if r.home_lost.is_empty() { "kept" } else { "LOST" },
                    r.route.iter().take(40).map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "),
                    if r.nap_stamps.is_empty() { "none seen".to_string() } else { r.nap_stamps.join(", ") },
                    r.nap_call.clone().unwrap_or_else(|| "none in the run".to_string())
                ));
            }
        }
        // (ironspec 14:47) the worst stamp age at a queue arrival (b8949c96's all-day row starts
        // from it)
        let worst = nappers.iter().filter_map(|(v, _, _)| recs.get(v).and_then(|r| r.queue_age)).max();
        // (ironspec 15:43) the villagers that slept in each night (day time 12,000..24,000), the
        // full nights inside the run held to 24
        let day_end = run.world.day_time();
        // (ironspec's E7 late-sleeper rule) a full night passes with 24 distinct nappers, or with 23
        // when the next night has 24
        let counts: BTreeMap<i64, usize> = nights.iter().map(|(d, vs)| (*d, vs.len())).collect();
        let night_rows: Vec<String> = nights
            .iter()
            .map(|(d, vs)| {
                let full = d * 24_000 + 12_000 >= day_start && (d + 1) * 24_000 <= day_end;
                let late_ok = vs.len() == 23 && counts.get(&(d + 1)).is_some_and(|n| *n >= 24);
                format!(
                    "night {d}{}: {} villagers napped (by lie-down){}",
                    if full { "" } else { " (partial)" },
                    vs.len(),
                    if full { if vs.len() >= 24 { " (PASS)".to_string() } else if late_ok { " (PASS by the late-sleeper rule: 23, the next night 24)".to_string() } else { " (FAIL, bar 24)".to_string() } } else { String::new() }
                )
            })
            .collect();
        s.push_str(&format!("  naps per night (ironspec 15:43, bar 24 a full night; E7: each nap keyed by its lie-down to its REST night, the late-sleeper rule): {}\n", if night_rows.is_empty() { "no nap".to_string() } else { night_rows.join("; ") }));
        // (ironspec 06:31 / 06:38 PT) ValidateNearbyPoi's 16-block exposure per night, bar 0; the
        // bed ticket's state the run started in
            // (ironspec 06:31 / 06:38 PT; G1 N13) ValidateNearbyPoi's 16-block exposure, every night
            // printed (0 included), with the rail cells whose bottom centre lies inside the sphere
            let exp_nights: Vec<i64> = {
                let (a, b) = (o.day0.div_euclid(24_000), (o.day0 + o.ticks).div_euclid(24_000));
                (a..=b).collect()
            };
            let inside: Vec<String> = spec
                .blocks
                .iter()
                .filter(|(p, b)| b.to_string().contains("rail") && poi_near(o.bed, redsim::entity::Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5)))
                .map(|(p, _)| format!("({},{},{})", p.x, p.y, p.z))
                .collect();
            s.push_str(&format!(
                "  the riders in REST within 16 of the bed while someone sleeps in it (ironspec 06:31 / 06:38 PT, G1 N13: ValidateNearbyPoi's exposure, the bed head's centre to the feet, squared < 256, while the bed is OCCUPIED; bar 0 each night; the run's start: {}): {}{}; the rail cells inside the sphere: {}; REST reads with no exact brain (ironspec D5(a), bar 0): {} ({})\n",
                if o.bed_ticket_taken { "state (ii), the bed's ticket taken by the bench (--bed-ticket-taken)" } else { "the design start (i), every HOME a memory and the bed's ticket free" },
                exp_nights.iter().map(|n| format!("night {n}: {} villager-ticks", rest_near_bed.get(n).copied().unwrap_or(0))).collect::<Vec<_>>().join(", "),
                if rest_near_rows.is_empty() { String::new() } else { format!("; the first: {}", rest_near_rows.join("; ")) },
                if inside.is_empty() { "none".to_string() } else { inside.join(" ") },
                brainless_reads,
                if !bed_ever_occupied { "NOT JUDGED (the bed never occupied): FAIL" } else if rest_near_bed.is_empty() && brainless_reads == 0 { "PASS" } else { "FAIL" }
            ));
        // (ironspec 06:42 real) the start-state line and the ticket lines
        s.push_str(&format!("  {start_state_line}\n"));
        s.push_str(&format!(
            "  the bed's tickets at every change (free count, HOME memory holders; ironspec 06:31 / 06:42 real; ticket state uncertain: pre-fix redsim): {}\n",
            if ticket_rows.is_empty() { "no change".to_string() } else { ticket_rows.join("; ") }
        ));
        // (ironspec 15:06b / 15:58 (2)) the night's end ticket state: free = the spare intact; taken = a
        // loss was absorbed (the spare spent); every take counted
        {
            let takes = ticket_rows.iter().filter(|r| r.contains("free 1 -> 0")).count();
            let frees = ticket_rows.iter().filter(|r| r.contains("free 0 -> 1")).count();
            let end_taken = ticket_rows.iter().rev().find_map(|r| if r.contains("-> 0,") || r.contains("free 0 -> 0") || r.contains("free 0,") { Some(true) } else if r.contains("-> 1,") || r.contains("free 1,") { Some(false) } else { None }).unwrap_or(false);
            s.push_str(&format!(
                "  the bed's ticket at the night's end (ironspec 15:06b): {}; spare spends (takes of the free ticket) {takes}, releases {frees}\n",
                if end_taken { "TAKEN (state (ii): the spare spent)" } else { "free (state (i): the spare intact)" }
            ));
            // (G1 16:40 (i), ironspec 16:40) the ticks in state (ii) (the ticket taken: a second HOME loss
            // locks napping), what ended each stretch, and the 16-ball exposure during (ii) (bar 0)
            let tick_of = |r: &str| -> Option<i64> { r.trim_start_matches("dusk ").strip_prefix("t ").and_then(|x| x.split(':').next()).and_then(|t| t.parse::<i64>().ok()) };
            let mut stretches: Vec<(i64, Option<i64>, String)> = Vec::new();
            let mut open: Option<i64> = None;
            for r in ticket_rows.iter() {
                let Some(t) = tick_of(r) else { continue };
                if r.starts_with("dusk") && r.contains("free 0") {
                    open = Some(t);
                } else if r.contains("free 1 -> 0") && open.is_none() {
                    open = Some(t);
                } else if r.contains("free 0 -> 1")
                    && let Some(a) = open.take()
                {
                    let who = r.split("lost ").nth(1).map_or("a release".to_string(), |w| format!("{}'s release", w.trim()));
                    stretches.push((a, Some(t), who));
                }
            }
            if let Some(a) = open {
                stretches.push((a, None, "still taken at the night's end".to_string()));
            }
            let in_ii = |t: i64| stretches.iter().any(|(a, b, _)| t >= *a && b.is_none_or(|b| t < b));
            let ex_ii = ctl.exposure_16.iter().filter(|(t, _, _)| in_ii(*t - 0)).count();
            let ticks_ii: i64 = stretches.iter().map(|(a, b, _)| b.unwrap_or(o.ticks as i64) - a).sum();
            s.push_str(&format!(
                "  state (ii) (G1 16:40 (i): the ticket taken; an exposure erase then LOCKS napping): {ticks_ii} ticks in {} stretches{}; the 16-ball exposure during (ii): {ex_ii} villager-ticks ({})\n",
                stretches.len(),
                if stretches.is_empty() { String::new() } else { format!(" ({})", stretches.iter().map(|(a, b, w)| format!("t {a}..{}: ended by {w}", b.map_or("the end".to_string(), |b| b.to_string()))).collect::<Vec<_>>().join("; ")) },
                if ex_ii == 0 { "PASS" } else { "FAIL" }
            ));
        }
        // (ironspec 06:24 / 06:31 / 07:02 real, G1: the decisive bar) every REST entry; the lane
        // riders' REST outside their own trip; those at a design position with a partial HOME path
        {
            let mut open: Vec<String> = Vec::new();
            let mut open_partial = 0i64;
            for r in recs.values() {
                if let Some((st, row, counted, at_design)) = &r.rest_spell {
                    if *counted && *at_design && r.rest_path == Some(false) {
                        open_partial += 1;
                    }
                    open.push(format!("{row}; its HOME path {}; still in REST at the run's end ({} gt)", r.rest_path.map_or("never computed".to_string(), |p| if p { "complete".to_string() } else { "PARTIAL".to_string() }), o.ticks - st));
                }
            }
            let partial = lane_rest_partial + open_partial;
            let trips: Vec<String> = recs
                .values()
                .filter(|r| !r.trips.is_empty())
                .map(|r| format!("{} {}", r.label.split(' ').next().unwrap_or(""), r.trips.iter().map(|(a, b)| format!("t {a}..{}", b.map_or("end".to_string(), |b| b.to_string()))).collect::<Vec<_>>().join(", ")))
                .collect();
            s.push_str(&format!(
                "  the lane riders in REST at night outside their own trip (ironspec 06:24 / 06:31 real; since ironspec 06:33 platform / 07:27 platform the COVERAGE count, information: the at-entry stamp bar is the gate; a napper's trip is its pick to R): {} entries, {} REST ticks ({}); of them at a design position (riding) with a partial HOME path: {} (bar 0: {}); the night window t {}; the entries by cell (9fc3155f's coverage fix, the most first): {}; each napper's excluded trips: {}; every REST entry ({} in all, sleepers and trips included): {}\n",
                lane_rest_entries,
                lane_rest_ticks,
                if night_window.is_none() { "NOT JUDGED (no night in the run): information" } else { "information" },
                partial,
                if night_window.is_none() { "NOT JUDGED: FAIL" } else if partial == 0 { "PASS" } else { "FAIL" },
                night_window.map_or("- (none)".to_string(), |(a, b)| format!("{a}..{b} (day time 12,000..24,000)")),
                {
                    let mut v: Vec<(&[i32; 3], &i64)> = rest_by_cell.iter().collect();
                    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                    if v.is_empty() { "none".to_string() } else { v.iter().take(40).map(|(c, n)| format!("({},{},{}) {n}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ") }
                },
                if trips.is_empty() { "none".to_string() } else { trips.join("; ") },
                rest_entries_total,
                if rest_rows.is_empty() && open.is_empty() { "none".to_string() } else { rest_rows.iter().chain(open.iter()).cloned().collect::<Vec<_>>().join("; ") }
            ));
            // (ironspec 06:36 / 07:06 real, G1) the runtime stamp bar, worded as its scope
            let rider_days: i64 = recs.values().map(|r| r.oldest_by_day.len() as i64).sum();
            let judged_days: std::collections::BTreeSet<i64> = recs.values().flat_map(|r| r.oldest_by_day.keys().copied()).collect();
            let per_rider: Vec<String> = recs
                .values()
                .map(|r| format!("{} {}", r.label.split(' ').next().unwrap_or(""), if r.oldest_by_day.is_empty() { "never armed".to_string() } else { r.oldest_by_day.iter().map(|(d, (a, t, _))| format!("day {d}: {a} gt (t {t})")).collect::<Vec<_>>().join(", ") }))
                .collect();
            s.push_str(&format!(
                "  the runtime stamp bar (ironspec 06:36 / 07:06 real: every rider's CANT_REACH stamp < 1,000 gt at all times, at every REST entry too): {} ({} riders, {} rider-days with a stamp, days {}, seed {}); the oldest {}; ticks at >= 1,000 gt: {}{}; each rider's oldest stamp per day: {}\n",
                if recs.is_empty() { "NOT JUDGED (no rider): FAIL".to_string() } else if stamp_over_n == 0 { format!("no stamp >= 1,000 gt in {} riders over {} gt, seed {}: PASS", recs.len(), o.ticks, o.seed) } else { "FAIL".to_string() },
                recs.len(),
                rider_days,
                judged_days.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", "),
                o.seed,
                stamp_max.as_ref().map_or("- (no stamp armed)".to_string(), |(a, t, w, c)| format!("{a} gt (t {t}, {w} at {c:?})")),
                stamp_over_n,
                if stamp_over.is_empty() { String::new() } else { format!(" ({})", stamp_over.join("; ")) },
                per_rider.join("; ")
            ));
            s.push_str(&format!(
                "  the CANT_REACH armings (G1: each with its target and position; at night with the scarer sight): by day {}{}; at night {}{}\n",
                armings_day_n,
                if armings_day.is_empty() { String::new() } else { format!(" ({})", armings_day.join("; ")) },
                armings_night_n,
                if armings_night.is_empty() { String::new() } else { format!(" ({})", armings_night.join("; ")) }
            ));
            // (G1 10:47 O3) the at-entry control's own record on the stamp line: its rider, the stamp
            // before and after the aging, and the next judged set's age
            let at_entry_control_note = match (o.at_entry_control, at_entry_rider, at_entry_before) {
                (true, Some(v), Some((t, before))) => format!(
                    "; the at-entry control: #{v}'s stamp aged at t {t} from {} to 1,100 gt (under the release branch's 1,200); its next judged HOME-target set {}; its HOME release after the aging (G1's P14, a sample): {}",
                    before.map_or("unarmed".to_string(), |a| format!("{a} gt")),
                    at_entry_next.map_or("never came: the control can't bite".to_string(), |(nt, a)| format!("at t {nt}, age {a} gt")),
                    at_entry_released.as_ref().map_or("none".to_string(), |(rt, m)| format!("at t {rt} by {m} (FAIL: the control claimed none)"))
                ),
                (true, _, _) => "; the at-entry control: no rider with a judged HOME-target set in REST after t 1,000 to age".to_string(),
                _ => String::new(),
            };
            // (ironspec 06:33 / 06:37 platform, G1) the at-entry stamp bar: the stamp at each REST
            // spell's first HOME-targeting tick
            s.push_str(&format!(
                "  the at-entry stamp bar (ironspec 06:33 / 06:37 platform; G1 X19: at EVERY tick in REST that SetWalkTargetFromBlockMemory set WALK_TARGET to HOME (not HOME the tick before), not only a spell's first, the stamp < 1,000 gt, any position; ironspec 08:13: the age it saw, from the PRE-tick stamp; the site as the report's site line states): {} HOME-target sets judged, the worst {} gt, {} at >= 1,000 ({}); its PASS stands only with the next line's at 0 (G1 08:22 / ironspec 08:23; since G1 10:47 O2 the HOME losses print on their own line, so the at-entry control rewrites only this one){}\n  the HOME losses by mechanism (G1 X20 / 08:22, ironspec 08:23: the release branch and in all, both 0 for a PASS): the release branch {}, in all {} ({})\n",
                at_home_target_n,
                at_home_target_max,
                at_home_target_over,
                if at_home_target_n == 0 { "NOT JUDGED (no spell targeted HOME; redsim's REST-path gap is open: cc767b78): FAIL" } else if at_home_target_over == 0 { "PASS" } else { "FAIL" },
                at_entry_control_note,
                release_branches,
                home_lost_total,
                if release_branches == 0 && home_lost_total == 0 { "PASS" } else { "FAIL" }
            ));
            // (ironspec D2) RS_ready's job: no R1 release before the previous napper passes R, read off
            // R's detector itself (a rise after the previous release, at least the latch's 8 gt
            // before this one), not the model's own RS_ready
            {
                let mut early: Vec<String> = Vec::new();
                for k in 1..ctl.release_log.len() {
                    let (tp, vp) = ctl.release_log[k - 1];
                    let (t, v) = ctl.release_log[k];
                    // (G1 X21) the previous napper's OWN pass over R's detector, at least the latch's
                    // 8 gt before this release (a rise by another rider doesn't count)
                    if !ctl.rdet_riders.iter().any(|(r, who)| *r > tp && *r + 8 <= t && *who == Some(vp)) && early.len() < 8 {
                        early.push(format!("t {t}: #{v} released while #{vp} (released t {tp}) hadn't passed R's detector 8 gt before"));
                    }
                }
                s.push_str(&format!(
                    "  no R1 release before the previous napper passes R (ironspec D2, RS_ready's job; bar 0): {} releases, {} pairs judged, {} early{} ({})\n",
                    ctl.release_log.len(),
                    ctl.release_log.len().saturating_sub(1),
                    early.len(),
                    if early.is_empty() { String::new() } else { format!(": {}", early.join("; ")) },
                    if ctl.release_log.len() < 2 { "NOT JUDGED (fewer than 2 releases): FAIL" } else if early.is_empty() { "PASS" } else { "FAIL" }
                ));
            }
            // (ironspec 06:22 platform + G1) the air-supply bar: each rider's least air per lift
            // transit; no rider's air below 0 (drowning)
            {
                let mut all: Vec<(String, (i64, i32, i64, [i32; 3]))> = Vec::new();
                for r in recs.values() {
                    for t in r.air_transits.iter().chain(r.air_open.iter()) {
                        all.push((r.label.split(' ').next().unwrap_or("").to_string(), *t));
                    }
                }
                let least = all.iter().min_by_key(|(_, t)| t.1);
                let drowning: Vec<String> = all.iter().filter(|(_, t)| t.1 < 0).take(8).map(|(n, t)| format!("{n} from t {}: air {} at t {} {:?}", t.0, t.1, t.2, t.3)).collect();
                let mut per_rider: BTreeMap<String, (i64, i32)> = BTreeMap::new();
                for (n, t) in &all {
                    let e = per_rider.entry(n.clone()).or_insert((0, 300));
                    e.0 += 1;
                    e.1 = e.1.min(t.1);
                }
                s.push_str(&format!(
                    "  the air-supply bar (ironspec 06:22 platform + G1: no rider's air below 0 in any lift transit): {} transits, the least air {}{} ({}); each rider's transits and least air: {}\n",
                    all.len(),
                    least.map_or("- (no transit)".to_string(), |(n, t)| format!("{} ({n} at t {} {:?})", t.1, t.2, t.3)),
                    if drowning.is_empty() { String::new() } else { format!("; drowning: {}", drowning.join("; ")) },
                    if all.is_empty() { "NOT JUDGED (no rider entered water): FAIL" } else if drowning.is_empty() { "PASS" } else { "FAIL" },
                    if per_rider.is_empty() { "none".to_string() } else { per_rider.iter().map(|(n, (k, a))| format!("{n} {k} / {a}")).collect::<Vec<_>>().join(", ") }
                ));
            }
            // (ironspec 06:31 platform) each rider's night H visits: the stamp at entry, what cleared
            // it since the previous visit
            {
                let wakes: Vec<String> = recs.values().filter(|r| !r.wake_rows.is_empty()).map(|r| format!("{}: {}", r.label.split(' ').next().unwrap_or(""), r.wake_rows.join(", "))).collect();
                s.push_str(&format!(
                    "  every wake (ironspec 06:40 platform, G1 1c: the waker's flee target, its path (brain), the stamp, and whether a stamp armed after it cleared before the napper's next REST entry): {}\n",
                    if wakes.is_empty() { "none".to_string() } else { wakes.join("; ") }
                ));
            }
            {
                let rows: Vec<String> = recs.values().filter(|r| !r.h_rows.is_empty()).map(|r| format!("{}: {}", r.label.split(' ').next().unwrap_or(""), r.h_rows.join(", "))).collect();
                let uncleared = recs.values().flat_map(|r| r.h_rows.iter()).filter(|l| l.contains("NOT CLEARED")).count();
                s.push_str(&format!(
                    "  each rider's night H visits (ironspec 06:31 platform: the stamp's age at entry and what cleared it since the previous visit): {} visits, {} NOT CLEARED; {}\n",
                    recs.values().map(|r| r.h_rows.len()).sum::<usize>(),
                    uncleared,
                    if rows.is_empty() { "none".to_string() } else { rows.join("; ") }
                ));
            }
            // (ironspec 07:10 real (2)) each rider's night ticks by activity
            s.push_str(&format!(
                "  each rider's night ticks (ironspec 07:10 real (2): PANIC / REST / other): {}\n",
                recs.values().map(|r| format!("{} {}/{}/{}", r.label.split(' ').next().unwrap_or(""), r.night_acts[0], r.night_acts[1], r.night_acts[2])).collect::<Vec<_>>().join(", ")
            ));
            // (G1, farmopt 06:12 PT) every rider's activity across the schedule switch
            let strips: Vec<String> = recs.values().filter(|r| !r.switch_strip.is_empty()).map(|r| format!("{} {}", r.label.split(' ').next().unwrap_or(""), r.switch_strip)).collect();
            // (G1 X40) the switch bar judges COVERED REST ticks only (G1's one-tick question); a DARK
            // one is the dark-spell class the at-entry bar judges, counted here as information
            let rest_at_switch = recs.values().filter(|r| r.switch_strip.contains('R') && r.switch_covered == Some(true)).count();
            let rest_at_switch_dark = recs.values().filter(|r| r.switch_strip.contains('R') && r.switch_covered != Some(true)).count();
            // (ironspec 06:55 platform (3)) each REST tick's place and scarer sight
            let switch_places: Vec<String> = recs.values().filter_map(|r| r.switch_rest_at.as_ref().map(|p| format!("{}: {p}", r.label.split(' ').next().unwrap_or("")))).collect();
            // (ironspec 06:55 platform (2)) the per-cell spell table
            let spell_rows: Vec<String> = {
                let mut v: Vec<(&[i32; 3], &(Vec<i64>, i64))> = spell_by_cell.iter().collect();
                v.sort_by(|a, b| b.1.0.len().cmp(&a.1.0.len()).then(a.0.cmp(b.0)));
                v.iter()
                    .take(40)
                    .map(|(c, (lens, held))| {
                        let mut l = lens.clone();
                        l.sort();
                        let med = l[l.len() / 2];
                        format!("({},{},{}) {} spells, median {} gt, max {} gt, {} over 100 gt, {} with WALK_TARGET held at entry", c[0], c[1], c[2], l.len(), med, l.last().copied().unwrap_or(0), l.iter().filter(|x| **x > 100).count(), held)
                    })
                    .collect()
            };
            s.push_str(&format!(
                "  the counted REST spells by entry cell (ironspec 06:55 platform (2): a spell can arm only once the stale flee sink stops (~101-250 gt) or when it starts with no flee target; the most spells first): {}\n",
                if spell_rows.is_empty() { "none".to_string() } else { spell_rows.join("; ") }
            ));
            s.push_str(&format!(
                "  each rider's activity at every tick of day time 11,990..=12,040 (G1: a covered rider enters REST for not even one tick at the switch; X40: only a COVERED REST tick fails, a DARK one is the at-entry bar's; P PANIC, R REST, r REST in its own trip, . other): {} riders seen, {} with a COVERED REST tick ({}), {} with a DARK one (information){}: {}\n",
                strips.len(),
                rest_at_switch,
                if strips.is_empty() { "NOT JUDGED (the run doesn't cross 12,000): FAIL" } else if rest_at_switch == 0 { "PASS" } else { "FAIL" },
                rest_at_switch_dark,
                if switch_places.is_empty() { String::new() } else { format!("; each one's first REST tick, place and scarer sight (ironspec 06:55 platform (3)): {}", switch_places.join("; ")) },
                if strips.is_empty() { "none".to_string() } else { strips.join("; ") }
            ));
            // (ironfarm / G1) each lane's queue occupancy per tick, V flagged
            s.push_str(&format!(
                "  each lane's queue occupancy per tick (H, R_1 and the five slopes; ironfarm / G1, farmopt 06:10 PT; ticks at each count): {}; V (the slope top (x,22,6)) occupied: {}{}\n",
                (0..3).map(|k| format!("lane x {}: {}", LANES[k], occ_hist[k].iter().map(|(n, t)| format!("{n} carts {t} ticks")).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; "),
                (0..3).map(|k| format!("lane x {} {} ticks", LANES[k], v_ticks[k])).collect::<Vec<_>>().join(", "),
                if v_rows.is_empty() { String::new() } else { format!(" (the first of each stretch: {})", v_rows.join(", ")) }
            ));
            // (ironspec 06:11 platform) V at night, a row per stretch
            {
                let mut rows = v_night_rows.clone();
                for (k, s) in v_night.iter().enumerate() {
                    if let Some(s) = s {
                        rows.push(format!("lane x {} from t {}: #{} {} (stamp {}, HOME {}; still at V at the run's end)", LANES[k], s.0, s.1, rle(&s.2), s.3.map_or("unarmed".to_string(), |a| format!("{a} gt")), if s.4 { "kept" } else { "LOST" }));
                    }
                }
                s.push_str(&format!(
                    "  V occupied at night (ironspec 06:11 platform: V is covered; each stretch's occupant, its activity per tick (P PANIC, R REST, . other), its stamp and HOME): {}\n",
                    if rows.is_empty() { "never".to_string() } else { rows.join("; ") }
                ));
            }
            // (ironspec 07:30 real (1), G1) every arming and its clearing, or NOT CLEARED
            let pairs: Vec<String> = recs
                .values()
                .filter(|r| !r.arm_pairs.is_empty() || r.arm_open.is_some())
                .map(|r| {
                    let mut v: Vec<String> = r.arm_pairs.iter().map(|(a, c, how)| format!("armed t {a}, cleared t {} ({} gt): {how}", c.map_or("-".to_string(), |c| c.to_string()), c.map_or(0, |c| c - a))).collect();
                    if let Some(a) = r.arm_open {
                        v.push(format!("armed t {a}: NOT CLEARED by the run's end ({} gt)", o.ticks - a));
                    }
                    format!("{}: {}", r.label.split(' ').next().unwrap_or(""), v.join(", "))
                })
                .collect();
            s.push_str(&format!(
                "  every CANT_REACH arming and its clearing (ironspec 07:30 real (1), G1: which place's complete path, the target reached or the lie-down, and the interval): {}\n",
                if pairs.is_empty() { "none".to_string() } else { pairs.join("; ") }
            ));
            s.push_str(&format!(
                "  the farm-stop bar (ironspec 06:31 real: no napper without HOME at the bed stop, bar 0): {} ticks{} ({})\n",
                stop_homeless_n,
                if stop_homeless.is_empty() { String::new() } else { format!(": {}", stop_homeless.join("; ")) },
                if nappers.is_empty() { "NOT JUDGED (no napper picked): FAIL" } else if stop_homeless_n == 0 { "PASS" } else { "FAIL" }
            ));
            // (ironspec 06:24 real, the 04:07 override rule) every redstone-block stand-in toggle
            let toggles = BLOCK_TOGGLES.with(|t| t.borrow().clone());
            if !o.block_standins.is_empty() {
                let rows: Vec<String> = o
                    .block_standins
                    .iter()
                    .map(|c| {
                        let mine: Vec<&([i32; 3], i64, String, String)> = toggles.iter().filter(|x| x.0 == *c).collect();
                        let noop = mine.iter().filter(|x| x.2 == x.3).count();
                        let mut kinds: BTreeMap<String, i64> = BTreeMap::new();
                        for x in &mine {
                            *kinds.entry(format!("{} -> {}", x.2.trim_start_matches("minecraft:"), x.3.trim_start_matches("minecraft:"))).or_default() += 1;
                        }
                        format!("({},{},{}) {} toggles ({}), no-ops {noop}{}", c[0], c[1], c[2], mine.len(), kinds.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "), if mine.is_empty() { " (NOT EXERCISED: FAIL)" } else if noop > 0 { " (FAIL)" } else { "" })
                    })
                    .collect();
                s.push_str(&format!("  the redstone-block stand-ins (day-1 05:45 PT, ironspec 06:24 real; the 04:07 override rule, each toggle's before -> after): {}\n", rows.join("; ")));
            }
        }
        // (ironspec 16:10 (1)) per night: the villagers picked twice and the riders never picked;
        // and per villager its naps and picks
        {
            let night_of = |rel: i64| (day_start + rel).div_euclid(24_000);
            let riders: Vec<u32> = recs.keys().copied().collect();
            let mut cover_rows: Vec<String> = Vec::new();
            for d in nights.keys().copied().chain(nappers.iter().map(|n| night_of(n.2))).collect::<std::collections::BTreeSet<i64>>() {
                let picked: Vec<u32> = nappers.iter().filter(|n| night_of(n.2) == d && n.2 > 0).map(|n| n.0).collect();
                let mut twice: Vec<String> = picked.iter().copied().collect::<std::collections::BTreeSet<u32>>().into_iter().filter(|v| picked.iter().filter(|p| *p == v).count() > 1).map(|v| recs.get(&v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string())).collect();
                twice.sort();
                let never = riders.iter().filter(|v| !picked.contains(v) && !nights.get(&d).is_some_and(|s| s.contains(v))).count();
                let distinct = picked.iter().collect::<std::collections::BTreeSet<_>>().len();
                // (G1's CHANGES 2 on 961090aef: a verdict, on complete nights only)
                let full = d * 24_000 + 12_000 >= day_start && (d + 1) * 24_000 <= day_end;
                let verdict = if !full { " (partial night)" } else if distinct >= riders.len() && twice.is_empty() && never == 0 { " (PASS)" } else { " (FAIL)" };
                cover_rows.push(format!("night {d}: {} picks of {distinct} villagers, picked twice {}, never picked or napped {never} of {}{verdict}", picked.len(), if twice.is_empty() { "none".to_string() } else { twice.join(" ") }, riders.len()));
            }
            s.push_str(&format!("  the picks per night (ironspec 16:10 (1): 24 distinct, no double pick): {}\n", if cover_rows.is_empty() { "none".to_string() } else { cover_rows.join("; ") }));
            let mut per_v: Vec<String> = Vec::new();
            for v in &riders {
                let naps = nap_times.get(v).cloned().unwrap_or_default();
                let picks: Vec<i64> = nappers.iter().filter(|n| n.0 == *v).map(|n| n.2).collect();
                if naps.is_empty() && picks.is_empty() {
                    continue;
                }
                per_v.push(format!("{} picked at t {} napped at t {}", recs.get(v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string()), picks.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("/"), if naps.is_empty() { "-".to_string() } else { naps.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("/") }));
            }
            s.push_str(&format!("  per villager (picks, naps): {}\n", if per_v.is_empty() { "none".to_string() } else { per_v.join("; ") }));
            s.push_str(&format!("  the picker's gates: {} picks gated at the pick (HELD{} {}, away {}, a picked cart not yet routed {}), {} launches with the lane's pocket empty (no pick), {} turned away by DAY at the row, {} by HELD at the row (divert sampling (i){})\n", ctl.picks_gated, if ctl.held_at_divert { " not read there" } else { "" }, ctl.gated_by[0], ctl.gated_by[1], ctl.gated_by[2], ctl.picks_empty, ctl.picks_day, ctl.picks_held_at_divert, if ctl.held_at_divert { "" } else { ": off" }));
            // (ironspec 23:56 PT (2)) HELD's invariant, checked: every set and reset with its cause;
            // a set still standing past the bound fails; (G1) a pick decided over a pending one fails
            s.push_str(&format!(
                "  HELD's sets and resets (ironspec 00:12 PT, F1: set at the queue's arrival detector, reset at the departure; ironspec's E2, day-1 06:51 PT: stuck when R1 is open (REST_l, DAY low, RS_ready) for more than 30 gt with HELD still set): {} sets, {} still set past the bound{} (bar 0: {}); the log: {}\n",
                ctl.held_log.iter().filter(|l| l.contains("HELD set by")).count(),
                ctl.held_stuck.len(),
                ctl.held_stuck.first().map_or(String::new(), |(t, c)| format!(", the first set at t {t} by {c}")),
                if ctl.held_log.iter().all(|l| !l.contains("HELD set by")) { "NOT JUDGED (HELD never set): FAIL" } else if ctl.held_stuck.is_empty() { "PASS" } else { "FAIL" },
                if ctl.held_log.is_empty() { "none".to_string() } else { ctl.held_log.iter().take(60).cloned().collect::<Vec<_>>().join("; ") }
            ));
            // (ironspec 00:12 / 00:16 PT; 02:37 PT: measured from the DIVERT, since a pick the divert
            // turns away never enters the way-in) the one-in-flight timing bar: each diverted
            // napper's divert -> arrival at the queue against the next divert (the margin must stay
            // > 0; a world open's N = 2 lead and the leg's spread print with the restart rows); the
            // turned-away picks print apart, with their causes
            let diverts: Vec<(i64, u32)> = ctl.diverts();
            let div_to_arr: Vec<i64> = diverts.iter().filter_map(|(dt, _)| ctl.q_arrivals.iter().find(|a| **a >= *dt).map(|a| a - dt)).collect();
            let worst_pa = div_to_arr.iter().copied().max();
            let mut least_margin: Option<(i64, i64)> = None;
            for (i, (dt, _)) in diverts.iter().enumerate() {
                let next = diverts.get(i + 1).map(|p| p.0);
                if let Some(a) = ctl.q_arrivals.iter().find(|a| **a >= *dt).copied()
                    && let Some(nx) = next
                {
                    let m = nx - a;
                    if least_margin.is_none_or(|(lm, _)| m < lm) {
                        least_margin = Some((m, *dt));
                    }
                }
            }
            let lost: Vec<&String> = ctl.held_log.iter().filter(|l| l.contains("LOST PICK")).collect();
            // (G1 N10) no divert dropped: one with no pick behind it, or no queue arrival after it
            let no_pick = ctl.divert_log.len().saturating_sub(diverts.len());
            let no_arrival = diverts.len().saturating_sub(div_to_arr.len());
            s.push_str(&format!(
                "  one in flight, the divert accounting (G1 N10: every divert joined to its pick and its queue arrival, bar 0 dropped): {} diverts logged, {} with no pick behind them, {} with no queue arrival after them ({})\n",
                ctl.divert_log.len(),
                no_pick,
                no_arrival,
                if ctl.divert_log.is_empty() { "NOT JUDGED (0 diverts): FAIL" } else if no_pick == 0 && no_arrival == 0 { "PASS" } else { "FAIL" }
            ));
            s.push_str(&format!(
                "  one in flight, the timing bar (ironspec 00:12 / 00:16 / 02:37 PT: each divert -> queue arrival against the next divert): {} diverts, {} arrivals after a divert, the largest divert -> arrival {}, the least margin to the next divert {} ({}); the picks turned away before the way-in (not in flight): {} ({} by DAY at the row){}\n",
                diverts.len(),
                div_to_arr.len(),
                worst_pa.map_or("-".to_string(), |w| format!("{w} gt")),
                least_margin.map_or("-".to_string(), |(m, dt)| format!("{m} gt (the divert at t {dt})")),
                match least_margin {
                    None => "NOT JUDGED (0 samples): FAIL",
                    Some((m, _)) if m > 0 => "PASS",
                    Some(_) => "FAIL",
                },
                lost.len(),
                ctl.picks_day,
                if lost.is_empty() { String::new() } else { format!(": {}", lost.iter().take(12).map(|l| l.as_str()).collect::<Vec<_>>().join("; ")) }
            ));
            s.push_str(&format!(
                "  a pick decided while another was pending (G1: an overwrite): {}{} (bar 0: {})\n",
                ctl.pick_overwrites.len(),
                ctl.pick_overwrites.first().map_or(String::new(), |f| format!(", the first: {f}")),
                if ctl.pick_overwrites.is_empty() { "PASS" } else { "FAIL" }
            ));
            // (day-1 22:06) the counter's log, for its release-by-release comparison
            s.push_str(&format!("  the counter's log (day:C at each release; the RISE / EIGHT loads; picks only inside C 141..221): {}{}\n", ctl.counter_log.join(" "), ctl.counter_unmodelled.map_or(String::new(), |d| format!("; the first release at day {d} is between RISE and EIGHT: the counter's start isn't modelled there (FAIL)"))));
            // (day-1 01:10 PT) the per-pick trace for the pick-by-pick diff against day-1's model:
            // every pick opportunity (tick, C, lane, verdict with HELD's holder); every picked
            // napper's divert, queue arrival, release, departure, stop entry, R, siding's R and back
            // in lane; the night's REST_l and DAY edges
            let opps: Vec<String> = ctl.pick_opps.iter().map(|(t, c, k, v)| format!("t {t} C {c} lane x {} {v}", LANES[*k])).collect();
            let mut unrouted_rows: Vec<String> = Vec::new();
            // (G1's P16) the entries too late to judge, printed as in flight
            let mut unrouted_in_flight: Vec<String> = Vec::new();
            // (G1 12:21) the stop visits over the accepted band, each with its whole event list
            let mut late_stop_rows: Vec<String> = Vec::new();
            let msn = |v: u32, what: &str, from: i64, to: i64| -> Option<i64> {
                past_marks.get(&v).into_iter().flatten().chain(recs.get(&v).map(|r| r.milestones.iter()).into_iter().flatten()).find(|(m, t)| m == what && *t >= from && *t < to).map(|(_, t)| *t)
            };
            let ms = |v: u32, what: &str, from: i64, to: i64| -> String { msn(v, what, from, to).map_or("-".to_string(), |t| t.to_string()) };
            // (G1 08:28 / ironspec 08:28 / 08:41 platform (3), NAPPER UNROUTED, bar 0) a napper that
            // entered the stop and has no R within the stop's threshold, or reached its siding's R and
            // isn't back in its lane within the siding's; both DERIVED from this run's measurements
            // (ironspec 08:41): the siding's, the lane's lap (its median D period times its 8 carts)
            // plus the longest measured late rejoin plus 100 gt; the stop's, the worst measured entry
            // -> R plus the longest tap wait (none in this module) plus 100 gt
            let unrouted_derivation: String;
            {
                let end = run.world.time - t0;
                let mut lap = [0i64; 3];
                let mut period = [0i64; 3];
                for k in 0..3 {
                    let ts: Vec<i64> = ctl.dpress_log.iter().filter(|(_, dk)| *dk == k).map(|(t, _)| *t).collect();
                    let mut gaps: Vec<i64> = ts.windows(2).map(|p| p[1] - p[0]).filter(|g| *g > 0).collect();
                    gaps.sort();
                    period[k] = gaps.get(gaps.len() / 2).copied().unwrap_or(0);
                    lap[k] = period[k] * 8;
                }
                let mut late_max = 0i64;
                let mut entry_r_max = 0i64;
                for (v, k, t) in nappers.iter() {
                    let to = ctl.pick_carts.iter().filter(|(pt, _, pv, _)| pv == v && pt > t).map(|x| x.0).min().unwrap_or(i64::MAX);
                    // (information only since G1 10:47 O7: the measured late rejoins and entry -> R no
                    // longer set the thresholds, so a judged napper's own delay can't widen its own)
                    if let (Some(r), Some(b)) = (msn(*v, "on its siding's R", *t, to), msn(*v, "back in its lane", *t, to))
                        && b - r <= 2 * lap[*k]
                    {
                        late_max = late_max.max(b - r);
                    }
                    if let (Some(e), Some(r)) = (msn(*v, "at A6's entry", *t, to), msn(*v, "at R", *t, to)) {
                        entry_r_max = entry_r_max.max(r - e);
                    }
                }
                // (G1 10:47 blocking 7 / O7) the thresholds from the design and the controller, never
                // from the judged nappers: on the siding, the lane's lap (its median D period x 8 carts)
                // + one more D period and the release's 30-gt window (a napper missing its release
                // leaves at the lane's next D press: the fail-safe) + 100 gt; at the stop, 58f770cc's
                // accepted entry -> R band's top 161 gt (09:24 PT, the dense sweep 104 / 104) + the
                // longest tap wait (no taps in this module) + 100 gt; a lane with no D period (no two
                // D presses) is NOT JUDGED
                let siding_thr: [i64; 3] = [0, 1, 2].map(|k| lap[k] + period[k] + 30 + 100);
                let stop_thr = 161 + 100;
                unrouted_derivation = format!(
                    "the thresholds (G1 10:47 O7: from the design and the controller, not the judged nappers): on the siding, lane x {} / {} / {}: the lap {} / {} / {} gt (the median D period {} / {} / {} gt x 8 carts) + one D period + the 30-gt release window (the emulated siding release: R off 30 gt after it powers, siding_off = now + 30, day-1's latch line 14) + 100 (ironspec 08:41's margin) = {} / {} / {} gt; at the stop: 58f770cc's entry -> R band top 161 gt (09:24 PT, the dense sweep 104 / 104) + no tap wait + 100 (ironspec 08:41's margin) = {stop_thr} gt; measured, for information: the longest late rejoin {late_max} gt, the worst entry -> R {entry_r_max} gt{}",
                    LANES[0], LANES[1], LANES[2], lap[0], lap[1], lap[2], period[0], period[1], period[2], siding_thr[0], siding_thr[1], siding_thr[2],
                    if period.contains(&0) { format!("; NOT JUDGED on lane(s) {} (no D period): FAIL", (0..3).filter(|k| period[*k] == 0).map(|k| format!("x {}", LANES[k])).collect::<Vec<_>>().join(", ")) } else { String::new() }
                );
                for (v, k, t) in nappers.iter() {
                    let to = ctl.pick_carts.iter().filter(|(pt, _, pv, _)| pv == v && pt > t).map(|x| x.0).min().unwrap_or(i64::MAX);
                    let entry = msn(*v, "at A6's entry", *t, to);
                    let rr = msn(*v, "at R", *t, to);
                    let sr = msn(*v, "on its siding's R", *t, to);
                    let back = msn(*v, "back in its lane", *t, to);
                    // (G1 12:21) a stop visit over the accepted band (entry -> R > 161): its WHOLE
                    // event list from the entry to R (the drive's stop events: lie-down, F, the wake,
                    // the release, each pickup), with its own milestones, for its classification
                    if let (Some(e), Some(r)) = (entry, rr)
                        && r - e > 161
                    {
                        let lead = |f: &str| -> Option<i64> { f.strip_prefix("t ").and_then(|x| x.split(':').next()).and_then(|n| n.trim().parse().ok()) };
                        let evs: Vec<&String> = ctl.nap_events.iter().filter(|ev| lead(ev).is_some_and(|n| n >= e - 20 && n <= r + 5)).collect();
                        let own: Vec<String> = recs.get(v).map(|rc| past_marks.get(v).into_iter().flatten().chain(rc.milestones.iter()).filter(|(_, mt)| *mt >= e - 20 && *mt <= r + 5).map(|(m, mt)| format!("t {mt} {m}")).collect()).unwrap_or_default();
                        late_stop_rows.push(format!("#{v} lane x {} picked t {t}: entry t {e} -> R t {r} ({} gt, over the band's 161); its milestones: {}; the stop's events: {}", LANES[*k], r - e, if own.is_empty() { "none".to_string() } else { own.join(", ") }, if evs.is_empty() { "none".to_string() } else { evs.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(" | ") }));
                    }
                    // (G1 10:47 O7: a LATE R fails too, not only a missing one)
                    if let Some(e) = entry
                        && rr.is_none_or(|r| r - e > stop_thr)
                        && end - e >= stop_thr
                    {
                        unrouted_rows.push(format!("#{v} lane x {} picked t {t}: entered the stop at t {e}, R {}", LANES[*k], rr.map_or(format!("none within {stop_thr} gt"), |r| format!("at t {r}, {} gt later (over {stop_thr})", r - e))));
                    } else if let Some(e) = entry
                        && rr.is_none()
                        && end - e < stop_thr
                    {
                        // (G1's P16) an entry too late to judge is printed as in flight, never skipped
                        unrouted_in_flight.push(format!("#{v} lane x {} entered the stop at t {e}, {} gt before the end (under {stop_thr}): in flight", LANES[*k], end - e));
                    }
                    if period[*k] == 0 {
                        continue;
                    }
                    if let Some(r) = sr
                        && back.is_none_or(|b| b - r > siding_thr[*k])
                        && end - r >= siding_thr[*k]
                    {
                        unrouted_rows.push(format!("#{v} lane x {} picked t {t}: on its siding's R at t {r}, back in its lane {}", LANES[*k], back.map_or("never".to_string(), |b| format!("at t {b} ({} gt later)", b - r))));
                    }
                    if let (Some(r), Some(b)) = (sr, back)
                        && b - r > period[*k] + 30
                    {
                        late_rejoin.push((*v, r, b));
                    }
                }
            }
            let visits: Vec<String> = ctl
                .pick_carts
                .iter()
                .map(|(t, cid, v, k)| {
                    // (a visit runs from its pick to the same napper's next pick)
                    let to = ctl.pick_carts.iter().filter(|(pt, _, pv, _)| pv == v && pt > t).map(|x| x.0).min().unwrap_or(i64::MAX);
                    let div = ctl.divert_log.iter().find(|(dt, c)| c == cid && dt >= t).map_or("-".to_string(), |(dt, _)| dt.to_string());
                    let rel = ctl.release_log.iter().find(|(rt, rv)| rv == v && rt >= t).map_or("-".to_string(), |(rt, _)| rt.to_string());
                    let dep = ctl.dep_log.iter().find(|(dt, dv)| dv == v && dt >= t).map_or("-".to_string(), |(dt, _)| dt.to_string());
                    // (day-1 01:27 PT, m per visit) the H2 sample it caught, its K, PRESENT, the
                    // siding's R and its lane's D presses around R
                    let hold = msn(*v, "on the hold (9,0,8)", *t, to);
                    let h2e = hold.and_then(|h| ctl.h2_log.iter().find(|(ht, _, _)| *ht >= h).copied());
                    let h2 = h2e.map(|x| x.0);
                    let kt = msn(*v, "on its K", *t, to);
                    let present = h2.and_then(|h| ctl.present_log.iter().find(|(pt, pk)| *pk == *k && *pt >= h).map(|x| x.0));
                    let sr = msn(*v, "on its siding's R", *t, to);
                    let presses: Vec<String> = ctl.dpress_log.iter().filter(|(dt, dk)| *dk == *k && sr.is_some_and(|r| *dt >= r - 60 && *dt <= r + 120)).map(|x| x.0.to_string()).collect();
                    let next_press = sr.and_then(|r| ctl.dpress_log.iter().find(|(dt, dk)| *dk == *k && *dt > r).map(|x| x.0));
                    let o = |x: Option<i64>| x.map_or("-".to_string(), |v| v.to_string());
                    // (ironspec 01:59 PT) the visit's decomposition: pick phase, dwell, way-in leg,
                    // descent (with any stall), lie-down -> wake, pickup, R, lift exit, Q-up
                    let dd = |a: Option<i64>, b: Option<i64>| match (a, b) { (Some(a), Some(b)) => (b - a).to_string(), _ => "-".to_string() };
                    let arr = msn(*v, "at the queue's arrival", *t, to);
                    let reln = ctl.release_log.iter().find(|(rt, rv)| rv == v && rt >= t).map(|x| x.0);
                    let entry = msn(*v, "at A6's entry", *t, to);
                    let lie = msn(*v, "asleep in the bed", *t, to);
                    let woke = msn(*v, "woke in A6", *t, to);
                    let pick_up = msn(*v, "a cart at P after the wake", *t, to);
                    let rr = msn(*v, "at R", *t, to);
                    let exit = rr.and_then(|r| ctl.lift_exit_log.iter().find(|x| **x >= r).copied());
                    let qup = exit.and_then(|x| ctl.k_lock.map(|kl| x + kl));
                    let c_at = ctl.pick_opps.iter().find(|x| x.0 == *t).map_or("-".to_string(), |x| x.1.to_string());
                    // (ironspec 04:56 PT: the calm -> release distribution) its first calm tick and
                    // its CANT_REACH arming on the way to the queue
                    let calm = msn(*v, "calm before the queue", *t, to);
                    let armed = msn(*v, "CANT_REACH armed before the queue", *t, to);
                    let calm_row = format!(
                        "the calm -> release row (ironspec 04:56 PT): calm before the queue t {}, CANT_REACH armed before the queue t {}, queue arrival t {}, release t {}, armed -> release {} gt",
                        o(calm),
                        o(armed),
                        o(arr),
                        o(reln),
                        dd(armed, reln)
                    );
                    let decomposition = format!(
                        "the decomposition: pick at C {c_at} (u {}), pick -> queue arrival {}, dwell {}, way-in leg (release -> A6's entry) {}, entry -> lie-down (descent, any stall) {}, lie-down -> wake {}, wake -> a cart at P {}, P -> R {}, R -> lift exit {}, lift exit -> Q-up {}, Q-up -> H2 {}",
                        c_at.parse::<i64>().map_or("-".to_string(), |c| (c % 10).to_string()),
                        dd(Some(*t), arr),
                        dd(arr, reln),
                        dd(reln, entry),
                        dd(entry, lie),
                        dd(lie, woke),
                        dd(woke, pick_up),
                        dd(pick_up, rr),
                        dd(rr, exit),
                        dd(exit, qup),
                        dd(qup, h2)
                    );
                    format!(
                        "#{v} lane x {} picked t {t}: divert {div}, queue arrival {}, release {rel}, departure {dep}, stop entry {}, R {}, hold {}, H2 sample {} (the catch window: late slack {}, early slack {}), K {}{}, PRESENT {}, siding's R {}, back in lane {}; the lane's D presses within R - 60 .. R + 120: [{}]; the next D press after R minus R: {}",
                        LANES[*k],
                        ms(*v, "at the queue's arrival", *t, to),
                        ms(*v, "at A6's entry", *t, to),
                        ms(*v, "at R", *t, to),
                        o(hold),
                        o(h2),
                        h2e.map_or("-".to_string(), |x| format!("{} gt", x.2)),
                        h2e.map_or("-".to_string(), |x| format!("{} gt", 100 - x.2)),
                        o(kt),
                        match (h2, kt) { (Some(h), Some(kk)) => format!(" (the K leg {} gt after H2)", kk - h), _ => String::new() },
                        o(present),
                        o(sr),
                        ms(*v, "back in its lane", *t, to),
                        presses.join(" "),
                        match (next_press, sr) { (Some(p), Some(r)) => format!("{:+}", p - r), _ => "-".to_string() }
                    ) + "; " + &decomposition + "; " + &calm_row
                })
                .collect();
            s.push_str(&format!(
                "  the per-pick trace (day-1 01:10 PT): {} opportunities: {}; {} picked nappers: {}; REST_l / DAY edges: {}\n",
                opps.len(),
                if opps.is_empty() { "none".to_string() } else { opps.join(", ") },
                visits.len(),
                if visits.is_empty() { "none".to_string() } else { visits.join("; ") },
                if ctl.edge_log.is_empty() { "none".to_string() } else { ctl.edge_log.join(", ") }
            ));
            // (G1 08:28 / ironspec 08:28 platform (3)) the napper-unrouted bar
            unrouted_total = unrouted_rows.len();
            s.push_str(&format!(
                "  no napper unrouted (G1 08:28 / ironspec 08:28 / 08:41 platform: a napper entering the stop with no R, or on its siding's R and not back in its lane, within the derived thresholds; bar 0; {unrouted_derivation}): {} ({}){}; in flight at the end (G1's P16: too late to judge, printed): {}; the late rejoins (a missed release, then the lane's next D press under the built latch): {}\n",
                unrouted_rows.len(),
                if nappers.is_empty() { "NOT JUDGED (no napper): FAIL" } else if unrouted_rows.is_empty() { "PASS" } else { "FAIL" },
                if unrouted_rows.is_empty() { String::new() } else { format!(": {}", unrouted_rows.join("; ")) },
                if unrouted_in_flight.is_empty() { "none".to_string() } else { unrouted_in_flight.join("; ") },
                if late_rejoin.is_empty() { "none".to_string() } else { late_rejoin.iter().map(|(v, r, b)| format!("#{v}: on its siding's R at t {r}, back in its lane at t {b} ({} gt)", b - r)).collect::<Vec<_>>().join("; ") }
            ));
            // (G1 12:21) each stop visit over the accepted band, with its whole event list (for its
            // classification: a re-sleep, a missed pickup, a late wake, a missed tap, an occupied bed)
            s.push_str(&format!(
                "  the stop visits over the band (G1 12:21: entry -> R > 161 gt, the accepted bench band 141..161; information for the classification): {}{}\n",
                late_stop_rows.len(),
                if late_stop_rows.is_empty() { String::new() } else { format!(": {}", late_stop_rows.join(" || ")) }
            ));
            // (ironspec 01:26 / 01:28 PT) the catch window: each fired H2 sample's late slack (the
            // sample minus Q-up, the waiting RS's set + k_lock) and early slack (Q-up minus the
            // previous sample, 100 gt before), the least of each over the run, both bars >= 2
            let least_catch = ctl.h2_log.iter().map(|x| x.2).min();
            let least_early = ctl.h2_log.iter().map(|x| 100 - x.2).min();
            s.push_str(&format!(
                "  the catch window (ironspec 01:26 / 01:28 PT: late slack = the caught H2 sample minus Q-up, early slack = Q-up minus the previous sample): {} samples, the least late slack {} (bar >= 2: {}), the least early slack {} (bar >= 2: {})\n",
                ctl.h2_log.len(),
                least_catch.map_or("-".to_string(), |m| format!("{m} gt")),
                match least_catch {
                    None => "NOT JUDGED (0 samples): FAIL",
                    Some(m) if m >= 2 => "PASS",
                    Some(_) => "FAIL",
                },
                least_early.map_or("-".to_string(), |m| format!("{m} gt")),
                match least_early {
                    None => "NOT JUDGED (0 samples): FAIL",
                    Some(m) if m >= 2 => "PASS",
                    Some(_) => "FAIL",
                }
            ));
        }
        s.push_str(&format!(
            "  the nappers' stamp at their queue arrival: {}; the picker: {} picks gated (HELD, away), {} turned away by DAY at the row; the queue's releases at t {}\n",
            match worst {
                None => "no arrival seen".to_string(),
                Some(None) => "unarmed at every arrival".to_string(),
                Some(Some(a)) => format!("the worst {a} gt"),
            },
            ctl.picks_gated,
            ctl.picks_day,
            ctl.q_released.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")
        ));
        s.push_str(&format!(
            "  the hold's pulses fired with the hold empty: {} (bar 0: {}); fired with the napper still rolling: {} (bar 0: {})\n",
            ctl.empty_pulses,
            if ctl.empty_pulses == 0 { "PASS" } else { "FAIL" },
            ctl.rolling_pulses,
            if ctl.rolling_pulses == 0 { "PASS" } else { "FAIL" }
        ));
        // (ironspec 17:16) each visit's queue dwell, its arrival to its release (the stop's wait
        // included): <= 5 until the way-in booster and lane A's 19-gt cut land, <= 50 after
        {
            let dwells: Vec<(i64, Option<i64>)> = ctl.q_arrivals.iter().map(|a| (*a, ctl.q_released.iter().find(|r| **r >= *a).map(|r| r - a))).collect();
            let worst = dwells.iter().filter_map(|d| d.1).max();
            let unreleased = dwells.iter().filter(|d| d.1.is_none()).count();
            s.push_str(&format!(
                "  the queue's dwell (information, ironspec 06:35 platform: the dwell is design, judged through the catch window, the distinct count and one in flight; the hold term {} gt; arrival to release, the stop's wait included; the module's cap {}): {}; the worst {}{}\n",
                ctl.queue_hold,
                o.dwell_cap.map_or("none (no cap of record)".to_string(), |c| format!("{c} gt (HELD's bound only)")),
                dwells.iter().take(30).map(|(a, d)| format!("t {a}: {}", d.map_or("not released".to_string(), |d| d.to_string()))).collect::<Vec<_>>().join(", "),
                worst.map_or("-".to_string(), |w| w.to_string()),
                if unreleased > 0 { format!("; {unreleased} arrivals never released") } else { String::new() }
            ));
            // (ironspec 19:29 (a) SINGLE OCCUPANCY: at most one napper between the queue's release
            // and its 16-ball exit after R; (b) ONE NAPPER IN FLIGHT: at most one between its pick
            // and the queue's departure; each with its first bad event)
            {
                let name = |v: u32| recs.get(&v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string());
                let overlap = |starts: &[(i64, u32)], ends: &[(i64, u32)]| -> (usize, Option<String>) {
                    let iv: Vec<(i64, Option<i64>, u32)> = starts.iter().map(|(s, v)| (*s, ends.iter().find(|(e, w)| *w == *v && *e >= *s).map(|(e, _)| *e), *v)).collect();
                    let mut n = 0usize;
                    let mut first: Option<String> = None;
                    for (j, (sj, _, vj)) in iv.iter().enumerate() {
                        if let Some((si, ei, vi)) = iv[..j].iter().find(|(si, ei, vi)| *si <= *sj && ei.is_none_or(|e| e > *sj) && *vi != *vj) {
                            n += 1;
                            if first.is_none() {
                                first = Some(format!("t {sj}: {} entered while {} (since t {si}) was still in, until {}", name(*vj), name(*vi), ei.map_or("the run's end".to_string(), |e| format!("t {e}"))));
                            }
                        }
                    }
                    (n, first)
                };
                let (na, fa) = overlap(&ctl.release_log, &exit_log);
                s.push_str(&format!(
                    "  (a2) single occupancy (ironspec 20:22: at most one napper in the stop, A6's box from its entry to R, and no other napper's cart in its track while one is inside): two nappers (carts, or one asleep) in the stop's box at once {stop_two_ticks} ticks{} (bar 0: {}); informational: {} queue releases, {na} before the previous napper's 16-ball exit{}\n",
                    stop_two.as_ref().map_or(String::new(), |(t, vs)| format!(", the first t {t} ({})", vs.iter().map(|v| name(*v)).collect::<Vec<_>>().join(", "))),
                    if stop_two_ticks == 0 { "PASS" } else { "FAIL" },
                    ctl.release_log.len(),
                    fa.map_or(String::new(), |f| format!(", the first: {f}"))
                ));
                let diverts = ctl.diverts();
                let (nb, fb) = overlap(&diverts, &ctl.dep_log);
                s.push_str(&format!(
                    "  (b) one napper in flight (ironspec 19:29 / 02:37 PT: one from its divert to the queue's departure; a pick the divert turns away is not in flight): {} diverts of {} picks, {nb} while another was in flight{} (bar 0: {})\n",
                    diverts.len(),
                    ctl.pick_log.len(),
                    fb.as_ref().map_or(String::new(), |f| format!(", the first: {f}")),
                    if nb == 0 { "PASS" } else { "FAIL" }
                ));
                // (ironspec 22:56 / 22:58 PT, the loud bars) the park braked at every arrival; no
                // riderless cart past A6's R; no napper held in A6 past the class-b bound (the end of
                // the REST after the one it entered in); each failure with its first-bad-event trace
                let unbraked: Vec<&(i64, bool, Option<u32>)> = park_arrivals.iter().filter(|p| !p.1).collect();
                s.push_str(&format!(
                    "  A6's park at every arrival (ironspec 22:56 PT: braked, its slope (1,y+1,-5) / (0,y,-5) unpowered, or it fails): {} arrivals ({} with a napper aboard) against {} A6 entries, unbraked {}{} (bar 0: {})\n",
                    park_arrivals.len(),
                    park_arrivals.iter().filter(|p| p.2.is_some()).count(),
                    a6_entries.len(),
                    unbraked.len(),
                    unbraked.first().map_or(String::new(), |p| format!(", the first t {}", p.0)),
                    if unbraked.is_empty() { "PASS" } else { "FAIL" }
                ));
                s.push_str(&format!(
                    "  riderless carts past A6's R (ironspec 22:56 PT: by design none leaves A6): {}{} (bar 0: {})\n",
                    riderless_r.len(),
                    riderless_r.first().map_or(String::new(), |(t, c)| format!(", the first t {t} (cart {c})")),
                    if riderless_r.is_empty() { "PASS" } else { "FAIL" }
                ));
                s.push_str(&format!(
                    "  carts on the leg's cells (8,25,9) / (9,25,8) other than the picked napper's (ironspec 23:44 / 23:47 PT, B4): {}{} (bar 0: {})\n",
                    leg_wrong.len(),
                    leg_wrong.first().map_or(String::new(), |(t, c, r)| format!(", the first t {t} (cart {c}, {})", r.map_or("empty".to_string(), name))),
                    if leg_wrong.is_empty() { "PASS" } else { "FAIL" }
                ));
                let mut held: Vec<(i64, String)> = Vec::new();
                let mut undetermined: Vec<String> = Vec::new();
                for (rid, r) in recs.iter().filter(|(id, _)| nappers.iter().any(|n| n.0 == **id)) {
                    let Some(te) = r.milestones.iter().find(|(m, _)| m == "at A6's entry").map(|(_, t)| *t) else { continue };
                    // (G1: REST ends at day time 24,010, i.e. 10 of the next day) the bound: the end
                    // of the REST after the one the entry fell in (or the next REST for an entry by day)
                    let d = (o.day0 + te).rem_euclid(24_000);
                    let bound = if d >= 12_000 {
                        te + (24_010 - d) + 24_000
                    } else if d < 10 {
                        te + (10 - d) + 24_000
                    } else {
                        te + (24_010 - d)
                    };
                    let at_r = r.milestones.iter().find(|(m, t)| m == "at R" && *t >= te).map(|(_, t)| *t);
                    match at_r {
                        Some(tr) if tr > bound => held.push((bound, format!("{} entered t {te}, bound t {bound}, at R t {tr}", name(*rid)))),
                        Some(_) => {}
                        None if o.ticks > bound => held.push((bound, format!("{} entered t {te}, bound t {bound}, at R never", name(*rid)))),
                        None => undetermined.push(format!("{} entered t {te}, bound t {bound} past the run's end", name(*rid))),
                    }
                }
                held.sort();
                s.push_str(&format!(
                    "  a napper held in A6 past the class-b bound (ironspec 22:58 PT: R by the end of the REST after the one it entered in; REST ends at day time 24,010): {} judged, {}{} (bar 0: {}); undetermined (the bound past the run's end): {}\n",
                    held.len() + recs.iter().filter(|(id, r)| nappers.iter().any(|n| n.0 == **id) && r.milestones.iter().any(|(m, _)| m == "at A6's entry")).count() - undetermined.len() - held.len(),
                    held.len(),
                    held.first().map_or(String::new(), |(_, h)| format!(", the first: {h}")),
                    if held.is_empty() { "PASS" } else { "FAIL" },
                    if undetermined.is_empty() { "none".to_string() } else { undetermined.join("; ") }
                ));
                // the first-bad-event traces, printed on every failed bar above: the drive events of
                // the 400 gt before its first failure
                let lead = |f: &str| -> Option<i64> { f.strip_prefix("t ").and_then(|r| r.split(':').next()).and_then(|n| n.trim().parse().ok()) };
                let mut firsts: Vec<(&str, i64)> = Vec::new();
                if let Some((t, _)) = stop_two.as_ref() {
                    firsts.push(("(a2) two nappers in the stop", *t));
                }
                if let Some(t) = fb.as_deref().and_then(lead) {
                    firsts.push(("(b) one napper in flight", t));
                }
                if let Some(p) = unbraked.first() {
                    firsts.push(("A6's park unbraked at an arrival", p.0));
                }
                if let Some((t, _)) = riderless_r.first() {
                    firsts.push(("a riderless cart past A6's R", *t));
                }
                if let Some((t, _, _)) = leg_wrong.first() {
                    firsts.push(("a cart other than the picked napper's on the leg", *t));
                }
                if let Some((t, _)) = held.first() {
                    firsts.push(("a napper held in A6 past the class-b bound", *t));
                }
                for (bar, t) in firsts {
                    let evs: Vec<&String> = ctl.nap_events.iter().filter(|e| lead(e).is_some_and(|n| n >= t - 400 && n <= t)).collect();
                    let tail: Vec<String> = evs.iter().rev().take(30).rev().map(|e| e.to_string()).collect();
                    s.push_str(&format!("  the first-bad-event trace of \"{bar}\" (t {t}; the drive events of the 400 gt before it): {}\n", if tail.is_empty() { "none".to_string() } else { tail.join("; ") }));
                }
            }
            // (each napper's PRESENT_l set against its predecessor's D pass: the predecessor must
            // pass its siding's D before the next PRESENT)
            let mut rows: Vec<String> = Vec::new();
            let mut least: Option<i64> = None;
            for (n, (p, k)) in ctl.present_log.iter().enumerate() {
                if n == 0 {
                    rows.push(format!("PRESENT t {p} (lane x {}): no predecessor", LANES[*k]));
                    continue;
                }
                // (the D pass that consumed the previous PRESENT: the passes come in PRESENT order)
                match ctl.dpass_log.get(n - 1) {
                    Some((d, dk)) => {
                        let m = p - d;
                        least = Some(least.map_or(m, |l: i64| l.min(m)));
                        rows.push(format!("PRESENT t {p} (lane x {}), the predecessor's D pass t {d} (lane x {}): {m:+}", LANES[*k], LANES[*dk]));
                    }
                    None => {
                        least = Some(least.map_or(-1, |l: i64| l.min(-1)));
                        rows.push(format!("PRESENT t {p} (lane x {}) with its predecessor's D never passed", LANES[*k]));
                    }
                }
            }
            let passes = ctl.dpass_log.len();
            s.push_str(&format!(
                "  PRESENT against the predecessor's D pass (ironspec 17:16): {} PRESENT, {passes} D passes; {}; the least margin {}\n",
                ctl.present_log.len(),
                if rows.is_empty() { "none".to_string() } else { rows.iter().take(30).cloned().collect::<Vec<_>>().join("; ") },
                least.map_or("-".to_string(), |l| format!("{l} (bar >= 7, ironspec 19:29 PT: the longest freeze 4 + tick order 1 + 2: {})", if l >= 7 { "PASS" } else { "FAIL" }))
            ));
        }
        // (ironspec 00:40 PT) each napper's m = its D pass - its PRESENT (the restated bar:
        // m >= max(L_latch + 1, settle - 14) + 2, 9 if settle <= 23; a late rejoin, m < 0, must be 0
        // in normal operation): printed per napper with the late rejoins as a bar
        {
            let ms: Vec<(i64, usize, i64)> = ctl.present_log.iter().enumerate().filter_map(|(n, (p, k))| ctl.dpass_log.get(n).map(|(d, _)| (*p, *k, d - p))).collect();
            let late = ms.iter().filter(|(_, _, m)| *m < 0).count();
            s.push_str(&format!(
                "  each napper's m = its D pass - its PRESENT (ironspec 00:40 PT; the bar max(L_latch + 1, settle - 14) + 2, 9 if settle <= 23, is day-1's to evaluate): {} nappers: {}; the least {}; late rejoins (m < 0) {late} (bar 0: {})\n",
                ms.len(),
                ms.iter().take(30).map(|(p, k, m)| format!("PRESENT t {p} lane x {} m {m:+}", LANES[*k])).collect::<Vec<_>>().join(", "),
                ms.iter().map(|x| x.2).min().map_or("-".to_string(), |m| format!("{m:+}")),
                if ms.is_empty() { "NOT JUDGED (0 samples): FAIL" } else if late == 0 { "PASS" } else { "FAIL" }
            ));
        }
        // (G1's note) each siding's R: its observed rise against D + k_sw + 8 (the emulation's
        // input timing assumes the line's latency: 8 gt through the (a) feed stack, 2 through the
        // siding repeater)
        if !ctl.r_obs.is_empty() || ctl.r_expect.iter().any(|e| e.is_some()) {
            let off: Vec<i64> = ctl.r_obs.iter().map(|(_, e, o2)| o2 - e).collect();
            s.push_str(&format!(
                "  the sidings' R against D + k_sw + 8 (G1): {} rises: {}{}; {}\n",
                ctl.r_obs.len(),
                ctl.r_obs.iter().take(20).map(|(k, e, o2)| format!("lane x {} t {o2} ({:+})", LANES[*k], o2 - e)).collect::<Vec<_>>().join(", "),
                if ctl.r_expect.iter().any(|e| e.is_some()) { "; an R still expected at the run's end" } else { "" },
                if off.iter().all(|d| *d == 0) { "all on time (PASS)" } else { "off time (FAIL)" }
            ));
        }
        s.push_str(&format!("  the drives: {}\n", ctl.nap_events.iter().take(120).cloned().collect::<Vec<_>>().join("; ")));
        // (farmopt / ironspec 00:20 PT, evidence for the seat's sight) the hatch's state at each
        // seat arrival and release (in the drives above), and every panic starting at the seat
        let hatch_rows: Vec<String> = ctl.nap_events.iter().filter(|e| e.contains("the hatch (6,")).map(|e| {
            let t = e.split(':').next().unwrap_or("");
            let what = if e.contains("arrival") { "arrival" } else { "release" };
            let st = if e.ends_with("open") { "open" } else { "closed" };
            format!("{t} {what} {st}")
        }).collect();
        s.push_str(&format!(
            "  the queue hatch at each seat arrival and release (farmopt 00:21 PT): {}; panics starting at the queue seat: {} ({}); the seat's NEAREST_HOSTILE (ironspec 00:22 PT; information until the hatch input N OR HELD is in): {seat_hostile_ticks} napper-ticks{}\n",
            if hatch_rows.is_empty() { "none".to_string() } else { hatch_rows.join(", ") },
            seat_panics.len(),
            if seat_panics.is_empty() { "none".to_string() } else { seat_panics.join("; ") },
            seat_hostile_first.as_ref().map_or(String::new(), |f| format!(", the first {f}"))
        ));
        s.push_str(&format!(
            "  the nappers' paths at the queue seat, at each change (farmopt 00:38 PT, b8949c96's probe): {}\n",
            if seat_paths.is_empty() { "none".to_string() } else { seat_paths.join("; ") }
        ));
        // (ironspec 00:33 PT) the stranded-napper bar
        // (ironspec 02:18 PT (3a)) each lane's away at every release, bar <= 1, and the free
        // caller's margin at the lane's least occupancy, (carts + 1) x 100 - 32 - GOLEM_TTL (69 at 6
        // carts), bar >= 3
        if let Some(full) = occ_full {
            let margin = |c: i64| (c + 1) * 100 - 32 - GOLEM_TTL;
            let design_ok = lane_ids_t0.iter().all(|v| v.len() == 8);
            let ok = design_ok && (0..3).all(|k| away_max[k] <= 1 && margin(occ_min[k]) >= 3);
            s.push_str(&format!(
                "  each lane's away at every release (ironspec 02:18 PT (3a): the away latch allows <= 1 per lane; the free caller's margin at the lane's least occupancy, (carts + 1) x 100 - 32 - GOLEM_TTL 599, bar >= 3; G1 N6 / ironspec D4: each lane's carts by id at t0, the design's 8 a lane (P, H, R_1 and five slopes): {}): {}{} ({})\n",
                (0..3).map(|k| format!("lane x {} {}{}", LANES[k], lane_ids_t0[k].len(), if lane_ids_t0[k].len() == 8 { "" } else { " (not 8)" })).collect::<Vec<_>>().join(", "),
                (0..3).map(|k| format!("lane x {}: {} carts at the first release, the least {}, the most away {}, the margin there {:+}", LANES[k], full[k], occ_min[k], away_max[k], margin(occ_min[k]))).collect::<Vec<_>>().join("; "),
                if away_rows.is_empty() { String::new() } else { format!("; more than one away: {}", away_rows.join(", ")) },
                if ok { "PASS" } else { "FAIL" }
            ));
        } else {
            s.push_str("  each lane's away at every release (ironspec 02:18 PT (3a)): NOT JUDGED (0 releases): FAIL\n");
        }
        // (ironspec 02:18 PT (2)) DAY's validation row
        s.push_str(&format!(
            "  the DAY stand-in's validation (ironspec 02:18 PT (2), G1 N3 / ironspec D3: DAY = the detector at level >= {}; at every 20-gt update after a 40-gt settle the stored power against redsim's analytic detector, zero tolerance; the run's weather {}): {} ticks read, {} updates checked, {} updates where the stored power differs ({}){}; the DAY edges: {}\n  the weather rows (ironspec 02:37 PT (3); G1 X23: their own line, so a control on the DAY line never masks them): {}\n",
            ctl.day_threshold,
            format!("on {}, rain {:.2}, thunder {:.2}, sky darken now {}", run.world.weather.on, run.world.weather.rain_level, run.world.weather.thunder_level, run.world.sky_darken()),
            ctl.day_ticks,
            ctl.day_updates,
            ctl.day_mismatch,
            if ctl.day_updates == 0 { "NOT JUDGED (0 updates): FAIL" } else if ctl.day_mismatch == 0 { "PASS" } else { "FAIL" },
            if ctl.day_mismatch_rows.is_empty() { String::new() } else { format!(": {}", ctl.day_mismatch_rows.join("; ")) },
            if ctl.day_edges.is_empty() { "none".to_string() } else { ctl.day_edges.join(", ") },
            day_rows
        ));
        s.push_str(&format!(
            "  stranded nappers (ironspec 00:33 PT: a napper waking in A6 with no cart at P by its wake + the bound fails; P and the bound from the module's `# a6-pickup:`): {}\n",
            match o.a6_pickup {
                None => "NOT JUDGED (the module declares no `# a6-pickup:`): FAIL".to_string(),
                Some((p, b)) => format!(
                    "P ({},{},{}), bound +{b}; {wakes_seen} wakes, {stranded_n} stranded{} ({})",
                    p[0], p[1], p[2],
                    if stranded.is_empty() { String::new() } else { format!(": {}", stranded.join("; ")) },
                    if wakes_seen == 0 { "NOT JUDGED (0 samples): FAIL" } else if stranded_n == 0 { "PASS" } else { "FAIL" }
                ),
            }
        ));
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
    // (G1 16:36, ironspec 16:35) a scarer outage's time-to-lock: each rider's stamp recomputed WITHOUT the
    // flee-path clears (a clear through "a flee point" dropped): from dusk (t 1000, the stamp armed) it
    // resets only at its own-path clears; the tick it first passes 1,200; the second such rider's tick
    // is the outage's time-to-lock (the farm absorbs one HOME loss)
    {
        let mut crosses: Vec<(i64, u32)> = Vec::new();
        let mut own_by_rider: Vec<(u32, usize)> = Vec::new();
        for (id, r) in recs.iter() {
            let mut resets: Vec<i64> = r
                .clears
                .iter()
                .filter(|c| !c.contains("a flee point") && !c.contains("a reload"))
                .filter_map(|c| c.strip_prefix("t ").and_then(|x| x.split(' ').next()).and_then(|t| t.parse::<i64>().ok()))
                .filter(|t| *t >= 1000)
                .collect();
            resets.sort();
            own_by_rider.push((*id, resets.len()));
            let mut last = 1000i64;
            let mut cross: Option<i64> = None;
            for t in resets.iter().copied().chain(std::iter::once(i64::MAX)) {
                if t.saturating_sub(last) > 1200 {
                    cross = Some(last + 1200);
                    break;
                }
                last = t;
            }
            if let Some(c) = cross
                && c <= o.ticks as i64
            {
                crosses.push((c, *id));
            }
        }
        crosses.sort();
        s.push_str(&format!(
            "the scarer outage's time-to-lock (G1 16:36: every rider's stamp recomputed without its flee-path clears, from dusk t 1000; the farm absorbs one HOME loss, the second locks napping): {} riders pass 1,200 gt; the first {}; the second (the time-to-lock) {}; riders with no own-path clear at night: {}\n",
            crosses.len(),
            crosses.first().map_or("never".to_string(), |(t, v)| format!("#{v} at t {t}")),
            crosses.get(1).map_or("never".to_string(), |(t, v)| format!("#{v} at t {t} ({} gt after dusk)", t - 1000)),
            own_by_rider.iter().filter(|(_, n)| *n == 0).count()
        ));
    }
    // (ironspec 16:37 (3), G1 16:37) per lane (by each villager's seat at the start): the worst calm
    // clear-to-clear interval (the arm -> clear pairs), the clears by 26.3 mechanism (a flee path, its own
    // path home, a stroll path, a lie-down, other), lane 7 printed separately (it never passes the lift
    // foot); the lift foot's passes and clears (the per-pass clear rate)
    {
        let lane_of = |r: &Rec| -> String {
            let x = r.label.split("seated at [").nth(1).and_then(|s| s.split(',').next()).and_then(|s| s.trim().parse::<i32>().ok());
            match x {
                Some(x) if LANES.contains(&x) => format!("lane x {x}"),
                Some(x) => format!("x {x}"),
                None => "unseated".to_string(),
            }
        };
        let mut by_lane: BTreeMap<String, (i64, String, [u32; 5], u32)> = BTreeMap::new();
        let mut phases: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut triggers: BTreeMap<String, [u32; 3]> = BTreeMap::new();
        let (mut foot_passes, mut foot_clears_flee, mut foot_clears_own, mut foot_clears_other) = (0u32, 0u32, 0u32, 0u32);
        for (id, r) in recs.iter() {
            let ln = lane_of(r);
            phases.entry(ln.clone()).or_default().push(format!("#{id} {}/{}", r.hostile_phase.map_or("-".to_string(), |p| p.to_string()), r.living_phase.map_or("-".to_string(), |p| p.to_string())));
            let tg = triggers.entry(ln.clone()).or_insert([0; 3]);
            for c in &r.clears {
                if c.contains("[panic trigger: NEAREST_HOSTILE") {
                    tg[0] += 1;
                } else if c.contains("[panic trigger: HURT_BY") {
                    tg[1] += 1;
                } else if c.contains("[panic trigger: none held]") {
                    tg[2] += 1;
                }
            }
            let e = by_lane.entry(ln).or_insert((0, String::new(), [0; 5], 0));
            e.3 += 1;
            for (a, b, place) in &r.arm_pairs {
                let iv = b.unwrap_or(o.ticks as i64) - a;
                if iv > e.0 {
                    e.0 = iv;
                    e.1 = format!("#{id} t {a}..{} ({place})", b.map_or("the end".to_string(), |b| b.to_string()));
                }
            }
            for c in &r.clears {
                let k = if c.contains("a flee point") {
                    0
                } else if c.contains("(the bed") {
                    1
                } else if c.contains("a stroll point") {
                    2
                } else if c.contains("a nap (") {
                    3
                } else {
                    4
                };
                if !c.contains("a reload") {
                    e.2[k] += 1;
                }
                if c.contains("at [-4, 2, 7]") {
                    match k {
                        0 => foot_clears_flee += 1,
                        1 => foot_clears_own += 1,
                        _ => foot_clears_other += 1,
                    }
                }
            }
            foot_passes += r.foot_passes;
        }
        let row = |(ln, (iv, at, k, n)): (&String, &(i64, String, [u32; 5], u32))| -> String {
            format!(
                "{ln} ({n} villagers): the worst arm -> clear interval {iv} gt ({at}){}; clears: a flee path {}, its own path home {}, a stroll path {}, a lie-down {}, other {}",
                if *iv >= 1000 { " (over 1,000: FAIL)" } else { "" },
                k[0], k[1], k[2], k[3], k[4]
            )
        };
        let lane7: Vec<String> = by_lane.iter().filter(|(l, _)| l.as_str() == "lane x 7").map(row).collect();
        let others: Vec<String> = by_lane.iter().filter(|(l, _)| l.as_str() != "lane x 7").map(row).collect();
        s.push_str(&format!(
            "the clear-to-clear intervals per lane (ironspec 16:37 (3), G1 16:37: per villager the arm -> clear interval, bar < 1,000; clears by 26.3 mechanism; LANE 7 SEPARATELY, the binding lane with no structural clearer): LANE 7: {}; the others: {}\n",
            if lane7.is_empty() { "none".to_string() } else { lane7.join("") },
            others.join("; ")
        ));
        s.push_str(&format!(
            "the lift foot (-4,2,7) as a clearer (G1 16:37 (2): its 3-5-tick pass clears by cadence): {foot_passes} rider passes, clears there: {foot_clears_own} by the rider's own path home, {foot_clears_flee} by a flee path, {foot_clears_other} other; the per-pass own-path clear rate {}\n",
            if foot_passes == 0 { "-".to_string() } else { format!("{:.3}", foot_clears_own as f64 / foot_passes as f64) }
        ));
        s.push_str(&format!(
            "the panic clears' triggers per lane (7b90c3f1 / G1 16:51: NEAREST_HOSTILE held, HURT_BY held, none) and each villager's scan phases mod 20, VILLAGER_HOSTILES / NEAREST_LIVING_ENTITIES (G1 16:47, ironspec 16:50 (3), tick-exact 17:03 (iii)): {}\n",
            triggers
                .iter()
                .map(|(l, t)| format!("{l}: NEAREST_HOSTILE {}, HURT_BY {}, none {}; phases {}", t[0], t[1], t[2], phases.get(l).map_or(String::new(), |v| v.join(" "))))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        // (ironspec 17:10 (3)) per lane and cell: panic ticks, stroll attempts, draws with a target, clears
        for l in LANES {
            let rows: Vec<String> = panic_cells
                .iter()
                .filter(|((x, _, _), _)| *x == l)
                .map(|((_, y, z), v)| format!("({y},{z}): panic {} attempts {} targets {} clears {}", v[0], v[1], v[2], v[3]))
                .collect();
            s.push_str(&format!(
                "the panic periods by cell, lane x {l} (ironspec 17:10 (3): PANIC ticks, the stroll's attempts (WALK_TARGET absent, one draw per tick), the draws that set a target, the clears in PANIC): {}\n",
                if rows.is_empty() { "none".to_string() } else { rows.join("; ") }
            ));
        }
    }
    // (ironspec 16:08, G1 16:05) the longest continuous pathless dwell at night by position class: per
    // class, the stamp's oldest age at any REST tick over the villagers, against the 1,200 release
    {
        let names = ["the lane queue", "the station hold / pocket", "the split row", "the lift", "the queue gate (G2)", "the way-in / stop", "other"];
        let mut row: Vec<String> = Vec::new();
        let mut worst_all = 0i64;
        for (k, n) in names.iter().enumerate() {
            let best = recs.iter().map(|(id, r)| (r.class_peak[k], *id)).max_by_key(|(p, _)| p.0);
            if let Some(((age, t, c), id)) = best
                && age > 0
            {
                worst_all = worst_all.max(age);
                row.push(format!("{n} {age} gt (#{id} t {t} at ({},{},{}))", c[0], c[1], c[2]));
            } else {
                row.push(format!("{n} -"));
            }
        }
        s.push_str(&format!(
            "the stamp's oldest age at any REST tick by position class (ironspec 16:08 / G1 16:05: the longest continuous pathless dwell at night, against the 1,200 release; bar < 1,000): {} ({})\n",
            row.join("; "),
            if worst_all < 1000 { "PASS" } else { "FAIL" }
        ));
    }
    s.push_str(&format!(
        "the bar: the largest stamp age at a calm REST tick with no walk target {worst} (<= 1,000: {}); calm REST ticks over 1,000: {over}; HOME lost by {lost} of {} villagers{}{}\n",
        if worst <= 1000 { "PASS" } else { "FAIL" },
        recs.len(),
        if home_erased_id.is_some() { " (the farm-stop control's planted erasure excluded: its own sample, G1 10:47 O6)" } else { "" },
        if lost == 0 { " (PASS)" } else { " (FAIL)" }
    ));
    if o.napper {
        let outside: Vec<String> = a6_entries.iter().filter(|(_, _, sp)| !(0.11..=0.21).contains(sp)).map(|(t, v, sp)| format!("{} t {t} at {sp:.3}", recs.get(v).map_or(format!("#{v}"), |r| r.label.split(' ').next().unwrap_or("").to_string()))).collect();
        s.push_str(&format!(
            "  A6's entries (ironspec 19:31 PT (5): an entry outside [0.11, 0.21] is (C)'s failure, its own class): {} entries, speeds {}; outside the band {}{}\n",
            a6_entries.len(),
            if a6_entries.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", a6_entries.iter().map(|x| x.2).fold(f64::INFINITY, f64::min), a6_entries.iter().map(|x| x.2).fold(0.0, f64::max)) },
            outside.len(),
            if outside.is_empty() { String::new() } else { format!(" ({})", outside.join("; ")) }
        ));
        s.push_str(&format!(
            "  the way-in's fall (C pending) (ironspec 19:57 PT (2a): its own class until b8949c96's descent is laid, then 0): stamp peaks there {}{}; HOME losses there {}{}\n",
            fall_peaks.len(),
            if fall_peaks.is_empty() { String::new() } else { format!(" ({})", fall_peaks.join("; ")) },
            fall_lost.len(),
            if fall_lost.is_empty() { String::new() } else { format!(" ({})", fall_lost.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }
        ));
    }
    // (ironspec 16:55) each lane's villagers (the lane each last rode): the largest calm-REST stamp
    // against 1,000 (lane -1's seats lost F3's z 9 floors, own-cell clearers there)
    {
        let rows: Vec<String> = LANES
            .iter()
            .map(|x| {
                let vs: Vec<&Rec> = recs.values().filter(|r| r.lane == Some(*x)).collect();
                let worst = vs.iter().filter_map(|r| r.peak.map(|p| (p.0, r.label.split(' ').next().unwrap_or("").to_string()))).max_by_key(|p| p.0);
                format!("lane x {x}: {} villagers, the largest {}", vs.len(), worst.map_or("none".to_string(), |(a, l)| format!("{a} ({l}){}", if a <= 1000 { " PASS" } else { " FAIL" })))
            })
            .collect();
        s.push_str(&format!("the largest calm-REST stamp by lane (ironspec 16:55, <= 1,000): {}\n", rows.join("; ")));
    }
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
    // (G1 14:11, CHANGES 2 on ed4978c0c) the window overlaps (bar 0) and each napper's K writes on its
    // own lane's K cell (the --nap-lane-control moves nap_lane inside every window: still all its own)
    {
        let mut mism: Vec<String> = Vec::new();
        for (i, (t, k, kx)) in ctl.k_windows.iter().enumerate() {
            let next = ctl.k_windows.get(i + 1).map_or(i64::MAX, |w| w.0);
            // its window's writes: ticks t + 93 .. t + 156 + delta (before the next sample)
            for (ft, fx, on) in ctl.k_fired.iter().filter(|f| f.0 > *t && f.0 <= (*t + 160 + o.hold_pulse_delta).min(next + 93)) {
                if fx != kx {
                    mism.push(format!("lane x {}'s window (sample t {t}): K({fx}) {} at t {ft}, not its K({kx})", LANES[*k], if *on { "on" } else { "off" }));
                }
            }
        }
        s.push_str(&format!(
            "the nappers' windows (G1 14:11, F4(a): K windows disjoint, one napper; an H2 sample with an earlier window pending is a failure): {} overlaps ({}){}; each napper's K writes on its own lane's K cell{}: {} writes off its cell ({})\n",
            ctl.overlaps.len(),
            if ctl.overlaps.is_empty() { "PASS" } else { "FAIL" },
            if ctl.overlaps.is_empty() { String::new() } else { format!(": {}", ctl.overlaps.join("; ")) },
            if o.nap_lane_control { " (--nap-lane-control: nap_lane moved inside every window)" } else { "" },
            mism.len(),
            if mism.is_empty() { "PASS".to_string() } else { format!("FAIL: {}", mism.join("; ")) }
        ));
    }
    // (ironfarm 12:47, --lip-dump) every lip rest's lane over its 60 gt before, to the file
    if let Some((tid, tf, tt)) = o.villager_trace {
        s.push_str(&format!("villager #{tid}'s trace over t {tf}..{tt} (G1 12:21; design frame): {}\n", if vtrace_rows.is_empty() { "no tick (not alive in the window)".to_string() } else { vtrace_rows.join(" | ") }));
    }
    if let Some(p) = &o.lip_dump {
        std::fs::write(p, format!("{}\n", lip_dumps.join("\n"))).map_err(|e| format!("--lip-dump {}: {e}", p.display()))?;
        s.push_str(&format!("the lip dumps (ironfarm 12:47): {} rests dumped to {}\n", lip_dumps.len(), p.display()));
    }
    // (ironspec 12:41 (4) / 12:51, R6 returning, the hold's Delta) each returning napper's pass of each
    // split cell up to its own K (the K cells (x-2,25,9) and the J cells (x,25,9) west of its K): the
    // gap to the batch cart before it and after it at that cell; the least per cell over the run
    {
        let mut least_before: BTreeMap<[i32; 3], (i64, String)> = BTreeMap::new();
        let mut least_after: BTreeMap<[i32; 3], (i64, String)> = BTreeMap::new();
        let mut n = 0usize;
        for p in junction_passes.iter().filter(|p| p.2 == "returning napper") {
            let Some(k) = p.3 else { continue };
            let own_k = LANES[k] - 2;
            // the cells up to its own K: K's and J's with x <= its K's
            if p.1[0] > own_k || p.1[1] != 25 || p.1[2] != 9 {
                continue;
            }
            n += 1;
            // (any cart not a napper: a batch cart, ridden or empty)
            let before = junction_passes.iter().filter(|q| q.1 == p.1 && (q.2 == "batch" || q.2 == "empty") && q.0 < p.0).map(|q| p.0 - q.0).min();
            let after = junction_passes.iter().filter(|q| q.1 == p.1 && (q.2 == "batch" || q.2 == "empty") && q.0 > p.0).map(|q| q.0 - p.0).min();
            if let Some(g) = before {
                let e = least_before.entry(p.1).or_insert((i64::MAX, String::new()));
                if g < e.0 {
                    *e = (g, format!("t {} lane x {}", p.0, LANES[k]));
                }
            }
            if let Some(g) = after {
                let e = least_after.entry(p.1).or_insert((i64::MAX, String::new()));
                if g < e.0 {
                    *e = (g, format!("t {} lane x {}", p.0, LANES[k]));
                }
            }
        }
        let fmt = |m: &BTreeMap<[i32; 3], (i64, String)>| if m.is_empty() { "none".to_string() } else { m.iter().map(|(c, (g, w))| format!("({},{},{}) {g} gt ({w})", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ") };
        s.push_str(&format!(
            "the returning napper's windows on the split row (ironspec 12:41 (4) / 12:51, R6 returning: each returning napper's entry into each K / J cell up to its own K, the batch cart's entry before it and after it at that cell; the hold pulse's Delta {} gt; rider doPush onto carts: {}; information): {n} passes; the least gap to the batch cart before: {}; after: {}\n",
            o.hold_pulse_delta,
            if o.rider_push_off { "off (a counterfactual, no game twin)" } else { "on (the game)" },
            fmt(&least_before),
            fmt(&least_after)
        ));
    }
    // (ironfarm 13:36, mobfarm::lipclass) every lip arrival's class per lane, and the class of the
    // arrival before each off-rail rest in a drop region (ironspec 13:25 (4)'s per-night classification)
    {
        let mut per_lane: Vec<String> = Vec::new();
        for k in 0..3 {
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for a in &lipc_arrivals[k] {
                *counts.entry(a.class.name()).or_default() += 1;
            }
            let odd: Vec<String> = lipc_arrivals[k]
                .iter()
                .filter(|a| !matches!(a.class, crate::lipclass::LipClass::Clear))
                .map(|a| format!("t {} cart #{} {} (others in the slots {}, gap {}, step {:.3}, occupants {:?})", a.tick, a.cart, a.class.name(), a.others_in_slots, a.gap.map_or("-".to_string(), |g| g.to_string()), a.step, a.occupants))
                .collect();
            per_lane.push(format!(
                "lane x {}: {} arrivals ({}){}",
                LANES[k],
                lipc_arrivals[k].len(),
                counts.iter().map(|(c, n)| format!("{c} {n}")).collect::<Vec<_>>().join(", "),
                if odd.is_empty() { String::new() } else { format!(": {}", odd.join("; ")) }
            ));
        }
        let rests: Vec<String> = off_rest_events
            .iter()
            .map(|(t, k, cid)| {
                let before = lipc_arrivals[*k].iter().rev().find(|a| a.tick <= *t);
                format!("cart #{cid} at rest in lane x {}'s drop region from t {t}: the arrival before it {}", LANES[*k], before.map_or("none".to_string(), |a| format!("t {} cart #{} {}{}", a.tick, a.cart, a.class.name(), if a.class.explains_a_stall() { " (explains a stall)" } else { "" })))
            })
            .collect();
        s.push_str(&format!(
            "the lip arrivals' classes (ironfarm 13:36, mobfarm::lipclass 9a3c61c2d: a 9th cart / rollback / a gap under 23 / UNCLASSIFIED / slow entry / clear, in that order; information): {}; the arrival before each off-rail rest: {}\n",
            per_lane.join(" | "),
            if rests.is_empty() { "no rest".to_string() } else { rests.join("; ") }
        ));
    }
    // (day-1 13:43, ironspec 13:47) the per-batch passage table: each batch's (REL) pocket releases'
    // carts (slot 1 / 2 / 3 = lanes -1 / 3 / 7), each one's first and LAST tick in J(-1) / J(3) / J(7)
    // relative to REL, split by the batch's pick; design figures = the least L's over seeds x positions
    {
        let mut exits: Vec<(i64, u32, usize)> = Vec::new();
        for (cid, ents) in cart_entries.iter() {
            for p in ents.windows(2) {
                if let Some(k) = LANES.iter().position(|x| p[0].1 == [*x, 0, 0]) && p[1].1 != p[0].1 {
                    exits.push((p[1].0, *cid, k));
                }
            }
        }
        // per (pick lane or none, slot, J): (min first, max first, min last, max last)
        let mut table: BTreeMap<(i32, usize, i32), (i64, i64, i64, i64)> = BTreeMap::new();
        for (rel, pick) in ctl.rt_batches.iter() {
            for (t, cid, slot) in exits.iter().filter(|(t, _, _)| *t >= *rel && *t <= *rel + 40) {
                let Some(ents) = cart_entries.get(cid) else { continue };
                for jx in [-1, 3, 7] {
                    let cellj = [jx, 25, 9];
                    let Some(i) = ents.iter().position(|(et, c)| *et > *t && *c == cellj) else { continue };
                    let first = ents[i].0 - rel;
                    let last = ents.get(i + 1).map_or(first, |n| n.0 - 1 - rel);
                    let key = (pick.map_or(-9, |p| LANES[p]), *slot + 1, jx);
                    let e = table.entry(key).or_insert((i64::MAX, i64::MIN, i64::MAX, i64::MIN));
                    *e = (e.0.min(first), e.1.max(first), e.2.min(last), e.3.max(last));
                }
            }
        }
        let rows: Vec<String> = table
            .iter()
            .map(|((p, slot, jx), (f0, f1, l0, l1))| format!("{} slot {slot} J({jx}): first {f0}..{f1}, last {l0}..{l1}", if *p == -9 { "no pick".to_string() } else { format!("lane x {p}'s pick") }))
            .collect();
        s.push_str(&format!(
            "the per-batch passage table (day-1 13:43 / ironspec 13:47: each batch's pocket-released carts (slot 1 / 2 / 3 = lanes -1 / 3 / 7), first and LAST tick in J(-1) / J(3) / J(7) relative to REL, by the batch's pick; information): {} batches; {}\n",
            ctl.rt_batches.len(),
            if rows.is_empty() { "none".to_string() } else { rows.join("; ") }
        ));
        // (day-1's G11 rev 15:42, ironspec 15:37 (2): delta7 = the measured maximum over the set and
        // seeds of the lane-7 pick's napper's last tick in J(-1)'s cell minus lane 3's real D(3) press,
        // + 3) the measurement per lane-7-pick batch
        let mut d7: Vec<(i64, i64)> = Vec::new();
        for (rel, pick) in ctl.rt_batches.iter() {
            if *pick != Some(2) {
                continue;
            }
            let Some(press) = ctl.d3_presses.iter().copied().find(|t| *t >= rel + 85 && *t <= rel + 200) else { continue };
            for (t, cid, slot) in exits.iter().filter(|(t, _, slot)| *t >= *rel && *t <= *rel + 40 && *slot == 2) {
                let Some(ents) = cart_entries.get(cid) else { continue };
                let Some(i) = ents.iter().position(|(et, c)| *et > *t && *c == [-1, 25, 9]) else { continue };
                let last = ents.get(i + 1).map_or(ents[i].0, |n| n.0 - 1);
                d7.push((*rel, last - press));
            }
        }
        if !d7.is_empty() {
            let mx = d7.iter().map(|x| x.1).max().unwrap_or(0);
            s.push_str(&format!(
                "delta7's measurement (day-1's G11 rev 15:42: the lane-7 pick's napper's last tick in J(-1)'s cell minus lane 3's real D(3) press, per lane-7-pick batch): {} batches, least {}, most {mx}; the design's delta7 = the most + 3 = {} (this run used {}){}\n",
                d7.len(),
                d7.iter().map(|x| x.1).min().unwrap_or(0),
                mx + 3,
                ctl.delta7,
                if ctl.a_stop && ctl.delta7 < mx + 3 { " (FAIL: the run's delta7 below the measured most + 3)" } else { "" }
            ));
        }
    }
    // (day-1's canonical gates rev 15:42, ironspec 15:37 (6): THE STANDING CHECK) the gates this run's
    // emulation implements against day-1's list, each with its delay as emulated; a missing gate or a
    // NOT-BOUGHT gate present FAILS the run (with the (A) stop's design controller); RE_ARMED's setters
    {
        let ds = ctl.a_stop;
        let g = |on: bool, txt: &str| format!("{} {txt}", if on { "IMPLEMENTED" } else { "MISSING" });
        let gates = [
            g(true, "G1 RS_ready: rider-keyed (a ridden cart on R's detector (4,-28,4)), + d_RS 10, reset at the queue release"),
            g(ds, "G2 R1: REST_l AND NOT DAY AND RS_ready AND the held napper's stale hold elapsed, + the dwell 6"),
            g(ds, "G3 IN-STOP_l: set at a lane-l queue release, reset at R's ridden press"),
            g(ds, "G4 the slope taps: REL + 43 / 41 / 33 AND IN-STOP_l, the tap lever (1,-29,2) held 10"),
            g(ds, "G5 the hold's at-most-one gate on the RETURN bits"),
            g(ds, "G6 REL_l = D_l AND string_l (a ridden cart on R_l), R = D + 34 (the feed's input at D + 26 with the stack's 8)"),
            g(ds, "G7 AWAY_l: set at the pick, reset at string_l's fall after the return; a pick skipped while set"),
            g(ds, "G8 F1: REL = holds full (a cart on each R_1) AND RE_ARMED; G8 / G8a (ironspec 17:28 platform, day-1's rev 17:30, the cheaper C): RE_ARMED's set = D(7)'s press (real, or a lane-7 pick's emulated D(7)) AND SEEN3 (set by D(3), real or G11-emulated, reset at REL), reset at REL; the request + 6"),
            g(true, "G9 every REL-keyed timing off the gated REL"),
            g(true, "G10 the ring F2: D-keyed, J(7) on by default, no timed re-arm"),
            g(ctl.ring_timed, "G11 the no-string outbound: HOLD_3 / HOLD_7 reset at REL + L + k, latch writes + 2, J switch + 4; lane 7's emulated D(7) at lane 3's real D(3) + delta7"),
            g(ds, "G12 RETURN_x: set at the launching tap, cleared by string_x's rise or T_ret 450"),
            g(ds, "G13 K's sampled lane: the RETURN bits latched at T_d (the napper's first end-of-tick in (9,0,7))"),
            g(ds, "G14 K(x)'s window [T_d + tau_K - 12, T_d + tau_K + 8] (writes + 4 / + 6), the west J's [T_d + tau_J - 12, T_d + tau_J + 8] (writes + 6 / + 4); tau 94 / 109 / 125, 102, 117, s 4"),
            g(true, "G15 the keyed hold pulse: the H2 sample and its 12-gt pulse at delta"),
            g(ds, "G16 SLEPT: toggled by each bed-head observer pulse after ARM, reset at the release; drives OPEN only"),
            g(ds, "G17 SLEPT_ONCE: set by the first pulse after ARM, reset at ARM"),
            g(ds, "G18 ARMED70: ARM + 70, its set gated by NOT released"),
            g(ds, "G19 OPEN: set by the edge of (ARMED70 AND SLEPT), reset at the release"),
            g(ds, "G20 the shutter = NOT OPEN (torch 2): close = OPEN's fall + 2; the park's release = OPEN's fall + 6"),
            g(ds, "G21 the release (ironspec 16:00 / G1 16:02): P_STRING continuous from the latest observer pulse L through L + 12, with PARKED (ARM + 52), by L + 50; one-shot per visit; PARK = RLS + 4 (ironspec 17:35: p = 4 PROVISIONAL); OPEN set = A70 AND SLEPT_ONCE AND NOT P_STRING, reset = RLS OR P_STRING"),
            g(ds, "G22 P_STRING at (-2,-26,3): a villager on foot over P"),
        ];
        let missing = gates.iter().filter(|x| x.starts_with("MISSING")).count();
        let not_bought: Vec<&str> = [
            (ctl.evidence_keyed, "evidence strings on the split row"),
            (!ds && ctl.wayin, "the any-cart (detector) RS_ready"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| *n)
        .collect();
        s.push_str(&format!(
            "the gate list (day-1's canonical list rev 15:42, https://artifacts.internalmeta.com/a/2801090903621150 ; ironspec 15:37 (6): the STANDING CHECK, a missing gate or a NOT-BOUGHT gate present FAILS with the (A) stop's design controller): {} of 22 implemented ({}); NOT-BOUGHT present: {}; RE_ARMED's setters: {} ({}){}\n",
            22 - missing,
            gates.join("; "),
            if not_bought.is_empty() { "none".to_string() } else { not_bought.join(", ") },
            ctl.re_armed_setters.len(),
            ctl.re_armed_setters.iter().take(6).cloned().collect::<Vec<_>>().join(", "),
            if ds { if missing == 0 && not_bought.is_empty() { " (PASS)".to_string() } else { " (FAIL)".to_string() } } else { " (information: today's controller as emulated, no (A) stop composed)".to_string() }
        ));
        {
            s.push_str(&format!(
                "the design controller's F4(a) (G12–G14): {} windows keyed at T_d{}; faults (RETURN timeouts, T_d with 0 or 2+ bits, the hold's pulse withheld by G5): {}{}\n",
                ctl.f4_windows.len(),
                if ctl.f4_windows.is_empty() { String::new() } else { format!(": {}", ctl.f4_windows.iter().take(8).cloned().collect::<Vec<_>>().join(" | ")) },
                ctl.f4_faults.len(),
                if ctl.f4_faults.is_empty() { String::new() } else { format!(": {}", ctl.f4_faults.iter().take(8).cloned().collect::<Vec<_>>().join(" | ")) }
            ));
            s.push_str(&format!("the rider-keyed RS_ready (G1): R's detector pressed by an EMPTY cart {} times (none set RS_ready)\n", ctl.rdet_empty_presses));
            // (G1 16:00, night check 4, bar 0) every awake HOME-holder within 16 of the bed head while occupied
            let mut ex_by: BTreeMap<u32, (i64, i64, f64)> = BTreeMap::new();
            for (t, v, d) in &ctl.exposure_16 {
                let e = ex_by.entry(*v).or_insert((*t, *t, *d));
                e.1 = *t;
                e.2 = e.2.min(*d);
            }
            s.push_str(&format!(
                "the 16-ball exposure (G1 16:00 / night check 4, tick-exact 16:03: ValidateNearbyPoi erases HOME of every awake HOME-holder within 16 of the bed head's centre while the bed is occupied; bar 0): {} villager-ticks, {} villagers{} ({})\n",
                ctl.exposure_16.len(),
                ex_by.len(),
                if ex_by.is_empty() { String::new() } else { format!(": {}", ex_by.iter().take(8).map(|(v, (a, b, d))| format!("#{v} t {a}..{b} (the least distance {d:.2})")).collect::<Vec<_>>().join(", ")) },
                if ctl.exposure_16.is_empty() { "PASS" } else { "FAIL" }
            ));
            s.push_str(&format!(
                "the least distance of an awake HOME-holder to the occupied bed's head (G1 16:06 (a): the held napper at the queue seat, 18.9 by geometry; bar > 16): {}\n",
                if ctl.exposure_least.0 == 0.0 { "none (the bed never occupied with an awake holder present)".to_string() } else { format!("{:.3} (t {}, #{}){}", ctl.exposure_least.0, ctl.exposure_least.1, ctl.exposure_least.2, if ctl.exposure_least.0 > 16.0 { " (PASS)" } else { " (FAIL)" }) }
            ));
            s.push_str(&format!("the (A) stop's releases (ironspec 16:00 / G1 16:02, sustained presence; G1 16:04: the close at the wake press and the release per visit): {} ({})\n", ctl.a_rls_log.len(), ctl.a_rls_log.iter().take(8).cloned().collect::<Vec<_>>().join(" | ")));
            s.push_str(&format!(
                "the park rest against PARKED's timer (G1 17:33: PARKED = ARM + 52 is a timer; a cart resting after it is held unreleased; the margin per visit, bar > 0): {}{}\n",
                if ctl.a_park_log.is_empty() { "no visit".to_string() } else { ctl.a_park_log.join(" | ") },
                if ctl.a_park_log.iter().any(|l| l.contains("margin -") || l.ends_with("margin 0")) { " (FAIL)" } else { " (PASS)" }
            ));
            let gaps: Vec<i64> = ctl.a_so_gaps.iter().copied().filter(|g| *g != i64::MAX).collect();
            s.push_str(&format!(
                "the stale P_STRING check (G1 15:40: per visit, P_STRING's last release (its last press + ~10, the string's re-check) to SLEPT_ONCE's set, bar >= 2): {} visits, {} with an earlier press; the least gap {}{}\n",
                ctl.a_so_gaps.len(),
                gaps.len(),
                gaps.iter().min().map_or("none".to_string(), |g| format!("{g} gt")),
                if gaps.iter().any(|g| *g < 2) { " (FAIL)" } else { " (PASS)" }
            ));
            s.push_str(&format!("the stand-ins' update path (G1 16:18 (1), 26.3 LeverBlock.updateNeighbours 133-139): every commanded lever write updates the lever's neighbours AND its attached block's neighbours ({} such writes this run): the (A) stop's F output (-5,-26,7) (floor: attached (-5,-27,7)), the park drive (3,-27,3) (wall east: attached the conductor (2,-27,3)), the slope tap (1,-29,2) (floor: attached (1,-30,2)), the R line's (x+2,17,1), the hold input (7,-1,8), the junction levers (x,26,9); the trapdoor and redstone-block stand-ins are set with the full update flags (a block's own neighbours: what a placement does)\n", LEVER_FULL_UPDATES.with(|n| *n.borrow())));
            s.push_str("the design controller's propagation delays as emulated (day-1's list rev 15:42; DESIGNED values owed to their benches): G1 a ridden press -> RS_ready + 10, its reset the queue release + 2; G2 the AND -> the release + 10 (NOT-ready's measured 4 + the dwell 6); G3 at the release / R's ridden press; G4 the tap lever at REL + phase + 2 (DAY-1's slope repeater + 2 in the world), held 10; G6 D -> R powered + 34 (the feed's input at + 26, the stack's measured 8); G7 string_l's fall -> AWAY reset + 14 (the string's ~10 + 4); G8 the gate -> REL + 6; G10 / G11 a D press -> the latch + 2, J's switch + 4; G14 K's writes at the window's start + 4 / end + 6, a held J's at + 6 / + 4; G16 a bed change -> SLEPT toggled + 6 (the observer 2 + 4); G17 -> SLEPT_ONCE + 4; G18 ARM -> ARMED70 + 70; G19 the AND's edge -> OPEN + 6; G20 OPEN -> the window open + 2, the release pulse -> close + 2, -> the park's release + 6; G21 P_STRING AND SLEPT_ONCE -> the release pulse + 6; G22 P_STRING released 10 gt after the box leaves\n");
        }
    }
    if !positions_log.is_empty() {
        s.push_str(&format!("the carts' positions at --positions-at (G1 15:37, the lane queues' packing; design frame, 6 dp): {}\n", positions_log.join("; ")));
    }
    if !integrity.is_empty() {
        s.push_str(&format!(
            "the fold's integrity (G1 16:18 (2): 58f770cc's `# integrity:` line read from --integrity-from at its move, each declared cell's shape judged every tick): {} cells ({}); {} ticks off their shape{} ({})\n",
            integrity.len(),
            integrity.iter().map(|(c, sh, _)| format!("({},{},{})={sh}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", "),
            integrity_bad.len(),
            if integrity_bad.is_empty() { String::new() } else { format!(": {}", integrity_bad.iter().take(6).cloned().collect::<Vec<_>>().join(" | ")) },
            if integrity_bad.is_empty() { "PASS" } else { "FAIL" }
        ));
    }
    s.push_str(&format!(
        "the queue tail's worst z (G1 15:55: the farthest-up z of any cart on each lane's slope cells (x, 17..22, 1..6) over the night, against V (x,22,6)'s far edge z 7.0): lane x -1 {:.4} (margin {:.4}), lane x 3 {:.4} (margin {:.4}), lane x 7 {:.4} (margin {:.4})\n",
        ctl.tail_worst[0],
        7.0 - ctl.tail_worst[0],
        ctl.tail_worst[1],
        7.0 - ctl.tail_worst[1],
        ctl.tail_worst[2],
        7.0 - ctl.tail_worst[2]
    ));
    // (day-1 13:43, --ring-timed) the no-string ring's margins per pick: HOLD_3 (lane -1's pick: lane 3's
    // cart's first tick in J(3) - (reset + 4)), HOLD_7 (the next cart's first tick in J(7) - (reset +
    // 4)), lane 7's pick's re-arm (the next batch's first cart at J(-1) - (the timed D(7) + 4)); J(-1)
    // re-armed at every batch's end; bars >= 10, >= 10, printed, every batch
    if o.ring_timed.is_some() || ctl.a_stop {
        let first_after = |jx: i32, after: i64| junction_passes.iter().filter(|p| p.1 == [jx, 25, 9] && (p.2 == "batch" || p.2 == "empty") && p.0 > after).map(|p| p.0).min();
        let mut rows: Vec<String> = Vec::new();
        let mut least3: Option<i64> = None;
        let mut least7: Option<i64> = None;
        for (t, jx, rel, pk) in ctl.rt_reset_log.iter() {
            // the napper's own pass of that J ends near its L: the next non-napper cart after REL + L - 10
            let l = match (jx, pk) {
                (3, _) => ctl.ring_l[0],
                (_, 0) => ctl.ring_l[1],
                (_, 1) => ctl.ring_l[2],
                _ => ctl.ring_l[3],
            };
            match first_after(*jx, rel + l - 10) {
                Some(f) => {
                    let m = f - (t + 4);
                    rows.push(format!("REL {rel} lane x {}'s pick: HOLD_{jx} reset t {t}, the next cart in J({jx}) t {f}, margin {m}", LANES[*pk]));
                    let slot = if *jx == 3 { &mut least3 } else { &mut least7 };
                    if slot.is_none_or(|x| m < x) {
                        *slot = Some(m);
                    }
                }
                None => rows.push(format!("REL {rel} lane x {}'s pick: HOLD_{jx} reset t {t}, no cart after it in J({jx})", LANES[*pk])),
            }
        }
        let mut rearm_rows: Vec<String> = Vec::new();
        for (rel, p) in ctl.rt_batches.iter().filter(|(_, p)| *p == Some(2)) {
            let td = rel + ctl.ring_l[4] + ctl.ring_k;
            if let Some(f) = first_after(-1, td) {
                rearm_rows.push(format!("REL {rel}: the timed D(7) t {td}, the next batch's first cart at J(-1) t {f}, margin {}", f - (td + 4)));
            }
            let _ = p;
        }
        // J(-1) re-armed at every batch's end: a re-arm event within REL + 50 .. REL + 260 of each
        // batch with time left after it
        let end = o.ticks as i64;
        let unarmed: Vec<i64> = ctl.rt_batches.iter().map(|b| b.0).filter(|r| *r + 300 < end && !ctl.rt_rearm_events.iter().any(|e| *e > r + 50 && *e < r + 260)).collect();
        let ok = least3.is_none_or(|m| m >= 10) && least7.is_none_or(|m| m >= 10) && unarmed.is_empty() && !ctl.rt_reset_log.is_empty();
        s.push_str(&format!(
            "the no-string ring (day-1 13:43, --ring-timed: L's {:?}, k {}): the least HOLD_3 margin {} (bar >= 10), the least HOLD_7 margin {} (bar >= 10); lane 7's picks' re-arm margins: {}; batches with no re-arm event {} ({}); per pick: {} ({})\n",
            ctl.ring_l,
            ctl.ring_k,
            least3.map_or("none".to_string(), |m| m.to_string()),
            least7.map_or("none".to_string(), |m| m.to_string()),
            if rearm_rows.is_empty() { "none".to_string() } else { rearm_rows.join("; ") },
            unarmed.len(),
            if unarmed.is_empty() { "none".to_string() } else { unarmed.iter().map(|r| format!("REL {r}")).collect::<Vec<_>>().join(", ") },
            if rows.is_empty() { "no pick".to_string() } else { rows.join("; ") },
            if ctl.rt_reset_log.is_empty() { "NOT JUDGED (no pick): FAIL" } else if ok { "PASS" } else { "FAIL" }
        ));
    }
    // (ironspec 13:04 (1) / 12:41 (4), R6 per pick) each outbound napper's pass of each J at or east of
    // its own lane's: the margin (b) - (a) - 4, (a) the napper clearing J (its exit from (j,25,9)),
    // (b) the next batch cart entering that J; with the hold pulse's Delta and the push switch named
    {
        let mut rows: Vec<String> = Vec::new();
        let mut least: BTreeMap<i32, (i64, String)> = BTreeMap::new();
        for (t_div, cid) in ctl.divert_log.iter() {
            let Some((_, _, v, k)) = ctl.pick_carts.iter().rev().find(|(pt, pc, _, _)| pc == cid && pt <= t_div) else { continue };
            let Some(ents) = cart_entries.get(cid) else { continue };
            for j in LANES.iter().copied().filter(|j| *j >= LANES[*k]) {
                // the napper's exit from J's cell after its divert
                let exit = ents.windows(2).find(|p| p[0].1 == [j, 25, 9] && p[1].1 != [j, 25, 9] && p[1].0 >= *t_div).map(|p| p[1].0);
                let Some(a) = exit else {
                    rows.push(format!("pick t {t_div} #{v} (lane x {}) at J({j}): no exit seen", LANES[*k]));
                    continue;
                };
                let b = junction_passes.iter().filter(|p| p.1 == [j, 25, 9] && (p.2 == "batch" || p.2 == "empty") && p.0 > a).map(|p| p.0).min();
                match b {
                    Some(b) => {
                        let m = b - a - 4;
                        rows.push(format!("pick t {t_div} #{v} (lane x {}) J({j}): clears {a}, batch {b}, margin {m}", LANES[*k]));
                        let e = least.entry(j).or_insert((i64::MAX, String::new()));
                        if m < e.0 {
                            *e = (m, format!("pick t {t_div} #{v}"));
                        }
                    }
                    None => rows.push(format!("pick t {t_div} #{v} (lane x {}) J({j}): clears {a}, no batch cart after it", LANES[*k])),
                }
            }
        }
        s.push_str(&format!(
            "the outbound napper's R6 margins per pick (ironspec 12:41 (4) / 13:04 (1): (b) - (a) - 4 at each J at or east of its own lane's, (a) its exit from J, (b) the next batch cart's entry; the hold pulse's Delta {} gt; rider doPush onto carts: {}; information): the least per J: {}; {}\n",
            o.hold_pulse_delta,
            if o.rider_push_off { "off (a counterfactual, no game twin)" } else { "on (the game)" },
            if least.is_empty() { "none".to_string() } else { least.iter().map(|(j, (m, w))| format!("J({j}) {m} gt ({w})")).collect::<Vec<_>>().join(", ") },
            if rows.is_empty() { "no pick".to_string() } else { rows.join("; ") }
        ));
        // (ironspec 13:04 (1)) the push switch's own record: a run with it on that skipped nothing
        // measured nothing
        if o.rider_push_off {
            let n = run.world.mob.rider_cart_pushes_skipped;
            s.push_str(&format!("rider doPush onto carts: off (a counterfactual, no game twin; cc767b78's switch): {n} pushes skipped ({})\n", if n > 0 { "PASS" } else { "FAIL: it measured nothing" }));
        }
        // (day-1 12:49 / 12:53, ironspec 12:52 / 12:54) the evidence keying's bars: only the napper
        // presses a held-off J's tripwire (bar 0); each J's settle margin per pick, the next batch
        // cart's entry into J minus the J's hold-off end, bar >= 3
        if o.rel_gate_armed {
            let mx = ctl.rel_delays.iter().map(|d| d.1).max();
            s.push_str(&format!(
                "the REL gate (ironspec 12:54 (2): F1's REL AND J(-1) armed, read from J(-1)'s latch): {} starts held, the longest {} gt{}{}; information\n",
                ctl.rel_delays.len(),
                mx.map_or("-".to_string(), |m| m.to_string()),
                if ctl.rel_delays.is_empty() { String::new() } else { format!(": {}", ctl.rel_delays.iter().map(|(t, d)| format!("requested t {t} held {d} gt")).collect::<Vec<_>>().join(", ")) },
                if ctl.gated_since.is_some() { "; a request still held at the end".to_string() } else { String::new() }
            ));
        }
        if o.evidence_keyed {
            let mut rows: Vec<String> = Vec::new();
            let mut least: Option<(i64, String)> = None;
            for (tr, jx, ncid) in ctl.ev_releases.iter() {
                let b = junction_passes.iter().filter(|p| p.1 == [*jx, 25, 9] && (p.2 == "batch" || p.2 == "empty") && p.0 > *tr).map(|p| p.0).min();
                match b {
                    Some(b) => {
                        let m = b - tr;
                        rows.push(format!("J({jx}) released t {tr} (napper cart #{ncid}): the next batch cart at t {b}, settle margin {m}"));
                        if least.as_ref().is_none_or(|l| m < l.0) {
                            least = Some((m, format!("J({jx}) t {tr}")));
                        }
                    }
                    None => rows.push(format!("J({jx}) released t {tr} (napper cart #{ncid}): no batch cart after it")),
                }
            }
            let napper_presses = ctl.ev_presses.iter().filter(|p| p.3).count();
            let ok = ctl.ev_faults.is_empty() && least.as_ref().is_none_or(|l| l.0 >= 3) && !ctl.ev_releases.is_empty();
            s.push_str(&format!(
                "the evidence-keyed outbound release (day-1 12:49 / 12:53, ironspec 12:52 / 12:54: the tripwires (0,26,9) / (4,26,9) / (8,26,9) ANDed with each J's hold-off, + {} gt): {} presses by the napper, {} by another cart (bar 0){}; the least settle margin {} (bar >= 3); {} ({})\n",
                o.evidence_delay,
                napper_presses,
                ctl.ev_faults.len(),
                if ctl.ev_faults.is_empty() { String::new() } else { format!(": {}", ctl.ev_faults.join("; ")) },
                least.as_ref().map_or("none".to_string(), |(m, w)| format!("{m} gt ({w})")),
                if rows.is_empty() { "no release".to_string() } else { rows.join("; ") },
                if ctl.ev_releases.is_empty() { "NOT JUDGED (no release): FAIL" } else if ok { "PASS" } else { "FAIL" }
            ));
        }
    }
    // (G1 blocking 8 / U11: nothing silently dropped) the run log and the controller's log, each cut
    // at a stated length with the count of the lines not printed
    for l in log.iter().take(200) {
        s.push_str(&format!("  {l}\n"));
    }
    if log.len() > 200 {
        s.push_str(&format!("  ({} more run-log lines not printed)\n", log.len() - 200));
    }
    for l in ctl.log.iter().take(60) {
        s.push_str(&format!("  ctl {l}\n"));
    }
    if ctl.log.len() > 60 {
        s.push_str(&format!("  ({} more controller-log lines not printed)\n", ctl.log.len() - 60));
    }
    // (G1's P10) both logs judged whole, not only their printed heads
    s.push_str(&format!("{}\n{}\n", log_fail_line("run log", &log), log_fail_line("controller log", &ctl.log)));
    // (G1's blocker 2 on 185e9ac47, H2 / H19 / N1) each requested control asserts ITS OWN bar: a bar
    // the control must fail that fails is marked "expected FAIL" (so the exit reads every other
    // bar) and the control prints CAUGHT; one that doesn't is NOT CAUGHT (a FAIL); a control that
    // never acted is NOT EXERCISED (a FAIL); the early bystander control must leave the harm bar
    // passing
    // (G1 N17) each control's failure must be its OWN sample, every other sample on the bar passing
    let mut controls: Vec<(String, &str, bool, bool, bool)> = Vec::new();
    // (the 27th) the controls whose bars print after the first judging (the count, the siding):
    // judged at the report's end
    let mut late_controls: Vec<(String, &str, bool, bool, bool)> = Vec::new();
    // (G1 X23) each of these controls' OWN sample computed, never assumed: the inert control's only
    // change is its own cell (-1,26,9) with no kind mismatch; the gossip control's gossip attempts and
    // off-schedule golems all at its tick (t0 + 1,000 .. + 1,002); the empty-pocket control's locked
    // misses all without a rider event at the emptied pocket
    if o.inert_control {
        let own = watched_kind_bad.is_empty() && !watched_changes.is_empty() && watched_changes.iter().all(|c| c.contains("(-1,26,9)"));
        controls.push(("--inert-control".to_string(), "the declared inert / always-on cells", true, true, own));
    }
    if o.gossip_control {
        let window = |t: i64| (t0 + 1_000..=t0 + 1_002).contains(&t);
        let gossip_ok = golem_tries.iter().filter(|x| x.4 == 5).all(|x| window(x.0));
        let off_ok = golem_tries.iter().filter(|x| x.3.is_some() && x.0.rem_euclid(100) != 0).all(|x| window(x.0));
        let any = golem_tries.iter().any(|x| x.4 == 5 && window(x.0));
        controls.push(("--gossip-control".to_string(), "the summon paths", true, controls_done.iter().any(|c| c.contains("gossip")), any && gossip_ok && off_ok));
    }
    if o.empty_pocket.is_some() {
        let own = direct.stand_misses_locked > 0 && direct.misses_not_empty == 0;
        controls.push(("--empty-pocket".to_string(), "the direct summon check", true, true, own));
    }
    if let Some(v) = o.bystander_control.as_deref() {
        // (N1: the control counts only once its target's first call back is judged; N17: late is
        // caught only when the target's own margin is < 3 and every other napper's passes)
        let target_call = bystander_target.and_then(|(tv, at)| nap_margins.iter().position(|(nv, t, _, _)| *nv == tv && *t >= at));
        let own = target_call.is_some_and(|i| nap_margins[i].2.is_some_and(|m| m < 3) && nap_margins.iter().enumerate().all(|(j, x)| j == i || x.2.is_none_or(|m| m >= 3)));
        controls.push((format!("--bystander-control={v}"), "the bystander stamps (ironspec 02:37", v == "late", target_call.is_some(), own));
    }
    if o.stamp_control {
        let own = stamp_control_rider.is_some_and(|v| stamp_over_ids.iter().all(|i| *i == v));
        controls.push(("--stamp-control".to_string(), "  the runtime stamp bar", true, stamp_control_rider.is_some(), own));
    }
    if o.home_erase_control {
        // (the erased napper: the first napper seen at the stop without HOME)
        // (G1 X8) the erased napper, by id, is the only one seen at the stop without HOME
        let own = home_erased_id.is_some_and(|e| stop_homeless_ids.len() == 1 && stop_homeless_ids.contains(&e));
        controls.push(("--home-erase-control".to_string(), "  the farm-stop bar", true, home_erased, own));
    }
    if o.air_control {
        // (G1 07:28 PT) the hold's own consequence by entity id and column geometry: the held rider
        // and the riders of the carts below it in its column at the hold's start; any other
        // drowning fails the control
        let drowned: Vec<u32> = recs.iter().filter(|(_, r)| r.air_transits.iter().chain(r.air_open.iter()).any(|t| t.1 < 0)).map(|(id, _)| *id).collect();
        let own = air_held.is_some_and(|h| air_control_own(&drowned, h.0, &air_stacked));
        controls.push(("--air-control".to_string(), "  the air-supply bar", true, air_held.is_some(), own));
    }
    if o.rs_ready_early_control {
        // (G1 X22) the control's own sample: the ONE early release is the first release after the
        // control's forced RS_ready set, the previous napper's own R pass missing
        let early: Vec<i64> = (1..ctl.release_log.len())
            .filter(|&k| {
                let (tp, vp) = ctl.release_log[k - 1];
                let t = ctl.release_log[k].0;
                !ctl.rdet_riders.iter().any(|(r, who)| *r > tp && *r + 8 <= t && *who == Some(vp))
            })
            .map(|k| ctl.release_log[k].0)
            .collect();
        let first_after = ctl.rs_early_at.and_then(|a| ctl.release_log.iter().map(|x| x.0).find(|t| *t >= a));
        controls.push(("--rs-ready-early-control".to_string(), "  no R1 release before the previous napper passes R", true, ctl.rs_early_done, first_after.is_some_and(|f| early == vec![f])));
    }
    if o.strand_control {
        controls.push(("--strand-control".to_string(), "  no napper unrouted", true, ctl.strand_done, unrouted_total == 1));
    }
    if o.at_entry_control {
        let own = at_entry_rider.is_some_and(|v| at_entry_over_ids.len() == 1 && at_entry_over_ids.contains(&v));
        controls.push(("--at-entry-control".to_string(), "  the at-entry stamp bar", true, at_entry_rider.is_some(), own));
    }
    // (ironspec 08:26 / 08:41 platform) the lap-late rejoin must PASS the unrouted bar (a held
    // control); acted only if the held napper really missed its first release and rejoined later
    if o.late_return_control {
        // (exercised only if the hold happened, did not time out, and the held napper rejoined late)
        let missed = late_rider.is_some_and(|v| late_rejoin.iter().any(|(rv, _, _)| *rv == v));
        controls.push(("--late-return-control".to_string(), "  no napper unrouted", false, late_hold.is_some() && !late_timed_out && missed, true));
    }
    // (G1 10:24 (c)) the misroute control: two extra arrivals sent into lane -1 must take it to a
    // 9th cart (the count bar failing on lane -1 alone)
    if o.misroute_control {
        // (G1's P13) its own sample: lane -1's first 9th comes at or after the control's first act, and
        // no lane reaches a 9th otherwise
        let act = ctl.misroute_at.first().copied();
        let own = act.is_some_and(|a| lane_count_nine[0].is_some_and(|t| t >= a)) && lane_count_nine[1].is_none() && lane_count_nine[2].is_none();
        late_controls.push(("--misroute-control".to_string(), "the per-lane cart count", true, ctl.misroute_at.len() == 2, own));
    }
    // (G1 10:47 blocking 4's control) the eject on R: PRESENT_l must stay set (a held control on its
    // own line)
    if o.siding_plant_control {
        let own = siding_plant.is_some_and(|(id, _)| siding_batch_ids.len() == 1 && siding_batch_ids.contains(&id));
        late_controls.push(("--siding-plant-control".to_string(), "  the carts on a siding's cells", true, siding_plant.is_some(), own));
    }
    // (G1 11:19 F4 (c)) the napper path's forced positives: with F4 every lane count stays unchanged
    // (HELD on the nappers' misroute bar and on the per-lane count); before F4 they show NOT HELD
    if let Some(mode) = o.vholds_control.as_deref() {
        // (G1's P1 controls) caught on the lip-pass bar by the planted cart alone: every pass row with
        // a hit or a push has the planted cart among its hits
        let own = vhold.is_some_and(|(id, _)| {
            let rows: Vec<&PassRow> = pass_rows.iter().flatten().collect();
            !rows.is_empty() && rows.iter().all(|r| r.hits.iter().any(|h| h.2 == id))
        });
        late_controls.push((format!("--vholds-control {mode}"), "the lip passes clipped or pushed", true, vhold.is_some(), own));
    }
    if o.k_misroute_control {
        // (G1's P6) exercised only if the napper cart really took K(3)'s branch (1,25,9) -> (1,25,8)
        // after the act and the lever write took; HELD only with all three count bars unchanged
        let took = kmis.as_ref().is_some_and(|(t, c, _)| cart_entries.get(c).is_some_and(|v| v.windows(2).any(|p| p[0].1 == [1, 25, 9] && p[1].1 == [1, 25, 8] && p[1].0 >= *t)));
        let ex = kmis.is_some() && kmis_ok && took;
        late_controls.push(("--k-misroute-control".to_string(), "the nappers' carts dropping into a lane not their own", false, ex, true));
        late_controls.push(("--k-misroute-control (the count)".to_string(), "the per-lane cart count", false, ex, true));
        late_controls.push(("--k-misroute-control (the ledger)".to_string(), "the conservation ledger", false, ex, true));
    }
    if o.westbound_control {
        // (G1's P6) exercised only if the reversed cart really ran west on the row (a westbound row
        // names it); HELD only with all three count bars unchanged
        let ran = westbound.is_some_and(|(_, c)| westbound_rows.iter().any(|r| r.contains(&format!("cart #{c} "))));
        let ex = westbound.is_some() && ran;
        late_controls.push(("--westbound-control".to_string(), "the per-lane cart count", false, ex, true));
        late_controls.push(("--westbound-control (the ledger)".to_string(), "the conservation ledger", false, ex, true));
        late_controls.push(("--westbound-control (the misroutes)".to_string(), "the nappers' carts dropping into a lane not their own", false, ex, true));
    }
    if o.eject_control {
        let line = format!(
            "the eject control's own bar (G1 10:47 blocking 4: a napper dismounted on R departs nothing, PRESENT_l stays set): {}",
            match (ctl.eject_done, eject_present_after) {
                (Some((t, k, r)), Some(true)) => format!("napper #{r} dismounted on lane x {}'s R at t {t}; PRESENT_l set 20 gt later (PASS)", LANES[k]),
                (Some((t, k, r)), Some(false)) => format!("napper #{r} dismounted on lane x {}'s R at t {t}; PRESENT_l CLEARED within 20 gt (FAIL)", LANES[k]),
                (Some((t, _, _)), None) => format!("dismounted at t {t}, the run ended within 20 gt: NOT JUDGED: FAIL"),
                (None, _) => "no napper seated on R with PRESENT_l set after t 1,000: NOT JUDGED: FAIL".to_string(),
            }
        );
        s.push_str(&format!("{line}\n"));
        controls.push(("--eject-control".to_string(), "the eject control's own bar", false, ctl.eject_done.is_some() && eject_present_after.is_some(), true));
    }
    // (G1 X35, ironspec 07:57: fault rows) --lift-block of 25 gt or more is the jam positive: the air
    // bar must fail, every drowning a rider of the lift column at the hold's start; a shorter hold is a
    // fault row printed as information (its air verdict stands as printed)
    if let Some((_, nb)) = o.lift_block
        && nb >= 25
    {
        let drowned: Vec<u32> = recs.iter().filter(|(_, r)| r.air_transits.iter().chain(r.air_open.iter()).any(|t| t.1 < 0)).map(|(id, _)| *id).collect();
        let own = !drowned.is_empty() && drowned.iter().all(|d| lift_stack_at_hold.contains(d));
        controls.push((format!("--lift-block (the jam positive, {nb} gt)"), "  the air-supply bar", true, !lift_stack_at_hold.is_empty() || controls_done.iter().any(|c| c.contains("lift-block")), own));
    }
    if o.day_phase_control {
        controls.push(("--day-phase-control".to_string(), "  the DAY stand-in's validation", true, ctl.day_phase_done, ctl.day_phase_at.is_some_and(|t| ctl.day_mismatch_at == vec![t])));
    }
    let s = judge_controls(&s, &controls);
    // (cc767b78) the traced entity's rows
    let s = if let Some((n, a, b)) = o.trace_entity {
        format!("{s}the traced entity e{n} over run ticks {a}..={b} (cc767b78, after each tick): {}\n", if trace_rows.is_empty() { "no row".to_string() } else { trace_rows.join("; ") })
    } else {
        s
    };
    // (G1 07:59 PT) the lift column around the lift-block control, per tick
    // (ironspec 08:26 platform (3)) the chain trace, per tick
    let s = format!("{s}{t1_line}\n");
    // (ironspec 10:04 / 10:06, G1 10:04 (d)) the off-rail rests anywhere, the moving off-rail ticks,
    // the siding's carts by rider; (ironspec 09:44 / 09:48, G1 09:45 (c)) the T shapes per tick
    let s = {
        let mut rows = off_rest_rows.clone();
        rows.extend(off_rest.iter().filter(|(_, (_, _, n))| *n >= 10).map(|(id, (t0, c, n))| format!("cart #{id} at ({},{},{}) from t {t0}, {n} gt (at the end){}", c[0], c[1], c[2], off_rest_snap.get(id).map_or(String::new(), |s| format!(" {s}")))));
        let moving: Vec<String> = off_moving.iter().map(|(c, n)| format!("({},{},{}) {n}", c[0], c[1], c[2])).collect();
        let batch = siding_by.get("batch villager").map_or(0, |(n, _)| *n);
        let stable: Vec<[i32; 3]> = o.stable_ts.clone();
        let bad_t: Vec<String> = t_changes.iter().filter(|(_, p, _, _)| stable.contains(p)).map(|(t, p, a, b)| format!("t {t} ({},{},{}) {a} -> {b}", p[0], p[1], p[2])).collect();
        let mut per_t: BTreeMap<[i32; 3], (usize, Vec<String>)> = BTreeMap::new();
        for (t, p, a, b) in &t_changes {
            let e = per_t.entry(*p).or_default();
            e.0 += 1;
            if e.1.len() < 3 {
                e.1.push(format!("t {t} {a} -> {b}"));
            }
        }
        format!(
            "{s}the per-lane cart count (G1 10:24 (a) / 10:47 blocking 3, the conservation premise under order routing, over D4's region: every cart in the lane's pocket P (x,0,0), on its rails H / R_1 / slopes, in its drop column (x,23..24,6) or at C (x,24,7) / D (x,24,8), every tick; a 9th FAILS): {} ({}){}\n  the tail never past V (G1 10:47 blocking 3: V = (x,22,6), the lane's top slope cell, the queue's 8th place: a cart at rest in the drop column, at C or (G1's P17) at D while V holds a cart FAILS): {} ({})\n  the cascade's seeds (G1 10:24: a cart off its rail in the drop region (x,24,6) / (x,23,6) for 1 gt or more while another cart is at C or D; information, the lip fix's target 0): {}{}\n  the drop region's denominator (ironspec 10:29 (3): per lane the drop passes (the drop cell into the column), the off-rail rests there (>= 10 gt), the C deflections (C west into c3 by any cart but a napper's) and the cascade's seeds, each a rate per pass with its 95% upper bound (Wilson)): {}\n  the C deflections (bar 0, ironspec 10:29 (2): no cart but the napper leaves C westward): {} ({}){}\nthe carts at rest off the rails (ironspec 10:04 / 10:06, G1 10:04 (d): every drop lip and every other cell; no rail in the cart's cell or the one below, its motion under 0.003 a tick for 10 gt or more; bar 0): {} ({}){}\n  the carts off the rails while moving (information: the drop columns' falls), cart-ticks per cell: {}\n  the carts on a siding's cells (x-2,25,9), (x-2,25,8), (x-2,24,7) by rider (ironspec 10:04: a batch villager there FAILS): {} ({})\n  the arrivals at each lane's C (x,24,7) (G1 10:14, information beside ironfarm's bench: speed in b/gt, spacing in gt): {}\nthe rail Ts' shapes per tick (ironspec 09:44 / 09:48, G1 09:45 (c); runtime evidence: every plain rail with 3+ rail neighbours at t0, {} Ts; a change of a declared never-switched T, the design's placement-order-ts {}, FAILS): {} changes in all; the declared Ts changed {} times ({}){}; per T: {}\n",
            (0..3).map(|k| format!("lane x {}: the most {} (first at t {}), ticks by count {}", LANES[k], lane_count_max[k].0, lane_count_max[k].1, lane_count_hist[k].iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; "),
            if lane_count_nine.iter().all(|t| t.is_none()) { "PASS" } else { "FAIL" },
            if lane_count_nine.iter().all(|t| t.is_none()) { String::new() } else { format!(": {}", (0..3).filter_map(|k| lane_count_nine[k].map(|t| format!("lane x {} at 9 from t {t}", LANES[k]))).collect::<Vec<_>>().join("; ")) },
            (0..3).map(|k| format!("lane x {}: {} ticks{}", LANES[k], past_v_ticks[k], past_v_first[k].as_ref().map_or(String::new(), |f| format!(" (the first: {f})")))).collect::<Vec<_>>().join("; "),
            if past_v_ticks.iter().all(|n| *n == 0) { "PASS" } else { "FAIL" },
            {
                let mut all = lip_seeds.clone();
                all.extend((0..3).filter_map(|k| lip_seed_open[k].map(|(t0, a, b)| (k, t0, o.ticks - t0, a, b))));
                let per: Vec<String> = (0..3).map(|k| {
                    let v: Vec<&(usize, i64, i64, u32, u32)> = all.iter().filter(|e| e.0 == k).collect();
                    format!("lane x {}: {} (the longest {} gt)", LANES[k], v.len(), v.iter().map(|e| e.2).max().unwrap_or(0))
                }).collect();
                per.join("; ")
            },
            {
                let mut all = lip_seeds.clone();
                all.extend((0..3).filter_map(|k| lip_seed_open[k].map(|(t0, a, b)| (k, t0, o.ticks - t0, a, b))));
                all.sort_by_key(|e| std::cmp::Reverse(e.2));
                if all.is_empty() { String::new() } else { format!("; the longest: {}", all.iter().take(12).map(|e| format!("lane x {} t {} {} gt: cart #{} off its rail, #{} behind", LANES[e.0], e.1, e.2, e.3, e.4)).collect::<Vec<_>>().join("; ")) }
            },
            {
                // (Wilson's 95% upper bound of k events in n passes)
                let wilson = |k: i64, n: i64| -> f64 {
                    if n == 0 {
                        return 1.0;
                    }
                    let (k, n, z) = (k as f64, n as f64, 1.96f64);
                    let p = k / n;
                    ((p + z * z / (2.0 * n)) + z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / (1.0 + z * z / n)
                };
                let mut seeds_per = [0i64; 3];
                for e in &lip_seeds {
                    seeds_per[e.0] += 1;
                }
                for (k, open) in lip_seed_open.iter().enumerate() {
                    if open.is_some() {
                        seeds_per[k] += 1;
                    }
                }
                (0..3)
                    .map(|k| {
                        let n = drop_passes[k];
                        let r = |e: i64| format!("{e} ({:.4} a pass, <= {:.4})", if n == 0 { 0.0 } else { e as f64 / n as f64 }, wilson(e, n));
                        format!("lane x {}: {n} passes; off-rail rests {}; C deflections {}; cascade seeds {}", LANES[k], r(lip_rests[k]), r(c_deflections[k].len() as i64), r(seeds_per[k]))
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            },
            c_deflections.iter().map(|v| v.len()).sum::<usize>(),
            if c_deflections.iter().all(|v| v.is_empty()) { "PASS" } else { "FAIL" },
            if c_deflections.iter().all(|v| v.is_empty()) { String::new() } else { format!(": {}", (0..3).filter(|k| !c_deflections[*k].is_empty()).map(|k| format!("lane x {}: {}", LANES[k], c_deflections[k].iter().take(10).cloned().collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ")) },
            rows.len(),
            if rows.is_empty() { "PASS" } else { "FAIL" },
            if rows.is_empty() { String::new() } else { format!(": {}", rows.join("; ")) },
            format!(
                "{}; at rest on the station's trapdoor holds H (by design, not barred), cart-ticks per cell: {}",
                if moving.is_empty() { "none".to_string() } else { moving.join(", ") },
                if trapdoor_rests.is_empty() { "none".to_string() } else { trapdoor_rests.iter().map(|(c, n)| format!("({},{},{}) {n}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ") }
            ),
            if siding_by.is_empty() { "none".to_string() } else { siding_by.iter().map(|(k, (n, first))| format!("{k} {n} cart-ticks (the first {})", first.clone().unwrap_or_default())).collect::<Vec<_>>().join("; ") },
            if batch == 0 { "PASS" } else { "FAIL" },
            (0..3)
                .map(|k| {
                    let a = &c_arrivals[k];
                    let q = |mut v: Vec<f64>| -> String {
                        if v.is_empty() {
                            return "-".to_string();
                        }
                        v.sort_by(|x, y| x.total_cmp(y));
                        format!("min {:.3}, median {:.3}, max {:.3}", v[0], v[v.len() / 2], v[v.len() - 1])
                    };
                    let gaps: Vec<f64> = a.windows(2).map(|w| (w[1].0 - w[0].0) as f64).collect();
                    let mut acts: BTreeMap<String, usize> = BTreeMap::new();
                    for r in a {
                        // (the activity alone for the count; each arrival's own row below)
                        *acts.entry(r.2.split(" (").next().unwrap_or("").to_string()).or_default() += 1;
                    }
                    format!(
                        "lane x {}: {} arrivals (the rider: {}); speed {}; y {}; z phase {}; spacing {}; each: {}",
                        LANES[k],
                        a.len(),
                        acts.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "),
                        q(a.iter().map(|r| r.1).collect()),
                        q(a.iter().map(|r| r.5).collect()),
                        q(a.iter().map(|r| r.4).collect()),
                        q(gaps),
                        a.iter().map(|r| format!("t {} #{} speed {:.3} y {:.3} z phase {:.3} rider {}", r.0, r.3, r.1, r.5, r.4, r.2)).collect::<Vec<_>>().join(" | ")
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            t_shapes.len(),
            if stable.is_empty() { "none declared".to_string() } else { stable.iter().map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" ") },
            t_changes.len(),
            bad_t.len(),
            if stable.is_empty() { "NOT JUDGED: no declared T" } else if bad_t.is_empty() { "PASS" } else { "FAIL" },
            if bad_t.is_empty() { String::new() } else { format!(": {}", bad_t.join("; ")) },
            if per_t.is_empty() { "no change".to_string() } else { per_t.iter().map(|(p, (n, first))| format!("({},{},{}) {n} ({})", p[0], p[1], p[2], first.join(", "))).collect::<Vec<_>>().join("; ") }
        )
    };
    // (ironspec 11:08 (2) / 11:37, G1's P1) the lip passes clipped or pushed: each arrival's pass
    // window judged every tick (boxes, at rest or moving, and pushes); a lane with no judged pass is
    // NOT JUDGED
    let s = {
        let wilson = |kk: f64, n: f64| -> f64 { if n == 0.0 { 1.0 } else { let (p, z) = (kk / n, 1.96f64); ((p + z * z / (2.0 * n)) + z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / (1.0 + z * z / n) } };
        let total: usize = pass_rows.iter().map(|v| v.len()).sum();
        let unjudged: Vec<String> = (0..3).filter(|k| pass_judged[*k] == 0).map(|k| format!("x {}", LANES[k])).collect();
        let per: Vec<String> = (0..3)
            .map(|k| {
                let rows = &pass_rows[k];
                let at_rest = rows.iter().filter(|r| r.hits.iter().any(|h| h.0)).count();
                let moving = rows.iter().filter(|r| r.hits.iter().any(|h| !h.0)).count();
                let pushed = rows.iter().filter(|r| !r.pushed.is_empty()).count();
                let shown: Vec<String> = rows.iter().take(6).map(|r| format!("#{} ({}) from t {}: least separation {:.3}; {}{}", r.arrival, r.kind, r.start, r.least, r.hits.iter().map(|h| h.1.clone()).collect::<Vec<_>>().join(", "), if r.pushed.is_empty() { String::new() } else { format!("; pushed {}", r.pushed.join(", ")) })).collect();
                format!(
                    "lane x {}: {} passes judged, {} with a box hit or a push (<= {:.4}): hit by a box at rest {at_rest}, by a moving box {moving}, pushed {pushed}; the least separation over the clean passes {}{}",
                    LANES[k],
                    pass_judged[k],
                    rows.len(),
                    wilson(rows.len() as f64, pass_judged[k] as f64),
                    if pass_clean_least[k].is_finite() { format!("{:.3}", pass_clean_least[k]) } else { "-".to_string() },
                    if shown.is_empty() { String::new() } else { format!(" ({})", shown.join(" | ")) }
                )
            })
            .collect();
        format!(
            "{s}the lip passes clipped or pushed (ironspec 11:08 (2) / 11:37, G1's P1: each arrival's pass window from its entry into C until its box falls below y 23.0 (or 60 gt: the game envelope's 12 off-rail gt plus the pass's ~15, with margin; or it leaves the lane's C / lip / column), every tick: any other cart's or rider's box intersecting the arrival's box inflated 0.015 horizontally (tick-exact 11:33's in-tick step), at rest or moving, every other cart in V's volume (x,22..23,6) during the window (ironspec 12:08 V-EMPTY's 4th sub-bar, G1 12:20: a rollback included), and every push on it; bar 0; a lane with no judged pass NOT JUDGED): {total} ({}): {}\n",
            if !unjudged.is_empty() { format!("NOT JUDGED on {}: FAIL", unjudged.join(", ")) } else if total == 0 { "PASS".to_string() } else { "FAIL".to_string() },
            per.join("; ")
        )
    };
    // (day-1 / ironspec 11:06, constant (4) and the loop transit) per lane: each napper's lip arrival
    // after its leader's (the pair gap), and each batch cart's loop transit, its hold H release (a rest
    // on H ending) to its next lip arrival
    let s = {
        // (ironspec 11:09 (1) / G1's P18) the conservation ledger on the one lane region: per lane its
        // count at the first tick plus the carts entering minus the carts leaving the region, against
        // the region's physical count every 500 gt (ledger - physical), and the ten events before the
        // ledger first reached 9
        let mut out: Vec<String> = Vec::new();
        for k in 0..3 {
            let base = ledger_base[k].unwrap_or(lane_ids_t0[k].len() as i64);
            let mut n = base;
            let mut first9: Option<usize> = None;
            let mut timeline: Vec<String> = Vec::new();
            let mut marks = region_marks[k].iter().peekable();
            let mut idx = 0usize;
            let evs = &ledger[k];
            while let Some((mt, phys)) = marks.next() {
                while idx < evs.len() && evs[idx].0 <= *mt {
                    n += evs[idx].2;
                    if n >= 9 && first9.is_none() {
                        first9 = Some(idx);
                    }
                    idx += 1;
                }
                timeline.push(format!("t {mt} {n} / {phys}{}", if n != *phys { format!(" (ledger - physical {:+})", n - phys) } else { String::new() }));
            }
            while idx < evs.len() {
                n += evs[idx].2;
                if n >= 9 && first9.is_none() {
                    first9 = Some(idx);
                }
                idx += 1;
            }
            let before9 = first9.map(|i| ledger[k][i.saturating_sub(9)..=i].iter().map(|(t, c, d, what)| format!("t {t} #{c} {} {what}", if *d > 0 { "+1" } else { "-1" })).collect::<Vec<_>>().join("; "));
            out.push(format!(
                "lane x {}: at the first tick {base}, in {}, out {}, at the end {n}; {}; {}",
                LANES[k],
                ledger[k].iter().filter(|e| e.2 > 0).count(),
                ledger[k].iter().filter(|e| e.2 < 0).count(),
                if timeline.is_empty() { "no timeline".to_string() } else { format!("every 500 gt, ledger / physical: {}", timeline.join(", ")) },
                before9.map_or("never at 9 (PASS)".to_string(), |b| format!("FIRST AT 9 (FAIL) after: {b}"))
            ));
        }
        let s = format!("{s}the conservation ledger (ironspec 11:09 (1) / G1's P18: per lane the one region's count at the first tick + the carts entering it - the carts leaving it, against its physical count; a 9th FAILS, with the ten events before it): {}\n", out.join(" | "));
        // (onechunk 11:16) the nappers' misroutes, bar 0
        let s = format!(
            "{s}F4(a)'s bars (ironspec 11:33: the returning-napper register; each bar 0): (i) a K switched while another lane's napper is on the split row: {} ({}){}; (ii) a K armed with its R occupied: {} ({}){}; (iii) more than one napper between the stop's exit and the K's: {} ({}){}\n",
            f4_rows[0].len(),
            if f4_rows[0].is_empty() { "PASS" } else { "FAIL" },
            if f4_rows[0].is_empty() { String::new() } else { format!(": {}", f4_rows[0].iter().take(8).cloned().collect::<Vec<_>>().join("; ")) },
            f4_rows[1].len(),
            if f4_rows[1].is_empty() { "PASS" } else { "FAIL" },
            if f4_rows[1].is_empty() { String::new() } else { format!(": {}", f4_rows[1].iter().take(8).cloned().collect::<Vec<_>>().join("; ")) },
            f4_rows[2].len(),
            if f4_rows[2].is_empty() { "PASS" } else { "FAIL" },
            if f4_rows[2].is_empty() { String::new() } else { format!(": {}", f4_rows[2].iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }
        );
        let s = format!(
            "{s}the sidings blocked by an unridden cart (ironspec 11:29 / 11:33: an empty cart at rest on a siding's R (x-2,24,7) for 20 gt or more; bar 0): {} ({}){}\n",
            r_unridden_rows.len(),
            if r_unridden_rows.is_empty() { "PASS" } else { "FAIL" },
            if r_unridden_rows.is_empty() { String::new() } else { format!(": {}", r_unridden_rows.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }
        );
        // (ironspec 11:48 / 11:50, G1 11:50) the returning nappers' stored speed entering their own K,
        // per lane; and each napper cart's rest on its R against the presence string's bar (z >= 7.100,
        // margin = z - 7.050); a lane with no rest NOT JUDGED
        let s = {
            let mut speed_rows: Vec<String> = Vec::new();
            let mut rest_rows: Vec<String> = Vec::new();
            let mut low: Vec<String> = Vec::new();
            let mut unjudged: Vec<String> = Vec::new();
            for k in 0..3 {
                let mut v: Vec<f64> = k_speeds[k].iter().map(|x| x.0).collect();
                v.sort_by(|a, b| a.total_cmp(b));
                speed_rows.push(format!("K({}): {}", LANES[k], if v.is_empty() { "none".to_string() } else { format!("n {}, min {:.3}, median {:.3}, max {:.3} b/t", v.len(), v[0], v[v.len() / 2], v[v.len() - 1]) }));
                let mut z: Vec<f64> = r_rests[k].iter().map(|x| x.0).collect();
                z.sort_by(|a, b| a.total_cmp(b));
                if z.is_empty() {
                    unjudged.push(format!("x {}", LANES[k]));
                }
                rest_rows.push(format!("lane x {}: {}", LANES[k], if z.is_empty() { "no rest".to_string() } else { format!("n {}, the lowest z {:.4} (margin {:+.4}), median {:.4}", z.len(), z[0], z[0] - 7.050, z[z.len() / 2]) }));
                low.extend(r_rests[k].iter().filter(|x| x.0 < 7.100).map(|(zz, t, c)| format!("lane x {} t {t} cart #{c} at z {zz:.4}", LANES[k])));
            }
            format!(
                "{s}the returning nappers' stored speed entering their own K (ironspec 11:48 / 11:50, G1 11:50: the cart's deltaMovement, which may exceed its per-tick move; information): {}\nthe napper carts' rests on their R below z 7.100 (ironspec 11:48 / 11:50: the presence string's rest bar, margin = z - 7.050; bar 0; a lane with no rest NOT JUDGED): {} ({}); {}{}\n",
                speed_rows.join("; "),
                low.len(),
                if !unjudged.is_empty() { format!("NOT JUDGED on {}: FAIL", unjudged.join(", ")) } else if low.is_empty() { "PASS".to_string() } else { "FAIL".to_string() },
                rest_rows.join("; "),
                if low.is_empty() { String::new() } else { format!("; below: {}", low.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }
            )
        };
        // (day-1 11:36 / ironspec 11:40 (3)) the passage ticks at each split junction by class, as
        // phases on the station's clock (t mod 100; REL is at 5 mod 100), with their circular spread
        let s = {
            let phase_txt = |ts: Vec<i64>| -> String {
                if ts.is_empty() {
                    return "none".to_string();
                }
                let mut ph: Vec<i64> = ts.iter().map(|t| t.rem_euclid(100)).collect();
                ph.sort();
                let widest = ph.windows(2).map(|p| p[1] - p[0]).chain(std::iter::once(ph[0] + 100 - ph[ph.len() - 1])).max().unwrap_or(100);
                format!("n {}, phases min {} / median {} / max {}, the circular spread {}", ph.len(), ph[0], ph[ph.len() / 2], ph[ph.len() - 1], if ph.len() < 2 { 0 } else { 100 - widest })
            };
            let speed_txt = |v: Vec<f64>| -> String {
                if v.is_empty() {
                    return String::new();
                }
                let mut v = v;
                v.sort_by(|a, b| a.total_cmp(b));
                format!("; stored speed min {:.3} / median {:.3} / max {:.3} b/t", v[0], v[v.len() / 2], v[v.len() - 1])
            };
            let mut rows: Vec<String> = Vec::new();
            let cells: Vec<(String, [i32; 3])> = [("J(-1)", [-1, 25, 9]), ("J(3)", [3, 25, 9]), ("J(7)", [7, 25, 9]), ("K(-1)", [-3, 25, 9]), ("K(3)", [1, 25, 9]), ("K(7)", [5, 25, 9]), ("the row's end", [8, 25, 9]), ("D(-1)", [-1, 24, 8]), ("D(3)", [3, 24, 8]), ("D(7)", [7, 24, 8])].into_iter().map(|(n, c)| (n.to_string(), c)).collect();
            for (name, x) in cells.iter() {
                for class in ["batch", "outbound napper", "returning napper"] {
                    let sel: Vec<&(i64, [i32; 3], &'static str, Option<usize>, f64)> = junction_passes.iter().filter(|p| p.1 == *x && p.2 == class).collect();
                    if !sel.is_empty() {
                        rows.push(format!("{name} {class}: {}{}", phase_txt(sel.iter().map(|p| p.0).collect()), speed_txt(sel.iter().map(|p| p.4).collect())));
                    }
                }
                // a K's own returning nappers (its lane's)
                if let Some(k) = LANES.iter().position(|l| x[1] == 25 && x[2] == 9 && l - 2 == x[0]) {
                    let sel: Vec<&(i64, [i32; 3], &'static str, Option<usize>, f64)> = junction_passes.iter().filter(|p| p.1 == *x && p.2 == "returning napper" && p.3 == Some(k)).collect();
                    rows.push(format!("{name} its own lane's returning nappers: {}{}", phase_txt(sel.iter().map(|p| p.0).collect()), speed_txt(sel.iter().map(|p| p.4).collect())));
                }
            }
            // (ironspec 11:55 (2)) the stored speed at each lane's C (the C arrivals' recorded speed)
            for k in 0..3 {
                rows.push(format!("C({}) every arrival: n {}{}", LANES[k], c_arrivals[k].len(), speed_txt(c_arrivals[k].iter().map(|a| a.1).collect())));
            }
            format!("{s}the split junctions' passage ticks and stored speeds (day-1 11:36 / ironspec 11:40 (3) / 11:55 (2), checklist 1c: each cart's entry into J (x,25,9) / K (x-2,25,9) / the row's end (8,25,9) / D (x,24,8) by class, t mod 100 on the station's clock, REL at 5, and its STORED speed (deltaMovement); each C arrival's stored speed; information): {}\n", rows.join("; "))
        };
        // (ironspec 12:58) the least lift-mouth gap between consecutive carts, per pair class
        let s = {
            let mut per: BTreeMap<(&'static str, &'static str), Vec<i64>> = BTreeMap::new();
            for w2 in lift_entries.windows(2) {
                per.entry((w2[0].2, w2[1].2)).or_default().push(w2[1].0 - w2[0].0);
            }
            let rows: Vec<String> = per
                .iter_mut()
                .map(|((a, b), v)| {
                    v.sort();
                    format!("{a} -> {b}: n {}, the least {} gt, median {}", v.len(), v[0], v[v.len() / 2])
                })
                .collect();
            let napper_least = per.iter().filter(|((a, b), _)| *a == "napper" || *b == "napper").map(|(_, v)| v[0]).min();
            let batch_least = per.get(&("batch", "batch")).map(|v| v[0]);
            let ok = napper_least.is_none_or(|m| m >= 23) && batch_least.is_none_or(|m| m >= 13);
            format!(
                "{s}the shared lift's mouth gaps (ironspec 12:58 / 12:38 / 12:51: each cart's entry into the lift column (-5,1..3,7) from outside it; the gap to the cart before, per pair class (a returning napper, else batch); bars: batch -> batch >= 13 (the lift-pair bar), every pair with a napper >= 13 + 10 = 23): {} entries; {} ({})\n",
                lift_entries.len(),
                if rows.is_empty() { "no pair".to_string() } else { rows.join("; ") },
                if lift_entries.len() < 2 { "NOT JUDGED (no pair): FAIL" } else if ok { "PASS" } else { "FAIL" }
            )
        };
        // (G1 12:50 / ironspec 12:54 (1), evidence keying's swaps) the in-batch swap rate: each REL's
        // (dfb89499 14:19, ironspec 14:13 (3)(a)) the y-12 pairs: consecutive entries into (-5,12,7)
        // (a pass rising in the column, lane 7's side entry from (-4,12,7)), per pair class, bar >= 16
        let s = {
            let mut per: BTreeMap<String, Vec<i64>> = BTreeMap::new();
            for w2 in y12_entries.windows(2) {
                per.entry(format!("{} {} -> {} {}", w2[0].2, w2[0].3, w2[1].2, w2[1].3)).or_default().push(w2[1].0 - w2[0].0);
            }
            let least = per.values().filter_map(|v| v.iter().min().copied()).min();
            let rows: Vec<String> = per
                .iter_mut()
                .map(|(c, v)| {
                    v.sort();
                    format!("{c}: n {}, the least {} gt, median {}", v.len(), v[0], v[v.len() / 2])
                })
                .collect();
            format!(
                "{s}the y-12 pairs (dfb89499 14:19 / ironspec 14:13 (3)(a): consecutive entries into the column's cell (-5,12,7), a pass rising in the column or lane 7's side entry from the braked slope (-4,12,7); bar >= 16, every pair): {} entries; {}; the least {} ({})\n",
                y12_entries.len(),
                if rows.is_empty() { "no pair".to_string() } else { rows.join("; ") },
                least.map_or("-".to_string(), |m| format!("{m} gt")),
                if y12_entries.len() < 2 { "NOT JUDGED (no pair): FAIL" } else if least.is_some_and(|m| m >= 16) { "PASS" } else { "FAIL" }
            )
        };
        // (ironspec 15:17) the napper's lip arrival against the batch's lane carts; (dfb89499 15:11) the
        // station's touching contacts; (ironspec 15:23 (5) for 7b90c3f1) the cart-cart contacts and the
        // napper's seat on the hold
        let s = format!("{s}{}{}", napper_lip_line(&lipc_arrivals, &lipc_who, &ctl.k_windows, &kwin_napper, &ctl.k_fired, o.hold_pulse_delta), touch.report());
        // (G1 12:50 / ironspec 12:54 (1), evidence keying's swaps) the in-batch swap rate: each REL's
        // pocket releases (a cart leaving P (x,0,0), grouped within 40 gt) against their order of
        // arrival at the split row's first J (-1,25,9)
        let s = {
            let mut exits: Vec<(i64, u32, usize)> = Vec::new();
            for (cid, ents) in cart_entries.iter() {
                for p in ents.windows(2) {
                    if let Some(k) = LANES.iter().position(|x| p[0].1 == [*x, 0, 0]) && p[1].1 != p[0].1 {
                        exits.push((p[1].0, *cid, k));
                    }
                }
            }
            exits.sort();
            let mut batches: Vec<Vec<(i64, u32, usize)>> = Vec::new();
            for e in exits {
                match batches.last_mut() {
                    Some(b) if e.0 - b[0].0 <= 40 => b.push(e),
                    _ => batches.push(vec![e]),
                }
            }
            let mut judged = 0usize;
            let mut swapped: Vec<String> = Vec::new();
            for b in batches.iter().filter(|b| b.len() >= 2) {
                let arr: Vec<Option<i64>> = b.iter().map(|(t, cid, _)| cart_entries.get(cid).and_then(|v| v.iter().find(|(et, c)| *et > *t && *c == [-1, 25, 9]).map(|(et, _)| *et))).collect();
                if arr.iter().any(|a| a.is_none()) {
                    continue;
                }
                judged += 1;
                let mut order: Vec<usize> = (0..b.len()).collect();
                order.sort_by_key(|i| arr[*i]);
                if order.iter().enumerate().any(|(n, i)| n != *i) {
                    swapped.push(format!(
                        "released t {} (lanes {}) arrived as lanes {}",
                        b[0].0,
                        b.iter().map(|x| LANES[x.2].to_string()).collect::<Vec<_>>().join(" / "),
                        order.iter().map(|i| LANES[b[*i].2].to_string()).collect::<Vec<_>>().join(" / ")
                    ));
                }
            }
            format!(
                "{s}the in-batch swaps (G1 12:50 / ironspec 12:54 (1): each REL's pocket releases (a cart leaving P (x,0,0), grouped within 40 gt) against their arrival order at J(-1) (-1,25,9); information): {} of {} batches swapped{}\n",
                swapped.len(),
                judged,
                if swapped.is_empty() { String::new() } else { format!(": {}", swapped.join("; ")) }
            )
        };
        // (onechunk 11:16) the nappers' misroutes, bar 0
        let s = format!(
            "{s}the westbound carts on the split row (ironspec 11:23 / G1 11:19 F4 (b), R4: a cart on (x,25,9) moving west, each episode; bar 0): {} ({}){}\n",
            westbound_rows.len(),
            if westbound_rows.is_empty() { "PASS" } else { "FAIL" },
            if westbound_rows.is_empty() { String::new() } else { format!(": {}", westbound_rows.iter().take(10).cloned().collect::<Vec<_>>().join("; ")) }
        );
        let s = format!(
            "{s}the nappers' carts dropping into a lane not their own (onechunk 11:16: s3 / s8's 9th cart; bar 0; G1's P16: no napper drop at all is NOT JUDGED): {} of {} napper drops ({}){}\n",
            napper_wrong_lane.len(),
            napper_drops,
            if napper_drops == 0 { "NOT JUDGED (no napper drop): FAIL" } else if napper_wrong_lane.is_empty() { "PASS" } else { "FAIL" },
            if napper_wrong_lane.is_empty() { String::new() } else { format!(": {}", napper_wrong_lane.iter().take(8).cloned().collect::<Vec<_>>().join(" || ")) }
        );
        let q = |mut v: Vec<i64>| -> String {
            if v.is_empty() {
                return "-".to_string();
            }
            v.sort();
            format!("n {}, min {}, median {}, max {}", v.len(), v[0], v[v.len() / 2], v[v.len() - 1])
        };
        let mut h_end: Vec<(u32, i64)> = h_rests.iter().map(|r| (r.3, r.1 + r.2)).collect();
        h_end.sort_by_key(|(_, t)| *t);
        let per: Vec<String> = (0..3)
            .map(|k| {
                let mine: Vec<&(usize, i64, u32, bool)> = lip_arrivals.iter().filter(|a| a.0 == k).collect();
                let gaps: Vec<i64> = mine.windows(2).filter(|w| w[1].3).map(|w| w[1].1 - w[0].1).collect();
                let transits: Vec<i64> = mine.iter().filter(|a| !a.3).filter_map(|a| h_end.iter().filter(|(c, t)| *c == a.2 && *t <= a.1).map(|(_, t)| a.1 - t).min()).collect();
                format!("lane x {}: the pair gaps (leader -> napper at the lip) {}; the loop transits (H release -> lip, batch carts) {}", LANES[k], q(gaps), q(transits))
            })
            .collect();
        format!("{s}the lip arrivals' pair gaps and loop transits (day-1's constant (4) and loop-transit spread, information): {}\n", per.join("; "))
    };
    // (G1 10:34) every ridden departure from a siding's R by rider kind (a batch rider's departure, the
    // detector pressed by a cart not the napper's, FAILS), each with whether PRESENT_l was set; (G1
    // 10:36) the drop-entry envelope per pass; --lip-hold's control line; the siding trace
    let s = {
        let mut lip_beyond_total = 0usize;
        let (ok_d, bad_d): (Vec<&(i64, usize, u32, Option<u32>, bool)>, Vec<&(i64, usize, u32, Option<u32>, bool)>) = ctl.r_departures.iter().partition(|(_, _, _, r, _)| r.is_some_and(|r| nappers.iter().any(|(v, _, _)| *v == r)));
        let (eg, edx) = o.lip_envelope;
        let mut per = String::new();
        let mut env_unjudged: Vec<String> = Vec::new();
        for k in 0..3 {
            // (G1's P2 / P3) the envelope on the whole pass window: the off-rail ticks over the pass,
            // the largest lateral deviation from the lane's centre line; a window closing any way but
            // into the column is an escape (counted beyond the envelope)
            let v: Vec<&(usize, i64, f64, bool, &'static str, u32, i64, [i32; 3])> = env_passes.iter().filter(|p| p.0 == k && p.4 == "into the column").collect();
            let escapes: Vec<String> = env_passes.iter().filter(|p| p.0 == k && p.4 != "into the column").map(|p| format!("#{} from t {}: {} (at ({},{},{}), {} off-rail gt, lateral {:.3})", p.5, p.6, p.4, p.7[0], p.7[1], p.7[2], p.1, p.2)).collect();
            if v.is_empty() && escapes.is_empty() {
                env_unjudged.push(format!("x {}", LANES[k]));
            }
            let mut t: Vec<i64> = v.iter().map(|p| p.1).collect();
            t.sort();
            let mut d: Vec<f64> = v.iter().map(|p| p.2).collect();
            d.sort_by(|a, b| a.total_cmp(b));
            let beyond: Vec<String> = v.iter().filter(|p| (p.3 && p.1 > eg) || p.2 > edx).map(|p| format!("#{} {} gt / {:.3}{}", p.5, p.1, p.2, if p.3 { " (a cart behind)" } else { "" })).collect();
            lip_beyond_total += beyond.len() + escapes.len();
            per.push_str(&format!(
                "{}lane x {}: {} passes; off-rail ticks over the pass {}; lateral deviation from the centre line {}; beyond the envelope: {}{}; escapes (the window closing other than into the column): {}{}",
                if k == 0 { "" } else { "; " },
                LANES[k],
                v.len(),
                if t.is_empty() { "-".to_string() } else { format!("min {}, median {}, max {}", t[0], t[t.len() / 2], t[t.len() - 1]) },
                if d.is_empty() { "-".to_string() } else { format!("min {:.3}, median {:.3}, max {:.3}", d[0], d[d.len() / 2], d[d.len() - 1]) },
                beyond.len(),
                if beyond.is_empty() { String::new() } else { format!(" ({})", beyond.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) },
                escapes.len(),
                if escapes.is_empty() { String::new() } else { format!(" ({})", escapes.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
            ));
        }
        let n_beyond = lip_beyond_total;
        format!(
            "{s}the ridden departures from a siding's R (G1 10:34: R's departure, the detector that resets PRESENT_l; a cart not the napper's pressing it FAILS): {} by nappers, {} by others ({}){}\nthe drop-entry envelope (G1 10:36 / P2 / P3: per pass window (C's entry into the column), the off-rail ticks over the pass and the largest lateral deviation from the lane's centre line; a pass over {eg} off-rail gt with a cart at C or D behind it, or over {edx:.3} b lateral, or a window closing any way but into the column (an escape), FAILS; the defaults are tick-exact 10:55's game maxima; a lane with no pass NOT JUDGED): {n_beyond} ({}); {per}\n{}{}",
            ok_d.len(),
            bad_d.len(),
            if ok_d.is_empty() && bad_d.is_empty() { "NOT JUDGED (no departure from any R; G1's P16): FAIL" } else if bad_d.is_empty() { "PASS" } else { "FAIL" },
            if bad_d.is_empty() { String::new() } else { format!(": {}", bad_d.iter().take(12).map(|(t, k, c, r, p)| format!("t {t} lane x {} cart #{c} rider {} (PRESENT_l {})", LANES[*k], r.map_or("-".to_string(), |r| format!("#{r}")), if *p { "set: CLEARED by it" } else { "not set" })).collect::<Vec<_>>().join("; ")) },
            if !env_unjudged.is_empty() { format!("NOT JUDGED on {}: FAIL", env_unjudged.join(", ")) } else if n_beyond == 0 { "PASS".to_string() } else { "FAIL".to_string() },
            match (o.lip_hold, lip_hold_cart) {
                (Some(n), Some((c, t, _))) => format!("the forced lip row (G1 10:34): cart #{c} held {n} gt on the drop cell from t {t}\n"),
                (Some(n), None) => format!("the forced lip row (G1 10:34): --lip-hold {n}: no cart reached a drop cell after t 1,000: NOT JUDGED: FAIL\n"),
                _ => String::new(),
            },
            if siding_rows.is_empty() { String::new() } else { format!("the siding trace (G1 10:34), per tick:\n{}\n", siding_rows.join("\n")) }
        )
    };
    // (ironspec 10:51 (g)) the H holds' rests, declared and bounded: a cart at rest on a station hold's
    // closed trapdoor (x,16,0), under H's tripwire (x,17,0), up to --h-rest-bound gt; a longer one FAILS
    let s = {
        let mut all = h_rests.clone();
        all.extend(h_open.iter().map(|(id, (t0, c, n))| (*c, *t0, *n, *id)));
        let mut per: BTreeMap<[i32; 3], (usize, i64)> = BTreeMap::new();
        for (c, _, n, _) in &all {
            let e = per.entry(*c).or_default();
            e.0 += 1;
            e.1 = e.1.max(*n);
        }
        let over: Vec<String> = all.iter().filter(|r| r.2 > o.h_rest_bound).map(|(c, t0, n, id)| format!("cart #{id} at ({},{},{}) from t {t0}, {n} gt", c[0], c[1], c[2])).collect();
        format!(
            "{s}the H holds' rests (ironspec 10:51 (g): declared: a cart at rest on a station hold's closed trapdoor (x,16,0) under H's tripwire (x,17,0); bounded at {} gt (2 D periods + the longest release wait); a longer one FAILS): {}; over the bound: {} ({}){}\n",
            o.h_rest_bound,
            if per.is_empty() { "none".to_string() } else { per.iter().map(|(c, (n, m))| format!("({},{},{}) {n} rests, the longest {m} gt", c[0], c[1], c[2])).collect::<Vec<_>>().join("; ") },
            over.len(),
            if over.is_empty() { "PASS" } else { "FAIL" },
            if over.is_empty() { String::new() } else { format!(": {}", over.iter().take(10).cloned().collect::<Vec<_>>().join("; ")) }
        )
    };
    let s = if let Some((a, b)) = o.chain_trace {
        format!("{s}the chain trace over run ticks {a}..={b} (ironspec 08:26 platform (3): every napper's position, cart, activity, HOME, WALK_TARGET and stamp; the bed's occupant; RS_ready; each lane's siding R occupancy):\n{}\n", chain_rows.join("\n"))
    } else {
        s
    };
    let s = if let Some((tb, nb)) = o.lift_block {
        format!("{s}the lift column (x -5, z 6..8, y >= 2) from t {} to t {} around the lift-block control (an empty cart held still on the exit booster (-5,23,8) from t {tb} for {nb} gt; a fault row, ironspec 07:57): {}\n", tb - 5, tb + 150, lift_rows.join("; "))
    } else {
        s
    };
    // (G1 X16) the emulation's no-op writes: each listed; a write of a cell's existing state is no
    // write in the game (setBlock returns early). Justified per writer (G1: "FAIL emulated no-ops,
    // or justify each writer"): the split's J / K latch levers (x,26,9), set / reset on their branch
    // detectors' edges, pulse an RS latch into the state it holds (no change in the built latch
    // either); any other writer's no-op FAILS
    let s = {
        let n = NOOP_WRITES.with(|n| *n.borrow());
        let rows = NOOP_ROWS.with(|r| r.borrow().clone());
        let latch = |r: &str| [-3, -1, 1, 3, 5, 7].iter().any(|x| r.contains(&format!(" ({x},26,9) ")));
        let (ok, bad): (Vec<&String>, Vec<&String>) = rows.iter().partition(|r| latch(r));
        format!(
            "{s}the emulation's no-op writes (G1 X16: a write of the cell's existing state; each writer justified or FAIL): {n} ({} the split's J / K latch levers (x,26,9), set / reset on their branch detectors' edges into the state they hold: justified; {} by another writer: {}){}{}\n",
            ok.len(),
            bad.len(),
            if bad.is_empty() && rows.len() as i64 == n { "PASS" } else if bad.is_empty() { "the list is capped at 40: NOT JUDGED past it: FAIL" } else { "FAIL" },
            if ok.is_empty() { String::new() } else { format!("; justified: {}", ok.iter().map(|r| r.as_str()).collect::<Vec<_>>().join("; ")) },
            if bad.is_empty() { String::new() } else { format!("; UNJUSTIFIED: {}", bad.iter().map(|r| r.as_str()).collect::<Vec<_>>().join("; ")) }
        )
    };
    // (ironspec 08:13 / 08:14 platform (2), G1 08:12 PT) the cart-conservation check
    let s = format!(
        "{s}the cart-conservation check (ironspec 08:13 / 08:14 platform, G1 08:12: 8 carts a lane, the napper's included, so a stopped cart on the exit row needs a ninth or a fault): each lane's most carts on H, R_1 and the slopes: {}; the farthest-back tail per lane: {}; carts standing still (two ticks running) on the exit booster, the exit row or the split / merge row (z 8..9, y 23..26, x -5..9): {} cart-ticks{} (bar 0: {})\n",
        LANES.iter().zip(lane_max.iter()).map(|(x, (n, t, tail))| format!("lane x {x} {n} (first at t {t}, its tail {})", tail.map_or("-".to_string(), |c| format!("({},{},{})", c[0], c[1], c[2])))).collect::<Vec<_>>().join(", "),
        LANES.iter().zip(lane_tail_max.iter()).map(|(x, t)| format!("lane x {x} {}", t.map_or("-".to_string(), |c| format!("({},{},{})", c[0], c[1], c[2])))).collect::<Vec<_>>().join(", "),
        stopped_ticks,
        if stopped_rows.is_empty() { String::new() } else { format!(": {}", stopped_rows.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>().join("; ")) },
        if o.lift_block.is_some() { "not judged: --lift-block's fault row" } else if stopped_ticks == 0 { "PASS" } else { "FAIL" }
    );
    // (G1 X31, ironspec 08:13: the lane carts by id) every lane cart at t0 still in the world at the
    // end and no stray cart in a lane (bar 0); the lane changes by id as information (the split's J
    // latches route a batch's arrivals by order: 1st to lane -1, 2nd to 3, 3rd through to 7)
    let lost_carts: Vec<u32> = {
        let alive: std::collections::BTreeSet<u32> = run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
        cart_lane0.keys().filter(|id| !alive.contains(id)).copied().collect()
    };
    let s = format!(
        "{s}the lane carts by id (G1 X31 / ironspec 08:13: every lane cart at t0 still in the world at the end, no stray cart in a lane; bar 0): {} carts tracked, {} lost{}, {} strays{} ({}); lane changes by id (information: the split's J latches route a batch's 1st / 2nd / 3rd arrival to lanes x -1 / 3 / 7 by arrival order, not by origin): {}{}\n",
        cart_lane0.len(),
        lost_carts.len(),
        if lost_carts.is_empty() { String::new() } else { format!(" ({})", lost_carts.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(", ")) },
        stray_carts.len(),
        if stray_carts.is_empty() { String::new() } else { format!(" ({})", stray_carts.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>().join("; ")) },
        if cart_lane0.is_empty() { "NOT JUDGED (no lane carts at t0): FAIL" } else if lost_carts.is_empty() && stray_carts.is_empty() { "PASS" } else { "FAIL" },
        wrong_lane.len(),
        if wrong_lane.is_empty() { String::new() } else { format!(" (the first: {})", wrong_lane.iter().take(6).map(|(_, r)| r.clone()).collect::<Vec<_>>().join("; ")) }
    );
    // (G1 08:21 PT) each late-path pair against the actual cart trajectories: the carts that were in
    // the draw cell and then entered the path cell within the pair's interval
    let mut s = s;
    // (tick-exact / ironspec 09:21-09:25 platform: the rider push, a measured design term for day-1's
    // --post-spread / --t-d-spread) per lane: the pushes on its carts (per lane lap = its median D
    // period x 8 carts), each impulse and its component along the cart's motion, by the pusher's
    // activity; each cart's stacked along-motion sum per lap (its worst); and the OBSERVED timing
    // spread that every push and every other cause sum to: the D-press gaps' deviation from their
    // median (the worst, how often >= 5 gt) and each visit's A6 R -> siding R ride
    {
        let mut rows: Vec<String> = Vec::new();
        for k in 0..3 {
            let ts: Vec<i64> = ctl.dpress_log.iter().filter(|(_, dk)| *dk == k).map(|(t, _)| *t).collect();
            let mut gaps: Vec<i64> = ts.windows(2).map(|p| p[1] - p[0]).filter(|g| *g > 0).collect();
            gaps.sort();
            let period = gaps.get(gaps.len() / 2).copied().unwrap_or(0);
            let lap = (period * 8).max(1);
            let dev: Vec<i64> = gaps.iter().map(|g| (g - period).abs()).collect();
            let dev_max = dev.iter().copied().max().unwrap_or(0);
            let dev5 = dev.iter().filter(|d| **d >= 5).count();
            let mine: Vec<&(i64, u32, Option<usize>, String, f64, f64)> = pushes.iter().filter(|p| p.2 == Some(k)).collect();
            let mut by_lap: BTreeMap<i64, usize> = BTreeMap::new();
            let mut stacked: BTreeMap<(u32, i64), f64> = BTreeMap::new();
            let mut by_act: BTreeMap<String, usize> = BTreeMap::new();
            for p in &mine {
                *by_lap.entry(p.0 / lap).or_default() += 1;
                *stacked.entry((p.1, p.0 / lap)).or_default() += p.5;
                *by_act.entry(p.3.clone()).or_default() += 1;
            }
            let laps = (run.world.time - t0) / lap;
            let per_lap_max = by_lap.values().copied().max().unwrap_or(0);
            let mut imp: Vec<f64> = mine.iter().map(|p| p.4).collect();
            imp.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let worst_stack = stacked.values().fold(0.0f64, |m, v| if v.abs() > m.abs() { *v } else { m });
            // each visit's ride from A6's R to its siding's R (this lane's nappers)
            let mut rides: Vec<i64> = Vec::new();
            for (v, vk, t) in nappers.iter().filter(|n| n.1 == k) {
                let _ = vk;
                let to = ctl.pick_carts.iter().filter(|(pt, _, pv, _)| pv == v && pt > t).map(|x| x.0).min().unwrap_or(i64::MAX);
                let ms = |what: &str| past_marks.get(v).into_iter().flatten().chain(recs.get(v).map(|r| r.milestones.iter()).into_iter().flatten()).find(|(m, mt)| m == what && *mt >= *t && *mt < to).map(|(_, mt)| *mt);
                if let (Some(a), Some(b)) = (ms("at R"), ms("on its siding's R")) {
                    rides.push(b - a);
                }
            }
            rides.sort();
            // (day-1 11:12 / ironspec 11:15: the D-press spread, gating lane -1's first lever) each D
            // press's phase in the 100-gt cycle (t mod 100), its min / median / max and the circular
            // spread (100 minus the widest empty arc)
            let mut ph: Vec<i64> = ts.iter().map(|t| t.rem_euclid(100)).collect();
            ph.sort();
            let spread = if ph.len() < 2 {
                0
            } else {
                let widest = ph.windows(2).map(|p| p[1] - p[0]).chain(std::iter::once(ph[0] + 100 - ph[ph.len() - 1])).max().unwrap_or(100);
                100 - widest
            };
            rows.push(format!(
                "lane x {}: the D presses' phases (t mod 100; day-1 / ironspec 11:15, the D-press spread): {} presses, min {}, median {}, max {}, the circular spread {spread} gt",
                LANES[k],
                ph.len(),
                ph.first().copied().unwrap_or(0),
                ph.get(ph.len() / 2).copied().unwrap_or(0),
                ph.last().copied().unwrap_or(0)
            ));
            rows.push(format!(
                "lane x {}: the lap {lap} gt ({} laps); {} pushes ({:.2} a lap, the most in one lap {per_lap_max}); |impulse| median {:.4}, max {:.4} b/t; by the pusher: {}; the worst stacked along-motion sum on one cart in one lap {worst_stack:+.4} b/t; the observed D-press gaps (median {period} gt): the worst deviation {dev_max} gt, {dev5} of {} gaps off by >= 5 gt; A6 R -> siding R rides: {}",
                LANES[k],
                laps,
                mine.len(),
                if laps > 0 { mine.len() as f64 / laps as f64 } else { 0.0 },
                imp.get(imp.len() / 2).copied().unwrap_or(0.0),
                imp.last().copied().unwrap_or(0.0),
                if by_act.is_empty() { "none".to_string() } else { by_act.iter().map(|(a, n)| format!("{a} {n}")).collect::<Vec<_>>().join(", ") },
                gaps.len(),
                if rides.is_empty() { "none".to_string() } else { format!("{} (min {}, median {}, max {})", rides.len(), rides[0], rides[rides.len() / 2], rides[rides.len() - 1]) }
            ));
        }
        let other = pushes.iter().filter(|p| p.2.is_none()).count();
        s.push_str(&format!(
            "the rider pushes (tick-exact / ironspec 09:21-09:25 platform: a measured design term; redsim's push log; information for day-1's --post-spread / --t-d-spread, judged by the margins it feeds): {} pushes in all ({other} on carts outside the lanes' t0 ids); {}\n",
            pushes.len(),
            rows.join("; ")
        ));
    }
    // (9fc3155f / G1 09:05: every observed cart route walkable in the declared travel table)
    // --cart-cells FILE: every cart's cell transitions through the split row, the sidings, the merges,
    // the S-bends and the drops (y 22..26, z 5..10, x -6..10), one line each: cart, its lane at t0,
    // tick, from -> to
    if let Some(path) = &o.cart_cells {
        // (9fc3155f / G1 09:39: the y 0 merge row and the foot ramp too) the box, by default the split
        // row's region, else --cart-cells-box
        let (ba, bb) = o.cart_cells_box.unwrap_or(([-6, 22, 5], [10, 26, 10]));
        let inside = |c: &[i32; 3]| (0..3).all(|i| c[i] >= ba[i] && c[i] <= bb[i]);
        let mut out = format!("# onechunk f1q2-run --cart-cells: cart, its lane at t0 (or -), run tick, from -> to, for every cell change with either cell in ({},{},{})..=({},{},{})\n", ba[0], ba[1], ba[2], bb[0], bb[1], bb[2]);
        let mut n = 0usize;
        for (id, v) in &cart_entries {
            let lane = cart_lane0.get(id).map_or("-".to_string(), |k| LANES[*k].to_string());
            for p in v.windows(2) {
                let ((_, a), (t, b)) = (p[0], p[1]);
                if inside(&a) || inside(&b) {
                    out.push_str(&format!("#{id} lane {lane} t {t} ({},{},{}) -> ({},{},{})\n", a[0], a[1], a[2], b[0], b[1], b[2]));
                    n += 1;
                }
            }
        }
        std::fs::write(path, out).map_err(|e| format!("--cart-cells {}: {e}", path.display()))?;
        s.push_str(&format!("the cart cells (9fc3155f's walkability check): {n} transitions written to {}\n", path.display()));
    }
    for (a, b, dmin, dmax) in &o.pair_checks {
        let mut rides: Vec<String> = Vec::new();
        for (id, v) in &cart_entries {
            for (i, (ta, ca)) in v.iter().enumerate() {
                if ca != a {
                    continue;
                }
                if let Some((tb, _)) = v[i + 1..].iter().find(|(tb, cb)| cb == b && tb - ta >= *dmin && tb - ta <= *dmax) {
                    rides.push(format!("cart #{id}: in ({},{},{}) at t {ta}, in ({},{},{}) at t {tb} ({} gt)", a[0], a[1], a[2], b[0], b[1], b[2], tb - ta));
                    break;
                }
            }
        }
        s.push_str(&format!(
            "the pair check (G1 08:21 PT: a late-path pair is real only if a cart rides it) ({},{},{}) -> ({},{},{}) within {dmin}..={dmax} gt: {} cart{} ride it{}\n",
            a[0], a[1], a[2], b[0], b[1], b[2],
            rides.len(),
            if rides.len() == 1 { "" } else { "s" },
            if rides.is_empty() { " (no cart: an orient artefact on this run)".to_string() } else { format!(": {}", rides.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }
        ));
    }
    // (ironspec 06:33 platform) the night eye positions for 9fc3155f's coverage map
    if let Some(p) = &o.rider_trace {
        let mut t = format!("# onechunk f1q2-run --rider-trace: every distinct night eye position of a rider (redsim's seated eye: the cart's position + the seat + the villager's eye height), to 0.01; columns: x y z ticks rest_ticks panic_ticks; seed {}, {} gt from day {}\n", o.seed, o.ticks, o.day0);
        for (k, v) in &eye_hist {
            t.push_str(&format!("{:.2} {:.2} {:.2} {} {} {}\n", k[0] as f64 / 100.0, k[1] as f64 / 100.0, k[2] as f64 / 100.0, v[0], v[1], v[2]));
        }
        std::fs::write(p, t).map_err(|e| format!("--rider-trace {}: {e}", p.display()))?;
    }
    let s = judge_controls(&s, &late_controls);
    Ok((s, drive_head.map(|h| (h, t0)), scene_entities))
}

/// (G1's blocker 2, N17) Each requested control judged on its own bar's line: `(control, the bar
/// line's prefix, the control must fail it, the control acted, the bar's failure is the control's own
/// sample with every other sample passing)`. A bar it must fail that fails (not NOT JUDGED) on the
/// control's own sample alone has its FAIL marked "expected FAIL" and the control prints CAUGHT;
/// otherwise NOT CAUGHT (a FAIL, the line kept as it is, so a natural failure beside the control
/// still counts). A control that must leave its bar passing prints HELD, or NOT HELD (the bar's own
/// FAIL stands). A control that never acted is NOT EXERCISED (a FAIL), whatever its bar says.
/// (ironspec 11:08 (2) / 11:37, G1's P1) An arrival's pass window: its lane, its entry into C, its
/// kind, the least separation between its (inflated) box and any other box, each other box that
/// intersected it (at rest?, the row, the other's id) once, and each push on it.
struct PassWindow {
    lane: usize,
    start: i64,
    kind: String,
    least: f64,
    hits: Vec<(bool, String, u32)>,
    seen: std::collections::BTreeSet<u32>,
    pushed: Vec<String>,
    /// (G1's P2 / P3, tick-exact's quantities over the whole pass) the ticks off a rail (no rail in
    /// the cart's cell or the one below), the largest lateral (x) deviation from the lane's centre
    /// line, and whether a cart stood at C / D behind it while it was on the lip
    off_rail: i64,
    max_dx: f64,
    behind: bool,
}

/// (G1 10:47 blocking 1, G1's P9) An id across a world open: a renewed one's new id; a LIVE one the
/// open didn't renew is an error; a dead one keeps its id unless that id now names a renewed entity
/// (an alias: an error).
fn open_follow(id: u32, m: &std::collections::HashMap<u32, u32>, pre_live: &std::collections::BTreeSet<u32>) -> Result<u32, String> {
    match m.get(&id) {
        Some(n) => Ok(*n),
        None if pre_live.contains(&id) => Err(format!("the world open didn't renew the live entity #{id} (G1 10:47: a live id the open doesn't renew is an error)")),
        None if m.values().any(|n| *n == id) => Err(format!("the dead id #{id} kept across the open now names a renewed entity (G1's P9: an alias)")),
        None => Ok(id),
    }
}

/// (cc767b78 11:15) A spec moved into the world by `d` (the design + `d`): its blocks, entities, block
/// entities, output chest, box, and its Nether part's anchor (the Nether image follows the anchor as
/// the game's portal link does: x / 8 and z / 8, the Nether y as written).
fn shift_spec_world(spec: &FarmSpec, d: [i32; 3]) -> FarmSpec {
    if d == [0, 0, 0] {
        return spec.clone();
    }
    let sh = |p: redsim::pos::BlockPos| redsim::pos::BlockPos::new(p.x + d[0], p.y + d[1], p.z + d[2]);
    let mut out = spec.clone();
    out.blocks = spec.blocks.iter().map(|(p, b)| (sh(*p), *b)).collect();
    out.entities = spec.entities.iter().map(|(t, p, n)| (t.clone(), [p[0] + d[0] as f64, p[1] + d[1] as f64, p[2] + d[2] as f64], n.clone())).collect();
    out.output = sh(spec.output);
    out.lo = sh(spec.lo);
    out.hi = sh(spec.hi);
    out.block_entities = spec.block_entities.iter().map(|(p, s)| (sh(*p), s.clone())).collect();
    if let Some(n) = out.nether.as_mut() {
        n.anchor = sh(n.anchor);
    }
    out
}

/// (G1's P18) A lane's region, the one definition of "the lane's carts" for the count bar, the
/// conservation ledger and the per-stall print: its pocket P (x,0,0), the fall from H to P (x,1..15,0),
/// its rails H (x,16,0), R_1 and the slopes up to V (x,16..22, z 0..6), its drop column and lip
/// (x,23..24,6), and its merge C (x,24,7) / D (x,24,8).
fn in_lane_region(c: [i32; 3], x: i32) -> bool {
    c[0] == x && ((c[2] == 0 && (0..=15).contains(&c[1])) || ((16..=22).contains(&c[1]) && (0..=6).contains(&c[2])) || ((23..=24).contains(&c[1]) && c[2] == 6) || (c[1] == 24 && (7..=8).contains(&c[2])))
}

/// (G1's P1) A judged pass with a hit or a push.
struct PassRow {
    arrival: u32,
    start: i64,
    kind: String,
    least: f64,
    hits: Vec<(bool, String, u32)>,
    pushed: Vec<String>,
}

/// (onechunk 11:16: s3 / s8's lane-7 napper turned into K(3)'s branch, bounced back and was caught
/// by J(-1)) Every split-row turn of cart `id` in the 400 gt before `rel`: the junction (J(x) at
/// (x,25,9) or K(x) at (x-2,25,9)), the tick, that T's shape then (t0's plus the changes before
/// it), its lever (x,26,9)'s state then, the clearance to the cart through it before, and where the
/// cart went next (back onto the row is a bounce).
fn split_turns(
    id: u32,
    rel: i64,
    cart_entries: &BTreeMap<u32, Vec<(i64, [i32; 3])>>,
    t_changes: &[(i64, [i32; 3], String, String)],
    t_shapes0: &BTreeMap<[i32; 3], String>,
    lever_hist: &BTreeMap<i32, Vec<(i64, bool)>>,
) -> String {
    let Some(v) = cart_entries.get(&id) else { return "no cell history".to_string() };
    let shape_at = |p: [i32; 3], t: i64| -> String { t_changes.iter().filter(|(ct, cp, _, _)| *cp == p && *ct <= t).last().map(|(_, _, _, to)| to.clone()).or_else(|| t_shapes0.get(&p).cloned()).unwrap_or_else(|| "not a T".to_string()) };
    let lever_at = |j: i32, t: i64| -> String { lever_hist.get(&j).and_then(|h| h.iter().filter(|(lt, _)| *lt <= t).last()).map_or("-".to_string(), |(_, on)| if *on { "on".to_string() } else { "off".to_string() }) };
    let turns: Vec<&[(i64, [i32; 3])]> = v.windows(2).filter(|p| p[0].1[1] == 25 && p[0].1[2] == 9 && (p[1].1[2] == 8 || p[1].1[1] == 24) && p[1].0 <= rel && rel - p[1].0 <= 400).collect();
    if turns.is_empty() {
        return "no split turn in the last 400 gt".to_string();
    }
    turns
        .iter()
        .map(|p| {
            let (j, t) = (p[0].1[0], p[1].0);
            let kind = if LANES.contains(&j) { format!("J({j})") } else if LANES.contains(&(j + 2)) { format!("K({})", j + 2) } else { format!("({j},25,9)") };
            let prev = cart_entries.iter().filter(|(oid, _)| **oid != id).flat_map(|(_, ov)| ov.windows(2).filter(|q| q[0].1 == [j, 25, 9] && q[1].0 < t).map(|q| q[1].0).collect::<Vec<_>>()).max();
            let next = v.iter().find(|(nt, _)| *nt > t).map_or("-".to_string(), |(nt, nc)| format!("({},{},{}) at t {nt}", nc[0], nc[1], nc[2]));
            format!("{kind} at t {t} (the T {}, its lever {}; the cart through it before: {}; next {next})", shape_at([j, 25, 9], t), lever_at(j, t), prev.map_or("none".to_string(), |pt| format!("{} gt earlier", t - pt)))
        })
        .collect::<Vec<_>>()
        .join(" then ")
}

pub fn judge_controls(rep: &str, controls: &[(String, &str, bool, bool, bool)]) -> String {
    if controls.is_empty() {
        return rep.to_string();
    }
    let mut lines: Vec<String> = rep.lines().map(str::to_string).collect();
    let mut verdicts: Vec<String> = Vec::new();
    for (name, prefix, must_fail, acted, own) in controls {
        let k = lines.iter().position(|l| l.starts_with(prefix));
        let Some(k) = k else {
            verdicts.push(format!("{name}: its bar ({}) printed no line: NOT JUDGED, FAIL", prefix.trim()));
            continue;
        };
        if !acted {
            verdicts.push(format!("{name}: NOT EXERCISED (the control found no moment to act), FAIL"));
            continue;
        }
        let fails = lines[k].replace("expected FAIL", "").contains("FAIL");
        let not_judged = lines[k].contains("NOT JUDGED");
        if *must_fail {
            if fails && !not_judged && *own {
                lines[k] = lines[k].replace("expected FAIL", "\u{1}").replace("FAIL", "expected FAIL").replace('\u{1}', "expected FAIL");
                verdicts.push(format!("{name}: CONTROL CAUGHT by {} (its own sample alone)", prefix.trim()));
            } else if fails && !not_judged {
                verdicts.push(format!("{name}: NOT CAUGHT ALONE: {} fails, but not on the control's own sample alone (a natural failure stands beside it), FAIL", prefix.trim()));
            } else {
                verdicts.push(format!("{name}: NOT CAUGHT by {} (its line {}), FAIL", prefix.trim(), if not_judged { "NOT JUDGED" } else { "passing" }));
            }
        } else if fails {
            verdicts.push(format!("{name}: NOT HELD: {} fails, FAIL", prefix.trim()));
        } else {
            verdicts.push(format!("{name}: HELD: {} passes as required", prefix.trim()));
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out.push_str(&format!("the controls, each on its own bar (G1's blocker 2): {}\n", verdicts.join("; ")));
    out
}

fn anyhow_free(ok: bool, why: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.to_string()) }
}

/// The report's lines whose bar printed FAIL (ironspec 17:01 (4); G1's CHANGES 2 on 961090aef):
/// every "expected FAIL" token (a declared placement-order T) is stripped first, so a real FAIL on
/// the same line still counts.
pub fn fail_lines(rep: &str) -> Vec<&str> {
    rep.lines().filter(|l| has_fail_verdict(&strip_expected(l))).collect()
}

/// (onechunk 14:26, G1's P10 sharpened) Whether a line carries a FAIL VERDICT: a whole-word "FAIL"
/// (no letter after it: "FAILS" in a bar's description is no verdict) that isn't the description of
/// a log's "FAIL lines" (the uncapped log judges print "the run log's FAIL lines …: 0 (PASS)").
/// Before this, any "FAIL" substring counted, so every PASS line describing what FAILS was a
/// "FAIL line" (18–22 per composed night).
pub fn has_fail_verdict(l: &str) -> bool {
    let b = l.as_bytes();
    let mut from = 0;
    while let Some(i) = l[from..].find("FAIL").map(|i| i + from) {
        let before_ok = i == 0 || !(b[i - 1] as char).is_ascii_alphanumeric();
        let after = &l[i + 4..];
        let after_ok = after.chars().next().is_none_or(|c| !c.is_ascii_alphabetic());
        // (onechunk 15:56, childA's finding) a bar's description "each writer justified or FAIL" is
        // no verdict: a FAIL right after "or " describes the alternative
        let described = l[..i].ends_with(" or ") || l[..i].ends_with(" else ");
        if before_ok && after_ok && !after.starts_with(" lines") && !described {
            return true;
        }
        from = i + 4;
    }
    false
}

/// (G1's P10) A line with every "expected FAIL" token removed, a token counting only as a whole
/// word ("unexpected FAIL" keeps its FAIL).
fn strip_expected(l: &str) -> String {
    let mut out = String::with_capacity(l.len());
    let mut rest = l;
    while let Some(i) = rest.find("expected FAIL") {
        let whole = rest[..i].chars().last().is_none_or(|c| !c.is_alphanumeric());
        out.push_str(&rest[..i]);
        if !whole {
            out.push_str("expected FAIL");
        }
        rest = &rest[i + "expected FAIL".len()..];
    }
    out.push_str(rest);
    out
}

/// (G1's P10) The FAIL lines anywhere in a whole log (never only its printed head): one report line
/// counting them, FAIL when there is one, with the first few.
pub fn log_fail_line(name: &str, log: &[String]) -> String {
    let fails: Vec<(usize, &String)> = log.iter().enumerate().filter(|(_, l)| strip_expected(l).contains("FAIL")).collect();
    format!(
        "the {name}'s FAIL lines over all its {} lines (G1's P10: judged uncapped): {} ({}){}",
        log.len(),
        fails.len(),
        if fails.is_empty() { "PASS" } else { "FAIL" },
        if fails.is_empty() { String::new() } else { format!(": {}", fails.iter().take(4).map(|(i, l)| format!("line {}: {}", i + 1, l.chars().take(120).collect::<String>())).collect::<Vec<_>>().join(" | ")) }
    )
}

/// (ironspec 02:37 PT (3)) The module's daylight detector (-5,15,3) under each weather: the day
/// time its level first falls below `threshold` in the evening and first reaches it again at dawn,
/// from redsim's detector formula (`daylight::detector_power`) at the cell's sky light with
/// `weather::sky_darken_at(day, rain, thunder)`, against tick-exact's official DAY-off edges (the
/// detector's 6 -> 5 at level 6: clear 12,230, rain 12,002, thunder 11,556), compared exactly, each
/// reference shifted by 1 gt as the control through the same comparison.
pub fn day_weather_rows(w: &redsim::world::World, threshold: u32) -> String {
    let pos = dbp(-5, 15, 3);
    let sky = match redsim::daylight::sky_light(w, pos) {
        Ok(s) => s,
        Err(e) => return format!("the detector's sky light unreadable ({e}): NOT JUDGED: FAIL"),
    };
    let power = |d: i64, rain: f32, thunder: f32| redsim::daylight::detector_power(sky, redsim::weather::sky_darken_at(d, rain, thunder), d, false) as u32;
    // (tick-exact 05:49 PT, G1: the SAME edge on both sides) the official DAY-off ticks, the
    // detector's 6 -> 5 step at level 6 under each weather (https://artifacts.internalmeta.com/a/1871762297585648);
    // redsim's analytic first day time below the level must equal them, and the 1-gt-shifted
    // control (each official tick + 1) must fail
    let rows: Vec<String> = [("clear", 0.0f32, 0.0f32, 12_230i64), ("rain", 1.0, 0.0, 12_002), ("thunder", 1.0, 1.0, 11_556)]
        .iter()
        .map(|(name, rain, thunder, official)| {
            let fall = (6_000..18_000i64).find(|d| power(*d, *rain, *thunder) < threshold);
            let rise = (18_000..30_000i64).find(|d| power(d.rem_euclid(24_000), *rain, *thunder) >= threshold).map(|d| d.rem_euclid(24_000));
            // (G1 N16: the control runs the SAME comparison) the edge matches a reference only when
            // equal; the official edge must match, and each reference shifted by 1 gt must not
            let edge_matches = |reference: i64| fall == Some(reference);
            let verdict = match fall {
                _ if threshold != 6 => "not judged: the official rows are level 6's".to_string(),
                Some(_) if edge_matches(*official) => {
                    let shifted = [official - 1, official + 1].map(|r| (r, edge_matches(r)));
                    let caught = shifted.iter().all(|(_, m)| !m);
                    format!(
                        "PASS; the 1-gt-shifted controls through the same comparison: {} ({})",
                        shifted.iter().map(|(r, m)| format!("{r} {}", if *m { "matches" } else { "differs" })).collect::<Vec<_>>().join(", "),
                        if caught { "caught" } else { "NOT CAUGHT: FAIL" }
                    )
                }
                Some(_) => "FAIL".to_string(),
                None => "NOT JUDGED (no fall): FAIL".to_string(),
            };
            format!(
                "{name}: falls below {threshold} (the 6 -> 5 off-edge at level 6) at day time {} (tick-exact's official off-edge {official}: {}), back at {}",
                fall.map_or("-".to_string(), |f| f.to_string()),
                fall.map_or("-".to_string(), |f| format!("{:+} gt, {verdict}", f - official)),
                rise.map_or("-".to_string(), |r| r.to_string())
            )
        })
        .collect();
    format!("the detector's sky light {sky}; {}", rows.join("; "))
}

/// (G1's blocker 3, day-1 06:07 PT) The day times day-1's first-release formula holds between, on a
/// clear clock at the module's detector (-5,15,3): EIGHT, the morning's first day time the detector's
/// level reaches 8 (trigger 2 loads C 19 there), and RISE, the first day time after 18,000 its level
/// reaches 1 (the detector's 0 -> 1 loads C 0); None when the detector's sky light can't be read.
pub fn counter_window(w: &redsim::world::World) -> Option<(i64, i64)> {
    let sky = redsim::daylight::sky_light(w, dbp(-5, 15, 3)).ok()?;
    let power = |d: i64| redsim::daylight::detector_power(sky, redsim::weather::sky_darken_at(d, 0.0, 0.0), d, false) as u32;
    let eight = (0..12_000i64).find(|d| power(*d) >= 8)?;
    let rise = (18_000..24_000i64).find(|d| power(*d) >= 1)?;
    Some((eight, rise))
}

/// The picker's C at the first release from its day time `d`: day-1's BUILT counter (ctlcounter.rs,
/// G1's blocker 3 on 185e9ac47, ironspec 07:18 real): `141 + ceil((d - 12,540) / 100)`, a half
/// tick rounding down, so a release 50 gt after the 12,490 grid is still C 141 and 50 gt before it
/// C 140.
pub fn counter_c(d: i64) -> i64 {
    141 + (d - 12_540 + 99).div_euclid(100)
}

/// (ironspec 15:23 (5) for 7b90c3f1) The gap between two carts' boxes (0.98 wide, 0.7 tall, each on
/// its centre): the largest per-axis separation; negative, an overlap (its least depth).
fn cart_box_gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).abs() - 0.98).max((a[1] - b[1]).abs() - 0.7).max((a[2] - b[2]).abs() - 0.98)
}

/// (dfb89499 15:11) A cell's "on" for the touch prints: a redstone block, or a state whose text
/// carries `powered=true` (a powered / activator rail's and a detector rail's read from its fields).
fn on_state(b: redsim::block::Block) -> bool {
    use redsim::block::Block;
    match b {
        Block::RedstoneBlock => true,
        Block::Rail { powered, .. } | Block::DetectorRail { powered, .. } => powered,
        Block::Air | Block::Simple(_) => false,
        _ => b.to_string().contains("powered=true"),
    }
}

/// The least, the most and the count of some values (None when there are none).
fn least_most<T: Copy + PartialOrd>(v: impl IntoIterator<Item = T>) -> Option<(T, T, usize)> {
    v.into_iter().fold(None, |acc, x| match acc {
        None => Some((x, x, 1)),
        Some((lo, hi, n)) => Some((if x < lo { x } else { lo }, if x > hi { x } else { hi }, n + 1)),
    })
}

/// Ticks as `least..most (n N)`, `-` when none.
fn lm_ticks(v: impl IntoIterator<Item = i64>) -> String {
    least_most(v).map_or("-".to_string(), |(lo, hi, n)| format!("{lo}..{hi} (n {n})"))
}

/// Ticks from a batch's REL as `T+a..T+b (n N)`, `-` when none.
fn lm_rel(v: impl IntoIterator<Item = i64>) -> String {
    least_most(v).map_or("-".to_string(), |(lo, hi, n)| format!("T{lo:+}..T{hi:+} (n {n})"))
}

/// Signed lengths as `least..most (n N)` to 7 decimals, `-` when none.
fn lm_len(v: impl IntoIterator<Item = f64>) -> String {
    least_most(v).map_or("-".to_string(), |(lo, hi, n)| format!("{lo:+.7}..{hi:+.7} (n {n})"))
}

/// Coordinates as `least..most` to 6 decimals, `-` when none.
fn lm_pos(v: impl IntoIterator<Item = f64>) -> String {
    least_most(v).map_or("-".to_string(), |(lo, hi, _)| format!("{lo:.6}..{hi:.6}"))
}

/// (ironspec 15:17) Each napper arrival in one lane's lip arrivals (tick, a returning napper's?) in
/// tick order: its index, t minus the nearest batch arrival at or before it, and the nearest batch
/// arrival at or after it minus t.
fn napper_lip_gaps(seq: &[(i64, bool)]) -> Vec<(usize, Option<i64>, Option<i64>)> {
    seq.iter()
        .enumerate()
        .filter(|(_, a)| a.1)
        .map(|(i, a)| {
            let before = seq.iter().filter(|b| !b.1 && b.0 <= a.0).map(|b| a.0 - b.0).min();
            let after = seq.iter().filter(|b| !b.1 && b.0 >= a.0).map(|b| b.0 - a.0).min();
            (i, before, after)
        })
        .collect()
}

/// (ironspec 15:17) K window `i`'s own K(x) writes, from `ctl.k_windows` (its H2 sample's tick, its
/// lane, its K's x) and `ctl.k_fired` (tick, K's x, on): the first on and the first off on its K cell
/// after its sample, within the span the K-write audit gives a window's writes (the sample + 160 + the
/// hold pulse's Delta, and up to the next window's sample + 93).
fn k_window_on_off(windows: &[(i64, usize, i32)], i: usize, fired: &[(i64, i32, bool)], delta: i64) -> (Option<i64>, Option<i64>) {
    let Some(&(t, _, kx)) = windows.get(i) else { return (None, None) };
    let end = (t + 160 + delta).min(windows.get(i + 1).map_or(i64::MAX, |w| w.0.saturating_add(93)));
    let first = |on: bool| fired.iter().find(|f| f.0 > t && f.0 <= end && f.1 == kx && f.2 == on).map(|f| f.0);
    (first(true), first(false))
}

/// (ironspec 15:17) The napper's lip arrival against the batch's lane carts: per lane, each returning
/// napper's lip arrival (mobfarm::lipclass, tick t) against the nearest batch-class arrival there at or
/// before it (t - t_before) and at or after it (t_after - t), and against its own K window, the one
/// keyed at the latest H2 sample before t that caught this napper on the hold (t - K_on, K_off - t).
fn napper_lip_line(arr: &[Vec<crate::lipclass::LipArrival>; 3], who: &[Vec<Option<u32>>; 3], windows: &[(i64, usize, i32)], win_napper: &[Option<u32>], fired: &[(i64, i32, bool)], delta: i64) -> String {
    let per: Vec<String> = (0..3)
        .map(|k| {
            let seq: Vec<(i64, bool)> = arr[k].iter().zip(who[k].iter()).map(|(a, w)| (a.tick, w.is_some())).collect();
            let gaps = napper_lip_gaps(&seq);
            let (mut kon, mut koff, mut no_win) = (Vec::new(), Vec::new(), 0usize);
            for (i, _, _) in gaps.iter() {
                let t = seq[*i].0;
                let own = (0..windows.len()).rev().find(|j| windows[*j].0 < t && win_napper.get(*j).copied().flatten() == who[k][*i]);
                match own.map(|j| k_window_on_off(windows, j, fired, delta)) {
                    Some((on, off)) if on.is_some() || off.is_some() => {
                        kon.extend(on.map(|on| t - on));
                        koff.extend(off.map(|off| off - t));
                    }
                    _ => no_win += 1,
                }
            }
            format!(
                "lane x {}: {} napper arrivals of {}; t - t_before {}; t_after - t {}; t - K_on {}; K_off - t {}{}",
                LANES[k],
                gaps.len(),
                seq.len(),
                lm_ticks(gaps.iter().filter_map(|g| g.1)),
                lm_ticks(gaps.iter().filter_map(|g| g.2)),
                lm_ticks(kon),
                lm_ticks(koff),
                if no_win > 0 { format!("; {no_win} without its own K window") } else { String::new() }
            )
        })
        .collect();
    format!(
        "the napper's lip arrival against the batch's lane carts (ironspec 15:17: per lane L, each returning napper's lip arrival (mobfarm::lipclass; a returning napper from its stop's R until back in its lane, as the lift's mouth classes it; every other cart batch) at tick t against the nearest batch-class lip arrival at L at or before it (t - t_before) and at or after it (t_after - t), and against its own K window (keyed at the latest H2 sample before t that caught this napper on the hold (9,0,8); its K(x) on / off): t - K_on, K_off - t; least..most per lane; information): {}\n",
        per.join(" | ")
    )
}

/// (dfb89499 15:11 (a)) One cart's visit to a station hold H (x,16,0), an oak trapdoor where a cart
/// rests at about (x + 0.5, 16.1875, 0.49) against the ring's north glass (x,16,-1) (its south face z
/// 0.0): whether it stood there at the sampler's first tick (placed, it never entered); its settle (its
/// first tick in the cell with |dpos| <= 1e-6) and its position then; against the glass (gap z - 0.49:
/// positive clear, negative into the glass) the longest run of ticks at gap <= 1e-6 and the largest
/// depth 0.49 - z; the trapdoor's first open tick in the visit; any move (|dpos| > 1e-6) after the
/// settle before the trapdoor opened (its first tick, the largest |dpos|); its last tick in the cell.
#[derive(Clone, Copy, Debug)]
struct HoldVisit {
    cart: u32,
    placed: bool,
    settle: Option<(i64, [f64; 3])>,
    run: i64,
    best_run: i64,
    depth: f64,
    opened: Option<i64>,
    moved: Option<(i64, f64)>,
    last: i64,
}

impl HoldVisit {
    fn new(cart: u32, t: i64, placed: bool) -> HoldVisit {
        HoldVisit { cart, placed, settle: None, run: 0, best_run: 0, depth: f64::NEG_INFINITY, opened: None, moved: None, last: t }
    }

    /// One tick in the hold's cell: the cart's design-frame position, its |dpos| since the tick before
    /// (infinite when it wasn't seen then) and whether the trapdoor stands open.
    fn tick(&mut self, t: i64, p: [f64; 3], step: f64, open: bool) {
        self.last = t;
        let gap = p[2] - 0.49;
        if gap <= 1e-6 {
            self.run += 1;
            self.best_run = self.best_run.max(self.run);
        } else {
            self.run = 0;
        }
        self.depth = self.depth.max(-gap);
        if open && self.opened.is_none() {
            self.opened = Some(t);
        }
        if self.settle.is_none() {
            if step <= 1e-6 {
                self.settle = Some((t, p));
            }
        } else if self.opened.is_none() && step > 1e-6 {
            let m = self.moved.get_or_insert((t, step));
            m.1 = m.1.max(step);
        }
    }
}

/// (dfb89499 15:11 (b)) One cart's stay on a lane's queue cell (slot 1 R_1 (x,17,1), 2..6 the slopes
/// (x,16+s,s)): the lane, the slot, whether it stood there at the sampler's first tick, the last tick
/// seen there and its z at its latest tick there with |dpos| <= 1e-6.
#[derive(Clone, Copy, Debug)]
struct SlotVisit {
    lane: usize,
    slot: usize,
    placed: bool,
    seen: i64,
    rest_z: Option<f64>,
}

/// (dfb89499 15:11 (b)) A closed queue-cell stay on R_1 or slot 2: its resting z (and whether it was
/// placed there), or one more stay that left without a rest.
fn close_slot(rest_z: &mut [[Vec<(f64, bool)>; 2]; 3], no_rest: &mut [[usize; 2]; 3], v: SlotVisit) {
    if v.slot == 1 || v.slot == 2 {
        match v.rest_z {
            Some(z) => rest_z[v.lane][v.slot - 1].push((z, v.placed)),
            None => no_rest[v.lane][v.slot - 1] += 1,
        }
    }
}

/// (dfb89499 15:11 (b)) The station's batch events, in `StBatch::ticks`'s order.
const ST_EVENTS: [&str; 15] = [
    "the trapdoor open",
    "the trapdoor shut",
    "the input rise",
    "the input fall",
    "R_1 on",
    "R_1 off",
    "the hold cart's last tick in H",
    "the R_1 cart's first move",
    "the R_1 cart off R_1",
    "the R_1 cart into H",
    "the R_1 cart at rest in H",
    "the slot-2 cart's first move",
    "the slot-2 cart off slot 2",
    "the slot-2 cart onto R_1",
    "the slot-2 cart at rest on R_1",
];

/// (dfb89499 15:11 (b)) One batch at one lane's station, from its REL: the trapdoor (x,16,0) open /
/// shut, the R line input (x+2,17,1) rise / fall and R_1 (x,17,1) powered on / off (each the first
/// edge in the batch); the hold cart at REL and its last tick in H; the R_1 cart and the slot-2 cart at
/// REL, each: its first move, its first tick off its cell, its entry into the cell below (H; R_1) and
/// its first tick at rest there.
#[derive(Clone, Copy, Debug, Default)]
struct StBatch {
    rel: i64,
    trap: [Option<i64>; 2],
    input: [Option<i64>; 2],
    r1: [Option<i64>; 2],
    hold: Option<(u32, Option<i64>)>,
    carts: [Option<(u32, [Option<i64>; 4])>; 2],
}

impl StBatch {
    /// Its events' ticks, in ST_EVENTS's order.
    fn ticks(&self) -> [Option<i64>; 15] {
        let c = |r: usize, i: usize| self.carts[r].and_then(|(_, e)| e[i]);
        [self.trap[0], self.trap[1], self.input[0], self.input[1], self.r1[0], self.r1[1], self.hold.and_then(|h| h.1), c(0, 0), c(0, 1), c(0, 2), c(0, 3), c(1, 0), c(1, 1), c(1, 2), c(1, 3)]
    }

    /// Its sequence: every event seen, in tick order (ties in ST_EVENTS's order), T+n from its REL.
    fn sequence(&self) -> String {
        let id = |c: Option<u32>| c.map_or("-".to_string(), |c| format!("#{c}"));
        let (h, a, b) = (id(self.hold.map(|h| h.0)), id(self.carts[0].map(|c| c.0)), id(self.carts[1].map(|c| c.0)));
        let mut ev: Vec<(i64, String)> = self
            .ticks()
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let n = ST_EVENTS[i];
                t.map(|t| (t, match i {
                    6 => format!("{n} ({h})"),
                    7..=10 => format!("{n} ({a})"),
                    11..=14 => format!("{n} ({b})"),
                    _ => n.to_string(),
                }))
            })
            .collect();
        ev.sort_by_key(|e| e.0);
        ev.iter().map(|(t, n)| format!("T{:+} {n}", t - self.rel)).collect::<Vec<_>>().join(", ")
    }
}

/// (dfb89499 15:11 (b)) A cell's place in a lane's queue: (lane, slot), slot 0 the hold H (x,16,0), 1
/// R_1 (x,17,1), 2..6 the slopes (x,16+s,s) up to V (x,22,6).
fn queue_slot(c: [i32; 3]) -> Option<(usize, usize)> {
    let k = LANES.iter().position(|x| *x == c[0])?;
    (c[1] == 16 + c[2] && (0..=6).contains(&c[2])).then_some((k, c[2] as usize))
}

/// (ironspec 15:23 (5)(a)) The cart-cart contact regions, and a cell in one.
const TOUCH_REGIONS: [&str; 3] = ["the split row (y 24..26, z 7..10)", "the shared lift column (x -6..-4)", "the merge row (y 0..3, z 6..8)"];

fn in_touch_region(r: usize, c: [i32; 3]) -> bool {
    match r {
        0 => (24..=26).contains(&c[1]) && (7..=10).contains(&c[2]),
        1 => (-6..=-4).contains(&c[0]),
        _ => (0..=3).contains(&c[1]) && (6..=8).contains(&c[2]),
    }
}

/// (dfb89499 15:11 (b)) A batch's first rise of a cell's state and the first fall after it.
fn st_edge(v: &mut [Option<i64>; 2], on: bool, was: bool, t: i64) {
    if on && !was && v[0].is_none() {
        v[0] = Some(t);
    }
    if !on && was && v[0].is_some() && v[1].is_none() {
        v[1] = Some(t);
    }
}

/// (the touch sampler) One cart this tick: its id, design-frame position, cell, |dpos| since the tick
/// before (infinite: not seen then) and rider.
#[derive(Clone, Copy, Debug)]
struct CartNow {
    id: u32,
    p: [f64; 3],
    c: [i32; 3],
    step: f64,
    rider: Option<u32>,
}

/// (ironspec 15:23 (5)(b)) One napper cart's visit to the hold (9,0,8): its settle (its first tick
/// there with |dpos| <= 1e-6 before the hold rail rises, and its position) and the rail's rise.
#[derive(Clone, Copy, Debug)]
struct SeatVisit {
    cart: u32,
    settle: Option<(i64, [f64; 3])>,
    rise: Option<i64>,
}

/// (dfb89499 15:11, ironspec 15:23 (5) for 7b90c3f1) The touching contacts, sampled every tick: per
/// lane the station's hold H (x,16,0) visits, the R_1 (x,17,1) / slot 2 (x,18,2) rests, the gaps
/// between consecutive carts at rest on the slope and each batch's sequence; every pair of carts within
/// 0.01 of touching per region; the napper's seat on the hold (9,0,8) against the hold rail's rise.
#[derive(Default)]
struct TouchSampler {
    /// the first tick sampled (a cart in a cell then was placed there, it never entered)
    t0: Option<i64>,
    /// every cart's design-frame position at the tick it was last seen (its |dpos|)
    prev: BTreeMap<u32, ([f64; 3], i64)>,
    /// this tick's carts (reused every tick)
    carts: Vec<CartNow>,
    /// per lane last tick: the trapdoor open, the R line input on, R_1 powered
    trap_prev: [bool; 3],
    input_prev: [bool; 3],
    r1_prev: [bool; 3],
    /// the open hold visits (lane, visit) by cart, and the closed ones per lane
    hold_open: BTreeMap<u32, (usize, HoldVisit)>,
    hold_done: [Vec<HoldVisit>; 3],
    /// the open queue-cell stays by cart; per lane the closed stays' resting z on R_1 / slot 2 (placed
    /// there?) and those that left without a rest
    slot_open: BTreeMap<u32, SlotVisit>,
    rest_z: [[Vec<(f64, bool)>; 2]; 3],
    no_rest: [[usize; 2]; 3],
    /// per lane the gaps of consecutive carts both at rest on the slope, R_1-slot 2 and the pairs above
    /// it: [least, most] of |dz| - 0.98, then of the box gap
    pair_r1s2: [Option<[f64; 4]>; 3],
    pair_up: [Option<[f64; 4]>; 3],
    /// per lane every batch
    batches: [Vec<StBatch>; 3],
    /// per contact region: the pair-ticks within 0.01, those overlapping, and the least (gap, tick,
    /// carts)
    contact_n: [usize; 3],
    contact_overlap: [usize; 3],
    contact_least: [Option<(f64, i64, u32, u32)>; 3],
    /// the napper's seat on the hold (9,0,8): the open visit, the closed ones, the rail's power last
    /// tick
    seat_open: Option<SeatVisit>,
    seat_done: Vec<SeatVisit>,
    seat_rail_prev: bool,
}

impl TouchSampler {
    /// One tick, after the world's tick and the controller's step: `rels` the controller's batches
    /// (REL, pick), `nappers` the run's nappers (rider, lane, tick).
    fn tick(&mut self, w: &redsim::world::World, rel: i64, rels: &[(i64, Option<usize>)], nappers: &[(u32, usize, i64)]) {
        let placed = *self.t0.get_or_insert(rel) == rel;
        self.carts.clear();
        for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))) {
            let d = dpos(e.pos);
            let p = [d.x, d.y, d.z];
            let step = match self.prev.get(&e.id) {
                Some((q, t)) if *t == rel - 1 => ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt(),
                _ => f64::INFINITY,
            };
            self.carts.push(CartNow { id: e.id, p, c: cell(e.pos), step, rider: e.passengers.first().copied() });
        }
        for c in &self.carts {
            self.prev.insert(c.id, (c.p, rel));
        }
        self.prev.retain(|_, v| v.1 == rel);
        self.station(w, rel, rels, placed);
        self.contacts(rel);
        self.seat(w, rel, nappers);
    }

    /// (dfb89499 15:11) The station's lanes this tick: each batch's events, the hold visits, the
    /// queue cells' stays and the at-rest gaps on the slope.
    fn station(&mut self, w: &redsim::world::World, rel: i64, rels: &[(i64, Option<usize>)], placed: bool) {
        let new_batch = rels.iter().rev().take(4).any(|(r, _)| *r == rel);
        let mut occ: [[Option<usize>; 7]; 3] = [[None; 7]; 3];
        for (i, c) in self.carts.iter().enumerate() {
            if let Some((k, s)) = queue_slot(c.c)
                && occ[k][s].is_none()
            {
                occ[k][s] = Some(i);
            }
        }
        let mut trap = [false; 3];
        for k in 0..3 {
            let x = LANES[k];
            trap[k] = matches!(w.get(dbp(x, 16, 0)), redsim::block::Block::Trapdoor { open: true, .. });
            let input = on_state(w.get(dbp(x + 2, 17, 1)));
            let r1 = on_state(w.get(dbp(x, 17, 1)));
            if new_batch {
                let id = |s: usize| occ[k][s].map(|i| self.carts[i].id);
                self.batches[k].push(StBatch { rel, hold: id(0).map(|c| (c, None)), carts: [id(1).map(|c| (c, [None; 4])), id(2).map(|c| (c, [None; 4]))], ..Default::default() });
            }
            if let Some(b) = self.batches[k].last_mut() {
                st_edge(&mut b.trap, trap[k], self.trap_prev[k], rel);
                st_edge(&mut b.input, input, self.input_prev[k], rel);
                st_edge(&mut b.r1, r1, self.r1_prev[k], rel);
                // the R_1 cart (from slot 1 down to H) and the slot-2 cart (from slot 2 down to R_1)
                for (r, s0) in [(0usize, 1usize), (1, 2)] {
                    let Some((cid, ev)) = b.carts[r].as_mut() else { continue };
                    let Some(c) = self.carts.iter().find(|c| c.id == *cid) else { continue };
                    let at = queue_slot(c.c).filter(|q| q.0 == k).map(|q| q.1);
                    if ev[0].is_none() && c.step.is_finite() && c.step > 1e-6 {
                        ev[0] = Some(rel);
                    }
                    if ev[1].is_none() && at != Some(s0) {
                        ev[1] = Some(rel);
                    }
                    if ev[2].is_none() && at == Some(s0 - 1) {
                        ev[2] = Some(rel);
                    }
                    if ev[2].is_some() && ev[3].is_none() && at == Some(s0 - 1) && c.step <= 1e-6 {
                        ev[3] = Some(rel);
                    }
                }
            }
            self.trap_prev[k] = trap[k];
            self.input_prev[k] = input;
            self.r1_prev[k] = r1;
        }
        // the hold visits (slot 0) and the queue cells' stays (slots 1..6)
        for c in &self.carts {
            let Some((k, s)) = queue_slot(c.c) else { continue };
            if s == 0 {
                self.hold_open.entry(c.id).or_insert((k, HoldVisit::new(c.id, rel, placed))).1.tick(rel, c.p, c.step, trap[k]);
                continue;
            }
            let fresh = SlotVisit { lane: k, slot: s, placed, seen: rel, rest_z: None };
            let v = self.slot_open.entry(c.id).or_insert(fresh);
            if v.lane != k || v.slot != s {
                close_slot(&mut self.rest_z, &mut self.no_rest, *v);
                *v = fresh;
            }
            v.seen = rel;
            if c.step <= 1e-6 {
                v.rest_z = Some(c.p[2]);
            }
        }
        // the visits and stays whose cart left (or vanished) this tick
        let (done, batches) = (&mut self.hold_done, &mut self.batches);
        self.hold_open.retain(|_, (k, v)| {
            if v.last == rel {
                return true;
            }
            if let Some(b) = batches[*k].last_mut()
                && b.hold.is_some_and(|h| h.0 == v.cart && h.1.is_none())
            {
                b.hold = Some((v.cart, Some(v.last)));
            }
            done[*k].push(*v);
            false
        });
        let (rest_z, no_rest) = (&mut self.rest_z, &mut self.no_rest);
        self.slot_open.retain(|_, v| {
            if v.seen == rel {
                return true;
            }
            close_slot(rest_z, no_rest, *v);
            false
        });
        // the gaps of consecutive carts both at rest on the slope
        for k in 0..3 {
            for s in 1..6 {
                let (Some(i), Some(j)) = (occ[k][s], occ[k][s + 1]) else { continue };
                let (a, b) = (self.carts[i], self.carts[j]);
                if a.step > 1e-6 || b.step > 1e-6 {
                    continue;
                }
                let zg = (a.p[2] - b.p[2]).abs() - 0.98;
                let bg = cart_box_gap(a.p, b.p);
                let slot = if s == 1 { &mut self.pair_r1s2[k] } else { &mut self.pair_up[k] };
                let e = slot.get_or_insert([zg, zg, bg, bg]);
                *e = [e[0].min(zg), e[1].max(zg), e[2].min(bg), e[3].max(bg)];
            }
        }
    }

    /// (ironspec 15:23 (5)(a)) Every pair of carts within 0.01 of touching this tick, in each region
    /// either cart's cell is in.
    fn contacts(&mut self, rel: i64) {
        let n = self.carts.len();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (self.carts[i], self.carts[j]);
                let g = cart_box_gap(a.p, b.p);
                if g > 0.01 {
                    continue;
                }
                for r in 0..3 {
                    if !(in_touch_region(r, a.c) || in_touch_region(r, b.c)) {
                        continue;
                    }
                    self.contact_n[r] += 1;
                    if g < 0.0 {
                        self.contact_overlap[r] += 1;
                    }
                    if self.contact_least[r].is_none_or(|l| g < l.0) {
                        self.contact_least[r] = Some((g, rel, a.id.min(b.id), a.id.max(b.id)));
                    }
                }
            }
        }
    }

    /// (ironspec 15:23 (5)(b)) The napper's seat on the hold (9,0,8) this tick.
    fn seat(&mut self, w: &redsim::world::World, rel: i64, nappers: &[(u32, usize, i64)]) {
        let seat = self.carts.iter().copied().find(|c| c.c == [9, 0, 8] && c.rider.is_some_and(|r| nappers.iter().any(|(v, _, _)| *v == r)));
        let rail = on_state(w.get(dbp(9, 0, 8)));
        if let Some(v) = self.seat_open
            && seat.is_none_or(|c| c.id != v.cart)
        {
            self.seat_done.push(v);
            self.seat_open = None;
        }
        if let Some(c) = seat {
            let v = self.seat_open.get_or_insert(SeatVisit { cart: c.id, settle: None, rise: None });
            if v.settle.is_none() && v.rise.is_none() && c.step <= 1e-6 {
                v.settle = Some((rel, c.p));
            }
            if rail && !self.seat_rail_prev && v.rise.is_none() {
                v.rise = Some(rel);
            }
        }
        self.seat_rail_prev = rail;
    }

    /// Its carts across a world open (`f`: old -> new id; G1's P8: the id-keyed open state follows
    /// the open): the open keys' (old, new) for the open's log.
    fn remap(&mut self, f: &dyn Fn(u32) -> u32) -> (Vec<u32>, Vec<u32>) {
        let old: Vec<u32> = self.prev.keys().copied().collect();
        let new: Vec<u32> = old.iter().map(|id| f(*id)).collect();
        self.prev = std::mem::take(&mut self.prev).into_iter().map(|(id, v)| (f(id), v)).collect();
        self.hold_open = std::mem::take(&mut self.hold_open)
            .into_iter()
            .map(|(id, (k, mut v))| {
                v.cart = f(v.cart);
                (f(id), (k, v))
            })
            .collect();
        self.slot_open = std::mem::take(&mut self.slot_open).into_iter().map(|(id, v)| (f(id), v)).collect();
        for b in self.batches.iter_mut().flatten() {
            if let Some(h) = b.hold.as_mut() {
                h.0 = f(h.0);
            }
            for c in b.carts.iter_mut().flatten() {
                c.0 = f(c.0);
            }
        }
        if let Some(v) = self.seat_open.as_mut() {
            v.cart = f(v.cart);
        }
        (old, new)
    }

    /// The report's lines: the station's hold contacts (dfb89499 15:11 (a)), its R_1 / slot 2 rests
    /// and batch sequence ((b)), the cart-cart contacts and the napper's seat (ironspec 15:23 (5)).
    fn report(&self) -> String {
        let mut hold_rows: Vec<String> = Vec::new();
        let mut seq_rows: Vec<String> = Vec::new();
        for k in 0..3 {
            let bs = &self.batches[k];
            let rel_of = |t: i64| bs.iter().rev().find(|b| b.rel <= t).map(|b| t - b.rel);
            let open: Vec<&HoldVisit> = self.hold_open.values().filter(|(kk, _)| *kk == k).map(|(_, v)| v).collect();
            let placed = self.hold_done[k].iter().chain(open.iter().copied()).filter(|v| v.placed).count();
            let still = open.iter().filter(|v| !v.placed).count();
            let all: Vec<&HoldVisit> = self.hold_done[k].iter().chain(open.iter().copied()).filter(|v| !v.placed).collect();
            let settled: Vec<[f64; 3]> = all.iter().filter_map(|v| v.settle.map(|s| s.1)).collect();
            let moved: Vec<(u32, i64, f64)> = all.iter().filter_map(|v| v.moved.map(|(t, d)| (v.cart, t, d))).collect();
            let worst = moved.iter().copied().max_by(|a, b| a.2.total_cmp(&b.2));
            hold_rows.push(format!(
                "lane x {}: {} visits ({} still in H at the end, {} never settled; {} placed in H at t0 not counted); the rest gap z - 0.49 {}; at rest x {}, y {}, z {}; the longest touching run (gap <= 1e-6) {} gt; the largest depth 0.49 - z {}; moved after settling before the trapdoor opened: {}{}; the trapdoor open {}, shut {}; the last tick in H {}",
                LANES[k],
                all.len(),
                still,
                all.len() - settled.len(),
                placed,
                lm_len(settled.iter().map(|p| p[2] - 0.49)),
                lm_pos(settled.iter().map(|p| p[0])),
                lm_pos(settled.iter().map(|p| p[1])),
                lm_pos(settled.iter().map(|p| p[2])),
                lm_ticks(all.iter().map(|v| v.best_run)),
                lm_len(all.iter().map(|v| v.depth)),
                moved.len(),
                worst.map_or(String::new(), |(c, t, d)| format!(" (the largest |dpos| {d:.7}: cart #{c} from t {t}{})", rel_of(t).map_or(String::new(), |r| format!(", T{r:+}")))),
                lm_rel(bs.iter().filter_map(|b| b.trap[0].map(|t| t - b.rel))),
                lm_rel(bs.iter().filter_map(|b| b.trap[1].map(|t| t - b.rel))),
                lm_rel(self.hold_done[k].iter().filter_map(|v| rel_of(v.last)))
            ));
            let rest = |slot: usize, placed: bool| -> Vec<f64> {
                self.rest_z[k][slot - 1]
                    .iter()
                    .filter(|r| r.1 == placed)
                    .map(|r| r.0)
                    .chain(self.slot_open.values().filter(|v| v.lane == k && v.slot == slot && v.placed == placed).filter_map(|v| v.rest_z))
                    .collect()
            };
            let zs = |v: Vec<f64>| if v.is_empty() { "-".to_string() } else { format!("{} (n {})", lm_pos(v.iter().copied()), v.len()) };
            let pair = |p: Option<[f64; 4]>| p.map_or("-".to_string(), |e| format!("|dz| - 0.98 {:+.7}..{:+.7}, the box gap {:+.7}..{:+.7}", e[0], e[1], e[2], e[3]));
            let full_n = bs.iter().filter(|b| b.ticks().iter().all(|t| t.is_some())).count();
            let full = bs.iter().find(|b| b.ticks().iter().all(|t| t.is_some()));
            let fullest = bs.iter().max_by_key(|b| (b.ticks().iter().filter(|t| t.is_some()).count(), std::cmp::Reverse(b.rel)));
            let evs: Vec<String> = (2..15).filter(|i| *i != 6).map(|i| format!("{} {}", ST_EVENTS[i], lm_rel(bs.iter().filter_map(|b| b.ticks()[i].map(|t| t - b.rel))))).collect();
            seq_rows.push(format!(
                "lane x {}: the resting z on R_1 {} ({} left without a rest; placed at t0 {}), on slot 2 {} ({} left without a rest; placed at t0 {}); at rest R_1-slot 2: {}; the pairs above it: {}; {} batches, {} full: {}; {}",
                LANES[k],
                zs(rest(1, false)),
                self.no_rest[k][0],
                zs(rest(1, true)),
                zs(rest(2, false)),
                self.no_rest[k][1],
                zs(rest(2, true)),
                pair(self.pair_r1s2[k]),
                pair(self.pair_up[k]),
                bs.len(),
                full_n,
                evs.join(", "),
                match (full, fullest) {
                    (Some(b), _) => format!("the first full batch (REL t {}): {}", b.rel, b.sequence()),
                    (None, Some(b)) => format!("no full batch; the first fullest (REL t {}): {}", b.rel, b.sequence()),
                    (None, None) => "no batch".to_string(),
                }
            ));
        }
        let contacts: Vec<String> = (0..3)
            .map(|r| format!("{}: {} pair-ticks, {} overlapping, the least {}", TOUCH_REGIONS[r], self.contact_n[r], self.contact_overlap[r], self.contact_least[r].map_or("-".to_string(), |(g, t, a, b)| format!("{g:+.7} at t {t} (#{a}, #{b})"))))
            .collect();
        let seats: Vec<SeatVisit> = self.seat_done.iter().copied().chain(self.seat_open).collect();
        let settled: Vec<[f64; 3]> = seats.iter().filter_map(|v| v.settle.map(|s| s.1)).collect();
        let waits: Vec<i64> = seats.iter().filter_map(|v| Some(v.rise? - v.settle?.0)).collect();
        format!(
            "the station's hold contacts (dfb89499 15:11 (a): per lane over all batches, each cart's visit to the hold H (x,16,0), an oak trapdoor, against the ring's north glass (x,16,-1) (its south face z 0.0; gap z - 0.49, positive clear, negative into the glass): its rest once settled (its first tick in H with |dpos| <= 1e-6), the longest run of ticks at gap <= 1e-6, its largest depth 0.49 - z (negative: clear), a move (|dpos| > 1e-6) after the settle before the trapdoor opened (open=true); the trapdoor's open / shut and the cart's last tick in H, T+n from the batch's REL; least..most; a cart placed in H at t0 never entered it; information): {}\n\
             the station's R_1 / slot 2 rests and batch sequence (dfb89499 15:11 (b): per lane, each cart's resting z on R_1 (x,17,1) and on slot 2 (x,18,2) before it leaves (its z at its last tick there with |dpos| <= 1e-6; the stays still open at the end included, a cart placed there at t0 apart), the gaps between consecutive carts both at rest on the slope (R_1-slot 2, and the pairs above it up to V (x,22,6): |dz| - 0.98 along the slope, and the box gap of the 0.98 x 0.7 boxes); per batch, T+n from its REL: the R line input (x+2,17,1) rise / fall (powered=true or a redstone block), R_1 powered on / off, the R_1 cart's first move (|dpos| > 1e-6), off R_1, into H and at rest in H, the slot-2 cart's first move, off slot 2, onto R_1 and at rest on R_1; least..most over batches, and the first full batch's sequence verbatim; information): {}\n\
             the cart-cart contacts (ironspec 15:23 (5)(a) for 7b90c3f1: every tick, every pair of carts whose boxes (0.98 x 0.7 x 0.98 on each centre) are within 0.01 of touching, in each region either cart's cell is in; per region the pair-ticks, those overlapping and the least box gap (negative = overlap) with its tick and carts; information): {}\n\
             the napper's seat on the hold (ironspec 15:23 (5)(b) for 7b90c3f1: each napper cart's visit to the hold (9,0,8): its settled position (its first tick there with |dpos| <= 1e-6, before the hold rail rises) and the tick the rail (9,0,8) turns powered=true; least..most of rise - settle; information): {} visits; rise - settle {} gt; settled at x {}, y {}, z {}; {} rises before a settle; {} visits with no rise\n",
            hold_rows.join(" | "),
            seq_rows.join(" | "),
            contacts.join("; "),
            seats.len(),
            lm_ticks(waits),
            lm_pos(settled.iter().map(|p| p[0])),
            lm_pos(settled.iter().map(|p| p[1])),
            lm_pos(settled.iter().map(|p| p[2])),
            seats.iter().filter(|v| v.settle.is_none() && v.rise.is_some()).count(),
            seats.iter().filter(|v| v.rise.is_none()).count()
        )
    }
}

#[cfg(test)]
mod counter_tests {
    /// (G1's P10) a FAIL anywhere in a whole log fails its line (one at line 230 of 250 included);
    /// "unexpected FAIL" keeps its FAIL; a lone "expected FAIL" token doesn't fail a report line
    #[test]
    fn a_fail_deep_in_a_log_fails() {
        let mut log: Vec<String> = (0..250).map(|i| format!("t {i}: fine")).collect();
        assert!(super::log_fail_line("run log", &log).contains("(PASS)"));
        log[229] = "t 229: the bar FAIL".to_string();
        let l = super::log_fail_line("run log", &log);
        assert!(l.contains(": 1 (FAIL)") && l.contains("line 230"), "{l}");
        assert_eq!(super::fail_lines("x: 3 (expected FAIL)\ny: unexpected FAIL\nz: PASS"), vec!["y: unexpected FAIL"]);
    }

    /// (onechunk 15:56, childA's finding) a bar's description "each writer justified or FAIL" is no
    /// verdict; the fold's integrity line parses with its move (G1 16:18 (2))
    #[test]
    fn the_fold_integrity_line_parses_moved() {
        let t = "# integrity: -2,0,4=connections -1,0,4+-2,0,5; holds -2,0,3=north_east; holds -1,0,3=east_west; holds -1,0,4=east_west\n";
        let v = super::parse_integrity(t, [0, -27, -1]).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].0, [-2, -27, 3]);
        assert_eq!(v[0].1, "south_east");
        assert_eq!(v[1].0, [-2, -27, 2]);
        assert_eq!(v[1].1, "north_east");
        assert!(super::parse_integrity("# integrity: 1,2,3=sideways\n", [0, 0, 0]).is_err());
    }

    /// (onechunk 14:26) a FAIL verdict is a whole-word FAIL, not a bar's description of what FAILS
    /// nor a log judge's "FAIL lines"; every verdict form the report prints still fails
    #[test]
    fn only_a_fail_verdict_fails_a_line() {
        for pass in [
            "the ridden departures (a cart not the napper's pressing it FAILS): 22 by nappers, 0 by others (PASS)",
            "the run log's FAIL lines over all its 0 lines (G1's P10: judged uncapped): 0 (PASS)",
            "the per-lane cart count (a 9th FAILS): lane x -1 the most 8 (PASS)",
            "a FAILED word and FAILURE are no verdicts: PASS",
            "the emulation's no-op writes (G1 X16: a write of the cell's existing state; each writer justified or FAIL): 0 (PASS)",
        ] {
            assert!(!super::has_fail_verdict(pass), "{pass}");
        }
        for fail in [
            "naps per night: night 0: 14 villagers napped (FAIL, bar 24)",
            "the catch window: the least late slack 1 gt (bar >= 2: FAIL), the least early",
            "F4(a)'s bars: (i) a K switched: 8 (FAIL): t 4669",
            "the no-string ring: NOT JUDGED (no pick): FAIL",
            "each napper's K writes: 2 writes off its cell (FAIL: lane x 3's window)",
            "the run log's FAIL lines over all its 9 lines: 1 (FAIL): line 3",
            "FAIL",
        ] {
            assert!(super::has_fail_verdict(fail), "{fail}");
        }
    }

    /// (G1 10:47 blocking 1, G1's P9) across a world open: a renewed id follows; a live id the open
    /// didn't renew errs; a dead id keeps its id unless that id now names a renewed entity (an alias)
    #[test]
    fn a_dead_id_never_aliases_a_renewed_one() {
        let m: std::collections::HashMap<u32, u32> = [(10, 0), (11, 1), (12, 7)].into_iter().collect();
        let live: std::collections::BTreeSet<u32> = [10, 11, 12, 13].into_iter().collect();
        assert_eq!(super::open_follow(11, &m, &live), Ok(1));
        assert!(super::open_follow(13, &m, &live).is_err());
        assert_eq!(super::open_follow(5, &m, &live), Ok(5));
        assert!(super::open_follow(7, &m, &live).unwrap_err().contains("alias"));
    }

    /// (G1's note on 6248be579) C at a first release 49 / 50 / 51 gt either side of the 12,490 grid
    #[test]
    fn the_first_release_s_c_at_the_grid_s_ties() {
        let c = |off: i64| super::counter_c(12_490 + off);
        // (G1's four ties, day-1's built counter: 141 + ceil((d - 12,540) / 100))
        assert_eq!([c(50), c(150), c(-50), c(-150)], [141, 142, 140, 139]);
        assert_eq!([c(49), c(51), c(-49), c(-51)], [141, 142, 141, 140]);
        assert_eq!([c(0), c(100), c(-100)], [141, 142, 140]);
    }

    /// (G1 N13) ValidateNearbyPoi's sphere at 16 ± 0.01 from the bed head's centre, along each axis
    #[test]
    fn the_exposure_sphere_is_strictly_sixteen() {
        let bed = [-3, -27, 3];
        let c = |dx: f64, dy: f64, dz: f64| super::poi_near(bed, redsim::entity::Vec3::new(-2.5 + dx, -26.5 + dy, 3.5 + dz));
        assert!(c(15.99, 0.0, 0.0) && c(0.0, -15.99, 0.0) && c(0.0, 0.0, 15.99));
        assert!(!c(16.01, 0.0, 0.0) && !c(0.0, 16.01, 0.0) && !c(0.0, 0.0, -16.01));
        assert!(!c(16.0, 0.0, 0.0), "strict: exactly 16 is outside");
    }

    /// (G1 X23) judge_controls: a control caught on its own sample turns its line's FAIL into
    /// "expected FAIL" (fail_lines then empty); a natural failure beside it is NOT CAUGHT ALONE, the
    /// line left failing (fail_lines non-empty); a control that never acted is NOT EXERCISED
    #[test]
    fn judge_controls_bites_only_on_its_own_sample() {
        let rep = "  the air-supply bar (x): 3 transits (FAIL)\nother line (PASS)\n";
        let own = super::judge_controls(rep, &[("--air-control".to_string(), "  the air-supply bar", true, true, true)]);
        assert!(own.contains("CONTROL CAUGHT"), "{own}");
        assert!(super::fail_lines(&own).is_empty(), "{own}");
        let natural = super::judge_controls(rep, &[("--air-control".to_string(), "  the air-supply bar", true, true, false)]);
        assert!(natural.contains("NOT CAUGHT ALONE"), "{natural}");
        assert!(!super::fail_lines(&natural).is_empty());
        let idle = super::judge_controls(rep, &[("--air-control".to_string(), "  the air-supply bar", true, false, true)]);
        assert!(idle.contains("NOT EXERCISED"));
        assert!(!super::fail_lines(&idle).is_empty());
        // a held control (must pass) on a passing line: HELD, nothing failing
        let held = super::judge_controls("other line (PASS)\n", &[("--dud".to_string(), "other line", false, true, true)]);
        assert!(held.contains("HELD") && super::fail_lines(&held).is_empty(), "{held}");
    }

    /// (G1 / ironspec 08:31-08:33 platform; G1 10:47 blocking 1 / O1) an emulated stand-in across a
    /// world open: every controller key and log follows the open's id map (a napper re-created
    /// mid-route stays routed); a dead id keeps its old id; a LIVE id the open didn't renew is an
    /// error, never kept
    #[test]
    fn the_emulation_follows_a_world_open() {
        let mut ctl = super::Ctl { held_by: Some(40), pick_cart: Some((41, 0)), divert: Some((42, 2)), release_log: vec![(10, 40)], dep_log: vec![(11, 41)], pick_log: vec![(12, 40)], divert_log: vec![(13, 42)], pick_carts: vec![(14, 41, 40, 0)], ..Default::default() };
        let mut nappers = vec![(40u32, 0usize, 100i64), (7, 1, 50)];
        let map: std::collections::HashMap<u32, u32> = [(40, 140), (41, 141), (42, 142)].into_iter().collect();
        // (7 died before the open: kept, said so by its unchanged pair; 99 is live and unrenewed)
        let live: std::collections::BTreeSet<u32> = [40, 41, 42, 99].into_iter().collect();
        let fol = |id: u32| -> Result<u32, String> {
            match map.get(&id) {
                Some(n) => Ok(*n),
                None if live.contains(&id) => Err(format!("#{id} live and not renewed")),
                None => Ok(id),
            }
        };
        let keys = super::follow_open(&mut ctl, &mut nappers, fol).unwrap();
        assert_eq!(ctl.held_by, Some(140));
        assert_eq!(ctl.pick_cart, Some((141, 0)));
        assert_eq!(ctl.divert, Some((142, 2)));
        assert_eq!(nappers[0].0, 140, "the visit in flight follows its napper");
        assert_eq!(nappers[1].0, 7, "a napper dead before the open keeps its id");
        assert_eq!((ctl.release_log[0].1, ctl.dep_log[0].1, ctl.pick_log[0].1, ctl.divert_log[0].1), (140, 141, 140, 142), "every log follows");
        assert_eq!((ctl.pick_carts[0].1, ctl.pick_carts[0].2), (141, 140));
        assert!(keys.iter().any(|k| k.contains("#40 -> #140")) && keys.iter().any(|k| k.contains("#7 -> #7")), "{keys:?}");
        // a live id the open didn't renew: an error
        let mut stuck = super::Ctl { held_by: Some(99), ..Default::default() };
        assert!(super::follow_open(&mut stuck, &mut [], fol).is_err());
        // nothing held: the keys print "none"
        let mut idle = super::Ctl::default();
        assert!(super::follow_open(&mut idle, &mut [], |id| Ok(id + 1)).unwrap().iter().all(|k| k.ends_with(": none")));
    }

    /// (G1 07:28 PT) the air control's footprint: the held rider and those stacked below it count as
    /// its own; a natural drowning elsewhere in the same run leaves it NOT CAUGHT ALONE
    #[test]
    fn the_air_controls_own_sample_is_its_column() {
        assert!(super::air_control_own(&[7], 7, &[9, 11]));
        assert!(super::air_control_own(&[7, 9], 7, &[9, 11]), "the cart stacked below it");
        assert!(!super::air_control_own(&[7, 9, 30], 7, &[9, 11]), "a natural drowning elsewhere");
        assert!(!super::air_control_own(&[9], 7, &[9, 11]), "the held rider itself must drown");
    }

    /// (G1 07:28 PT) a clearing is the stamp erased, nothing else: a complete path (a navigation
    /// recompute) with the stamp still set is no clearing; the sources named by the game's tests
    #[test]
    fn a_recompute_with_the_stamp_set_is_no_clearing() {
        let me = [-2, 0, 4];
        // the stamp still set after any path change: no clearing
        assert_eq!(super::clearing_source(Some(100), Some(100), false, Some(([-9, 0, 9], 0)), me, "a flee point"), None);
        // no stamp before: nothing to clear
        assert_eq!(super::clearing_source(None, None, false, None, me, ""), None);
        // erased within close-enough Manhattan (P is Manhattan 1 from the bed head, HOME's close 1)
        assert!(super::clearing_source(Some(100), None, false, Some(([-3, 0, 4], 1)), me, "the bed").unwrap().starts_with("its target reached"));
        // erased farther away: the sink's own tryComputePath
        assert!(super::clearing_source(Some(100), None, false, Some(([-3, 0, 6], 1)), me, "the bed").unwrap().starts_with("the sink's own tryComputePath"));
        // the sleep start first
        assert!(super::clearing_source(Some(100), None, true, Some(([-3, 0, 6], 1)), me, "the bed").unwrap().starts_with("a nap"));
    }

    /// (ironspec's E7 control) a nap lying down at 23,990 and still asleep past 24,000 is one nap of
    /// night 0: its lie-down's night, the morning half belonging to the night before
    #[test]
    fn a_nap_spanning_24000_counts_once() {
        assert_eq!(super::night_of(23_990), 0);
        assert_eq!(super::night_of(24_005), 0, "the morning after is still night 0");
        assert_eq!(super::night_of(12_000), 0);
        assert_eq!(super::night_of(11_999), -1);
        assert_eq!(super::night_of(36_000), 1);
    }

    /// (G1 07:17 PT; X34: an independent source) the WRITTEN scene text checked against what the run
    /// spawned: run_inner's own drop of the stored queue cart, then the seeded carts; the matching
    /// text passes, an extra stored cart line FAILS, and so does a passenger off
    #[test]
    fn the_scene_lines_are_the_spawn_list() {
        let cart = |x: f64, y: f64, z: f64| ("minecraft:minecart".to_string(), [x, y, z], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()));
        let module = vec![cart(-0.5, 0.0625, 0.5), cart(9.5, -13.4375, -3.5), ("minecraft:oak_boat".to_string(), [2.5, -12.0, -5.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}".to_string()))];
        let mut list = module.clone();
        // (run_inner's own drop, the build guide's start: the bed at y -27)
        super::drop_stored_queue_cart(&mut list, -27);
        list.extend(super::lane_carts(5));
        // what the run spawned, as the world would hold it
        let view: Vec<(String, [f64; 3], Vec<String>)> = list
            .iter()
            .map(|(ty, p, n)| (ty.clone(), *p, if n.as_deref().is_some_and(|n| n.contains("zombie")) { vec!["minecraft:zombie".to_string()] } else if n.is_some() { vec!["minecraft:villager".to_string()] } else { Vec::new() }))
            .collect();
        let text = super::scene_entity_lines(&list);
        super::check_scene_text(&view, &text).unwrap();
        assert!(!text.contains("9.5 -13.4375 -3.5"), "the dropped queue cart isn't in the scene");
        assert!(text.contains("# e2\nentity: minecraft:minecart -0.5 17.5625 1.5"), "the first seeded cart follows the module's kept lines: {text}");
        // the old writer's defect: the module's text (the stored cart kept) with the tail sliced
        let mut stale = module.clone();
        stale.extend(super::lane_carts(5).into_iter().skip(1));
        assert!(super::check_scene_text(&view, &super::scene_entity_lines(&stale)).is_err(), "an extra stored cart line must fail");
        // a passenger off
        let mut wrong = list.clone();
        wrong[0].2 = Some("{Passengers:[{id:\"minecraft:zombie\"}]}".to_string());
        assert!(super::check_scene_text(&view, &super::scene_entity_lines(&wrong)).is_err(), "a passenger off must fail");
    }
}

#[cfg(test)]
mod fail_line_tests {
    /// (ironspec 20:10 PT, the 7th property's slab test) a segment through a cell, one beside it,
    /// one ending short of it, and one along an axis
    #[test]
    fn a_segment_hits_only_the_cells_it_crosses() {
        use redsim::entity::Vec3;
        let (a, b) = (Vec3::new(0.5, 0.5, 0.5), Vec3::new(4.5, 0.5, 0.5));
        assert!(super::seg_hits_cell(a, b, [2, 0, 0]), "through (2,0,0)");
        assert!(!super::seg_hits_cell(a, b, [2, 1, 0]), "beside it");
        assert!(!super::seg_hits_cell(a, Vec3::new(1.5, 0.5, 0.5), [2, 0, 0]), "ending short of it");
        assert!(super::seg_hits_cell(Vec3::new(0.2, 0.2, 0.2), Vec3::new(2.8, 2.8, 2.8), [1, 1, 1]), "the diagonal through (1,1,1)");
        assert!(!super::seg_hits_cell(Vec3::new(0.2, 0.2, 0.2), Vec3::new(2.8, 2.8, 2.8), [2, 0, 0]), "not a cell the diagonal never enters");
    }

    #[test]
    fn a_real_fail_beside_an_expected_one_counts() {
        let rep = "rail junctions: 10 junctions, FAIL at 2, 3 declared placement-order (expected FAIL)\nall declared: 3 declared placement-order (expected FAIL)\nthe bar: 1,200 (<= 1,000: FAIL)\nthe dwell: 13 (PASS)\n";
        let f = super::fail_lines(rep);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].starts_with("rail junctions") && f[1].starts_with("the bar"));
    }
}

#[cfg(test)]
mod touch_tests {
    /// (ironspec 15:23 (5)) the box gap: end to end, 0.01 apart, an overlap (its least depth),
    /// diagonally apart (the largest axis separation) and stacked
    #[test]
    fn the_cart_box_gap_is_the_largest_axis_separation() {
        let g = |b: [f64; 3]| super::cart_box_gap([0.0, 0.0, 0.0], b);
        assert!(g([0.98, 0.0, 0.0]).abs() < 1e-12, "end to end along x");
        assert!((g([0.0, 0.0, -0.99]) - 0.01).abs() < 1e-12, "0.01 apart along z");
        assert!((g([0.5, 0.2, 0.1]) + 0.48).abs() < 1e-12, "an overlap: its least depth");
        assert!((g([1.5, 1.5, 0.0]) - 0.8).abs() < 1e-12, "diagonally: the y separation");
        assert!(g([0.0, 0.7, 0.0]).abs() < 1e-12, "stacked, touching");
    }

    /// (ironspec 15:17) each napper arrival against the nearest batch arrivals either side (a tie
    /// counts both ways), none after the last
    #[test]
    fn a_napper_lip_arrival_takes_the_nearest_batch_arrivals() {
        let seq = [(100, false), (130, true), (150, false), (160, false), (200, true), (210, true)];
        assert_eq!(super::napper_lip_gaps(&seq), vec![(1, Some(30), Some(20)), (4, Some(40), None), (5, Some(50), None)]);
        assert_eq!(super::napper_lip_gaps(&[(100, false), (100, true)]), vec![(1, Some(0), Some(0))]);
        assert!(super::napper_lip_gaps(&[(100, false)]).is_empty());
    }

    /// (ironspec 15:17) a K window's own writes: its K cell only, after its sample, before the next
    /// window's; the last window bounded by its own span (no overflow), a missing window none
    #[test]
    fn a_k_window_reads_its_own_k_writes() {
        let windows = [(1_000, 0usize, -3), (2_000, 1, 1)];
        let fired = [(1_097, -3, true), (1_125, -3, false), (2_100, -3, true), (2_112, 1, true), (2_140, 1, false), (2_300, 1, true)];
        assert_eq!(super::k_window_on_off(&windows, 0, &fired, 0), (Some(1_097), Some(1_125)));
        assert_eq!(super::k_window_on_off(&windows, 1, &fired, 0), (Some(2_112), Some(2_140)));
        assert_eq!(super::k_window_on_off(&windows, 2, &fired, 0), (None, None));
    }

    /// (dfb89499 15:11 (a)) a hold visit: its settle at its first still tick, the touching run and the
    /// depth against the glass, a move after the settle, and no move counted once the trapdoor is open
    #[test]
    fn a_hold_visit_settles_touches_and_moves() {
        let mut v = super::HoldVisit::new(7, 10, false);
        let at = |z: f64| [3.5, 16.1875, z];
        v.tick(10, at(0.60), f64::INFINITY, false);
        v.tick(11, at(0.50), 0.1, false);
        v.tick(12, at(0.49), 0.01, false);
        v.tick(13, at(0.49), 0.0, false);
        v.tick(14, at(0.4899995), 5e-7, false);
        v.tick(15, at(0.4899995), 0.0, false);
        v.tick(16, at(0.4999), 0.0099005, false);
        v.tick(17, at(0.4999), 0.0, true);
        v.tick(18, at(0.4999), 0.5, true);
        assert_eq!(v.settle.map(|s| s.0), Some(13));
        assert_eq!(v.best_run, 4, "ticks 12..15 at gap <= 1e-6");
        assert!((v.depth - 5e-7).abs() < 1e-9, "{}", v.depth);
        assert_eq!(v.moved.map(|m| m.0), Some(16));
        assert!((v.moved.map_or(0.0, |m| m.1) - 0.0099005).abs() < 1e-12);
        assert_eq!((v.opened, v.last), (Some(17), 18));
    }

    /// (dfb89499 15:11 (b)) a batch's sequence in tick order (ties in the events' order), T+n from its
    /// REL; the least..most forms; the queue's cells
    #[test]
    fn a_batch_sequence_reads_in_tick_order() {
        let b = super::StBatch {
            rel: 100,
            trap: [Some(130), Some(142)],
            input: [Some(146), Some(156)],
            r1: [Some(148), Some(160)],
            hold: Some((5, Some(131))),
            carts: [Some((6, [Some(149), Some(155), Some(155), Some(158)])), Some((7, [Some(156), Some(161), Some(161), Some(165)]))],
        };
        assert!(b.ticks().iter().all(|t| t.is_some()));
        let s = b.sequence();
        assert!(s.starts_with("T+30 the trapdoor open, T+31 the hold cart's last tick in H (#5), T+42 the trapdoor shut"), "{s}");
        assert!(s.contains("T+55 the R_1 cart off R_1 (#6), T+55 the R_1 cart into H (#6)"), "{s}");
        assert!(s.ends_with("T+65 the slot-2 cart at rest on R_1 (#7)"), "{s}");
        assert_eq!(super::lm_rel([5, -3, 12]), "T-3..T+12 (n 3)");
        assert_eq!(super::lm_ticks(std::iter::empty()), "-");
        assert_eq!([super::queue_slot([3, 16, 0]), super::queue_slot([3, 17, 1]), super::queue_slot([7, 22, 6])], [Some((1, 0)), Some((1, 1)), Some((2, 6))]);
        assert_eq!([super::queue_slot([3, 17, 2]), super::queue_slot([5, 16, 0]), super::queue_slot([-1, 23, 7])], [None, None, None]);
    }
}

/// The summon of a golem born off the pad (feet above y -4): its tick, the summoner (the
/// villager whose call spawned it) and where it stood, the willing count, the golem's cell and the
/// block it stood on, and where it was first seen (with the nearest villager then).
#[allow(clippy::type_complexity)]
fn off_pad_rows(w: &redsim::world::World, born: &BTreeMap<u32, i64>, birth_at: &BTreeMap<u32, ([f64; 3], String)>, summon: &BTreeMap<u32, (i64, u32, u32, Option<[i32; 3]>, Option<[f64; 3]>)>, t0: i64) -> Vec<String> {
    birth_at
        .iter()
        .filter(|(_, (p, _))| p[1] > -4.0)
        .map(|(id, (p, n))| {
            let s = summon.get(id).map_or("no summon event (not a villager's call)".to_string(), |(t, v, willing, at, vp)| {
                let floor = at.map_or(String::new(), |c| format!(", standing on ({},{},{}) {}", c[0], c[1] - 1, c[2], w.get(dbp(c[0], c[1] - 1, c[2])).to_string().trim_start_matches("minecraft:")));
                format!(
                    "summoned t {} by villager {v}{} with {willing} willing; its cell {}{floor}",
                    t - t0,
                    vp.map_or(String::new(), |q| format!(" at ({:.2},{:.2},{:.2})", q[0], q[1], q[2])),
                    at.map_or("-".to_string(), |c| format!("({},{},{})", c[0], c[1], c[2]))
                )
            });
            format!("golem {id} ({s}); first seen t {} at ({:.2},{:.2},{:.2}), the nearest {n}", born.get(id).map_or(0, |b| b - t0), p[0], p[1], p[2])
        })
        .collect()
}
