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
    branch_prev: [bool; 2],
    row_prev: bool,
    cycles: i64,
    log: Vec<String>,
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
                set(w, [x + 1, 17, 1], "minecraft:redstone_block", flags::ALL)?;
            }
            self.r_on = None;
            self.r_off = Some(now + 30);
        }
        if self.r_off == Some(now) {
            for x in LANES {
                set(w, [x + 1, 17, 1], "minecraft:glass", flags::ALL)?;
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
        if row && !self.row_prev {
            set(w, [-1, 26, 9], &lever(true), flags::ALL)?;
        }
        self.branch_prev = [b0, b1];
        self.row_prev = row;
        Ok(())
    }

    fn start(&mut self, rel: i64) {
        self.busy = true;
        self.rel = Some(rel);
        self.armed = [false; 3];
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
    /// A restart (liveness::restart) just reloaded it: its next missing stamp is the reload's.
    reloaded: bool,
    reload_drops: i64,
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
            if e.1[1] < -15.0 {
                e.1 = [-4.5, -18.0, 3.5];
            }
        }
    } else if o.lane_carts {
        sp.entities.extend(lane_carts(o.per_lane.saturating_sub(3)));
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
    let mut ctl = Ctl::default();
    if o.drive {
        ctl.init(&mut run.world)?;
    }
    let mut turns = [Turns::default(); 3];
    let mut turn_lines: Vec<String> = Vec::new();
    // (the panic draws whose first path was partial: (place, target cell, rider cell) -> count)
    let mut partial_list: BTreeMap<(usize, [i32; 3], [i32; 3]), i64> = BTreeMap::new();
    // (and each one's detail row)
    let mut partial_rows: Vec<String> = Vec::new();
    // (the official scene's per-rest-block tally of the panic draws: complete, partial, never)
    let mut by_cell: BTreeMap<[i32; 3], [i64; 3]> = BTreeMap::new();
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
        run.tick();
        if o.drive {
            ctl.step(&mut run.world, rel)?;
        }
        let w = &mut run.world;
        let now = w.time;
        let day = w.day_time();
        let dawn = day < prev_day || (prev_day < 23_999 && day >= 23_999);
        prev_day = day;
        let ids: Vec<u32> = recs.keys().copied().collect();
        for id in ids {
            let Some(i) = redsim::entity::index_of(w, id) else { continue };
            let pos = w.entities.list[i].pos;
            let (home, activity_rest, panic, sleeping, stamp, target, last_slept, reach, sink_cd, path_target, path_end, hostile) = {
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
                // (the path the sink holds: complete (`canReach`) or not)
                let reach = ex.and_then(|x| x.mem.path.as_ref()).map(|p| p.can_reach());
                // (the path's own target: a path computed for an earlier target is stale)
                let path_target = ex.and_then(|x| x.mem.path.as_ref()).map(|p| [p.target.x, p.target.y, p.target.z]);
                let path_end = ex.and_then(|x| x.mem.path.as_ref()).and_then(|p| p.nodes.last().map(|n| [n.x, n.y, n.z]));
                let hostile = b.nearest_hostile;
                // (MoveToTargetSink's remainingCooldown)
                let sink_cd = ex.and_then(|x| x.nodes.iter().find_map(|n| match &n.kind {
                    redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { remaining_cooldown, .. } => Some(*remaining_cooldown),
                    _ => None,
                }));
                (b.home.map(|h| [h.x, h.y, h.z]), rest, panic, sleeping, ex.and_then(|x| x.mem.cant_reach_since), target, b.last_slept, reach, sink_cd, path_target, path_end, hostile)
            };
            let r = recs.get_mut(&id).unwrap();
            let c = cell(pos);
            // (the control: its clears undone)
            if Some(id) == control_id && r.prev_stamp.is_some() && stamp.is_none() && home.is_some() {
                if let Some(ex) = redsim::entity::mob::ai::brain::brain_mut(w, i).ex.as_deref_mut() {
                    ex.mem.cant_reach_since = r.prev_stamp;
                }
                r.prev_target = target;
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
                let k = if tc != Some(want) {
                    Some(2)
                } else if path_target == Some(want) {
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
                                "t {at}: {} (lane x {}), rider cell {:?} ({}, {}; rider at ({:.4},{:.4},{:.4}){}), scarer {}, target {:?}, the partial path ends at {:?}",
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
                                path_end
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
            // (one draw per new target cell: a target re-set on the same cell is the same draw)
            let prev_tc = r.prev_target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]);
            if let (Some(t), true) = (tc, tc != prev_tc) {
                if r.pending_draw.is_none() {
                    r.pending_draw = Some((place_of(c), now - t0, panic, t, c));
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
                    r.draw_ctx = format!("the lane's carts at the draw tick: {}", carts.join("; "));
                }
            }
            if target.is_some() && reach == Some(false) && path_target == target.map(|t| [t[0].floor() as i32, t[1].floor() as i32, t[2].floor() as i32]) {
                r.stuck_ticks[place_of(c)] += 1;
            }
            r.prev_panic = panic;
            r.prev_stamp = stamp;
            r.prev_home = home;
            r.prev_target = target;
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
