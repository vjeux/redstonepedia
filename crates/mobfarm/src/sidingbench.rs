//! The napper's lane-exact siding (ironspec Nov 11 23:01; 9fc3155f's cells, budget f1b95a5451), one
//! lane in the budget's own coordinates (lane x = `lx`, descending toward -z): the hold (x,16,0) (a
//! bottom-half trapdoor floor, glass on three sides, its string at (x,17,0)); R_1 (x,17,1), the switched
//! powered slope (powered from the side, (x+1,17,1)); four plain pressed slopes (x,18..21,2..5); the
//! merge (x,22,6), the split's flat branch at level 22 travelling -z ((x,22,7..9)), with a rider string
//! over it at (x,23,6); the siding beside it: the feed flat (x-1,23,8), the siding slope (x-1,22,7) (a
//! braked switched powered slope descending north, powered from the side (x-2,22,7)), the curve
//! (x-1,22,6) turning east onto the merge. One release line opens the hold's trapdoor every `period`
//! gt; the R line restages once it shuts. A dropped cart returns on the branch after `ret` gt at
//! `v_in` b/t (the split, in drop order); every `nap_every`-th dropped cart is a napper instead: it
//! comes back onto the siding after `nap_ret` gt, braked, and is released (the siding slope powered for
//! `nap_pulse` gt) `k` gt after its predecessor (the cart dropped one cycle before it) passes the
//! merge's string. Prints the napper's way through the merge (it must turn north into the lane), the
//! curve rule at the curve and the merge (the follower's entry minus the leader's clear, >= 0, no
//! touching), its contact with its predecessor's back and the successor's with it (<= 0.46), the
//! lane's batch counts, and the siding's creep.

use crate::dropbench::DropOpts;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The bed stop's bed head in the loop frame (58f770cc's stop, head (-3,-18,3)): the riders' HOME
/// with --nap-ai.
const BED: (i32, i32, i32) = (-3, -18, 3);

/// The napper's brain over its siding wait (--nap-ai).
#[derive(Clone, Debug, Default)]
struct NapBrain {
    rest: i64,
    panic: i64,
    calm_run: i64,
    calm_run_max: i64,
    stamp_at_arrival: Option<i64>,
    stamp_calm_max: Option<i64>,
    /// The oldest stamp at a calm REST tick with no walk target held (the bar's ticks): on the
    /// siding, and after the release (down the lane to the window's end).
    stamp_nowalk_max: Option<i64>,
    lane_nowalk_max: Option<i64>,
    /// Stamp clears on the siding (Some -> None: a reachable walk, a lie-down, a reload).
    clears: i64,
    stamp_prev: Option<i64>,
    home_lost_at: Option<i64>,
    tracking: bool,
    arrived_t: Option<i64>,
    rested_t: Option<i64>,
    released_t: Option<i64>,
    late: bool,
    drop_stamp: Option<i64>,
    /// The walk target held the tick before (its block, close distance, under PANIC), the siding
    /// clears' notes, where HOME was lost.
    prev_target: Option<(BlockPos, i32, bool)>,
    clear_notes: Vec<String>,
    home_lost_place: Option<(f64, f64, f64)>,
}

/// The nappers' windows, closed (--nap-ai).
#[derive(Clone, Debug, Default)]
struct WinAgg {
    total: usize,
    tracked: usize,
    rows: Vec<String>,
    nowalk_siding_max: i64,
    nowalk_lane_max: i64,
    over_1000: usize,
    home_lost: usize,
    cleared: usize,
    dropped: usize,
    drop_stamp_max: i64,
    normal: usize,
    late: usize,
    /// Windows cut short (their cart picked again while open: a failure); still open at the end.
    cut: usize,
    open_end: usize,
}

impl WinAgg {
    fn close(&mut self, c: usize, nb: NapBrain, reason: String, _waited: Option<i64>, _t: i64) {
        if !nb.tracking {
            self.rows.push(format!("napper cart {c}: NOT TRACKED (its rider runs no exact brain); {reason}"));
            return;
        }
        self.tracked += 1;
        if nb.late {
            self.late += 1;
        } else {
            self.normal += 1;
        }
        let fmt = |v: Option<i64>| v.map_or("none".to_string(), |a| format!("{a} gt"));
        let s = nb.stamp_nowalk_max.unwrap_or(-1);
        let l = nb.lane_nowalk_max.unwrap_or(-1);
        self.nowalk_siding_max = self.nowalk_siding_max.max(s);
        self.nowalk_lane_max = self.nowalk_lane_max.max(l);
        if s.max(l) > 1000 {
            self.over_1000 += 1;
        }
        if nb.home_lost_at.is_some() {
            self.home_lost += 1;
        }
        if reason.starts_with("cleared") {
            self.cleared += 1;
        } else if reason.starts_with("reached the drop") {
            self.dropped += 1;
            self.drop_stamp_max = self.drop_stamp_max.max(nb.drop_stamp.unwrap_or(-1));
        }
        let waited = match (nb.rested_t, nb.released_t) {
            (Some(r), Some(ra)) => format!("{} gt", ra - r),
            _ => "-".to_string(),
        };
        self.rows.push(format!(
            "napper cart {c} ({}): waited {waited} on the siding; REST {} ticks, panic {}; the stamp at arrival {}, its clears on the siding {}{}; the oldest stamp at a calm REST tick with no walk target: siding {}, after the release {}; HOME {}; {reason}",
            if nb.late { "late" } else { "normal" },
            nb.rest,
            nb.panic,
            fmt(nb.stamp_at_arrival),
            nb.clears,
            if nb.clear_notes.is_empty() { String::new() } else { format!(" ({})", nb.clear_notes.join(", ")) },
            fmt(nb.stamp_nowalk_max),
            fmt(nb.lane_nowalk_max),
            match (nb.home_lost_at, nb.home_lost_place) {
                (Some(h), Some(p)) => format!("LOST at t {h} at {:.2},{:.2},{:.2}", p.0, p.1, p.2),
                (Some(h), None) => format!("LOST at t {h}"),
                _ => "kept".to_string(),
            }
        ));
    }
}

/// The rider (the first passenger) of cart `id`: its entity index.
fn rider_index(w: &redsim::world::World, id: u32) -> Option<usize> {
    let ci = redsim::entity::index_of(w, id)?;
    let rid = *w.entities.list[ci].passengers.first()?;
    redsim::entity::index_of(w, rid)
}

/// A spawned rider made a farm villager (--nap-ai): HOME the shared bed, slept last night; with
/// `stamp_age`, its CANT_REACH stamp armed that many gt ago (the napper's leg from the bed's exit R
/// to its siding, ridden in REST with no reachable walk).
fn farm_rider(w: &mut redsim::world::World, cart: u32, bed: BlockPos, stamp_age: Option<i64>) {
    use redsim::entity::mob::ai::brain;
    let Some(ri) = rider_index(w, cart) else { return };
    let now = w.time;
    let b = brain::brain_mut(w, ri);
    b.home = Some(bed);
    b.last_slept = Some(now - 100);
    if let Some(a) = stamp_age
        && brain::has_exact(w, ri)
    {
        brain::ex_mut(w, ri).mem.cant_reach_since = Some(now - a);
    }
}

fn st(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64, f64)> {
    let m = MIRROR_Z.with(|m| m.get());
    redsim::entity::index_of(w, id).map(|i| {
        let e = &w.entities.list[i];
        (e.pos.x, e.pos.y, if m { -e.pos.z } else { e.pos.z }, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
    })
}

thread_local! {
    /// --mirror-z for the bench running on this thread (st() reads positions back through it).
    static MIRROR_Z: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A block state mirrored in z: north and south swapped in shapes, facings and connection flags.
fn mirror_state_z(s: &str) -> String {
    let mut out = s.to_string();
    for (a, b) in [
        ("ascending_north", "\u{1}AN"),
        ("ascending_south", "\u{1}AS"),
        ("north_east", "\u{1}NE"),
        ("north_west", "\u{1}NW"),
        ("south_east", "\u{1}SE"),
        ("south_west", "\u{1}SW"),
        ("facing=north", "\u{1}FN"),
        ("facing=south", "\u{1}FS"),
        ("north=", "\u{1}N="),
        ("south=", "\u{1}S="),
    ] {
        out = out.replace(a, b);
    }
    for (a, b) in [
        ("\u{1}AN", "ascending_south"),
        ("\u{1}AS", "ascending_north"),
        ("\u{1}NE", "south_east"),
        ("\u{1}NW", "south_west"),
        ("\u{1}SE", "north_east"),
        ("\u{1}SW", "north_west"),
        ("\u{1}FN", "facing=south"),
        ("\u{1}FS", "facing=north"),
        ("\u{1}N=", "south="),
        ("\u{1}S=", "north="),
    ] {
        out = out.replace(a, b);
    }
    out
}

/// The game's cart-cart contact test (AbstractMinecart's push query): two carts' boxes (0.98 x 0.7
/// x 0.98, bottom-centred at their positions) meeting with one of them inflated `inf` in x and z
/// (0.2 for the push test), their y ranges overlapping.
fn push_contact(p: (f64, f64, f64, f64), q: (f64, f64, f64, f64), inf: f64) -> bool {
    (p.0 - q.0).abs() < 0.98 + inf && (p.2 - q.2).abs() < 0.98 + inf && p.1 < q.1 + 0.7 && q.1 < p.1 + 0.7
}

fn kill_with_rider(w: &mut redsim::world::World, id: u32) {
    let riders: Vec<u32> = redsim::entity::index_of(w, id).map(|i| w.entities.list[i].passengers.clone()).unwrap_or_default();
    for r in riders {
        let _ = redsim::entity::kill(w, r);
    }
    let _ = redsim::entity::kill(w, id);
}

/// The siding's own options.
#[derive(Clone, Debug)]
pub struct SidingOpts {
    pub k: i64,
    pub nap_every: usize,
    pub nap_ret: i64,
    pub nap_pulse: i64,
    pub merge_curve: bool,
    /// The lane's plain slopes behind R_1 (the merge at level 18 + N).
    pub slopes: i32,
    /// A guard string over the branch cell before the merge: release only while it is off.
    pub guard: bool,
    /// Release a napper only K after a predecessor's pass seen while it waits (else a lane-lap).
    pub early_only: bool,
    /// Day-1's option (ii): release K after ANY cart's pass that finds the napper waiting (the
    /// napper-present sample at the pass), behind whichever cart passed.
    pub any_pass: bool,
    /// Day-1's ring (option (ii)'s windows): ring positions (0: off; 10 for day-1's ring), the
    /// napper's pick position, its window `win_du` positions later, opening `win_tap` gt after that
    /// release for one beat.
    pub ring: i64,
    pub pick_u: i64,
    pub win_du: i64,
    pub win_tap: i64,
    /// The riders' AI on at night with HOME the bed stop's bed (the napper's calm-REST age, its
    /// CANT_REACH stamp, HOME kept); the napper's stamp pre-aged at its siding arrival (gt).
    pub ai: bool,
    pub stamp_at_arrival: Option<i64>,
    /// Fix (A) (ironspec 05:15): with the ring, a window passing without its napper waiting sets the
    /// lane's late latch; the napper then goes K after the first pass it is waiting for.
    pub late_latch: bool,
    /// Scarer posts (fix (B)'s probe): a zombie seated in a boat at each cell, on a glass floor.
    pub posts: Vec<(i32, i32, i32)>,
    /// 7b90c3f1's K re-lay (farmopt 05:28): the split row along z mz+2 at level m+1 through the
    /// feed cell, K there a plain rail held north_east (the unpowered T, through), with ridden
    /// through-carts on the row at this stored speed, eastbound, `k_offsets` gt after each release.
    pub k_row: Option<f64>,
    pub k_offsets: Vec<i64>,
    /// The napper arrives on the feed cell (K's cell) moving north at this stored speed, instead of
    /// spawning at rest on the rest.
    pub nap_feed_v: Option<f64>,
    /// With --nap-feed-v: the napper's spawn moved this far back along its path (south of K's
    /// cell centre), so its positions at tick boundaries take another phase against the cells
    /// (the rest sweep's phases: the rest z depends on the entry phase as well as the speed).
    pub nap_feed_phase: f64,
    /// --siding-north: R's input fed through the budget box (x-2..x-1, 22..23, z 9): dust at the
    /// input cell, a repeater facing west east of it; the switched cell (x,23,9).
    pub feed_path: bool,
    /// With --feed-path: the z-8 variant (dust down to (x-1,22,8), the feed repeater (x,22,8), the
    /// switched cell (x+1,22,8)), keeping the seat candidate's column (x,22..23,9) clear.
    pub feed_z8: bool,
    /// With --feed-path: day-1's lane -1 layout (the feed dust on slime, day-1's delay-3 repeater
    /// (x-3,22,9) strongly powering it; the switched cell (x-4,22,9)).
    pub feed_slime: bool,
    /// The siding's flat (x-1,m,mz) a detector rail (east_west): the proposed siding sensor; with
    /// --siding-north the feed F a detector rail (north_south).
    pub flat_detector: bool,
    /// Option (a) as an S-bend (ironspec 06:24): the feed F (x-2,m+1,mz+1), the rest R (x-2,m,mz)
    /// powered from below, c1 (x-2,m,mz-1), c2 (x-1,m,mz-1), c3 (x-1,m,mz) into the merge; no pass
    /// string (the pass is the guard detector's rising edge).
    pub siding_north: bool,
    /// tick-exact's queue fill scene: empty carts at these cells, then a ridden cart at the seat.
    pub fill_scene: Option<(Vec<(i32, i32, i32)>, (i32, i32, i32))>,
    /// The top cell north_south (a build without the official helper order).
    pub top_flat: bool,
    /// The world laid mirrored in z (onechunk's z-mirrored copy), read back through the mirror.
    pub mirror_z: bool,
    /// The start state as the steady state (ironspec 07:57): 7 riders in their slots, the 8th in the
    /// pocket; the R line off until the first release.
    pub start_steady: bool,
    /// --siding-north: the block under the rest (default stone; slime conducts and is not
    /// solid-render, so R's cell is no stable destination).
    pub r_support: Option<String>,
    /// The siding's distance west of the lane (1: adjacent to the branch, as laid; 2: one cell clear).
    pub siding_dx: i32,
    /// The siding's rest raised this many levels (9fc3155f: 1, out of the branch carts' push range).
    pub siding_dy: i32,
    /// The merge string north-south on glass supports (nothing over the siding's path at x-1/x-2).
    pub ns_merge: bool,
    /// The guard as a detector rail in the branch cell (x,m,mz+guard_dz) (no hooks near the siding).
    pub guard_det: bool,
    /// The detector guard's cell: mz + this (2 at first; 1 since 7b90c3f1's split shares it as its latch).
    pub guard_dz: i32,
    /// The passive drop entry (ironspec 00:41): the merge one level up, its north end a rail end over
    /// the drop onto the lane's top slope, a backstop wall two cells north of the merge.
    pub drop_entry: bool,
    /// Leave out the top slope's high-side support (the positive control: the rail pops).
    pub no_top_support: bool,
    /// The siding's side power as a real conductor: stone at (x-1,m,mz+1), strongly powered by a
    /// repeater at (x-1,m,mz+2) facing it (input from the south, away from the detector), the switch a
    /// redstone block at the repeater's input (x-1,m,mz+3).
    pub side_real: bool,
    /// The stray-power check's positive controls: the side cell as "dust", "dust-line" or
    /// "repeater-east" (see the bench).
    pub side_ctl: Option<String>,
}

/// The bench: returns the text.
pub fn siding_bench(o: &DropOpts, so: &SidingOpts) -> Result<String, String> {
    let lx = 3i32;
    // (--mirror-z (onechunk's z-mirrored copy, ironspec 07:47: Ts and powered rails are not mirror
    // symmetric): the world is laid mirrored in z (a cell z at -1 - z, a position z at -z, every
    // state's north and south swapped, every spawned Motion's z negated) and read back through the
    // same mirror (st(), the cells the bench looks at), so the bench's logic runs in the design's
    // frame on a mirrored world)
    MIRROR_Z.with(|m| m.set(so.mirror_z));
    let mzc = |z: i32| if so.mirror_z { -1 - z } else { z };
    let mzf = |z: f64| if so.mirror_z { -z } else { z };
    let wp = |p: BlockPos| BlockPos::new(p.x, p.y, mzc(p.z));
    let vz = |v: f64| if so.mirror_z { -v } else { v };
    let b = |s: &str| Block::parse(&if so.mirror_z { mirror_state_z(s) } else { s.to_string() });
    let glass = b("minecraft:glass")?;
    let stone = b("minecraft:stone")?;
    let air = b("minecraft:air")?;
    let slope_s = b("minecraft:rail[shape=ascending_south,waterlogged=false]")?;
    let flat_ns = b("minecraft:rail[shape=north_south,waterlogged=false]")?;
    let curve_se = b("minecraft:rail[shape=south_east,waterlogged=false]")?;
    let curve_nw = b("minecraft:rail[shape=north_west,waterlogged=false]")?;
    let flat_ew = b("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let curve_ne = b("minecraft:rail[shape=north_east,waterlogged=false]")?;
    let det_ew = b("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    let pslope_s = b("minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]")?;
    let rblock = b("minecraft:redstone_block")?;
    let trap_shut = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string_ew = b("minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,south=false,west=true,powered=false]")?;
    let hook_e = b("minecraft:tripwire_hook[attached=true,facing=east,powered=false]")?;
    let hook_w = b("minecraft:tripwire_hook[attached=true,facing=west,powered=false]")?;
    let string_ns = b("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_sf = b("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_nf = b("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let det_ns = b("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]")?;
    let curve_sw = b("minecraft:rail[shape=south_west,waterlogged=false]")?;
    let repeater_n = b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    let repeater_e = b("minecraft:repeater[delay=1,facing=east,locked=false,powered=false]")?;
    let dust = b("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, blk: &Block| blocks.push((BlockPos::new(x, y, mzc(z)), blk.clone()));
    // (the hold: trapdoor floor, glass far side (z -1, two high) and both sides, its string over it)
    put(lx, 16, 0, &trap_shut);
    put(lx, 16, -1, &glass);
    put(lx, 17, -1, &glass);
    put(lx - 1, 16, 0, &glass);
    put(lx + 1, 16, 0, &glass);
    put(lx, 17, 0, &string_ew);
    put(lx - 1, 17, 0, &hook_e);
    put(lx + 1, 17, 0, &hook_w);
    put(lx - 2, 17, 0, &stone);
    put(lx + 2, 17, 0, &stone);
    // (R_1 and the N slopes, each on glass; the merge level m = 18 + N at z mz = N + 2; with the drop
    // entry the merge one level up (m = 19 + N): its glass support (x,18+N,mz) beside the top slope's
    // high end (no rail joins them), the drop column (x,m..m-1,mz-1) empty, the backstop wall (x,m,mz-2))
    let ns = so.slopes;
    let m = 18 + ns + so.drop_entry as i32;
    let mz = ns + 2;
    if so.drop_entry {
        // (the backstop wall: glass, golem-proof (9fc3155f's flood: a stone wall's top is an off-pad
        // spawn spot seen from the lane tops))
        put(lx, m, mz - 2, &glass);
        // (the top slope's high-side support (x,17+N,2+N): an ascending rail pops without a rigid
        // block on its high side (BaseRailBlock.shouldBeRemoved); the merge's support moved up a
        // level with the drop entry, so this cell is laid for it (onechunk's find))
        if !so.no_top_support {
            put(lx, 17 + ns, 2 + ns, &glass);
        }
    }
    put(lx, 17, 1, &pslope_s);
    put(lx, 16, 1, &glass);
    for k in 0..ns {
        // (--top-flat: the top cell north_south, as a build without the official helper order
        // settles it (tick-exact's rail-slope control))
        put(lx, 18 + k, 2 + k, if so.top_flat && k == ns - 1 { &flat_ns } else { &slope_s });
        put(lx, 17 + k, 2 + k, &glass);
    }
    // (the merge and the branch: flats at level m from z mz + 3 to mz; the merge's string over it; with
    // --guard-det the branch cell (x,m,mz+2) a detector rail (straight: it can't be reshaped))
    for z in mz..=mz + 3 {
        // (the merge cell a north_west curve with --merge-curve (the siding from the west and the branch
        // from the south both leave north into the intake), else a straight north_south flat)
        let blk = if z == mz && so.merge_curve { &curve_nw } else if z == mz + so.guard_dz && so.guard_det { &det_ns } else { &flat_ns };
        put(lx, m, z, blk);
        put(lx, m - 1, z, &glass);
    }
    put(lx, m, mz + 4, &glass);
    if so.siding_north {
        // (no pass string with --siding-north: its cells are the S-bend's c1 and c2; the pass is the
        // guard detector's rising edge)
    } else if so.drop_entry {
        // (the pass string across the drop column at the merge level, (x,m,mz-1), its hooks (x+-1,m,mz-1)
        // on glass (x+-2,m,mz-1): every arrival flies through it; nothing over the branch or the siding)
        put(lx, m, mz - 1, &string_ew);
        put(lx - 1, m, mz - 1, &hook_e);
        put(lx + 1, m, mz - 1, &hook_w);
        put(lx - 2, m, mz - 1, &glass);
        put(lx + 2, m, mz - 1, &glass);
    } else if so.ns_merge {
        // (the merge string north-south: hooks (x,m+1,mz-1) facing south and (x,m+1,mz+1) facing north,
        // on glass at (x,m+1,mz-2) and (x,m+1,mz+2): nothing at x-1 or x-2 over the siding's path)
        put(lx, m + 1, mz, &string_ns);
        put(lx, m + 1, mz - 1, &hook_sf);
        put(lx, m + 1, mz + 1, &hook_nf);
        put(lx, m + 1, mz - 2, &glass);
        put(lx, m + 1, mz + 2, &glass);
    } else {
        put(lx, m + 1, mz, &string_ew);
        put(lx - 1, m + 1, mz, &hook_e);
        put(lx + 1, m + 1, mz, &hook_w);
        put(lx - 2, m + 1, mz, &stone);
        put(lx + 2, m + 1, mz, &stone);
    }
    // (the siding, `siding_dx` west of the lane (sx = x - dx): the feed flat (sx,m+1,mz+2) on glass with
    // glass behind it, the braked slope (sx,m,mz+1), the curve (sx,m,mz) turning east, then (dx 2) one
    // east-west flat (x-1,m,mz) onto the merge)
    let sx = lx - so.siding_dx.max(1);
    if so.siding_dy > 0 {
        // (9fc3155f's raised siding: the rest a braked switched powered slope one level up (x-1,m+1,mz+2),
        // then a plain slope (x-1,m,mz+1), the curve (x-1,m,mz) east onto the merge; fed by a flat at
        // (x-1,m+2,mz+3); the rest's box clears the branch carts' heights (no push range))
        put(sx, m + 2, mz + 3, &flat_ns);
        put(sx, m + 1, mz + 3, &glass);
        put(sx, m + 2, mz + 4, &glass);
        put(sx, m + 1, mz + 2, &pslope_s);
        put(sx, m, mz + 2, &glass);
        put(sx, m, mz + 1, &slope_s);
        put(sx, m - 1, mz + 1, &glass);
        put(sx, m, mz, &curve_se);
        put(sx, m - 1, mz, &glass);
    } else {
        if so.k_row.is_some() {
            // (7b90c3f1's K re-lay: the split row along z mz+2 at level m+1, K at the feed cell
            // (sx,m+1,mz+2) held north_east (the unpowered T: through-carts pass east, snapped), the
            // row's cells (sx-2) and (sx-1) flat east_west, all on glass; nothing at z mz+3. The
            // row's next cell (sx+1) is left out (its rail would give the bench's pre-split branch
            // flat (x,m,mz+2) a third potential connection): through-carts leave K east and are
            // taken at x - 0.6, past the napper's push reach)
            for xx in sx - 2..=sx {
                put(xx, m + 1, mz + 2, if xx == sx { &curve_ne } else { &flat_ew });
                put(xx, m, mz + 2, &glass);
            }
        } else {
            put(sx, m + 1, mz + 2, &flat_ns);
            put(sx, m, mz + 2, &glass);
            put(sx, m + 1, mz + 3, &glass);
        }
        if so.siding_north {
            // (option (a) (ironspec 06:24, provisional), as an S-bend: the feed F (sx,m+1,mz+1)
            // north_south on glass, the rest R (sx,m,mz) a braked powered slope ascending south (its
            // switched power from below: stone (sx,m-1,mz), see s_pow), then c1 (sx,m,mz-1) south_east,
            // c2 (sx+1,m,mz-1) south_west, c3 (sx+1,m,mz) north_east into the merge from the west; the
            // pass string's cells are c1 and c2, so the pass is the guard detector's rising edge)
            put(sx, m + 1, mz + 1, if so.flat_detector { &det_ns } else { &flat_ns });
            put(sx, m, mz + 1, &glass);
            put(sx, m, mz, &pslope_s);
            put(sx, m, mz - 1, &curve_se);
            put(sx, m - 1, mz - 1, &glass);
            put(sx + 1, m, mz - 1, &curve_sw);
            put(sx + 1, m - 1, mz - 1, &glass);
            put(sx + 1, m, mz, &curve_ne);
            put(sx + 1, m - 1, mz, &glass);
        } else {
            put(sx, m, mz + 1, &pslope_s);
            put(sx, m - 1, mz + 1, &glass);
            put(sx, m, mz, &curve_se);
            put(sx, m - 1, mz, &glass);
        }
    }
    for xx in sx + 1..lx {
        if so.siding_north {
            break;
        }
        put(xx, m, mz, if so.flat_detector && xx == lx - 1 { &det_ew } else { &flat_ew });
        put(xx, m - 1, mz, &glass);
    }
    // (the siding's rest rail and where its side power goes)
    let (rest_y, rest_z) = if so.siding_dy > 0 {
        (m + 1, mz + 2)
    } else if so.siding_north {
        (m, mz)
    } else {
        (m, mz + 1)
    };
    // (the siding's switched side power: with the siding two west of the lane, its EAST side (x-1, at
    // the rest's level), between it and the branch (the west side x-3 is the lift's shell for lane -1);
    // else its west side; with --siding-north the block under the rest (its east side is c3))
    let s_pow = if so.siding_north {
        BlockPos::new(sx, rest_y - 1, rest_z)
    } else if so.siding_dx >= 2 && so.siding_dy == 0 {
        BlockPos::new(sx + 1, rest_y, rest_z)
    } else {
        BlockPos::new(sx - 1, rest_y, rest_z)
    };
    // (--side-real: the side-power cell a real conductor (stone), strongly powered only through its
    // south face by a repeater facing it (input from the south, away from the detector), the switch a
    // redstone block at the repeater's input; the controller toggles that switch)
    let s_switch = match so.side_ctl.as_deref() {
        // (the positive controls of the stray-power check (G1): drivers the guard detector's weak
        // power CAN reach: "dust" (G1's literal swap: dust in the side cell, the repeater behind it),
        // "dust-line" (dust whose only connection is the detector: a line pointing into the rest),
        // "repeater-east" (a repeater taking its input from the detector's side, output into the
        // rest); the switch stays at the repeater's old input, so only the guard can power the rest)
        Some(kind) => {
            put(s_pow.x, s_pow.y - 1, s_pow.z, &glass);
            match kind {
                "dust" => {
                    put(s_pow.x, s_pow.y, s_pow.z, &dust);
                    put(s_pow.x, s_pow.y, s_pow.z + 1, &repeater_n);
                    put(s_pow.x, s_pow.y - 1, s_pow.z + 1, &glass);
                }
                "dust-line" => put(s_pow.x, s_pow.y, s_pow.z, &dust),
                "repeater-east" => put(s_pow.x, s_pow.y, s_pow.z, &repeater_e),
                other => return Err(format!("--side-ctl {other}: dust, dust-line or repeater-east")),
            }
            BlockPos::new(s_pow.x, s_pow.y, s_pow.z + 2)
        }
        None if so.side_real || so.siding_north => {
            // (--r-support: the conductor under the S-bend's rest another block (ironspec 08:46, the
            // row partials: a support that is not solid-render leaves R's cell no stable
            // destination; slime conducts and is not solid-render), else stone)
            let support = match so.r_support.as_deref() {
                Some(id) if so.siding_north => b(id)?,
                _ => stone.clone(),
            };
            put(s_pow.x, s_pow.y, s_pow.z, &support);
            put(s_pow.x, s_pow.y, s_pow.z + 1, &repeater_n);
            put(s_pow.x, s_pow.y - 1, s_pow.z + 1, &glass);
            if so.siding_north && so.feed_path && so.feed_slime {
                // (--feed-slime (day-1's lane -1 layout, ironspec 11:17, option (a)): R's input dust
                // (x-2,23,9) [north, south: one connection, a line] on a slime block (x-2,22,9);
                // day-1's switched cell a delay-3 repeater (x-3,22,9) facing west (outputs east)
                // into the slime, strongly powering it, on glass (x-3,21,9); the switch its input
                // (x-4,22,9); R powers 8 gt after it)
                put(s_pow.x, s_pow.y, s_pow.z + 2, &b("minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]")?);
                put(s_pow.x, s_pow.y - 1, s_pow.z + 2, &b("minecraft:slime_block")?);
                put(s_pow.x - 1, s_pow.y - 1, s_pow.z + 2, &b("minecraft:repeater[delay=3,facing=west,locked=false,powered=false]")?);
                put(s_pow.x - 1, s_pow.y - 2, s_pow.z + 2, &glass);
                BlockPos::new(s_pow.x - 2, s_pow.y - 1, s_pow.z + 2)
            } else if so.siding_north && so.feed_path && so.feed_z8 {
                // (--feed-z8 (the lane -1 option (a), keeping the seat candidate's column
                // (x,22..23,9) clear): R's input dust (x-2,23,9) [north, east]; dust (x-1,23,9) [west,
                // north] stepping down to the north onto dust (x-1,22,8) [south=up, east] ((x-1,23,8)
                // air); the feed repeater (x,22,8) facing west into it (state facing=east); all on
                // glass; the switched cell the repeater's input (x+1,22,8), day-1's line along z 8)
                put(s_pow.x, s_pow.y, s_pow.z + 2, &b("minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=none]")?);
                put(s_pow.x, s_pow.y - 1, s_pow.z + 2, &glass);
                put(s_pow.x + 1, s_pow.y, s_pow.z + 2, &b("minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=side]")?);
                put(s_pow.x + 1, s_pow.y - 1, s_pow.z + 2, &glass);
                put(s_pow.x + 1, s_pow.y - 1, s_pow.z + 1, &b("minecraft:redstone_wire[east=side,north=none,power=0,south=up,west=none]")?);
                put(s_pow.x + 1, s_pow.y - 2, s_pow.z + 1, &glass);
                put(s_pow.x + 2, s_pow.y - 1, s_pow.z + 1, &repeater_e);
                put(s_pow.x + 2, s_pow.y - 2, s_pow.z + 1, &glass);
                BlockPos::new(s_pow.x + 3, s_pow.y - 1, s_pow.z + 1)
            } else if so.siding_north && so.feed_path {
                // (--feed-path (ironspec 09:53, the budget box (x-2..x-1, 22..23, z 9)): the input
                // cell (x-2,23,9) dust turning from the east into the repeater's back, on glass; the
                // feed (x-1,23,9) a repeater facing west into it, on glass; the switched cell its
                // input (x,23,9), day-1's controller line arriving from the east or below)
                put(s_pow.x, s_pow.y, s_pow.z + 2, &b("minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=none]")?);
                put(s_pow.x, s_pow.y - 1, s_pow.z + 2, &glass);
                put(s_pow.x + 1, s_pow.y, s_pow.z + 2, &repeater_e);
                put(s_pow.x + 1, s_pow.y - 1, s_pow.z + 2, &glass);
                BlockPos::new(s_pow.x + 2, s_pow.y, s_pow.z + 2)
            } else {
                BlockPos::new(s_pow.x, s_pow.y, s_pow.z + 2)
            }
        }
        None => s_pow,
    };
    let rest_pos = BlockPos::new(sx, rest_y, rest_z);
    // (--nap-guard: a string over the branch cell before the merge (x,m+1,mz+1), hooks at x+-1, stone
    // at x+-2: the siding releases only while no cart is there)
    if so.guard {
        put(lx, m + 1, mz + 1, &string_ew);
        put(lx - 1, m + 1, mz + 1, &hook_e);
        put(lx + 1, m + 1, mz + 1, &hook_w);
        put(lx - 2, m + 1, mz + 1, &stone);
        put(lx + 2, m + 1, mz + 1, &stone);
    }
    // (--nap-ai: the shared bed at the bed stop's head, loop frame (-3,-18,3), facing north (its foot
    // at z + 1): every rider's HOME, its one ticket taken (the build's claims); the bench's box grows
    // to hold it; the run at night (day time 13,300 at the first bench tick: REST all along))
    let bed = wp(BlockPos::new(BED.0, BED.1, BED.2));
    if so.ai {
        put(BED.0, BED.1, BED.2, &b("minecraft:red_bed[facing=north,occupied=false,part=head]")?);
        put(BED.0, BED.1, BED.2 + 1, &b("minecraft:red_bed[facing=north,occupied=false,part=foot]")?);
        put(BED.0, BED.1 - 1, BED.2, &stone);
        put(BED.0, BED.1 - 1, BED.2 + 1, &stone);
    }
    for p in &so.posts {
        put(p.0, p.1 - 1, p.2, &glass);
    }
    let lo = if so.ai { BlockPos::new((lx - 6).min(BED.0 - 3), (-12).min(BED.1 - 3), -6) } else { BlockPos::new(lx - 6, -12, -6) };
    // (the bench's box in the world's frame: its z range mirrored with --mirror-z)
    let (lo, hi) = {
        let hi = BlockPos::new(lx + 6, 30, 14);
        let (za, zb) = (mzc(lo.z), mzc(hi.z));
        (BlockPos::new(lo.x, lo.y, za.min(zb)), BlockPos::new(hi.x, hi.y, za.max(zb)))
    };
    let spec = FarmSpec { blocks, entities: Vec::new(), output: wp(BlockPos::new(lx + 6, 30, 12)), lo, hi, block_entities: Vec::new(), rock: false, nether: None, land: false };
    // (ironspec's 11:05 audit, nothing silently dropped: the composer's world is the spec's box,
    // and a raw placement outside it would be skipped; every bench cell inside it, or the run fails)
    let outside: Vec<String> = spec.blocks.iter().filter(|(p, _)| p.x < lo.x || p.y < lo.y || p.z < lo.z || p.x > hi.x || p.y > hi.y || p.z > hi.z).map(|(p, b)| format!("{},{},{} {b}", p.x, p.y, p.z)).collect();
    if !outside.is_empty() {
        return Err(format!("siding bench: {} cells outside the world's box {lo:?}..{hi:?}: {}", outside.len(), outside.join("; ")));
    }
    let ro = if so.ai { RunOpts { ai: true, day0: 13_300, ..RunOpts::steady(1, 0, WalkMode::Random) } } else { RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) } };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    if so.ai {
        redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    }
    // (the scarer posts: a zombie seated in a boat (it stays put), persistent)
    for p in &so.posts {
        redsim::entity::spawn(&mut run.world, "minecraft:oak_boat", [p.0 as f64 + 0.5, p.1 as f64, mzf(p.2 as f64 + 0.5)], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b,IsBaby:0b}]}"))?;
    }
    // (every rail back to its laid shape without placement logic: in the bench all blocks go down at
    // once, so a rail placed beside another can be reshaped by the connection rule (the branch flat
    // beside the raised siding's lower slope turned toward it); the build places them in order)
    let mut reshaped = 0usize;
    let mut reshaped_note: Vec<String> = Vec::new();
    let mut popped: Vec<String> = Vec::new();
    for (p, blk) in &spec.blocks {
        // (a rail the placement changed: a rail of another shape (the connection rule reshaping a
        // neighbour placed at the same time) is put back RAW; a rail that is GONE (popped:
        // BaseRailBlock.shouldBeRemoved, no rigid block below or on an ascending rail's high side)
        // is a design failure, counted, never restored)
        let now_blk = run.world.get(*p);
        if matches!(blk, Block::PlainRail { .. } | Block::Rail { .. }) && now_blk != *blk && !matches!(now_blk, Block::PlainRail { .. } | Block::Rail { .. }) {
            popped.push(format!("({},{},{})", p.x, p.y, p.z));
            continue;
        }
        if matches!(blk, Block::PlainRail { .. } | Block::Rail { .. })
            && run.world.get(*p) != *blk
        {
            if reshaped_note.len() < 4 {
                let short = |b: &Block| format!("{b:?}").replace("PlainRail { shape: ", "").replace("Rail { activator: false, shape: ", "").replace(" }", "");
                reshaped_note.push(format!("({},{},{}) {} -> {}", p.x, p.y, p.z, short(&run.world.get(*p)), short(blk)));
            }
            run.world.set_block(*p, blk.clone(), redsim::world::flags::RAW);
            reshaped += 1;
        }
    }
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt0 = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let nbt_in = format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", vz(-o.v_in));
    // (the lane's carts: the front braked mid-R_1, the rest up the four slopes 0.79 apart (z), the
    // overflow arriving on the branch as returns)
    let zc = lx as f64 + 0.5;
    let slope_y = |z: f64| {
        let cz = z.floor() as i32;
        // (ascending south: block (x, 16 + cz, cz) rises toward +z)
        (16 + cz) as f64 + (z - cz as f64) + 0.0625
    };
    let mut carts: Vec<Option<u32>> = Vec::new();
    let mut pending: Vec<(i64, usize, bool)> = Vec::new();
    // (--start-steady (ironspec 07:57): the start state is the steady state: 7 riders in their slots
    // (H on the trapdoor, then R_1 and the slopes 0.79 apart from z 1.28), the 8th in the station's
    // pocket, leaving with the cycle before the first release (back on the branch at that release's
    // tick minus the period plus ret: the first release is warm + corner mod period, so a negative
    // corner's first release comes almost a period after warm); the R line off until the first
    // release's trapdoors shut)
    // (the drive's delay from the siding's switch to R: R's repeater 2 gt, the feed path's
    // repeater 2 more, day-1's delay-3 repeater 6 more with --feed-slime)
    let drive_delay: i64 = if so.siding_north && so.feed_path && so.feed_slime {
        8
    } else if so.siding_north && so.feed_path {
        4
    } else {
        2
    };
    let warm0 = 300i64;
    if so.start_steady {
        let first_release = warm0 + o.corner.rem_euclid(o.period);
        for k in 0..o.per_lane {
            if k >= 7 {
                carts.push(None);
                pending.push(((first_release - o.period + o.ret).max(1), carts.len() - 1, false));
                continue;
            }
            let (y, z) = if k == 0 { (16.1875, 0.49) } else { let z = 1.28 + 0.79 * (k - 1) as f64; (slope_y(z), z) };
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, y, mzf(z)], Some(&nbt0))?;
            if so.ai {
                farm_rider(&mut run.world, id, bed, None);
            }
            carts.push(Some(id));
        }
    }
    for k in 0..o.per_lane {
        if so.start_steady {
            break;
        }
        let z = 1.5 + 0.79 * k as f64;
        if z > mz as f64 - 0.4 {
            carts.push(None);
            pending.push((20 + 25 * k as i64, carts.len() - 1, false));
            continue;
        }
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, slope_y(z), mzf(z)], Some(&nbt0))?;
        if so.ai {
            farm_rider(&mut run.world, id, bed, None);
        }
        carts.push(Some(id));
    }
    let n = carts.len();
    let warm = 300i64;
    let rel_end = warm + o.cycles * o.period;
    // (the lane's steady state: every cart through D once and back, after the start's full lane; with
    // --start-steady the start is the steady state)
    let steady_from = if so.start_steady { warm } else { warm + o.per_lane as i64 * o.period + o.ret.max(so.nap_ret) };
    let ticks = rel_end + 400;
    let hold_hook = BlockPos::new(lx - 1, 17, 0);
    let merge_hook = if so.drop_entry {
        BlockPos::new(lx - 1, m, mz - 1)
    } else if so.ns_merge {
        BlockPos::new(lx, m + 1, mz - 1)
    } else {
        BlockPos::new(lx - 1, m + 1, mz)
    };
    // (the cell a pass is credited in: the drop column with the drop entry, else the merge; with
    // --siding-north the guard detector's cell (its rising edge is the pass))
    let pass_z = if so.siding_north { mz + so.guard_dz } else if so.drop_entry { mz - 1 } else { mz };
    let (mut trap_until, mut trap_is) = (-1i64, false);
    let (mut r_until, mut r_is) = (if so.start_steady { -1 } else { 10 + o.r_pulse }, false);
    let (mut s_until, mut s_is) = (-1i64, false);
    let mut held = 0i64;
    let mut drops: Vec<(i64, usize)> = Vec::new();
    let mut per_release: Vec<usize> = Vec::new();
    let mut fell = vec![false; n];
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    let mut before_prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    // (landings: a dropping cart's box meeting a queued cart's box during the fall or at its landing
    // tick (lift3's box test), once per drop: (cart, its speed (all three axes), q); the q values at
    // which a queued cart's box reached the top slope's cell (the landing cell))
    let mut landing_meets: Vec<(usize, f64, usize)> = Vec::new();
    let mut met_this_drop = vec![false; n];
    let mut was_falling = vec![false; n];
    let mut occupied_q: std::collections::BTreeSet<usize> = Default::default();
    let mut top_stay: Vec<Option<(usize, i64, i64, i64)>> = vec![None; n];
    let mut top_by_q: std::collections::BTreeMap<usize, (i64, i64)> = Default::default();
    let mut top_start: std::collections::BTreeMap<usize, (i64, i64)> = Default::default();
    let mut top_end: std::collections::BTreeMap<usize, (i64, i64)> = Default::default();
    // (the napper bookkeeping: the cart index of the napper waiting, its predecessor's index, the
    // predecessor's merge pass tick, the release tick)
    let mut nap: Option<(usize, usize)> = None;
    let mut pred_pass: Option<i64> = None;
    let mut released_at: Option<i64> = None;
    let mut v_rest_occ: Option<usize> = None;
    let mut v_hold_occ: Option<usize> = None;
    let (mut v_holds, mut v_holds_start, mut v_holds_end) = (0usize, 0usize, 0usize);
    let mut v_run: Vec<usize> = Vec::new();
    let mut stacked_ticks = 0usize;
    let mut stacked_note: Option<String> = None;
    let mut launch_for: Option<i64> = None;
    let mut launches: Vec<(i64, f64, bool)> = Vec::new();
    let mut launch_stuck = 0usize;
    let mut merge_was = false;
    let mut nap_rows: Vec<String> = Vec::new();
    let mut contacts: Vec<(usize, f64, f64, f64, bool)> = Vec::new();
    let mut had_contact = vec![true; n];
    let mut creep = 0.0f64;
    let mut nap_rest: Option<(f64, f64)> = None;
    let mut wrong_way: Vec<String> = Vec::new();
    // (the curve rule: who is on the curve (x-1,22,6) and the merge (x,22,6), entry and clear ticks)
    let in_cell = |p: (f64, f64, f64), cx: i32, cy: i32, cz: i32| p.0 + 0.49 > cx as f64 && p.0 - 0.49 < (cx + 1) as f64 && p.2 + 0.49 > cz as f64 && p.2 - 0.49 < (cz + 1) as f64 && p.1 > cy as f64 - 0.5 && p.1 < cy as f64 + 1.0;
    let mut merge_occ: Vec<(usize, i64, Option<i64>)> = Vec::new();
    let mut touching = 0usize;
    let mut last_pass: Vec<Option<i64>> = vec![None; n];
    let mut lap_wait = false;
    let mut nap_since = 0i64;
    let mut late_naps = 0usize;
    let mut nap_rested_at: Option<i64> = None;
    let mut nap_wait_max = 0i64;
    let mut last_any_pass: Option<(i64, usize)> = None;
    let mut release_lead: Option<usize> = None;
    let mut release_window: Option<usize> = None;
    let mut rel_times: Vec<i64> = Vec::new();
    let mut nap_pick_i: i64 = -1;
    // (the riders' brains (--nap-ai): the napper's over its siding wait: its REST and panic ticks,
    // its longest calm-REST run, its CANT_REACH stamp age at arrival, at release and the oldest at a
    // calm REST tick, HOME kept)
    // (--nap-ai: the napper's window (ironspec 05:18): from its arrival on the siding (its stamp
    // pre-aged by the leg since its last clear at the bed) to its first clear after the release,
    // or the drop, where the station continues it)
    let mut wins: std::collections::BTreeMap<usize, NapBrain> = Default::default();
    let mut reload_t: Option<i64> = None;
    let mut agg = WinAgg::default();
    // (--k-row: the through-carts on the split row (id, spawn tick), their exits, and their contact
    // with a waiting napper; the nappers' rest z; the branch returns' merge-entry speed; --flat-detector)
    let mut through: Vec<(u32, i64)> = Vec::new();
    let (mut th_spawned, mut th_exited, mut th_slowed, mut th_lost) = (0usize, 0usize, 0usize, 0usize);
    let mut th_min_exit = f64::MAX;
    let mut th_max_dz = 0f64;
    let (mut th_push_ticks, mut th_overlap_ticks) = (0usize, 0usize);
    let mut th_zgap = f64::MAX;
    let mut rest_zs: Vec<f64> = Vec::new();
    let mut merge_v: Vec<f64> = Vec::new();
    let (mut flat_false, mut flat_nap_passes, mut flat_was) = (0usize, 0usize, false);
    let mut lane_push_ticks = 0usize;
    let mut merge_west = f64::MAX;
    let mut lane_on_siding = 0usize;
    // (the siding's cells (a lane cart's centre on one is a failure), the napper's feed -> rest
    // transit, and D's timing: its rising edge (the pass with --siding-north) to the same cart's
    // merge-cell entry and clear)
    let siding_cells: Vec<(i32, i32, i32)> = if so.siding_north {
        vec![(sx, m + 1, mz + 1), (sx, m, mz), (sx, m, mz - 1), (sx + 1, m, mz - 1), (sx + 1, m, mz)]
    } else {
        vec![(sx, m + 1, mz + 2), (sx, m, mz + 1), (sx, m, mz), (sx + 1, m, mz)]
    };
    let mut nap_spawned_t: Option<i64> = None;
    let mut transits: Vec<i64> = Vec::new();
    let mut pass_of: Vec<Option<i64>> = vec![None; n];
    let (mut d_to_entry, mut d_to_clear): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
    let mut queue_rest: std::collections::BTreeMap<usize, Vec<Vec<(f64, f64, f64)>>> = Default::default();
    let mut first_snaps: Vec<(i64, Vec<(f64, f64, f64, f64)>)> = Vec::new();
    let (mut occ_max, mut occ8, mut occ_start_max) = (0usize, 0usize, 0usize);
    let nbt_feed = so.nap_feed_v.map(|v| format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", vz(-v)));
    let nbt_row = so.k_row.map(|v| format!("{{Motion:[{v:.4}d,0.0d,0.0d]{rider}}}"));
    let mut switch_last_on = -1000i64;
    let mut stray_power = 0usize;
    let mut stray_with_guard = 0usize;
    let mut guard_on_waiting = 0usize;
    let mut knocked = 0usize;
    let mut knocked_this = false;
    let mut frozen32: Option<[i32; 4]> = None;
    let mut touch_note: Vec<String> = Vec::new();
    let mut drop_count = 0usize;
    let mut entered: Vec<usize> = Vec::new();
    let mut order_note: Vec<String> = Vec::new();
    let mut order_broken = 0usize;
    let mut off_rest = vec![0i64; n];
    let mut off_rail = 0usize;
    let mut off_note: Vec<String> = Vec::new();
    let mut eye_bad: std::collections::BTreeMap<String, usize> = Default::default();
    let mut min_health = f32::MAX;
    let mut restage: Vec<i64> = Vec::new();
    let mut restage_from: Option<i64> = None;
    let mut hh_was = false;
    // (--fill-scene (tick-exact's df472311b `mobai seat-f3` queue fill, for a per-cart diff against
    // the official server): empty carts summoned at the fill cells' centres (+0.5 up), 10 gt apart,
    // then a ridden cart at the seat cell's centre (+0.5 up), 40 gt to settle; every cart's position,
    // sorted as the official print sorts them; the bench's normal run is skipped)
    if let Some((fill, seat)) = &so.fill_scene {
        let mut ids: Vec<u32> = Vec::new();
        for f in fill {
            ids.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [f.0 as f64 + 0.5, f.1 as f64 + 0.5, mzf(f.2 as f64 + 0.5)], None)?);
            for _ in 0..10 {
                run.tick();
            }
        }
        ids.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [seat.0 as f64 + 0.5, seat.1 as f64 + 0.5, mzf(seat.2 as f64 + 0.5)], Some(&nbt0))?);
        for _ in 0..40 {
            run.tick();
        }
        let mut ps: Vec<String> = ids.iter().filter_map(|id| st(&run.world, *id)).map(|p| format!("({:.2},{:.2},{:.2})", p.0, p.1, p.2)).collect();
        ps.sort();
        return Ok(format!(
            "the fill scene (empty carts summoned at {fill:?}, 10 gt apart, then a ridden cart at {seat:?}, 40 gt; the top cell {}; NOT A REFERENCE FOR EMPTY CARTS: tick-exact's recorded cart-pairs scene replays in redsim with 0 differences, the empty pair sliding; these empty carts' rests are this procedure's): the queue's carts at rest (the rider's included): {}\n",
            if so.top_flat { "north_south (--top-flat)" } else { "ascending_south (the design)" },
            ps.join(" ")
        ));
    }
    // (day-1's ring windows: the release index of the lane's window pass at `pp`, if any (usize::MAX:
    // no ring, every pass samples); LATCH marks a release by the late latch)
    const LATCH: usize = usize::MAX - 1;
    let window_open = |pp: i64, rel_times: &Vec<i64>| -> Option<usize> {
        if so.ring <= 0 {
            return Some(usize::MAX);
        }
        let win_u = (so.pick_u + so.win_du).rem_euclid(so.ring);
        rel_times.iter().enumerate().find(|(j, rt)| (*j as i64).rem_euclid(so.ring) == win_u && pp >= **rt + so.win_tap && pp < **rt + so.win_tap + o.period).map(|(j, _)| j)
    };
    let mut latch_at: Option<i64> = None;
    let mut released_by_latch = false;
    for t in 0..ticks {
        // (the logins and freezes as one schedule: frozen while any interval covers t, returns waiting
        // while frozen)
        let want_frozen = o.login.iter().chain(o.freeze32.iter()).any(|(a, d)| t >= *a && t < a + d);
        if want_frozen && frozen32.is_none() {
            frozen32 = Some(crate::liveness::freeze32(&mut run.world));
        }
        if !want_frozen
            && let Some(pb) = frozen32.take()
        {
            crate::liveness::thaw32(&mut run.world, pb);
        }
        let hh = matches!(run.world.get(wp(hold_hook)), Block::TripwireHook { powered: true, .. });
        held = if hh { held + 1 } else { 0 };
        // (the restage: from a release to the hold's string rising again once the trapdoor shut)
        if hh && !hh_was
            && t > trap_until
            && let Some(t0) = restage_from.take()
        {
            restage.push(t - t0);
        }
        hh_was = hh;
        let mh = if so.siding_north {
            matches!(run.world.get(wp(BlockPos::new(lx, m, mz + so.guard_dz))), Block::DetectorRail { powered: true, .. })
        } else {
            matches!(run.world.get(wp(merge_hook)), Block::TripwireHook { powered: true, .. })
        };
        // (every cart's last pass at the pass string: the rising edge, credited to the cart whose box
        // swept the string's slab (y + 1/16 .. 2.5/16 of its cell) during the tick that pressed it,
        // from its place before that tick to its place after (G1: a fast cart can be out of the cell
        // by the next tick's start))
        if mh && !merge_was {
            let slab = |p0: (f64, f64, f64, f64), p1: (f64, f64, f64, f64)| {
                let (zlo, zhi) = (p0.2.min(p1.2) - 0.49, p0.2.max(p1.2) + 0.49);
                let (ylo, yhi) = (p0.1.min(p1.1), p0.1.max(p1.1) + 0.7);
                (p1.0 - zc).abs() < 0.6 && zhi > pass_z as f64 && zlo < (pass_z + 1) as f64 && yhi > m as f64 + 0.0625 && ylo < m as f64 + 0.15625
            };
            let swept = (0..n).find(|c| match (before_prev[*c], prev[*c]) {
                (Some(p0), Some(p1)) => slab(p0, p1),
                (None, Some(p1)) => slab(p1, p1),
                _ => false,
            });
            let found = swept.or_else(|| (0..n).find(|c| carts[*c].and_then(|id| st(&run.world, id)).is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, pass_z))));
            if let Some(c) = found {
                last_pass[c] = Some(t);
                last_any_pass = Some((t, c));
                pass_of[c] = Some(t);
            }
            // (--nap-late-latch: the lane's window pass with its napper picked but not yet waiting on
            // the siding sets the late latch)
            if so.late_latch
                && so.ring > 0
                && nap.is_some()
                && released_at.is_none()
                && latch_at.is_none()
                && window_open(t, &rel_times).is_some()
                && !nap_rested_at.is_some_and(|r| r <= t)
            {
                latch_at = Some(t);
            }
        }
        merge_was = mh;
        // (the release (day-1's rule): the napper resting on the siding, its predecessor's last merge
        // pass seen and not yet followed by its successor's (else it waits for the predecessor's next
        // pass, a lane-lap later), K gt after that pass)
        let nap_ready = nap.is_some_and(|(nc, _)| carts[nc].and_then(|id| st(&run.world, id)).is_some_and(|p| p.3 < 0.002 && p.1 > rest_y as f64 - 0.1 && p.1 < rest_y as f64 + 1.0 && p.2.floor() as i32 == rest_z && (p.0 - (sx as f64 + 0.5)).abs() < 0.3));
        if nap_ready && nap_rested_at.is_none() {
            nap_rested_at = Some(t);
            if let Some((nc, _)) = nap
                && let Some(p) = carts[nc].and_then(|id| st(&run.world, id))
            {
                rest_zs.push(p.2);
                if let Some(s) = nap_spawned_t.take() {
                    transits.push(t - s);
                }
            }
        }
        // (--k-row: the through-carts: spawned on the row's west cell eastbound `k_offsets` gt after
        // each release (none while a napper is entering: the split's K is held for it), followed to
        // the row's east cell where they leave the bench; each tick, a through-cart whose box
        // inflated 0.2 in x and z (the push test) meets the napper's box while the napper waits)
        if let (Some(v), Some(nr)) = (so.k_row, nbt_row.as_ref()) {
            let entering = nap.is_some() && nap_rested_at.is_none();
            if !entering
                && frozen32.is_none()
                && let Some(&rt) = rel_times.last()
                && so.k_offsets.contains(&(t - rt))
            {
                // (each through-cart at its own x in the row's west cell (G1, the S-bend's margin): a
                // cart entering K's north_east curve from the west at x fraction xf is projected to z
                // (mz + 2) + xf / 2 (moveAlongTrack's curve projection), so the spawn x steps 0.0371
                // through 0.9 of the cell and the entry fractions cover the whole [0, v) range)
                let x0 = (sx - 2) as f64 + 0.05 + (th_spawned as f64 * 0.0371).rem_euclid(0.9);
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(nr))?;
                if so.ai {
                    farm_rider(&mut run.world, id, bed, None);
                }
                through.push((id, t));
                th_spawned += 1;
            }
            let napbox = nap.and_then(|(nc, _)| carts[nc]).and_then(|id| st(&run.world, id)).filter(|_| nap_rested_at.is_some() && released_at.is_none());
            let mut keep: Vec<(u32, i64)> = Vec::new();
            for (id, t0) in through.drain(..) {
                let Some(p) = st(&run.world, id) else {
                    th_lost += 1;
                    continue;
                };
                if let Some(q) = napbox {
                    if push_contact(p, q, 0.2) {
                        th_push_ticks += 1;
                    }
                    if push_contact(p, q, 0.0) {
                        th_overlap_ticks += 1;
                    }
                    // (the z gap where the boxes share x (within the push reach) and y)
                    if (p.0 - q.0).abs() < 0.98 + 0.2 && p.1 < q.1 + 0.7 && q.1 < p.1 + 0.7 {
                        th_zgap = th_zgap.min((p.2 - q.2).abs() - 0.98 - 0.2);
                    }
                }
                th_max_dz = th_max_dz.max(((mz + 2) as f64 + 0.5 - p.2).abs());
                if p.0 > (sx + 1) as f64 + 0.4 {
                    th_exited += 1;
                    th_min_exit = th_min_exit.min(p.3);
                    if p.3 < 0.95 * v {
                        th_slowed += 1;
                    }
                    kill_with_rider(&mut run.world, id);
                } else if t - t0 > 80 {
                    th_lost += 1;
                    kill_with_rider(&mut run.world, id);
                } else {
                    keep.push((id, t0));
                }
            }
            through = keep;
        }
        // (--flat-detector: the siding flat's detector powered with no napper over it is a false trip)
        // (every lane cart in push contact with the waiting napper (the bar: 0), and the westmost centre
        // of a cart in the merge cell (the curve's snap toward its west exit))
        if let Some((nc, _)) = nap
            && nap_rested_at.is_some()
            && released_at.is_none()
            && let Some(q) = carts[nc].and_then(|id| st(&run.world, id))
        {
            for c in 0..n {
                if c != nc
                    && let Some(p) = carts[c].and_then(|id| st(&run.world, id))
                    && push_contact(p, q, 0.2)
                {
                    lane_push_ticks += 1;
                }
            }
        }
        for c in 0..n {
            if nap.is_none_or(|(nc, _)| nc != c)
                && let Some(p) = carts[c].and_then(|id| st(&run.world, id))
            {
                if in_cell((p.0, p.1, p.2), lx, m, mz) {
                    merge_west = merge_west.min(p.0 - lx as f64);
                }
                // (a lane cart's centre on one of the siding's cells (the bar: 0))
                let cell = (p.0.floor() as i32, p.1.floor() as i32, p.2.floor() as i32);
                if siding_cells.iter().any(|sc| sc.0 == cell.0 && sc.2 == cell.2 && (cell.1 == sc.1 || cell.1 == sc.1 - 1)) {
                    lane_on_siding += 1;
                }
            }
        }
        if so.flat_detector {
            // (the detector: the siding's flat (x-1,m,mz), or with --siding-north the feed F
            // (sx,m+1,mz+1); a cart other than the napper whose box meets its detection box (the
            // block inset 0.2 in x and z, 0.8 high) is a false trip (the bar: 0), as ticks)
            let dc = if so.siding_north { BlockPos::new(sx, m + 1, mz + 1) } else { BlockPos::new(lx - 1, m, mz) };
            let on = matches!(run.world.get(wp(dc)), Block::DetectorRail { powered: true, .. });
            let nap_c = nap.map(|(nc, _)| nc);
            let meets = |p: (f64, f64, f64, f64)| {
                p.0 + 0.49 > dc.x as f64 + 0.2 && p.0 - 0.49 < dc.x as f64 + 0.8 && p.2 + 0.49 > dc.z as f64 + 0.2 && p.2 - 0.49 < dc.z as f64 + 0.8 && p.1 + 0.7 > dc.y as f64 && p.1 < dc.y as f64 + 0.8
            };
            let others = (0..n).filter(|c| Some(*c) != nap_c).filter_map(|c| carts[c].and_then(|id| st(&run.world, id))).chain(through.iter().filter_map(|(id, _)| st(&run.world, *id))).any(meets);
            if others {
                flat_false += 1;
            }
            let nap_there = nap_c.and_then(|nc| carts[nc]).and_then(|id| st(&run.world, id)).is_some_and(meets);
            if on && !flat_was && nap_there {
                flat_nap_passes += 1;
            }
            flat_was = on;
        }
        let guard_on = if so.guard_det {
            matches!(run.world.get(wp(BlockPos::new(lx, m, mz + so.guard_dz))), Block::DetectorRail { powered: true, .. })
        } else {
            so.guard && matches!(run.world.get(wp(BlockPos::new(lx - 1, m + 1, mz + 1))), Block::TripwireHook { powered: true, .. })
        };
        // (day-1's option (ii), --nap-any-pass (ironspec 04:19): the napper-present sample at a pass:
        // a napper resting on the siding when a cart's pass edge comes is released K after that pass
        // (held while the guard is powered), behind whichever cart passed. Without --nap-ring every
        // pass of the lane samples (the merge physics at every phase; its waits are not (ii)'s); with
        // --nap-ring the lane's window is open at one ring position in `ring` (day-1's ring: the
        // napper picked at ring position --nap-pick-u, its window --nap-win-du positions later, one
        // beat wide from that release + --nap-win-tap), so only the window's pass samples; with
        // --nap-late-latch (ironspec 05:15, fix (A)) a window passing without its napper waiting sets
        // the lane's late latch, and the napper then goes K after the first pass it is waiting for)
        let via = last_any_pass.and_then(|(pp, _)| window_open(pp, &rel_times).or_else(|| latch_at.filter(|la| pp > *la).map(|_| LATCH)));
        if so.any_pass || so.ring > 0 {
            if nap.is_some()
                && released_at.is_none()
                && nap_ready
                && !guard_on
                && let Some((pp, pc)) = last_any_pass
                && t >= pp + so.k
                && nap_rested_at.is_some_and(|r| r <= pp)
                && let Some(wj) = via
            {
                s_until = t + so.nap_pulse;
                released_at = Some(t);
                pred_pass = Some(pp);
                release_lead = Some(pc);
                release_window = (wj != usize::MAX && wj != LATCH).then_some(wj);
                released_by_latch = wj == LATCH;
            }
        } else if let Some((nc, pred)) = nap
            && released_at.is_none()
            && nap_ready
            && !guard_on
            && let Some(pp) = last_pass[pred]
            && t >= pp + so.k
            && (!so.early_only || nap_rested_at.is_some_and(|r| pp >= r))
        {
            let succ = drops.iter().position(|d| d.1 == nc).and_then(|i| drops.get(i + 1)).map(|d| d.1);
            let succ_after = succ.and_then(|s| last_pass[s]).is_some_and(|sp| sp > pp && sp >= nap_since);
            if succ_after {
                if !lap_wait {
                    lap_wait = true;
                }
            } else {
                s_until = t + so.nap_pulse;
                released_at = Some(t);
                pred_pass = Some(pp);
                release_lead = Some(pred);
            }
        }
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && t < rel_end && ph == 0 {
            trap_until = t + o.drop_w;
            per_release.push(0);
            rel_times.push(t);
            restage_from = Some(t);
            // (the queue at rest at the release tick (the trapdoor opens now): every lane cart's
            // position, H first, kept by occupancy when all of them are at rest)
            let lane_now: Vec<(f64, f64, f64, f64)> = (0..n).filter_map(|c| carts[c].and_then(|id| st(&run.world, id))).filter(|p| (p.0 - zc).abs() < 0.6 && p.2 < mz as f64 && p.1 < m as f64 - 0.5).collect();
            // (the lane's occupancy at each release after the start (the steady state): its maximum,
            // and the releases at 8 (the 8th slot is the top cell (x,m-2,mz-1)))
            if t >= steady_from {
                occ_max = occ_max.max(lane_now.len());
                if lane_now.len() >= 8 {
                    occ8 += 1;
                }
            } else {
                occ_start_max = occ_start_max.max(lane_now.len());
            }
            if !lane_now.is_empty() && lane_now.iter().all(|p| p.3 < 0.002) {
                let mut s: Vec<(f64, f64, f64)> = lane_now.iter().map(|p| (p.0, p.1, p.2)).collect();
                s.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                queue_rest.entry(s.len()).or_default().push(s);
            }
            // (the first three releases' queues, as they stand (the start-up check))
            if first_snaps.len() < 3 {
                let mut s: Vec<(f64, f64, f64, f64)> = lane_now.clone();
                s.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                first_snaps.push((t, s));
            }
        }
        let want_trap = t < trap_until;
        if want_trap != trap_is {
            trap_is = want_trap;
            run.world.set_block(wp(BlockPos::new(lx, 16, 0)), if want_trap { trap_open.clone() } else { trap_shut.clone() }, redsim::world::flags::ALL);
            if !want_trap {
                r_until = t + o.r_pulse;
            }
        }
        let want_r = t < r_until;
        if want_r != r_is {
            r_is = want_r;
            run.world.set_block(wp(BlockPos::new(lx + 1, 17, 1)), if want_r { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
        }
        let want_s = t < s_until;
        if want_s != s_is {
            s_is = want_s;
            run.world.set_block(wp(s_switch), if want_s { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
        }
        if s_is {
            switch_last_on = t;
        }
        // (every power reaching the siding rest: the rail powered while its declared switch has been
        // off longer than the drive's delay (R's repeater 2 gt, the feed path's repeater 2 more, or
        // day-1's delay-3 6 more with --feed-slime) plus 2 gt is a stray power (the bar: 0); the
        // guard detector powered in those ticks shows whether it was the guard)
        let rest_powered = matches!(run.world.get(wp(rest_pos)), Block::Rail { powered: true, .. });
        let det_powered = matches!(run.world.get(wp(BlockPos::new(lx, m, mz + so.guard_dz))), Block::DetectorRail { powered: true, .. });
        if det_powered && nap.is_some() && released_at.is_none() {
            guard_on_waiting += 1;
        }
        if rest_powered && t - switch_last_on > drive_delay + 2 {
            stray_power += 1;
            if det_powered {
                stray_with_guard += 1;
            }
            // (MCTAS_STRAY_DEBUG: the rest's and its power cell's neighbourhoods at the first stray tick)
            if stray_power == 1 && std::env::var("MCTAS_STRAY_DEBUG").is_ok() {
                for base in [rest_pos, s_pow] {
                    for (dx, dy, dz) in [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        let p = BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
                        eprintln!("t {t}: stray debug {:?} {:?}", (p.x, p.y, p.z), run.world.get(wp(p)));
                    }
                }
            }
        }
        // (returns: on the branch at z mz + 2.5 moving -z, or a napper onto the siding slope at rest;
        // nothing arrives while the bench is frozen)
        let due: Vec<(usize, bool)> = if frozen32.is_some() { Vec::new() } else { pending.iter().filter(|p| p.0 <= t).map(|p| (p.1, p.2)).collect() };
        for (c, is_nap) in due {
            if is_nap {
                let id = match &nbt_feed {
                    // (--nap-feed-v: the napper arrives on the feed cell (K's cell under --k-row) moving north)
                    Some(nf) => redsim::entity::spawn(&mut run.world, "minecraft:minecart", [sx as f64 + 0.5, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5 + so.nap_feed_phase)], Some(nf))?,
                    None => redsim::entity::spawn(&mut run.world, "minecraft:minecart", [sx as f64 + 0.5, rest_y as f64 + 0.5 + 0.0625, mzf(rest_z as f64 + 0.5)], Some(&nbt0))?,
                };
                nap_spawned_t = Some(t);
                if so.ai {
                    farm_rider(&mut run.world, id, bed, so.stamp_at_arrival);
                    // (one window per napper cart: a window still open when its own cart is picked
                    // again is cut short, a failure)
                    if let Some(onb) = wins.remove(&c) {
                        agg.close(c, onb, "CUT SHORT: still open when its cart was picked again".to_string(), None, t);
                        agg.cut += 1;
                    }
                    wins.insert(c, NapBrain { arrived_t: Some(t), ..Default::default() });
                    agg.total += 1;
                }
                carts[c] = Some(id);
                pending.retain(|p| p.1 != c);
                continue;
            }
            let zs = mz as f64 + 2.5;
            let clear = carts.iter().all(|id| id.and_then(|id| st(&run.world, id)).is_none_or(|(x, y, z, _)| !(y > m as f64 - 0.5 && (z - zs).abs() < 1.3 && (x - zc).abs() < 0.9)));
            if clear {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, m as f64 + 0.0625, mzf(zs)], Some(&nbt_in))?;
                if so.ai {
                    farm_rider(&mut run.world, id, bed, None);
                }
                carts[c] = Some(id);
                pending.retain(|p| p.1 != c);
            }
        }
        // (a restart between ticks t - 1 and t (cc767b78's helper, 58f6f0084): a save/stop/start in a
        // load order, the carts followed to their new ids; the bench's controller runs on (its timers
        // are game ticks, and no game time passes). The cold start is no restart: it is the fresh
        // design started at every phase (`--corners 0..period-1`).)
        for (rt, how) in &o.restart {
            if *rt == t {
                crate::liveness::restart(&mut run.world, *how)?;
                reload_t = Some(t);
                for id in carts.iter_mut().flatten() {
                    *id = crate::liveness::follow(&run.world, *id);
                }
                // (--k-row's through-carts followed to their new ids too)
                for (id, _) in through.iter_mut() {
                    *id = crate::liveness::follow(&run.world, *id);
                }
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|id| id.and_then(|id| st(&run.world, id))).collect();
        // (ironspec 10:02, the lane overflow: the occupancy at which a cart first rests at the
        // lane's top cell V (x,m-2,mz-1), and any cart resting on another cart's top (its floor
        // within the other's box height above it, the boxes overlapping in x and z): a failure)
        {
            let lane: Vec<(f64, f64, f64, f64)> = now.iter().flatten().copied().filter(|p| (p.0 - zc).abs() < 0.6 && p.2 < mz as f64 && p.1 < m as f64 - 0.5 && p.1 > 15.0).collect();
            let top_z = (mz - 1) as f64;
            // (the steady window: from the steady state to the last release; after it the lane
            // refills with no release and holds V by design (the bench's end))
            let steady_now = t >= steady_from && t < rel_end;
            if steady_now && lane.iter().any(|p| p.3 < 0.002 && p.2 >= top_z && p.2 < top_z + 1.0) {
                v_rest_occ = Some(v_rest_occ.map_or(lane.len(), |o: usize| o.min(lane.len())));
            }
            // (a cart resting at V for 20 gt: V held as a queue slot, not a landing's momentary stop;
            // from the steady state (G1, CHANGES 1 on 0979a3e03: the full-lane start holds V from t 0
            // with 8 carts in the lane, a start transient; under ironspec's 07:57 ruling slot 8 is
            // never occupied, so a hold from the steady state is a failure))
            for (c, p) in now.iter().enumerate() {
                let at_v = p.is_some_and(|p| (p.0 - zc).abs() < 0.6 && p.3 < 0.002 && p.2 >= top_z && p.2 < top_z + 1.0 && p.1 > 15.0 && p.1 < m as f64 - 0.5);
                if c >= v_run.len() {
                    v_run.resize(c + 1, 0);
                }
                v_run[c] = if at_v { v_run[c] + 1 } else { 0 };
                if v_run[c] == 20 {
                    if steady_now {
                        v_holds += 1;
                        v_hold_occ = Some(v_hold_occ.map_or(lane.len(), |o: usize| o.min(lane.len())));
                    } else if t < steady_from {
                        v_holds_start += 1;
                    } else {
                        v_holds_end += 1;
                    }
                }
            }
            // (a cart resting on another's top stands on its box: at rest in the lane off its rail
            // (more than 0.15 above the slopes' surface there), the boxes overlapping in x and z and
            // its floor 0.6..0.8 above the other's (the box top 0.7; a queue's neighbours on a slope
            // ride their rails, never counted))
            let all: Vec<(f64, f64, f64, f64)> = now.iter().flatten().copied().collect();
            for (i, a) in all.iter().enumerate() {
                if a.3 >= 0.002 || (a.0 - zc).abs() >= 0.6 || a.2 <= 1.9 || a.2 >= mz as f64 || (a.1 - slope_y(a.2)).abs() <= 0.15 {
                    continue;
                }
                if let Some(b) = all.iter().enumerate().find(|(j, b)| *j != i && (a.0 - b.0).abs() < 0.98 && (a.2 - b.2).abs() < 0.98 && a.1 - b.1 > 0.6 && a.1 - b.1 < 0.8).map(|(_, b)| *b) {
                    stacked_ticks += 1;
                    if stacked_note.is_none() {
                        stacked_note = Some(format!("t {t}: a cart at rest at {:.2},{:.2},{:.2} on the cart at {:.2},{:.2},{:.2}", a.0, a.1, a.2, b.0, b.1, b.2));
                    }
                }
            }
        }
        // (--nap-ai: every open napper window each tick, one per napper cart (G1's CHANGES 3: a window
        // is never cut by the next napper): its activity, calm or panicking, its CANT_REACH stamp's age
        // (at the bar's ticks: calm REST with no walk target held), its clears (on the siding they
        // reset the age; the first after the release ends the window; a clear on a restart tick is
        // "a reload"), HOME)
        let mut win_ends: Vec<(usize, String)> = Vec::new();
        if so.ai {
            use redsim::entity::mob::ai::brain::{self, Act};
            for (wc, nb) in wins.iter_mut() {
                let wc = *wc;
                let Some(ri) = carts[wc].and_then(|id| rider_index(&run.world, id)) else { continue };
                if !brain::has_exact(&run.world, ri) {
                    continue;
                }
                let now = run.world.time;
                let x = brain::ex(&run.world, ri);
                let stamp = x.mem.cant_reach_since.map(|s| now - s);
                if !nb.tracking {
                    nb.tracking = true;
                    nb.stamp_at_arrival = stamp;
                }
                // (the release of this napper: its tick and its rest tick, for the wait)
                if nb.released_t.is_none()
                    && let Some((nc, _)) = nap
                    && nc == wc
                    && let Some(ra) = released_at
                {
                    nb.released_t = Some(ra);
                    nb.rested_t = nap_rested_at;
                    nb.late = released_by_latch || release_window.is_some_and(|wj| wj as i64 > nap_pick_i + so.win_du);
                }
                let after = nb.released_t.is_some();
                let vpos = run.world.entities.list[ri].pos;
                if nb.stamp_prev.is_some() && stamp.is_none() {
                    // (the clear's cause and target, from the walk target held the tick before: a nap
                    // (asleep), the target reached (within its close distance), or a complete path; the
                    // target the bed, or under PANIC a stroll point (within 2) or a flee point; on a
                    // restart tick, a reload)
                    let asleep = redsim::entity::mob::mob(&run.world, ri).sleeping_pos.is_some();
                    let what = match nb.prev_target {
                        _ if reload_t == Some(t) => "a reload (the restart at this tick)".to_string(),
                        _ if asleep => "a nap".to_string(),
                        Some((tp, close, panic)) => {
                            let vb = BlockPos::new(vpos.x.floor() as i32, vpos.y.floor() as i32, vpos.z.floor() as i32);
                            let d = (tp.x - vb.x).abs().max((tp.y - vb.y).abs()).max((tp.z - vb.z).abs());
                            let kind = if (tp.x - bed.x).abs() <= 1 && (tp.y - bed.y).abs() <= 1 && (tp.z - bed.z).abs() <= 1 {
                                "the bed"
                            } else if panic && d <= 2 {
                                "a stroll point"
                            } else if panic {
                                "a flee point"
                            } else {
                                "another target"
                            };
                            format!("{} to {kind} ({},{},{})", if d <= close { "reached" } else { "a complete path" }, tp.x, tp.y, tp.z)
                        }
                        None => "no walk target held the tick before".to_string(),
                    };
                    if after {
                        win_ends.push((wc, format!("cleared at t {t} at {:.2},{:.2},{:.2} by {what} ({} gt after the release)", vpos.x, vpos.y, vpos.z, t - nb.released_t.unwrap_or(t))));
                    } else {
                        nb.clears += 1;
                        if nb.clear_notes.len() < 3 {
                            nb.clear_notes.push(format!("t {t} by {what}"));
                        }
                    }
                }
                nb.prev_target = x.mem.walk_target.as_ref().and_then(|wt| {
                    let tp = match &wt.target {
                        redsim::entity::mob::ai::brain::Tracker::Block(p) => Some(*p),
                        redsim::entity::mob::ai::brain::Tracker::Point(v) => Some(BlockPos::new(v.x.floor() as i32, v.y.floor() as i32, v.z.floor() as i32)),
                        _ => None,
                    };
                    tp.map(|p| (p, wt.close, x.active == Act::Panic))
                });
                nb.stamp_prev = stamp;
                if x.active == Act::Panic {
                    nb.panic += 1;
                    nb.calm_run = 0;
                } else if x.active == Act::Rest {
                    nb.rest += 1;
                    nb.calm_run += 1;
                    nb.calm_run_max = nb.calm_run_max.max(nb.calm_run);
                    if let Some(a) = stamp
                        && x.mem.walk_target.is_none()
                    {
                        let slot = if after { &mut nb.lane_nowalk_max } else { &mut nb.stamp_nowalk_max };
                        *slot = Some(slot.map_or(a, |m| m.max(a)));
                    }
                    if let Some(a) = stamp {
                        nb.stamp_calm_max = Some(nb.stamp_calm_max.map_or(a, |m| m.max(a)));
                    }
                } else {
                    nb.calm_run = 0;
                }
                if nb.home_lost_at.is_none() && brain::brain(&run.world, ri).home != Some(bed) {
                    nb.home_lost_at = Some(t);
                    nb.home_lost_place = Some((vpos.x, vpos.y, vpos.z));
                }
                // (the drop: the napper falling through its hold; the station continues its window)
                if carts[wc].and_then(|id| st(&run.world, id)).is_some_and(|p| p.1 < 16.0 - 1.0 && p.2 < 1.0) {
                    win_ends.push((wc, format!("reached the drop uncleared at t {t}: the stamp {} (the station continues the window)", stamp.map_or("none".to_string(), |a| format!("{a} gt")))));
                    nb.drop_stamp = stamp;
                }
            }
        }
        for (wc, reason) in win_ends {
            if let Some(nb) = wins.remove(&wc) {
                agg.close(wc, nb, reason, None, t);
            }
        }
        // (the riders: the block at every rider's eye (air, string, hooks and rails are fine; anything
        // else is a wall or a side the eye is inside) and the lowest health seen)
        for id in carts.iter().flatten() {
            let Some(i) = redsim::entity::index_of(&run.world, *id) else { continue };
            for rid in run.world.entities.list[i].passengers.clone() {
                let Some(ri) = redsim::entity::index_of(&run.world, rid) else { continue };
                let e = &run.world.entities.list[ri];
                if let redsim::entity::EntityKind::Mob(mb) = &e.kind {
                    min_health = min_health.min(mb.health);
                }
                let (ex, ey, ez) = (e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z);
                let eye = BlockPos::new(ex.floor() as i32, ey.floor() as i32, ez.floor() as i32);
                let blk = run.world.get(eye);
                // (inside the block's collider: the eye point within one of its boxes)
                let f = |v: f64| (v - v.floor()) * 16.0;
                let inside = redsim::behavior::collision_boxes(&blk).iter().any(|bx| f(ex) > bx[0] as f64 && f(ex) < bx[1] as f64 && f(ey) > bx[2] as f64 && f(ey) < bx[3] as f64 && f(ez) > bx[4] as f64 && f(ez) < bx[5] as f64);
                if inside {
                    let name = format!("{blk:?}");
                    let name = name.split([' ', '{', '(']).next().unwrap_or("?").to_string();
                    *eye_bad.entry(format!("{name} at {},{},{}", eye.x, eye.y, eye.z)).or_default() += 1;
                }
            }
        }
        if let Ok(tr) = std::env::var("MCTAS_SIDING_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 2 && t >= p[0] && t < p[1] {
                let xs: Vec<String> = (0..n).filter_map(|c| now[c].map(|s| format!("{c}:{:.2},{:.2},{:.2}/{:.2}", s.0, s.1, s.2, s.3))).collect();
                eprintln!("t {t}: trap {} R {} S {} hold {} merge {} : {}", trap_is as u8, r_is as u8, s_is as u8, hh as u8, mh as u8, xs.join(" "));
            }
        }
        // (the merge's occupants: entry and clear)
        for c in 0..n {
            let on = now[c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz));
            let open = merge_occ.iter().position(|mo| mo.0 == c && mo.2.is_none());
            match (on, open) {
                (true, None) => {
                    merge_occ.push((c, t, None));
                    if let Some(pt) = pass_of[c] {
                        d_to_entry.push(t - pt);
                    }
                    // (the branch returns' merge-entry speed, stored: not the napper)
                    if nap.is_none_or(|(nc, _)| nc != c)
                        && let Some(p) = now[c]
                    {
                        merge_v.push(p.3);
                    }
                }
                (false, Some(i)) => {
                    merge_occ[i].2 = Some(t);
                    if let Some(pt) = pass_of[c].take() {
                        d_to_clear.push(t - pt);
                    }
                }
                _ => {}
            }
        }
        let on_merge = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))).count();
        if on_merge > 1 {
            touching += 1;
            if touch_note.len() < 4 {
                let who: Vec<String> = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))).map(|c| { let p = now[c].unwrap(); format!("cart {c} at {:.2},{:.2},{:.2}/{:.2}", p.0, p.1, p.2, p.3) }).collect();
                touch_note.push(format!("t {t}: {}", who.join(", ")));
            }
        }
        // (landings: a cart in the drop (in the column, over the top slopes, more than 0.3 above the
        // rail's surface) or on its landing tick, whose box meets another cart's box (0.98 wide, 0.7
        // high, 0.01 slack): a contact, its speed on all three axes, at q (the queued carts on the
        // slopes behind R_1's cart then))
        let q_now = (0..n).filter(|j| had_contact[*j] && entered.contains(j) && now[*j].is_some_and(|(xj, yj, zj, _)| (xj - zc).abs() < 0.6 && zj > 1.9 && zj < mz as f64 && yj < slope_y(zj) + 0.3)).count();
        for c in 0..n {
            let Some((x, y, z, _)) = now[c] else { continue };
            let in_column = (x - zc).abs() < 0.6 && z < mz as f64 && z > 1.9;
            let falling = in_column && y > slope_y(z) + 0.3;
            let landing_tick = in_column && !falling && was_falling[c];
            if (falling || landing_tick) && !met_this_drop[c] {
                let meet = (0..n).any(|j| j != c && now[j].is_some_and(|(xj, yj, zj, _)| (x - xj).abs() < 0.99 && (z - zj).abs() < 0.99 && y < yj + 0.71 && yj < y + 0.71));
                if meet {
                    met_this_drop[c] = true;
                    let sp = carts[c].and_then(|id| redsim::entity::index_of(&run.world, id)).map_or(0.0, |i| {
                        let d = run.world.entities.list[i].delta_movement;
                        (d.x * d.x + d.y * d.y + d.z * d.z).sqrt()
                    });
                    landing_meets.push((c, sp, q_now));
                }
            }
            if y > m as f64 - 0.2 && z > mz as f64 - 0.2 {
                met_this_drop[c] = false;
            }
            was_falling[c] = falling;
            // (the top slope's cell (the drop's landing cell, (x,17+N,1+N)): every stay of a cart's
            // centre there on the rails, its length and its ticks at rest, with q when it came in)
            let in_top = in_column && !falling && x.floor() as i32 == lx && z.floor() as i32 == 1 + ns;
            if in_top {
                let e = top_stay[c].get_or_insert((q_now, 0, 0, t));
                e.1 += 1;
                if prev[c].is_some_and(|p| p.3 < 0.002) && now[c].is_some_and(|p| p.3 < 0.002) {
                    e.2 += 1;
                }
            } else if let Some((q0, st_ticks, rest_ticks, t0)) = top_stay[c].take() {
                if st_ticks > 30 && std::env::var("MCTAS_TOPCELL_DEBUG").is_ok() {
                    let lane: Vec<String> = (0..n).filter_map(|j| now[j].filter(|p| (p.0 - zc).abs() < 0.6 && p.2 < mz as f64 + 0.5).map(|p| format!("{j}:{:.2}{}", p.2, if had_contact[j] { "q" } else { "" }))).collect();
                    eprintln!("top-cell stay: cart {c} from t {t0} for {st_ticks} gt (rest {rest_ticks}) at q {q0}; out of the lane: {}; the lane now: {}", (0..n).filter(|j| now[*j].is_none_or(|p| (p.0 - zc).abs() >= 0.6 || p.2 >= mz as f64 + 0.5)).count(), lane.join(" "));
                }
                // (stays that began before the first full cycle are the bench's start: 7 carts laid in
                // the lane and the 8th arriving; apart from normal operation)
                let e = if t0 < steady_from { top_start.entry(q0).or_insert((0, 0)) } else if t0 >= rel_end { top_end.entry(q0).or_insert((0, 0)) } else { top_by_q.entry(q0).or_insert((0, 0)) };
                e.0 = e.0.max(st_ticks);
                e.1 = e.1.max(rest_ticks);
            }
            // (the landing cell: a queued cart's box reaching the top slope's cell)
            if had_contact[c] && entered.contains(&c) && in_column && !falling && z + 0.49 > (1 + ns) as f64 {
                occupied_q.insert(q_now);
            }
        }
        for c in 0..n {
            let (Some((x, y, z, v)), Some((_, _, _, pv))) = (now[c], prev[c]) else { continue };
            // (the napper waiting: its creep while braked; the wrong way: south past the merge on the branch)
            if let Some((nc, _)) = nap
                && nc == c
            {
                if released_at.is_none() {
                    // (the creep, measured only once the napper rests on the siding)
                    match nap_rest {
                        None if v < 0.002 && y > m as f64 - 0.1 && (x - (sx as f64 + 0.5)).abs() < 0.3 => nap_rest = Some((x, z)),
                        Some((x0, z0)) => {
                            creep = creep.max(((x - x0).powi(2) + (z - z0).powi(2)).sqrt());
                            // (knocked loose: moving with no release after it had come to rest)
                            if v > 0.02 && !knocked_this {
                                knocked_this = true;
                                knocked += 1;
                            }
                        }
                        _ => {}
                    }
                } else if (x - zc).abs() < 0.6 && z > mz as f64 + 1.2 && y > m as f64 - 0.5 {
                    if wrong_way.len() < 4 {
                        wrong_way.push(format!("t {t}: the napper (cart {c}) at {x:.2},{y:.2},{z:.2} heading up the branch"));
                    }
                }
            }
            // (contacts in the lane: a stored speed dropping by 0.04 or more with a cart within 1.3 ahead
            // (toward -z) in the lane's column, the cart on the slopes' rails (not in the drop: a wall hit
            // there is no contact); q = the carts on the slopes behind R_1's cart at that moment, not
            // counting the arrival (the lane before the arrival: D + R_1's cart + q))
            let on_rails = |yy: f64, zz: f64| zz < mz as f64 && yy < slope_y(zz) + 0.3;
            if pv - v >= 0.04
                && (x - zc).abs() < 0.6
                && on_rails(y, z)
                && (0..n).any(|j| j != c && now[j].is_some_and(|(xj, _, zj, _)| (xj - zc).abs() < 0.6 && zj < z && z - zj < 1.3))
            {
                let q = (0..n).filter(|j| *j != c && now[*j].is_some_and(|(xj, yj, zj, _)| (xj - zc).abs() < 0.6 && zj > 1.9 && yj > 16.5 && on_rails(yj, zj))).count();
                // (an arrival's contact is its first since it entered the lane; later ones are the
                // restage's shoves inside the queue)
                let arrival = !had_contact[c];
                had_contact[c] = true;
                contacts.push((c, pv, z, q as f64, arrival));
                // (order: the cart right ahead must be the one that entered the lane just before)
                let ahead = (0..n).filter(|j| *j != c && now[*j].is_some_and(|(xj, _, zj, _)| (xj - zc).abs() < 0.6 && zj < z)).max_by(|a, b2| now[*a].unwrap().2.total_cmp(&now[*b2].unwrap().2));
                let expect = entered.iter().position(|e| *e == c).and_then(|i| i.checked_sub(1)).map(|i| entered[i]);
                if let (Some(a), Some(e)) = (ahead, expect)
                    && a != e
                    && order_note.len() < 4
                {
                    order_note.push(format!("t {t}: cart {c} landed behind cart {a}, entered after cart {e}"));
                }
                if let (Some(a), Some(e)) = (ahead, expect)
                    && a != e
                {
                    order_broken += 1;
                }
            }
            // (the lane's entry order: a cart's first tick on the slopes' rails past the merge)
            if (x - zc).abs() < 0.6 && on_rails(y, z) && z > 1.9 && !entered.contains(&c) {
                entered.push(c);
                // (the carts laid in the lane at the start are no arrivals)
                had_contact[c] = t < 5;
            }
            // (a cart resting in the drop above the top slope (never landed), or resting off the rails)
            if (x - zc).abs() < 0.6 && z > 1.9 && z < mz as f64 && v < 0.002 && (y - slope_y(z)).abs() > 0.15 {
                off_rest[c] += 1;
                if off_rest[c] == 20 {
                    off_rail += 1;
                    if off_note.len() < 4 {
                        off_note.push(format!("t {t}: cart {c} at rest off the rails at {x:.2},{y:.2},{z:.2} (the rail's surface {:.2})", slope_y(z)));
                    }
                }
            } else {
                off_rest[c] = 0;
            }
            // (through the floor)
            if y < 16.0 - 1.5 && z < 1.0 {
                entered.retain(|e| *e != c);
                if !fell[c] {
                    fell[c] = true;
                    if let Some(r) = per_release.last_mut() {
                        *r += 1;
                    }
                    drop_count += 1;
                    drops.push((t, c));
                    // (the napper: with --nap-ring, the cart of the release at ring position
                    // --nap-pick-u (one a ring); else every nap_every-th drop; none while one waits;
                    // its predecessor = the cart dropped one release before)
                    let rel_i = per_release.len() as i64 - 1;
                    let picked = if so.ring > 0 { rel_i >= 0 && rel_i.rem_euclid(so.ring) == so.pick_u.rem_euclid(so.ring) } else { so.nap_every > 0 && drop_count % so.nap_every == 0 };
                    let is_nap = picked && nap.is_none() && drops.len() >= 2;
                    if is_nap {
                        nap_pick_i = rel_i;
                        let pred = drops[drops.len() - 2].1;
                        nap = Some((c, pred));
                        pred_pass = None;
                        released_at = None;
                        nap_rest = None;
                        creep = 0.0;
                        nap_since = t;
                        lap_wait = false;
                        nap_rested_at = None;
                        knocked_this = false;
                        release_lead = None;
                        release_window = None;
                        latch_at = None;
                        released_by_latch = false;
                    }
                    pending.push((t + if is_nap { so.nap_ret } else { o.ret }, c, is_nap));
                }
                if y < 16.0 - 4.0
                    && let Some(id) = carts[c].take()
                {
                    kill_with_rider(&mut run.world, id);
                    fell[c] = false;
                }
            }
            // (the napper's launch from the rest (ironspec 09:01, R on slime: slime's stepOn scales a
            // grounded cart's horizontal motion by 0.4 when |vy| < 0.1): the first tick after its
            // release its centre is off the rest's cell, its speed then; not off 40 gt after its
            // release: a stuck launch)
            if let (Some((nc, _)), Some(ra)) = (nap, released_at)
                && nc == c
                && launch_for != Some(ra)
                && t > ra
            {
                let in_rest = x.floor() as i32 == sx && z.floor() as i32 == rest_z;
                if !in_rest {
                    // (R's own powered flag at the departure tick: a launch after R's power ended)
                    let r_on = run.world.get(wp(BlockPos::new(sx, m, rest_z))).to_string().contains("powered=true");
                    launches.push((t - ra, v, r_on));
                    launch_for = Some(ra);
                } else if t > ra + 40 {
                    launch_stuck += 1;
                    launch_for = Some(ra);
                }
            }
            // (the napper at the lane's back: past the merge, at rest on the intake side; its row closes)
            if let (Some((nc, pred)), Some(ra)) = (nap, released_at)
                && nc == c
                && z < mz as f64
                && v < 0.002
                && t > ra + 5
                && (x - zc).abs() < 0.6
            {
                // (the leader it followed through the merge: its predecessor, or under option (ii) the
                // cart whose pass released it; late: with --nap-ring, released at a window later than
                // its first one (pick + win_du); without it, behind a cart other than its predecessor)
                let lead = release_lead.unwrap_or(pred);
                let first_window = nap_pick_i + so.win_du;
                let late_rejoin = if so.ring > 0 { released_by_latch || release_window.is_some_and(|wj| wj as i64 > first_window) } else { so.any_pass && lead != pred };
                let m_pred = merge_occ.iter().rev().find(|mo| mo.0 == lead && mo.1 <= ra).and_then(|mo| mo.2);
                let m_nap = merge_occ.iter().rev().find(|mo| mo.0 == c).map(|mo| mo.1);
                let curve_rule = match (m_pred, m_nap) { (Some(cl), Some(en)) => format!("{}", en - cl), _ => "-".to_string() };
                let c_nap = contacts.iter().filter(|k| k.0 == c && k.4).map(|k| k.1).fold(0.0f64, f64::max);
                let q_nap = contacts.iter().rev().find(|k| k.0 == c && k.4).map_or("-".to_string(), |k| format!("{}", k.3 as usize));
                // (its wait on the siding: from coming to rest there to its release (day-1's strand
                // margin and the siding's double occupancy))
                let waited = nap_rested_at.map(|r| ra - r);
                if let Some(w) = waited {
                    nap_wait_max = nap_wait_max.max(w);
                }
                nap_rows.push(format!(
                    "napper cart {c} (predecessor {pred}){}: the predecessor over the merge string t {}, at rest on the siding t {}, released t {ra} (waited {} gt on the siding), at the lane's back t {t} (z {z:.2}); the merge's curve rule (its entry minus the predecessor's clear) {curve_rule} gt; its contact {c_nap:.3} at q {q_nap}; braked creep {creep:.4}",
                    if lap_wait {
                        " (late: its successor passed first, it waited a lane-lap)".to_string()
                    } else if late_rejoin && released_by_latch {
                        format!(" (late: missed its window (release {first_window}); its lane's late latch released it K after the first pass it waited for, behind cart {lead})")
                    } else if late_rejoin && so.ring > 0 {
                        format!(" (late: missed its first window (release {first_window}), rejoined at the window of release {} behind cart {lead})", release_window.unwrap_or(0))
                    } else if late_rejoin {
                        format!(" (late: rejoined behind cart {lead}, whose pass found it waiting)")
                    } else {
                        String::new()
                    },
                    pred_pass.map_or("-".to_string(), |p| p.to_string()),
                    nap_rested_at.map_or("-".to_string(), |r| r.to_string()),
                    waited.map_or("-".to_string(), |w| w.to_string())
                ));
                if lap_wait || late_rejoin {
                    late_naps += 1;
                }
                nap = None;
            }
        }
        before_prev = prev;
        prev = now;
    }
    let mut ph: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &per_release {
        *ph.entry(*r).or_default() += 1;
    }
    let arr: Vec<&(usize, f64, f64, f64, bool)> = contacts.iter().filter(|k| k.4).collect();
    let cmax = arr.iter().map(|k| k.1).fold(0.0f64, f64::max);
    let imax = contacts.iter().filter(|k| !k.4).map(|k| k.1).fold(0.0f64, f64::max);
    let min_behind = arr.iter().map(|k| k.3 as usize).min();
    let worst_at_min = min_behind.map(|mb| arr.iter().filter(|k| k.3 as usize == mb).map(|k| k.1).fold(0.0f64, f64::max));
    let mut by_count: std::collections::BTreeMap<usize, f64> = Default::default();
    for k in &arr {
        let e = by_count.entry(k.3 as usize).or_insert(0.0);
        *e = e.max(k.1);
    }
    let empty_releases = per_release.iter().skip(1).filter(|r| **r == 0).count();
    let mut lines = vec![format!(
        "siding bench: lane x {lx} in the budget's cells (the hold (x,16,0), R_1 (x,17,1), {ns} slopes, the merge (x,{m},{mz}) with its string, the siding (x-1,{m},{}) and its curve{}), {} carts ({}), the R line {} gt, the trapdoor {} gt every {} gt (corner {:+}), returns on the branch after {} gt at {:.2} b/t, a napper every {} drops back on the siding after {} gt, released {} gt after its predecessor's merge pass for {} gt; {} cycles{}",
        mz + 1,
        if so.drop_entry { format!("; the drop entry: the merge one level up, its north end over the drop (x,{m}..{},{}) onto the top slope, the wall (x,{m},{}) — one level and one wall block a lane", m - 1, mz - 1, mz - 2) } else { String::new() },
        o.per_lane, if o.riders { "ridden" } else { "empty" }, o.r_pulse, o.drop_w, o.period, o.corner, o.ret, o.v_in, so.nap_every, so.nap_ret, so.k, so.nap_pulse, o.cycles,
        if o.login.is_empty() && o.freeze32.is_empty() { String::new() } else { format!(", logins {:?} {:?}", o.login, o.freeze32) }
    )];
    lines.push(format!("carts per release (exactly 1): {}; empty releases after the first (golems lost: a batch of 2 can't summon): {empty_releases}; late nappers (a lane-lap wait): {late_naps}", ph.iter().map(|(a, b2)| format!("{a}: {b2}")).collect::<Vec<_>>().join(", ")));
    lines.push(format!("nappers through the siding: {}", if nap_rows.is_empty() { "none".to_string() } else { nap_rows.join(" | ") }));
    lines.push(format!(
        "the longest napper wait on the siding (at rest there to its release): {nap_wait_max} gt [{}]",
        if so.ring > 0 {
            format!("day-1's ring: {} positions of {} gt, the napper picked at position {}, its window {} positions later, one beat from that release + {} gt", so.ring, o.period, so.pick_u, so.win_du, so.win_tap)
        } else if so.any_pass {
            "the any-pass bench: every pass of the lane samples, not (ii)'s window; a late napper waits up to a ring period more".to_string()
        } else {
            "the predecessor's pass".to_string()
        }
    ));
    // (the branch returns' merge-entry speed and the nappers' rest z)
    let mm = |v: &[f64]| if v.is_empty() { "none".to_string() } else { format!("{:.3}..{:.3}", v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max)) };
    lines.push(format!("the branch returns' stored speed entering the merge cell (x,{m},{mz}): {} over {} entries", mm(&merge_v), merge_v.len()));
    lines.push(format!(
        "lane carts on the siding's cells (a lane cart's centre over F, R, c1..c3 or the old siding's cells; the bar: 0): {lane_on_siding} ticks; the napper's feed -> rest transit {}; the pass edge -> the same cart's merge-cell entry {} gt, -> its clear {} gt ({})",
        if transits.is_empty() { "none".to_string() } else { format!("{}..{} gt", transits.iter().min().unwrap_or(&0), transits.iter().max().unwrap_or(&0)) },
        if d_to_entry.is_empty() { "none".to_string() } else { format!("{}..{}", d_to_entry.iter().min().unwrap_or(&0), d_to_entry.iter().max().unwrap_or(&0)) },
        if d_to_clear.is_empty() { "none".to_string() } else { format!("{}..{}", d_to_clear.iter().min().unwrap_or(&0), d_to_clear.iter().max().unwrap_or(&0)) },
        if so.siding_north { "the pass is the guard detector's rising edge" } else { "the pass is the string's edge" }
    ));
    // (the queue's rest positions at the releases, by occupancy: per slot (H first, by z), the
    // ranges of x, y and z over the releases with every lane cart at rest)
    let queue_txt: Vec<String> = queue_rest
        .iter()
        .map(|(k, snaps)| {
            let slots: Vec<String> = (0..*k)
                .map(|i| {
                    let r = |f: &dyn Fn(&(f64, f64, f64)) -> f64| {
                        let v: Vec<f64> = snaps.iter().map(|s| f(&s[i])).collect();
                        let (lo, hi) = (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max));
                        if hi - lo < 0.005 { format!("{lo:.3}") } else { format!("{lo:.3}..{hi:.3}") }
                    };
                    format!("{}: ({}, {}, {})", i + 1, r(&|p| p.0), r(&|p| p.1), r(&|p| p.2))
                })
                .collect();
            format!("{k} carts ({} releases): {}", snaps.len(), slots.join(" "))
        })
        .collect();
    lines.push(format!("the queue at rest at the releases (lane x {lx}; carts' positions, slot 1 the hold H, by z; x, y, z ranges): {}", if queue_txt.is_empty() { "none".to_string() } else { queue_txt.join(" | ") }));
    lines.push(format!("the lane's occupancy at the releases from the steady state (t {steady_from}): the largest {occ_max}, releases at 8 (the 8th slot is the top cell; the bar: 0): {occ8}; before it (the start's full lane) up to {occ_start_max}"));
    // (the first three releases' queues against the steady slots (H (16.188, 0.490), then 0.79 apart
    // from (17.343, 1.280)): each cart's offset from its slot, the carts moving at the release tick)
    let slot = |i: usize| if i == 0 { (16.1875, 0.49) } else { let z = 1.28 + 0.79 * (i - 1) as f64; (slope_y(z), z) };
    let snaps_txt: Vec<String> = first_snaps
        .iter()
        .enumerate()
        .map(|(k, (t, s))| {
            let dev = s.iter().enumerate().map(|(i, p)| { let (sy, sz) = slot(i); ((p.1 - sy).abs()).max((p.2 - sz).abs()) }).fold(0.0f64, f64::max);
            let moving = s.iter().filter(|p| p.3 >= 0.002).count();
            format!("release {} (t {t}): {} carts, the largest offset from its slot {dev:.3}, moving {moving}", k + 1, s.len())
        })
        .collect();
    lines.push(format!("the start-up (the first releases' queues against the steady slots; {}): {}", if so.start_steady { "--start-steady: 7 riders in their slots, the 8th in the pocket" } else { "the bench's full-lane start" }, if snaps_txt.is_empty() { "none".to_string() } else { snaps_txt.join("; ") }));
    lines.push(format!(
        "lane carts in push contact with the waiting napper (boxes 0.98 x 0.7, one inflated 0.2 in x and z, y ranges overlapping; the bar: 0): {lane_push_ticks} ticks; the westmost centre of a branch cart (not the napper) whose box is in the merge cell: x{} (a cart resting on the siding flat (x-1,{m},{mz}) is in push contact with it wherever its centre is east of that minus 1.18)",
        if merge_west == f64::MAX { " none".to_string() } else { format!("{:+.3}", merge_west) }
    ));
    lines.push(format!(
        "the nappers' launches from the rest (its centre off the rest's cell after the release; the rest's support {}): {} launches, departure {} gt after the release, speed then {}; stuck (still on the rest 40 gt after its release; the bar: 0): {launch_stuck}; the launch threshold (the latest departure, counted from the switch; R powers {} gt after it) {} gt against the pulse {} gt; launches after the pulse ended (R unpowered at the departure tick; the bar: 0): {}",
        if so.siding_north { so.r_support.as_deref().unwrap_or("minecraft:stone") } else { "-" },
        launches.len(),
        if launches.is_empty() { "-".to_string() } else { format!("{}..{}", launches.iter().map(|l| l.0).min().unwrap_or(0), launches.iter().map(|l| l.0).max().unwrap_or(0)) },
        if launches.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", launches.iter().map(|l| l.1).fold(f64::MAX, f64::min), launches.iter().map(|l| l.1).fold(0.0f64, f64::max)) },
        drive_delay,
        launches.iter().map(|l| l.0).max().map_or("-".to_string(), |d| d.to_string()),
        so.nap_pulse,
        launches.iter().filter(|l| !l.2).count()
    ));
    lines.push(format!(
        "nappers came to rest on the siding at z {} over {} nappers ({}; a napper resting above z {:.2} reaches above the split row's cart floor {:.4}; the static worst z gap to a through-cart projected onto K's north edge (z {:.1}, moveAlongTrack's entry at x fraction 0), from the highest rest: {})",
        mm(&rest_zs),
        rest_zs.len(),
        match so.nap_feed_v {
            Some(v) => format!("arriving from the feed cell (x-2,{},{}) at stored {v}", m + 1, mz + 2),
            None => "spawned at rest at the rest's centre".to_string(),
        },
        rest_z as f64 + ((m + 1) as f64 + 0.0625 - (m as f64 + 0.0625) - 0.7),
        (m + 1) as f64 + 0.0625,
        (mz + 2) as f64,
        rest_zs.iter().copied().fold(None, |a: Option<f64>, z| Some(a.map_or(z, |m| m.max(z)))).map_or("none".to_string(), |zh| format!("{:.3}", ((mz + 2) as f64 - 0.49) - (zh + 0.49 + 0.2)))
    ));
    if let Some(v) = so.k_row {
        lines.push(format!(
            "the split row's through-carts past K (x-2,{},{}) held north_east (--k-row stored {v}, {:?} gt after each release): spawned {th_spawned}, passed {th_exited}, slowed below 0.95 of it {th_slowed} (the slowest exit {}), lost {th_lost}, the largest z offset {th_max_dz:.3}; ticks a through-cart was in push range of the waiting napper (the bar: 0): {th_push_ticks}, its box overlapping the napper's {th_overlap_ticks}; the least z gap between a through-cart's box and the waiting napper's push box (the napper's inflated 0.2; positive is clear): {}",
            m + 1,
            mz + 2,
            so.k_offsets,
            if th_min_exit == f64::MAX { "none".to_string() } else { format!("{th_min_exit:.3}") },
            if th_zgap == f64::MAX { "none".to_string() } else { format!("{th_zgap:.3}") }
        ));
    }
    if so.flat_detector {
        lines.push(format!(
            "the siding flat's detector {} (the siding sensor): ticks a cart other than the napper met its detection box (the bar: 0): {flat_false}; napper passes {flat_nap_passes}",
            if so.siding_north { format!("(the feed F (x-2,{},{}))", m + 1, mz + 1) } else { format!("(x-1,{m},{mz})") }
        ));
    }
    if so.ai {
        // (a window still open at the run's end closes there)
        let open: Vec<usize> = wins.keys().copied().collect();
        for wc in open {
            if let Some(nb) = wins.remove(&wc) {
                agg.close(wc, nb, "still open at the run's end".to_string(), None, ticks);
                agg.open_end += 1;
            }
        }
        if agg.total > 0 && agg.tracked == 0 {
            return Err(format!("--nap-ai: {} nappers, none tracked (their riders run no exact brain)", agg.total));
        }
        let fmt = |v: i64| if v < 0 { "none".to_string() } else { format!("{v} gt") };
        lines.push(format!(
            "the nappers' windows (from the arrival on the siding, the stamp {}, to the first clear after the release or the drop where the station continues it; one window per napper cart; the riders' AI on at night, HOME the bed {:?} with its ticket taken{}): tracked {} of {} nappers ({} normal, {} late); the oldest stamp at a calm REST tick with no walk target held: on the siding {}, after the release {} (above 1,000 gt, the bar: {} nappers); windows ended by a clear after the release {}, reaching the drop uncleared {} (the stamp there up to {}); windows cut short (the bar: 0): {}, still open at the run's end {}; nappers that lost HOME (the bar: 0): {}{}",
            so.stamp_at_arrival.map_or("armed by the napper's first failed path on the siding".to_string(), |a| format!("pre-aged {a} gt (the leg since its last clear at the bed)")),
            BED,
            if so.posts.is_empty() { "; no scarer".to_string() } else { format!("; scarer posts {:?}", so.posts) },
            agg.tracked,
            agg.total,
            agg.normal,
            agg.late,
            fmt(agg.nowalk_siding_max),
            fmt(agg.nowalk_lane_max),
            agg.over_1000,
            agg.cleared,
            agg.dropped,
            fmt(agg.drop_stamp_max),
            agg.cut,
            agg.open_end,
            agg.home_lost,
            if agg.rows.is_empty() { String::new() } else { format!(" ({})", agg.rows.join(" | ")) }
        ));
    }
    lines.push(format!(
        "the siding rest powered with its declared switch off (a stray power; the bar: 0): {stray_power} ticks, {stray_with_guard} of them with the guard detector powered; ticks the guard was powered while a napper waited: {guard_on_waiting}; the side power {}",
        match so.side_ctl.as_deref() {
            Some("dust") => "the positive control: dust in the side cell, the repeater behind it".to_string(),
            Some("dust-line") => "the positive control: dust in the side cell, its only connection the detector".to_string(),
            Some("repeater-east") => "the positive control: a repeater in the side cell taking its input from the detector's side".to_string(),
            _ if so.siding_north => "a real conductor (stone) under the rest, driven through its south face by a repeater (--siding-north)".to_string(),
            _ if so.side_real => "a real conductor (stone), driven through its south face by a repeater".to_string(),
            _ => "emulated (a redstone block set and cleared)".to_string(),
        }
    ));
    lines.push(format!("the napper heading the wrong way at the merge (the bar: 0): {}{}", wrong_way.len(), if wrong_way.is_empty() { String::new() } else { format!(" ({})", wrong_way.join("; ")) }));
    lines.push(format!(
        "nappers knocked loose on the siding (moving with no release; the bar: 0): {knocked}; rails popped at placement (BaseRailBlock.shouldBeRemoved: no rigid block below, or on an ascending rail's high side; the bar: 0): {}{}; rails the bench restored to their laid shape after placement: {reshaped}{} (the build lays the merge before the branch flat, and the branch flat before a dx 1 siding's slope)",
        popped.len(),
        if popped.is_empty() { String::new() } else { format!(" ({})", popped.join(", ")) },
        if reshaped_note.is_empty() { String::new() } else { format!(" ({})", reshaped_note.join(", ")) }
    ));
    lines.push(format!("ticks with two carts over the merge (the curve rule's touching; the bar: 0): {touching}{}", if touch_note.is_empty() { String::new() } else { format!(" ({})", touch_note.join("; ")) }));
    lines.push(format!(
        "arrivals at the lanes' backs (each arrival's first contact in the lane): {}, {} above 0.46, the hardest {cmax:.3}; the fewest carts queued on the slopes behind R_1's cart at an arrival (q: the lane before it is D + R_1's cart + q) {} (the hardest there {}); the hardest by q: {}; the restage's shoves inside the queue (R's own, not arrivals) up to {imax:.3}",
        arr.len(),
        arr.iter().filter(|k| k.1 > 0.46).count(),
        min_behind.map_or("-".to_string(), |v| v.to_string()),
        worst_at_min.map_or("-".to_string(), |v| format!("{v:.3}")),
        by_count.iter().map(|(k, v)| format!("{k}: {v:.3}")).collect::<Vec<_>>().join(", ")
    ));
    lines.push(format!("arrivals landing out of order (the cart ahead is not the one that entered before; the bar: 0): {order_broken}{}", if order_note.is_empty() { String::new() } else { format!(" ({})", order_note.join("; ")) }));
    // (stays still open at the run's end count too)
    for (q0, st_ticks, rest_ticks, t0) in top_stay.iter().flatten() {
        let e = if *t0 < steady_from { top_start.entry(*q0).or_insert((0, 0)) } else if *t0 >= rel_end { top_end.entry(*q0).or_insert((0, 0)) } else { top_by_q.entry(*q0).or_insert((0, 0)) };
        e.0 = e.0.max(*st_ticks);
        e.1 = e.1.max(*rest_ticks);
    }
    let fmt_top = |m: &std::collections::BTreeMap<usize, (i64, i64)>| if m.is_empty() { "never".to_string() } else { m.iter().map(|(q, (s, r))| format!("q {q}: the longest stay {s} gt, at rest {r} gt")).collect::<Vec<_>>().join("; ") };
    lines.push(format!(
        "the top slope's cell (x,{},{}) (the landing cell), a cart's centre on the rails there, by q when it came in: {} [the bench's start (7 carts laid in the lane, the 8th arriving) until the lane's steady state, t < {steady_from}: {}] [the bench's end, after the last release (t >= {rel_end}: the lane refills with no drops): {}]",
        17 + ns,
        1 + ns,
        fmt_top(&top_by_q),
        fmt_top(&top_start),
        fmt_top(&top_end)
    ));
    lines.push(format!(
        "landings meeting another cart's box (during the fall or at the landing tick; lift3's box test, its speed on all three axes against 0.46): {}, {} above 0.46{}; the landing cell (x,{},{}) held a queued cart at q: {}",
        landing_meets.len(),
        landing_meets.iter().filter(|k| k.1 > 0.46).count(),
        if landing_meets.is_empty() { String::new() } else { format!(" ({})", landing_meets.iter().take(8).map(|k| format!("cart {} {:.3} at q {}", k.0, k.1, k.2)).collect::<Vec<_>>().join(", ")) },
        17 + ns,
        1 + ns,
        if occupied_q.is_empty() { "none".to_string() } else { occupied_q.iter().map(|q| q.to_string()).collect::<Vec<_>>().join(", ") }
    ));
    lines.push(format!("carts at rest off the rails (in the drop or beside a rail, 20 gt; the bar: 0): {off_rail}{}", if off_note.is_empty() { String::new() } else { format!(" ({})", off_note.join("; ")) }));
    lines.push(format!(
        "riders' eyes inside a block's collider (the sight test; rider-ticks; the bar: 0): {}{}; the riders' lowest health {} (covers the game's suffocation, isInWall: full suffocating blocks only)",
        eye_bad.values().sum::<usize>(),
        if eye_bad.is_empty() { String::new() } else { format!(" ({})", eye_bad.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")) },
        if min_health == f32::MAX { "-".to_string() } else { format!("{min_health:.1}") }
    ));
    lines.push(format!(
        "the lane's top cell V (x,{},{}) from the steady state to the last release (t {steady_from}..{rel_end}): carts holding V 20 gt (V held as a queue slot: slot 8; the bar: 0): {v_holds}, the first at the lane's occupancy {}; a first stop there at all (a landing's momentary stop included) at occupancy {}; V held before the steady state (the start's transient) {v_holds_start} times and after the last release (the lane refilling with no release, the bench's end) {v_holds_end} times, neither counted; carts resting on another cart's top (the overflow; the bar: 0): {stacked_ticks} cart-ticks{}",
        m - 2,
        mz - 1,
        v_hold_occ.map_or("never".to_string(), |o| o.to_string()),
        v_rest_occ.map_or("never".to_string(), |o| o.to_string()),
        stacked_note.as_ref().map_or(String::new(), |n| format!(" (the first: {n})"))
    ));
    lines.push(format!(
        "the restage (a release to the hold's string rising again): the slowest {} gt of the {}-gt beat over {} cycles",
        restage.iter().max().map_or("-".to_string(), |v| v.to_string()),
        o.period,
        restage.len()
    ));
    if let Some((c, _)) = nap {
        // (a napper still waiting at the end is stuck only past a lane-lap and two beats since it came
        // back (day-1's late rule waits up to one lane-lap); else it is a wait the run's end cut)
        let waited = ticks - nap_since;
        let lap = if so.ring > 0 { (so.ring + 1) * o.period + so.k } else { (o.per_lane as i64 + 2) * o.period };
        lines.push(format!(
            "a napper still waiting at the end: cart {c} (predecessor pass {:?}, released {:?}), {waited} gt since its drop{}",
            pred_pass,
            released_at,
            if waited > lap + so.nap_ret { format!(" — stuck (over a lane-lap, {lap} gt, past its return)") } else { format!(" — within a lane-lap ({lap} gt) of its return: the run's end cut its wait") }
        ));
    }
    Ok(lines.join("\n"))
}

/// The three lanes (x -1 / 3 / 7) as a design file in the loop frame, the cells the bench lays with
/// `--merge-curve --siding-dx 2 --guard-det --drop-entry` (onechunk's F1 + Q2 module; farmopt
/// 2026-11-12 01:30): R_1 and the hold string, the N slopes on glass, the drop entry (the merge one
/// level up, the empty drop column, the wall, the pass string), the 2-cell branch with its detector
/// guard, the x-2 siding. The switched cells are left air and named in the header.
pub fn lanes_design(ns: i32, sbend: bool, row: bool, r_support: Option<&str>, rest_range: Option<&str>) -> Result<String, String> {
    let m = 19 + ns;
    let mz = ns + 2;
    // (--r-support: the block under the S-bend's rest R (ironspec 08:46, the row partials): stone, or a
    // slime block (a conductor that is not solid-render: R's cell is no stable destination))
    let support: &'static str = match r_support {
        None | Some("minecraft:stone") | Some("stone") => "minecraft:stone",
        Some("minecraft:slime_block") | Some("slime_block") => "minecraft:slime_block",
        Some(other) => return Err(format!("--r-support {other}: stone or slime_block")),
    };
    let tripwire_ew = "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]";
    let hook_e = "minecraft:tripwire_hook[attached=true,facing=east,powered=false]";
    let hook_w = "minecraft:tripwire_hook[attached=true,facing=west,powered=false]";
    let glass = "minecraft:glass";
    let stone = "minecraft:stone";
    let pslope = "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]";
    let slope = "minecraft:rail[shape=ascending_south,waterlogged=false]";
    let flat_ns = "minecraft:rail[shape=north_south,waterlogged=false]";
    let flat_ew = "minecraft:rail[shape=east_west,waterlogged=false]";
    let curve_nw = "minecraft:rail[shape=north_west,waterlogged=false]";
    let curve_se = "minecraft:rail[shape=south_east,waterlogged=false]";
    let curve_sw = "minecraft:rail[shape=south_west,waterlogged=false]";
    let curve_ne = "minecraft:rail[shape=north_east,waterlogged=false]";
    let repeater_n = "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]";
    let repeater_w = "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]";
    let dust_ne = "minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=none]";
    let dust_ns = "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]";
    let slime = "minecraft:slime_block";
    let det = "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]";
    let mut cells: std::collections::BTreeMap<(i32, i32, i32), &str> = Default::default();
    for x in [-1, 3, 7] {
        let mut put = |x: i32, y: i32, z: i32, s: &'static str| {
            cells.insert((x, y, z), s);
        };
        // (the hold's string row (x-2..x+2,17,0) and R_1 (x,17,1) are the station's cells (dfb89499's
        // file owns them; the bench lays its own copy), not laid here)
        let _ = (tripwire_ew, hook_e, hook_w, stone, pslope);
        for k in 0..ns {
            put(x, 18 + k, 2 + k, slope);
            put(x, 17 + k, 2 + k, glass);
        }
        // (the drop entry)
        put(x, m, mz, curve_nw);
        put(x, m - 1, mz, glass);
        put(x, m, mz - 2, glass);
        // (the top slope's high-side support: an ascending rail needs a rigid block there)
        put(x, 17 + ns, 2 + ns, glass);
        if sbend {
            // (the S-bend (ironspec 06:24, option (a)): the feed F off the split's K (x-2,m+1,mz+2) (the
            // split's cell, with its glass (x-2,m,mz+2) and the branch's detector slope (x,m,mz+1)),
            // the rest R powered from below (stone, a north-facing repeater on glass, its input the
            // switched cell (x-2,m-1,mz+2)), c1, c2, c3 into the merge; no pass string)
            put(x - 2, m + 1, mz + 1, flat_ns);
            put(x - 2, m, mz + 1, glass);
            put(x - 2, m, mz, pslope);
            put(x - 2, m - 1, mz, support);
            put(x - 2, m - 1, mz + 1, repeater_n);
            put(x - 2, m - 2, mz + 1, glass);
            // (the feed path, the budget box (x-2..x-1, m-2..m-1, mz+2) (ironspec 09:53): dust at R's
            // input (x-2,m-1,mz+2) turning from the east into the repeater's back, the feed repeater
            // (x-1,m-1,mz+2) facing west into it, both on glass; the switched cell is the feed's input
            // (x,m-1,mz+2), day-1's controller line. Lanes -1 and 3 take day-1's feed from below
            // (ironspec 11:22, option (a), keeping the seat candidates' columns clear): R's input dust
            // [north, south] on a slime block (x-2,m-2,mz+2), strongly powered by day-1's cells; no
            // feed repeater)
            if x == -1 || x == 3 {
                put(x - 2, m - 1, mz + 2, dust_ns);
                put(x - 2, m - 2, mz + 2, slime);
            } else {
                put(x - 2, m - 1, mz + 2, dust_ne);
                put(x - 2, m - 2, mz + 2, glass);
                put(x - 1, m - 1, mz + 2, repeater_w);
                put(x - 1, m - 2, mz + 2, glass);
            }
            put(x - 2, m, mz - 1, curve_se);
            put(x - 2, m - 1, mz - 1, glass);
            put(x - 1, m, mz - 1, curve_sw);
            put(x - 1, m - 1, mz - 1, glass);
            put(x - 1, m, mz, curve_ne);
            put(x - 1, m - 1, mz, glass);
            continue;
        }
        put(x, m, mz - 1, tripwire_ew);
        put(x - 1, m, mz - 1, hook_e);
        put(x + 1, m, mz - 1, hook_w);
        put(x - 2, m, mz - 1, glass);
        put(x + 2, m, mz - 1, glass);
        // (the 2-cell branch: the detector guard first, (x,m,mz+1), shared with the split's latch
        // (7b90c3f1), two cells off the split's junction; then the flat (x,m,mz+2))
        put(x, m, mz + 1, det);
        put(x, m - 1, mz + 1, glass);
        put(x, m, mz + 2, flat_ns);
        put(x, m - 1, mz + 2, glass);
        // (the siding at x-2)
        put(x - 2, m, mz + 1, pslope);
        put(x - 2, m - 1, mz + 1, glass);
        put(x - 2, m, mz, curve_se);
        put(x - 2, m - 1, mz, glass);
        put(x - 1, m, mz, flat_ew);
        put(x - 1, m - 1, mz, glass);
        put(x - 2, m + 1, mz + 2, flat_ns);
        put(x - 2, m, mz + 2, glass);
    }
    // (the placement order (ironspec's rule): each rail's natural shape when placed last among its
    // neighbours (vanilla's RailState, simplified: the neighbouring rails it can connect to: the same
    // level, one up (it ascends toward that side), or one down when that rail ascends toward it), against
    // the designed shape; a differing cell needs a temporary helper rail while it is placed)
    // (--emit-with-row: a scan file, not the lanes file: the split's row along z mz+2 at m+1 from x -4
    // to 8 on glass, K (x-2) and J (x) held north_east (unpowered), each lane's detector slope D
    // (x,m,mz+1) on glass, for the push-scan and the T scan against the S-bend)
    if row {
        for xx in -4..=8 {
            let k_or_j = [-3, 1, 5, -1, 3, 7].contains(&xx);
            cells.insert((xx, m + 1, mz + 2), if k_or_j { "minecraft:rail[shape=north_east,waterlogged=false]" } else { "minecraft:rail[shape=east_west,waterlogged=false]" });
            cells.insert((xx, m, mz + 2), "minecraft:glass");
        }
        for x in [-1, 3, 7] {
            cells.insert((x, m, mz + 1), "minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]");
            cells.insert((x, m - 1, mz + 1), "minecraft:glass");
        }
    }
    let natural_mismatch = natural_shapes(&cells);
    let (x0, x1) = (cells.keys().map(|k| k.0).min().unwrap_or(0), cells.keys().map(|k| k.0).max().unwrap_or(0));
    let (y0, y1) = (cells.keys().map(|k| k.1).min().unwrap_or(0), cells.keys().map(|k| k.1).max().unwrap_or(0));
    let (z0, z1) = (cells.keys().map(|k| k.2).min().unwrap_or(0), cells.keys().map(|k| k.2).max().unwrap_or(0));
    let mut states: Vec<&str> = cells.values().copied().collect();
    states.sort();
    states.dedup();
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let ch = |s: &str| chars.as_bytes()[states.iter().position(|t| *t == s).unwrap_or(0)] as char;
    let mut out = String::new();
    if sbend {
        out.push_str(&format!(
            "# F1 + Q2 lanes, IN-CHUNK SPLIT with the S-bend siding (ironfarm; ironspec 2026-11-12 06:24 option (a), provisional pick; bench, tool unreviewed): three lanes x -1 / 3 / 7, loop frame (paste with no move), {ns} slopes behind R_1, the merge at m = {m}, mz = {mz}. Benched as `mctas mobfarm escape-bench --siding --merge-curve --siding-dx 2 --guard-det --drop-entry --slopes {ns} --r-pulse 30 --drop-w 20 --per-lane 8 --siding-north --k-row 0.30 --k-offsets 0,12,24 --nap-feed-v 0.30` (explore/ironfarm-go): one lane, these cells at x 3, the split's K and row laid by the bench for the through-carts.\n"
        ));
        out.push_str("# Per lane x: the hold string (x-2..x+2,17,0) and R_1 (x,17,1): the station's file (dfb89499). The split's file (7b90c3f1) owns the row along z 9 at y 25, K (x-2,25,9) (a power-forced T: north_east through, north_west into the siding; driver from day-1), J (x,25,9), their glass (x-2,24,9) and (x,24,9), and the branch's detector slope D (x,24,8) (the guard and the pass). Not laid here.");
        out.push_str(&format!(" Here: the slopes (x,18..{},2..{}) plain, ascending south, on glass, the top one's high side (x,{},{}) glass; the merge C (x,{m},{mz}) a north_west curve on glass (x,{},{mz}), its north end the rail end over the drop column (x,{m}..{},{}) (empty) onto the top slope, the wall (x,{m},{}) glass (golem-proof);", 17 + ns, 1 + ns, 17 + ns, 2 + ns, m - 1, m - 1, mz - 1, mz - 2));
        out.push_str(&format!(" the siding (the S-bend): the feed F (x-2,{},{}) north_south on glass (x-2,{m},{}), the rest R (x-2,{m},{mz}) a powered rail ascending south (braked unpowered) on {} (x-2,{},{mz}) (its switched power: strongly powered by the repeater (x-2,{},{}) facing north, on glass (x-2,{},{})), c1 (x-2,{m},{}) south_east, c2 (x-1,{m},{}) south_west, c3 (x-1,{m},{mz}) north_east into C, the curves on glass. No pass string: the pass is D's rising edge (D -> the same cart's merge-cell entry 2 gt, clear 9 gt).{}\n", m + 1, mz + 1, mz + 1, support.trim_start_matches("minecraft:"), m - 1, m - 1, mz + 1, m - 2, mz + 1, mz - 1, mz - 1, if support == "minecraft:slime_block" { " R's support is a slime block (ironspec 08:46, the row partials: a conductor that is not solid-render, so R's cell is no stable destination for a panicking rider; the same repeater powers it)." } else { "" }));
        out.push_str(&format!("# Switched cells (left air here): R_1's side power (x+1,17,1) (the station's R line); the siding's switch: lane 7: the feed repeater's input (7,{},{}) (the feed path laid here: dust at R's input (5,{},{}), the delay-1 feed repeater facing west at (6,{},{}), both on glass); lanes -1 and 3 (day-1's feed from below, ironspec 11:22, option (a), the seat candidates' columns (x,22..23,9) kept clear): R's input dust (x-2,{},{}) laid here on a slime block (x-2,{},{}) that day-1's controller cells strongly power (lane -1: a delay-3 repeater (-4,22,9) facing east into it; day-1's cells, not this file); the siding pulse: on 30 gt (the launch threshold 8) at the napper's release, K gt after the window's pass (D's rising edge) while D is off; no other powered rail within 8 along R's line). No siding sensor: the napper-present bit is K's routing, delayed >= 16 gt plus the row's travel to K (K -> rest 12..15 gt measured). The holds' trapdoors (x,16,0) are the station's.\n", m - 1, mz + 2, m - 1, mz + 2, m - 1, mz + 2, m - 1, mz + 2, m - 2, mz + 2));
        out.push_str(&format!(
            "# Placement order (a rail placed last among its neighbours takes its natural shape; these differ from the design and need a temporary helper rail while placed, removed after: a removed rail reshapes nothing): {}. Lane x 3 shown; the same cells at x -1 and 7. c3 (x-1,{m},{mz}) sees three rails (c2, C, R): lay it before R. THE MERGE C (x,{m},{mz}) NEEDS A POWERED PLACEMENT (official, tick-exact): D (the split's) first; then a helper rail at (x,{m},{}) on a temporary support (x,{},{}), plus a temporary redstone block at (x+1,{m},{mz}); then C, which takes north_west; then remove the helper and its support, then the redstone block. THE TOP SLOPE S (x,{},{}) (official, tick-exact, `mctas mobai rail-slope`): the glass (x,{},{}) first, a helper rail (x,{},{}) on it, the slope below at its low end; then S comes out ascending_south and keeps it; with no helper it settles north_south.\n",
            if natural_mismatch.is_empty() { "none".to_string() } else { natural_mismatch.join("; ") },
            mz - 1,
            m - 1,
            mz - 1,
            17 + ns,
            1 + ns,
            17 + ns,
            2 + ns,
            18 + ns,
            2 + ns
        ));
        out.push_str("# Bench (the S-bend, wt-go; tool unreviewed): napper arrivals 0.10..0.44 on K's cell (the brief's band) rest on R at z 7.343..7.922 over their entry phases (the rest sweep, `--rest-sweep lo,hi,step --rest-phases N` with K laid: the ceiling 7.922 at arrival 0.102 over 64 phases, 0.10 at phase 0 7.895; rest z is not monotone in the arrival, its maxima fall with speed) and leave R 6..4 gt after R powers (lane 7: 10..8 gt after its switch, the feed repeater's input (7,23,9), R powering 4 gt later through the feed repeater and R's own; lanes -1 and 3 under day-1's feed from below: 14..12 gt after day-1's delay-3 repeater's input, R powering 8 gt later; without the feed path 8..6 gt, where tick-exact's official release, counted from the input pulse at R's repeater, reads one tick later: +9/+8/+7) (the launch threshold: the switch's pulse must be at least 8 gt, at the 0.10 rest and at the ceiling rest 7.922 alike: at 7 the napper leaves after R's power ended, at 0.231..0.233; 6 gt crawls or sticks at the high rests; the design's pulse 30); push contact with every passing cart 0, the least gap +0.084 at a 0.10 arrival and +0.239 at 0.20 against the split's ridden through-carts at 0.30, their entries into K swept (0.30 is the conservative end: the row's measured 0.35..0.39 moves a cart further along K's chord before the push search; tick-exact's official crossing on slime 1491498096433479: +0.0795 against 0.29 through-carts, and its first crossing (the napper placed static at its rest): +0.0838 against 0.30 (553dcf430, correcting f9d148591's +0.111), slime and stone alike; the final composition's crossing rows (0.29..0.44, their hashes printed) supersede both when they land); lane carts on the siding's cells 0; stray power on R 0. On the slime support with the feed path (lane 7's; day-1's feed from below for lanes -1 and 3 benched the same, `--feed-slime`): matrix (ii) and (A) 200 + 200 rows 0 failing (rets 132..400, merge entry 0.283..0.476, nap_rets 300..1700, +-2, the split row's through-carts at 0.30; occupancy 8, stamps over 1,000, cut windows and carts resting on another cart's top counted; the lane's largest occupancy at a release 7), restarts inside the napper's wait 1000/0 + 1000/0, the start-up from the steady state (7 riders, the 8th in the pocket) 3 cycles clean, the launch identical to stone's at every arrival and pulse; the control (glass under R) never launches. The lane overflow (ironspec 10:02): 0 carts resting on another cart's top in every row; V (x,22,6), slot 8, never held from the steady state to the last release (the bar: 0; every row of (ii) and (A), from both starts), held only in the full-lane start's transient (8 carts laid in the lane at t 0) and after the last release (the lane refilling with no release, the bench's end); an overfilled lane (9 carts, the control) leaves its extra cart at rest off the rails on the merge support's edge above V (a counted failure) with 14 empty releases of 40. The delivery floor: first failure stored 0.22, 0.23..1.00 clean.\n");
        if support == "minecraft:slime_block" {
            out.push_str("# R's slime support: lay it only together with F3's z 9 floors (9fc3155f's 4d76acbfb: lay with it, not before): each S-bend seat loses its own R cell as a stroll candidate (one of four at each seat in 8fb667bca's prints). Cited: tick-exact's napper release on the official server (`mctas mobai napper-release`, de0bcdeae; the stone baseline artifact 1012303384495744, the slime run matching it: the cart leaves R +9/+8/+7 gt from the input pulse at 0.37..0.52 b/gt from the rests 7.895 / 7.740 / 7.435; a cart on rails is never onGround, so slime's stepOn does not run). Pending: the seats' official P_null re-measured on slime.\n");
        }
        if let Some(rr) = rest_range {
            out.push_str("# The speeds line's nap-rest-z: the rest sweep (escape-bench --siding-north --k-row 0.30 --r-support slime_block --rest-sweep): 0.10..0.44 by 0.01 at 8 entry phases 7.343..7.912; 0.10..0.14 by 0.005 at 32 phases up to 7.920; 0.100..0.112 by 0.002 at 64 phases the ceiling 7.922 (arrival 0.102, phase 0.0606); no run without a napper at rest.\n");
            out.push_str(&format!("# speeds: nap-rest-z={rr}\n"));
        }
        // (c3 is a junction only through R beside it: ironspec's ruling (07:52), declared for push-scan)
        let why = "c3 (ironspec 07:52): its third arm is the siding rest R itself and it is never switched, and no other route's cart enters it (ironfarm escape-bench --siding-north: the westmost branch-cart centre in the merge cell x+0.24 over the matrix and restarts, lane carts on the siding's cells 0, push contact 0, one napper per siding at a time)";
        out.push_str(&format!("# own-route-junctions: {}\n", [-1, 3, 7].iter().map(|x| format!("{},{m},{mz}={why}", x - 1)).collect::<Vec<_>>().join("; ")));
    } else {
    out.push_str(&format!(
        "# F1 + Q2 lanes (ironfarm, the brake-and-rider escapement with the passive drop entry; ironspec 2026-11-12 01:21, lead): three lanes x -1 / 3 / 7, loop frame (paste with no move), {ns} slopes behind R_1, the merge at m = {m}, mz = {mz}. Benched as `mctas mobfarm escape-bench --siding --merge-curve --siding-dx 2 --guard-det --drop-entry --slopes {ns} --r-pulse 30 --drop-w 20 --per-lane 8` (drop-bench on explore/ironfarm): one lane, these cells at x 3.\n"
    ));
    out.push_str("# Per lane x: the hold string (x-2..x+2,17,0) and R_1 (x,17,1): the station's file (dfb89499 owns those cells; not laid here; the bench lays R_1 as a powered rail ascending south, its string's hooks on stone);");
    out.push_str(&format!(" the slopes (x,18..{},2..{}) plain, ascending south, on glass, the top one's high side (x,{},{}) glass (an ascending rail pops without it); the merge (x,{m},{mz}) a north_west curve on glass (x,{},{mz}), its north end the rail end over the drop column (x,{m}..{},{}) (empty but the pass string) onto the top slope (x,{},{}), the wall (x,{m},{}) glass (golem-proof);", 17 + ns, 1 + ns, 17 + ns, 2 + ns, m - 1, m - 1, mz - 1, 17 + ns, 1 + ns, mz - 2));
    out.push_str(&format!(" the pass string (x,{m},{}) with hooks (x+-1,{m},{}) on glass (x+-2,{m},{}) (shared at x 1 / 5); the 2-cell branch: (x,{m},{}) the detector guard (shared with the split's latch, 7b90c3f1) and (x,{m},{}) flat, both on glass, fed from the split's junction south of it (7b90c3f1's re-target);", mz - 1, mz - 1, mz - 1, mz + 1, mz + 2));
    out.push_str(&format!(" the siding at x-2: the rest (x-2,{m},{}) a powered rail ascending south (braked unpowered), the curve (x-2,{m},{mz}) south_east, the flat (x-1,{m},{mz}) east_west, the feed (x-2,{},{}) from the split's napper route, all on glass.\n", mz + 1, m + 1, mz + 2));
    out.push_str(&format!(
        "# Switched cells (left air here; the bench's emulation: a redstone block set and cleared with flags ALL): R_1's side power (x+1,17,1) (the R line: on 30 gt when the trapdoors shut); the siding's side power (x-1,{m},{}) (the siding pulse: on 30 gt at the napper's release, K = 4 gt after its predecessor's pass string edge, while the detector guard (x,{m},{}) is off; rule 2 (ironspec 03:11, provisional): only a napper already waiting when its predecessor passes is released, a later one waits for the predecessor's next pass). That cell is a face neighbour of the guard detector, which weakly powers it: built, it must be a solid conductor strongly powered only through a declared face away from the detector (no dust; no repeater or comparator taking input from the east); (x+1,{m},{}) stays air (it touches the next lane's siding rest); the block under the detector non-conductive unless it is the declared read path; never a comparator reading the detector. The holds' trapdoors (x,16,0) are the station's (the trapdoor line: open 20 gt).\n",
        mz + 1,
        mz + 1,
        mz + 1
    ));
    out.push_str("# PRE-SPLIT geometry: 7b90c3f1's in-chunk split (a row along z 9 at y 25, one detector slope (x,24,8) from its junction (x,25,9) into the merge) replaces this file's 2-cell branch ((x,24,8) detector, (x,24,9) flat); the drop entry, the siding and the slopes stand.\n");
    out.push_str(&format!(
        "# Placement order (a rail placed last among its neighbours takes its natural shape; these differ from the design and need a temporary helper rail while placed, removed after: a removed rail reshapes nothing): {}. Lane x 3 shown; the same cells at x -1 and 7. THE MERGE C (x,24,7) NEEDS A POWERED PLACEMENT (official, tick-exact; every unpowered order ends south_west): place the guard detector D (x,24,8) first; then a helper rail at (x,24,6) on a temporary support block at (x,23,6), plus a temporary redstone block at (x+1,24,7); then C, which takes north_west; then remove the helper and its support, then the redstone block. Once north_west, C holds through slope carts, siding carts and a restart. THE TOP SLOPE S (x,22,6) (official, tick-exact, `mctas mobai rail-slope`): the glass G (x,22,7) first, a helper rail H (x,23,7) on G, with the slope below at its low end; S then comes out ascending_south and keeps it through H's removal (G holds its high side), a neighbour update, glass placed where H stood (the merge's support), ridden carts, and a restart; with no H it settles north_south. The guard detector's raw connections (C and (x,24,9)) have 2 potential connections each (no T). R_1's side power (x+1,17,1) is the station's driver: it is face-adjacent to the hold string's hook (x+1,17,0), so no dust and no repeater or comparator reading from the north there.\n",
        if natural_mismatch.is_empty() { "none".to_string() } else { natural_mismatch.join("; ") }
    ));
    out.push_str("# The split's interface (the bench's v_in is the STORED speed, the cart's Motion at (x,24,9); a ridden cart moves 0.75 of it a tick): stored 0.30..0.48 benched clean (8 a lane, 5 slopes, ret 130..600, nap_ret 60/150/300, +-2: arrivals q0/q1 0.459, q2 0.392, q3 <= 0.332, nappers <= 0.459, landings meeting a cart 0); stored 0.26 fails the late napper with the guard at (x,24,8) (napper and successor land together); 0.22 rests on the merge's glass. Restarts (10 load orders at every phase) and the fresh-design cold start at every phase: 0 failing.\n");
    }
    out.push_str(if sbend { "name: f1q2-lanes-sbend\ncircuit: mob-farm iron_ingot\n" } else { "name: f1q2-lanes\ncircuit: mob-farm iron_ingot\n" });
    out.push_str(&format!("origin: {x0} 0 {z0}\npalette:\n"));
    for s in &states {
        out.push_str(&format!("  {} {}\n", ch(s), s));
    }
    for y in y0..=y1 {
        out.push_str(&format!("layer {y}:\n"));
        for z in z0..=z1 {
            let row: String = (x0..=x1).map(|x| cells.get(&(x, y, z)).map_or('.', |s| ch(s))).collect();
            // (an empty row is written as one '.': the parser skips whitespace-only lines, which
            // would move every later row of the layer north)
            let row = row.trim_end_matches('.');
            out.push_str(&format!("  {}\n", if row.is_empty() { "." } else { row }));
        }
    }
    // (the self-check: the file parsed back with the design parser must hold exactly these cells)
    // (and no rail would pop: the parsed blocks pasted raw into a world and every one asked
    // redsim's `rails::would_pop` (26.3's BaseRailBlock.shouldBeRemoved: a rigid block below, and on
    // an ascending rail's high side), the same code as `mctas design would-pop`; a cell the station
    // owns (R_1, its support) is outside this file and not checked)
    let d = mctas::design::Design::parse(&out).map_err(|e| format!("lanes_design: the written file does not parse: {e:#}"))?;
    {
        use redsim::pos::BlockPos;
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for p in d.blocks.keys() {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let mut w = redsim::world::World::new(BlockPos::new(lo[0] - 1, lo[1] - 1, lo[2] - 1), [hi[0] - lo[0] + 3, hi[1] - lo[1] + 3, hi[2] - lo[2] + 3]);
        let placed: Vec<(BlockPos, redsim::block::Block)> = d
            .blocks
            .iter()
            .map(|(p, s)| redsim::block::Block::parse(s).map(|b| (BlockPos::new(p[0], p[1], p[2]), b)).map_err(|e| format!("lanes_design: {s}: {e}")))
            .collect::<Result<_, _>>()?;
        w.place_raw(&placed);
        let pops: Vec<String> = d.blocks.keys().filter(|p| redsim::rails::would_pop(&w, BlockPos::new(p[0], p[1], p[2]))).map(|p| format!("{p:?}")).collect();
        if !pops.is_empty() {
            return Err(format!("lanes_design: rails that would pop (BaseRailBlock.shouldBeRemoved): {}", pops.join(", ")));
        }
    }
    let want: std::collections::BTreeMap<[i32; 3], String> = cells.iter().map(|(k, v)| ([k.0, k.1, k.2], v.to_string())).collect();
    if d.blocks != want {
        let missing: Vec<String> = want.iter().filter(|(k, v)| d.blocks.get(*k) != Some(*v)).take(4).map(|(k, v)| format!("{k:?} {v}")).collect();
        let extra: Vec<String> = d.blocks.iter().filter(|(k, v)| want.get(*k) != Some(*v)).take(4).map(|(k, v)| format!("{k:?} {v}")).collect();
        return Err(format!("lanes_design: the parsed file differs from the bench's cells: wanted {missing:?}, found {extra:?}"));
    }
    Ok(out)
}

/// The rails of a cell map whose natural shape (placed last among its neighbours) differs from the
/// designed one, at lane x 3, each with its fix (a temporary helper rail on the side the design
/// connects to and the neighbours do not offer).
fn natural_shapes(cells: &std::collections::BTreeMap<(i32, i32, i32), &str>) -> Vec<String> {
    let shape_of = |s: &str| -> Option<String> {
        let i = s.find("shape=")? + 6;
        Some(s[i..].split([',', ']']).next()?.to_string())
    };
    let is_rail = |s: &str| s.contains("rail[");
    // (a direction's unit vector)
    let dirs = [("north", (0, -1)), ("south", (0, 1)), ("east", (1, 0)), ("west", (-1, 0))];
    // (the ends a shape connects: (direction, dy of the neighbour's rail))
    let ends = |sh: &str| -> Vec<(&'static str, i32)> {
        match sh {
            "north_south" => vec![("north", 0), ("south", 0)],
            "east_west" => vec![("east", 0), ("west", 0)],
            "ascending_south" => vec![("north", 0), ("south", 1)],
            "ascending_north" => vec![("south", 0), ("north", 1)],
            "ascending_east" => vec![("west", 0), ("east", 1)],
            "ascending_west" => vec![("east", 0), ("west", 1)],
            "north_west" => vec![("north", 0), ("west", 0)],
            "north_east" => vec![("north", 0), ("east", 0)],
            "south_west" => vec![("south", 0), ("west", 0)],
            "south_east" => vec![("south", 0), ("east", 0)],
            _ => vec![],
        }
    };
    let opposite = |d: &str| match d {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        _ => "east",
    };
    let mut out = Vec::new();
    for (p, s) in cells {
        if !is_rail(s) || p.0 != 3 {
            continue;
        }
        let Some(designed) = shape_of(s) else { continue };
        // (the neighbours it can connect to: a rail at the same level, one up, or one down whose own
        // shape has an end reaching back up to this cell)
        let mut conn: Vec<(&str, i32)> = Vec::new();
        for (dn, (dx, dz)) in dirs {
            for dy in [0, 1, -1] {
                let q = (p.0 + dx, p.1 + dy, p.2 + dz);
                let Some(t) = cells.get(&q).filter(|t| is_rail(t)) else { continue };
                if dy == -1 {
                    let reaches = shape_of(t).is_some_and(|ts| ends(&ts).contains(&(opposite(dn), 1)));
                    if !reaches {
                        continue;
                    }
                }
                conn.push((dn, dy.max(0)));
                break;
            }
        }
        let has = |d: &str| conn.iter().find(|c| c.0 == d).map(|c| c.1);
        let natural = match (has("north"), has("south"), has("east"), has("west")) {
            (Some(_), Some(1), None, None) => "ascending_south",
            (Some(1), Some(_), None, None) => "ascending_north",
            (Some(_), Some(_), None, None) | (Some(_), None, None, None) | (None, Some(_), None, None) | (None, None, None, None) => {
                if has("south") == Some(1) {
                    "ascending_south"
                } else if has("north") == Some(1) {
                    "ascending_north"
                } else {
                    "north_south"
                }
            }
            (None, None, Some(1), _) => "ascending_east",
            (None, None, _, Some(1)) => "ascending_west",
            (None, None, Some(_), Some(_)) | (None, None, Some(_), None) | (None, None, None, Some(_)) => "east_west",
            (Some(_), None, None, Some(_)) => "north_west",
            (Some(_), None, Some(_), None) => "north_east",
            (None, Some(_), None, Some(_)) => "south_west",
            (None, Some(_), Some(_), None) => "south_east",
            _ => "a junction (3+ neighbours)",
        };
        // (powered and detector rails take no curves: a curve's natural shape on them is not offered)
        if natural != designed {
            let missing: Vec<String> = ends(&designed)
                .into_iter()
                .filter(|(d, dy)| has(d) != Some(*dy))
                .map(|(d, dy)| {
                    let (dx, dz) = dirs.iter().find(|x| x.0 == d).map(|x| x.1).unwrap_or((0, 0));
                    let h = (p.0 + dx, p.1 + dy, p.2 + dz);
                    match cells.get(&h) {
                        Some(b) if !is_rail(b) => format!("({},{},{}) (before the {} there is laid)", h.0, h.1, h.2, b.trim_start_matches("minecraft:").split('[').next().unwrap_or("block")),
                        _ => format!("({},{},{})", h.0, h.1, h.2),
                    }
                })
                .collect();
            out.push(format!("({},{},{}) designed {designed}, natural {natural}: place it with a helper rail at {}", p.0, p.1, p.2, missing.join(" and ")));
        }
    }
    out
}

/// Run `f` over `0..n` on `jobs` threads, results in order.
pub fn par_map<T: Send>(n: usize, jobs: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<T>>> = std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..jobs.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let r = f(i);
                out.lock().unwrap()[i] = Some(r);
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|x| x.expect("every job ran")).collect()
}

/// One siding run's numbers, for the matrix.
#[derive(Clone, Debug, Default)]
pub struct SidingRow {
    pub empty_releases: usize,
    pub not_one: usize,
    pub late_naps: usize,
    pub nap_contacts: Vec<f64>,
    pub nap_q: Vec<i64>,
    /// The napper's curve-rule margins at the merge (its entry minus its predecessor's clear, gt).
    pub nap_cr: Vec<i64>,
    /// The longest napper wait on the siding (gt).
    pub nap_wait_max: i64,
    /// (--nap-ai) The oldest CANT_REACH stamp at a calm REST tick with no walk target held over the
    /// nappers' windows (-1: none), on the siding and after the release; the nappers above 1,000 gt;
    /// windows tracked of all; ended by a clear after the release, or at the drop (the stamp there);
    /// nappers that lost HOME.
    pub stamp_calm_max: i64,
    pub stamp_siding_max: i64,
    pub stamp_lane_max: i64,
    pub stamp_over_1000: usize,
    pub calm_rest_max: i64,
    pub home_lost: usize,
    pub nap_tracked: usize,
    pub nap_windows: usize,
    pub win_cleared: usize,
    pub win_dropped: usize,
    /// Napper windows cut short (a failure).
    pub win_cut: usize,
    /// --k-row: the least z gap between a through-cart's box and the waiting napper's push box
    /// (boxes 0.98, the napper's inflated 0.2; negative is contact; f64::MAX none).
    pub th_zgap: f64,
    /// The nappers' launches from the rest: stuck (still on it 40 gt after the release) plus late
    /// (off it only after the pulse ended), both failures; the departure ticks after the release
    /// and the speeds then (min, max).
    pub launch_stuck: usize,
    pub launch_t: (i64, i64),
    pub launch_v: (f64, f64),
    /// The lane overflow (ironspec 10:02): cart-ticks a cart rested on another cart's top (a
    /// failure), and the occupancy at which a cart first rested at the lane's top cell V.
    pub stacked: usize,
    pub v_rest_occ: Option<usize>,
    /// Carts holding V 20 gt from the steady state (slot 8; a failure).
    pub v_holds: usize,
    pub drop_stamp_max: i64,
    /// The branch returns' merge-entry stored speed (min, max; -1 none); the nappers' rest z (min,
    /// max; -1 none); --k-row: through-carts slowed, lost, ticks in push range of the waiting napper,
    /// box overlaps; --flat-detector: false trips.
    pub merge_v_min: f64,
    pub merge_v_max: f64,
    pub rest_z_min: f64,
    pub rest_z_max: f64,
    pub th_slowed: usize,
    pub th_lost: usize,
    pub th_push: usize,
    pub th_overlap: usize,
    pub th_min_exit: f64,
    pub flat_false: usize,
    /// Lane carts in push contact with the waiting napper (ticks); lane carts on the siding's cells.
    pub lane_push: usize,
    pub lane_on_siding: usize,
    /// The lane's largest occupancy at a release from the steady state; releases at 8.
    pub occ_max: usize,
    pub occ8: usize,
    pub wrong: usize,
    pub knocked: usize,
    /// Rails popped at placement (no rigid support).
    pub popped: usize,
    /// Ticks the siding rest was powered with its declared switch off.
    pub stray_power: usize,
    /// Ticks the guard detector was powered while a napper waited (the scenario of the stray check).
    pub guard_on_waiting: usize,
    pub touching: usize,
    pub arrivals_over: usize,
    pub arrival_max: f64,
    pub by_q: std::collections::BTreeMap<usize, f64>,
    pub order_broken: usize,
    pub landing_meets: usize,
    pub landing_over: usize,
    pub occupied_q: String,
    pub top_cell: String,
    pub off_rail: usize,
    pub eyes: usize,
    pub min_health: f32,
    pub restage_max: i64,
    pub stuck: bool,
    pub text: String,
}

/// The bench's numbers read back from its own text (one source of truth: the printed lines).
pub fn siding_row(o: &DropOpts, so: &SidingOpts) -> Result<SidingRow, String> {
    let text = siding_bench(o, so)?;
    let mut r = SidingRow { min_health: f32::MAX, stamp_calm_max: -1, stamp_siding_max: -1, stamp_lane_max: -1, drop_stamp_max: -1, merge_v_min: -1.0, merge_v_max: -1.0, rest_z_min: -1.0, rest_z_max: -1.0, th_min_exit: -1.0, th_zgap: f64::MAX, ..Default::default() };
    let num_after = |s: &str, key: &str| -> Option<f64> {
        let i = s.find(key)? + key.len();
        let t: String = s[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
        t.parse().ok()
    };
    let req = |l: &str, key: &str| -> Result<f64, String> { num_after(l, key).ok_or(format!("siding_row: no number after \"{key}\" in: {l}")) };
    for l in text.lines() {
        if l.starts_with("carts per release (exactly 1): ") {
            let body = l.trim_start_matches("carts per release (exactly 1): ");
            let counts = body.split(';').next().unwrap_or("");
            for part in counts.split(", ") {
                let mut it = part.split(": ");
                if let (Some(a), Some(b2)) = (it.next(), it.next())
                    && a.trim() != "1"
                {
                    r.not_one += b2.trim().parse::<usize>().unwrap_or(0);
                }
            }
            r.empty_releases = req(l, "golems lost: a batch of 2 can't summon): ")? as usize;
            r.late_naps = req(l, "late nappers (a lane-lap wait): ")? as usize;
        } else if l.starts_with("nappers through the siding: ") {
            for seg in l.split(" | ") {
                if let Some(v) = num_after(seg, "its contact ") {
                    if let Some(cr) = num_after(seg, "minus the predecessor's clear) ") {
                        r.nap_cr.push(cr as i64);
                    }
                    r.nap_contacts.push(v);
                    r.nap_q.push(num_after(seg, " at q ").map_or(-1, |q| q as i64));
                }
            }
        } else if l.starts_with("the longest napper wait on the siding") {
            r.nap_wait_max = req(l, "to its release): ")? as i64;
        } else if l.starts_with("the branch returns' stored speed entering the merge cell") {
            if let Some(s) = l.split(": ").nth(1).and_then(|s| s.split(' ').next())
                && let Some((a, b)) = s.split_once("..")
            {
                r.merge_v_min = a.parse().map_err(|_| format!("siding_row: merge-entry speed {s}"))?;
                r.merge_v_max = b.parse().map_err(|_| format!("siding_row: merge-entry speed {s}"))?;
            }
        } else if l.starts_with("the lane's top cell V") {
            r.stacked = req(l, "carts resting on another cart's top (the overflow; the bar: 0): ")? as usize;
            r.v_holds = req(l, "carts holding V 20 gt (V held as a queue slot: slot 8; the bar: 0): ")? as usize;
            r.v_rest_occ = l.split(", the first at the lane's occupancy ").nth(1).and_then(|s| s.split(';').next()).and_then(|s| s.trim().parse().ok());
        } else if l.starts_with("the nappers' launches from the rest") {
            r.launch_stuck = req(l, "stuck (still on the rest 40 gt after its release; the bar: 0): ")? as usize + req(l, "launches after the pulse ended (R unpowered at the departure tick; the bar: 0): ")? as usize;
            if let Some(s) = l.split("departure ").nth(1).and_then(|s| s.split(" gt").next())
                && let Some((a, b)) = s.split_once("..")
            {
                r.launch_t = (a.parse().unwrap_or(-1), b.parse().unwrap_or(-1));
            }
            if let Some(s) = l.split("speed then ").nth(1).and_then(|s| s.split(';').next())
                && let Some((a, b)) = s.split_once("..")
            {
                r.launch_v = (a.parse().unwrap_or(-1.0), b.parse().unwrap_or(-1.0));
            }
        } else if l.starts_with("nappers came to rest on the siding at z ") {
            if let Some(s) = l.strip_prefix("nappers came to rest on the siding at z ").and_then(|s| s.split(' ').next())
                && let Some((a, b)) = s.split_once("..")
            {
                r.rest_z_min = a.parse().map_err(|_| format!("siding_row: rest z {s}"))?;
                r.rest_z_max = b.parse().map_err(|_| format!("siding_row: rest z {s}"))?;
            }
        } else if l.starts_with("the lane's occupancy at the releases from the steady state") {
            r.occ_max = req(l, "the largest ")? as usize;
            r.occ8 = req(l, "releases at 8 (the 8th slot is the top cell; the bar: 0): ")? as usize;
        } else if l.starts_with("lane carts on the siding's cells") {
            r.lane_on_siding = req(l, "or the old siding's cells; the bar: 0): ")? as usize;
        } else if l.starts_with("lane carts in push contact with the waiting napper") {
            r.lane_push = req(l, "y ranges overlapping; the bar: 0): ")? as usize;
        } else if l.starts_with("the split row's through-carts past K") {
            r.th_slowed = req(l, "slowed below 0.95 of it ")? as usize;
            r.th_lost = req(l, "), lost ")? as usize;
            r.th_push = req(l, "in push range of the waiting napper (the bar: 0): ")? as usize;
            r.th_overlap = req(l, "its box overlapping the napper's ")? as usize;
            r.th_min_exit = num_after(l, "(the slowest exit ").unwrap_or(-1.0);
            r.th_zgap = l.rsplit("(the napper's inflated 0.2; positive is clear): ").next().and_then(|s| s.split([' ', ',', ';']).next()).and_then(|s| s.parse().ok()).unwrap_or(f64::MAX);
        } else if l.starts_with("the siding flat's detector") {
            r.flat_false = req(l, "met its detection box (the bar: 0): ")? as usize;
        } else if l.starts_with("the nappers' windows") {
            r.nap_tracked = req(l, "tracked ")? as usize;
            r.nap_windows = req(l, "of ")? as usize;
            let s = num_after(l, "with no walk target held: on the siding ").map_or(-1, |v| v as i64);
            let a = num_after(l, ", after the release ").map_or(-1, |v| v as i64);
            r.stamp_calm_max = s.max(a);
            r.stamp_siding_max = s;
            r.stamp_lane_max = a;
            r.stamp_over_1000 = req(l, "(above 1,000 gt, the bar: ")? as usize;
            r.win_cleared = req(l, "windows ended by a clear after the release ")? as usize;
            r.win_dropped = req(l, "reaching the drop uncleared ")? as usize;
            r.drop_stamp_max = num_after(l, "(the stamp there up to ").map_or(-1, |v| v as i64);
            r.win_cut = req(l, "windows cut short (the bar: 0): ")? as usize;
            r.home_lost = req(l, "nappers that lost HOME (the bar: 0): ")? as usize;
        } else if l.starts_with("the napper heading the wrong way") {
            r.wrong = req(l, "(the bar: 0): ")? as usize;
        } else if l.starts_with("nappers knocked loose") {
            r.knocked = req(l, "(moving with no release; the bar: 0): ")? as usize;
            r.popped = req(l, "on an ascending rail's high side; the bar: 0): ")? as usize;
        } else if l.starts_with("the siding rest powered with its declared switch off") {
            r.stray_power = req(l, "(a stray power; the bar: 0): ")? as usize;
            r.guard_on_waiting = req(l, "ticks the guard was powered while a napper waited: ")? as usize;
        } else if l.starts_with("ticks with two carts over the merge") {
            r.touching = req(l, "the bar: 0): ")? as usize;
        } else if l.starts_with("arrivals at the lanes' backs") {
            r.arrivals_over = l.split("first contact in the lane): ").nth(1).and_then(|s| s.split(", ").nth(1)).and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok()).ok_or(format!("siding_row: no arrivals-over count in: {l}"))?;
            r.arrival_max = req(l, "the hardest ")?;
            if let Some(i) = l.find("the hardest by q: ") {
                let body = &l[i + "the hardest by q: ".len()..];
                let body = body.split(';').next().unwrap_or("");
                for part in body.split(", ") {
                    let mut it = part.split(": ");
                    if let (Some(a), Some(b2)) = (it.next(), it.next())
                        && let (Ok(q), Ok(v)) = (a.trim().parse::<usize>(), b2.trim().parse::<f64>())
                    {
                        r.by_q.insert(q, v);
                    }
                }
            }
        } else if l.starts_with("arrivals landing out of order") {
            r.order_broken = req(l, "the bar: 0): ")? as usize;
        } else if l.starts_with("landings meeting another cart's box") {
            r.landing_meets = req(l, "against 0.46): ")? as usize;
            r.landing_over = l.split("against 0.46): ").nth(1).and_then(|s| s.split(", ").nth(1)).and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok()).ok_or(format!("siding_row: no landings-over count in: {l}"))?;
            r.occupied_q = l.split("held a queued cart at q: ").nth(1).unwrap_or("-").to_string();
        } else if l.starts_with("the top slope's cell") {
            r.top_cell = l.split("by q when it came in: ").nth(1).and_then(|s| s.split(" [the bench's start").next()).unwrap_or("-").to_string();
        } else if l.starts_with("carts at rest off the rails") {
            r.off_rail = req(l, "the bar: 0): ")? as usize;
        } else if l.starts_with("riders' eyes inside") {
            r.eyes = req(l, "rider-ticks; the bar: 0): ")? as usize;
            r.min_health = num_after(l, "the riders' lowest health ").map(|v| v as f32).unwrap_or(f32::MAX);
        } else if l.starts_with("the restage") {
            r.restage_max = num_after(l, "the slowest ").unwrap_or(-1.0) as i64;
        } else if l.starts_with("a napper still waiting") {
            r.stuck = l.contains("— stuck");
        }
    }
    // (every bar line must have been read: a line the bench stopped printing is an error, not a 0)
    for key in ["carts per release (exactly 1)", "nappers through the siding", "the longest napper wait on the siding", "the siding rest powered with its declared switch off", "the napper heading the wrong way", "nappers knocked loose", "ticks with two carts over the merge", "arrivals at the lanes' backs", "arrivals landing out of order", "landings meeting another cart's box", "carts at rest off the rails", "riders' eyes inside", "the restage"] {
        if !text.lines().any(|l| l.starts_with(key)) {
            return Err(format!("siding_row: the bench printed no \"{key}\" line"));
        }
    }
    // (G1: with --nap-ai the windows line is required too; so are the --k-row and --flat-detector lines)
    if so.ai && !text.lines().any(|l| l.starts_with("the nappers' windows")) {
        return Err("siding_row: --nap-ai, but the bench printed no \"the nappers' windows\" line".to_string());
    }
    if so.k_row.is_some() && !text.lines().any(|l| l.starts_with("the split row's through-carts past K")) {
        return Err("siding_row: --k-row, but the bench printed no through-carts line".to_string());
    }
    if so.flat_detector && !text.lines().any(|l| l.starts_with("the siding flat's detector")) {
        return Err("siding_row: --flat-detector, but the bench printed no detector line".to_string());
    }
    for key in ["the branch returns' stored speed entering the merge cell", "nappers came to rest on the siding at z ", "lane carts in push contact with the waiting napper", "lane carts on the siding's cells", "the nappers' launches from the rest", "the lane's top cell V"] {
        if !text.lines().any(|l| l.starts_with(key)) {
            return Err(format!("siding_row: the bench printed no \"{key}\" line"));
        }
    }
    r.text = text;
    Ok(r)
}
