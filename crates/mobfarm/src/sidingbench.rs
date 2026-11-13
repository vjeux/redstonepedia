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
// (the riders' bed (--nap-ai): the design's HOME (−3,−27,3) (ironspec Nov 13 10:12, G1 10:09: the
// in-tick steps follow the walk target toward HOME, so the bench's bed sits where the design's does;
// before Nov 13 11:00 the bench laid it at (−3,−18,3), 9 higher))
const BED: (i32, i32, i32) = (-3, -27, 3);

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

/// `--seeds-matrix`'s list: comma-separated seeds and inclusive `A..B` ranges (`1..8`, `1,3,5..7`);
/// empty, a reversed range or a non-number is an error, and so is a repeated seed.
pub fn parse_seed_list(s: &str) -> Result<Vec<i64>, String> {
    let mut out: Vec<i64> = Vec::new();
    for part in s.split(',').map(str::trim) {
        let (a, b) = match part.split_once("..") {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: i64 = a.parse().map_err(|_| format!("--seeds-matrix {s}: {part:?} is not a seed or A..B"))?;
        let b: i64 = b.parse().map_err(|_| format!("--seeds-matrix {s}: {part:?} is not a seed or A..B"))?;
        if b < a {
            return Err(format!("--seeds-matrix {s}: {part:?} runs backwards"));
        }
        for k in a..=b {
            if out.contains(&k) {
                return Err(format!("--seeds-matrix {s}: seed {k} twice"));
            }
            out.push(k);
        }
    }
    Ok(out)
}

/// The drive's delay from the siding's switch to R powered, as the bench lays the feed: R's repeater 2
/// gt, the feed path's repeater 2 more, day-1's delay-3 repeater 6 more with --feed-slime (the rest
/// sweep's PRESENT lead reads it too: ironspec 10:54 (2)(d) on 1788d4f65, not a literal 8).
pub fn drive_delay_of(so: &SidingOpts) -> i64 {
    if so.siding_north && so.feed_path && so.feed_slime {
        8
    } else if so.siding_north && so.feed_path {
        4
    } else {
        2
    }
}

fn st(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64, f64)> {
    redsim::entity::index_of(w, id).map(|i| {
        let e = &w.entities.list[i];
        let (x, y, z) = dpos(e.pos);
        (x, y, z, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
    })
}

thread_local! {
    /// --mirror-z for the bench running on this thread (st() reads positions back through it).
    static MIRROR_Z: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// --pos-offset (ironspec Nov 13 10:49 (1), the build-position term): the whole bench laid that far
    /// from the design frame (world = mirror(design) + offset), every read taken back through it.
    static OFFSET: std::cell::Cell<(i32, i32, i32)> = const { std::cell::Cell::new((0, 0, 0)) };
}

fn off() -> (f64, f64, f64) {
    let o = OFFSET.with(|o| o.get());
    (o.0 as f64, o.1 as f64, o.2 as f64)
}

/// A spawn at a mirrored design position `p` (the caller's `mzf`), moved by the build offset.
fn spawn_off(w: &mut redsim::world::World, kind: &str, p: [f64; 3], nbt: Option<&str>) -> Result<u32, String> {
    let o = off();
    redsim::entity::spawn(w, kind, [p[0] + o.0, p[1] + o.1, p[2] + o.2], nbt)
}

/// A world position in the design frame (the offset removed, the mirror undone).
fn dpos(p: redsim::entity::vec3::Vec3) -> (f64, f64, f64) {
    let o = off();
    let m = MIRROR_Z.with(|m| m.get());
    let z = p.z - o.2;
    (p.x - o.0, p.y - o.1, if m { -z } else { z })
}

/// A world box in the design frame: (x0, x1, y0, y1, z0, z1).
fn dbox(bb: &redsim::entity::vec3::Aabb) -> (f64, f64, f64, f64, f64, f64) {
    let o = off();
    let m = MIRROR_Z.with(|m| m.get());
    let (za, zb) = (bb.min_z - o.2, bb.max_z - o.2);
    let (za, zb) = if m { (-zb, -za) } else { (za, zb) };
    (bb.min_x - o.0, bb.max_x - o.0, bb.min_y - o.1, bb.max_y - o.1, za, zb)
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

/// One row of the push grid (ironspec 03:00): a rest position (the napper's arrival on K and its
/// entry phase), the injected K-row passes that met the rest, and the resting napper's largest
/// displacement and speed over them.
#[derive(Clone, Debug, PartialEq)]
pub struct GridRow {
    pub z: f64,
    pub arrival: f64,
    pub phase: f64,
    pub passes: usize,
    pub disp: f64,
    pub speed: f64,
    /// The least box gap between a passing cart's box and the resting napper's push box.
    pub gap: f64,
    /// The planted control's displacement at this rest.
    pub planted: f64,
}

/// The push grid's table, as --push-grid-out writes it (only for a passing grid): '#' lines are
/// comments, the provenance header and the verdict line required, every other line "rest-z arrival
/// phase passes displacement speed least-gap planted-displacement" (8 columns).
pub fn parse_push_grid(text: &str) -> Result<Vec<GridRow>, String> {
    // (G1 (5) / ironspec 07:28 C4: only a passing grid's own table covers a rest: its provenance
    // header, its verdict line, and every row's 8 columns, the planted control's included)
    if !text.lines().any(|l| l.starts_with("# push grid (ironfarm escape-bench")) {
        return Err("push grid: no provenance header (# push grid (ironfarm escape-bench ...): not a grid run's table".to_string());
    }
    if !text.lines().any(|l| l.starts_with("# verdict: PASS")) {
        return Err("push grid: no \"# verdict: PASS\" line: a failing or unfinished grid's table covers nothing".to_string());
    }
    let mut out = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let v: Vec<&str> = l.split_whitespace().collect();
        if v.len() != 8 {
            return Err(format!("push grid: {} columns in {l:?} (8: rest-z arrival phase passes displacement speed least-gap planted-displacement)", v.len()));
        }
        let f = |i: usize| v.get(i).and_then(|s| s.parse::<f64>().ok()).ok_or(format!("push grid: bad column {} in {l:?}", i + 1));
        let (gap, planted) = (f(6)?, f(7)?);
        out.push(GridRow { z: f(0)?, arrival: f(1)?, phase: f(2)?, passes: f(3)? as usize, disp: f(4)?, speed: f(5)?, gap, planted });
    }
    if out.is_empty() {
        return Err("push grid: no rows".to_string());
    }
    Ok(out)
}

/// The bench's run-time overrides, checked (ironspec Nov 13 04:07: an override that leaves its cell
/// unchanged is a silent no-op, a failure): per kind, the sets and the no-ops, every override's cell
/// before -> after (counted per distinct transition), and the controls' cells.
#[derive(Default)]
struct Overrides {
    counts: std::collections::BTreeMap<&'static str, (usize, usize)>,
    trans: std::collections::BTreeMap<(&'static str, String), usize>,
    notes: Vec<String>,
}

impl Overrides {
    /// Set `blk` at `pos` (world frame) with `flags`, recording the cell's state before and after;
    /// `note`: print this one (a control's cell) beside the counts.
    fn set_with(&mut self, w: &mut redsim::world::World, pos: BlockPos, blk: &Block, kind: &'static str, note: bool, flags: u32) {
        let before = w.get(pos);
        w.set_block(pos, blk.clone(), flags);
        let after = w.get(pos);
        let e = self.counts.entry(kind).or_default();
        e.0 += 1;
        if before == after {
            e.1 += 1;
        }
        *self.trans.entry((kind, format!("{before} -> {after}"))).or_default() += 1;
        if note || before == after {
            self.notes.push(format!("{kind} ({},{},{}): {before} -> {after}{}", pos.x, pos.y, pos.z, if before == after { " (NO CHANGE)" } else { "" }));
        }
    }

    fn set(&mut self, w: &mut redsim::world::World, pos: BlockPos, blk: &Block, kind: &'static str, note: bool) {
        self.set_with(w, pos, blk, kind, note, redsim::world::flags::ALL);
    }

    fn noops(&self) -> usize {
        self.counts.values().map(|c| c.1).sum()
    }

    /// Every override kind's transitions, before -> after with their counts.
    fn transitions(&self) -> String {
        if self.trans.is_empty() {
            return "none".to_string();
        }
        self.trans.iter().map(|((k, t), n)| format!("{k}: {t} x{n}")).collect::<Vec<_>>().join("; ")
    }
}

/// The lip margin's outcome for one napper and its leader's rider (ironspec 00:02 (1): three outcomes,
/// G1's CHANGES 1 on a191f3dfb).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lip {
    /// The napper never reaches the rider over the lip region (no contact window: clean).
    Never,
    /// The napper reaches the rider at tick N, but the rider's box top never drops below the floor
    /// within its track (a rider held at the lip, or one leaving the region still high): STUCK, a
    /// failure whatever the bar.
    Held(i64),
    /// N - R - 1 (-1 or less: contact, the move cut at the rider's face).
    Margin(i64),
}

/// The lip margin (ironspec 21:14 (2), adopted 22:22 as the S-bend's gate quantity), in ticks, from
/// positions after each tick in the design frame (z grows south; the carts run north), the napper's
/// and its leader's rider's tracks over the lip region (the merge's south face to 3 cells north of it:
/// the lane's back queue, further north, is no lip contact):
/// - N: the first tick whose napper move reaches the leader's rider: the napper's box front (its
///   centre z - 0.49) at or north of the rider's box back face (the rider's centre z + its half
///   width), within 1e-4, as the rider stood after the previous tick (napper first in the tick order,
///   the worst order: the move sees the rider where the last tick left it), with the boxes sharing x;
/// - R: the first tick after which the rider's box top (its feet y + its height) is below the
///   napper's floor `floor` (the merge's rail level, y 24.0);
/// - Margin(N - R - 1): -1 or less is contact (the move is cut at the rider's face: stuck), 0 or more
///   is clean with that many ticks to spare; Never: the napper never reaches the rider (the rider
///   stays ahead: no contact window); Held(N): the napper reaches the rider and the rider never drops
///   below the floor (STUCK).
///
/// `napper`: (t, x, z); `rider`: (t, x, z, feet y, half width, height).
pub fn lip_margin(napper: &[(i64, f64, f64)], rider: &[(i64, f64, f64, f64, f64, f64)], floor: f64) -> Lip {
    let r_at = |t: i64| rider.iter().find(|r| r.0 == t);
    let Some(n) = napper.iter().find(|&&(t, x, z)| r_at(t - 1).is_some_and(|r| (x - r.1).abs() < 0.49 + r.4 && z - 0.49 <= r.2 + r.4 + 1e-4)).map(|k| k.0) else {
        return Lip::Never;
    };
    match rider.iter().find(|r| r.3 + r.5 < floor) {
        Some(r) => Lip::Margin(n - r.0 - 1),
        None => Lip::Held(n),
    }
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

/// `--j-rail` (ironspec Nov 13 10:18 (2)): a cart moved with its rider, as a teleport (the position,
/// its old position and the box set at `pos`, the motion `v`, no fall distance); the passenger follows
/// at its vehicle's next tick (`positionRider`), its brain unchanged (its route history kept).
fn teleport_cart(w: &mut redsim::world::World, id: u32, pos: [f64; 3], v: [f64; 3]) -> bool {
    let Some(i) = redsim::entity::index_of(w, id) else { return false };
    let o = off();
    let pos = [pos[0] + o.0, pos[1] + o.1, pos[2] + o.2];
    w.entities.set_pos(i, redsim::entity::vec3::Vec3::new(pos[0], pos[1], pos[2]));
    let e = &mut w.entities.list[i];
    e.x_old = pos[0];
    e.y_old = pos[1];
    e.z_old = pos[2];
    e.delta_movement = redsim::entity::vec3::Vec3::new(v[0], v[1], v[2]);
    e.fall_distance = 0.0;
    true
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
    /// The napper counts as present for the siding's release sample this many gt after its K tick
    /// (its spawn on K's cell with --nap-feed-v), whether or not it has come to rest on R (ironspec
    /// 17:58 / 18:06, day-1's PRESENT moved from K + 16 to K + 2); None: present once at rest on R.
    pub present_k: Option<i64>,
    /// day-1's release rule (v6 §7, day-1 18:24): the siding's switch fires K gt after D's rising
    /// edge whenever WINDOW and PRESENT held at that edge, with no wait for D off (the bench's
    /// default waits for D off, about 20 gt after the edge: the detector's recheck).
    pub no_d_gate: bool,
    /// The boundary probe (ironspec 19:52): the napper's release forced at this tick, whatever the
    /// window, PRESENT and D say (one napper; --inject-sweep runs one row per tick).
    pub inject_release: Option<i64>,
    /// r_joint's positive control (G1's CHANGES 1 on f1c00f223): ("turn", T) spawns a second cart on
    /// K's cell at tick T moving north toward F (it must count), ("pass", T) one on the row's west
    /// cell eastbound through K's cell (it must not).
    pub r_joint_control: Option<(String, i64)>,
    /// The lip bar's positive control through the rows' code path (ironspec 00:27): glass in the
    /// drop column two cells under the lip (x,m-2,mz-1), laid at the napper's release: the leader
    /// held one block down, its rider above the floor.
    pub held_rider_control: bool,
    /// The S-bend's stray-power positive control (ironspec 02:18): a redstone block beside R
    /// (x-3,m,mz) from tick T for 10 gt, R powered with its switch off: the stray count must bite.
    pub stray_ctl: Option<i64>,
    /// The push grid's injected pass (ironspec 03:00): (speed, entry fraction through the row's west
    /// cell, delay after the napper comes to rest, planted); the K row's own spawns are off. Planted
    /// (the grid's positive control through its own path, ironspec 04:06): the cart starts on K's
    /// cell moving north, into F and onto the resting napper, and counts as a pass while it lives.
    pub inject_pass: Option<(f64, f64, i64, bool)>,
    /// The push grid's table (--push-grid): a row with no K-row pass meeting the napper's rest names
    /// the grid row covering each rest z, and a rest z no grid row covers fails.
    pub push_grid: Option<Vec<GridRow>>,
    /// The worst PRESENT -> at rest of the rest sweep (gt), for a row whose nappers never came to rest
    /// (ironspec 04:14 (7)): 22 by default, the rest sweep's worst (`--rest-sweep` over arrivals
    /// 0.10..0.44, its "PRESENT -> at rest" line: 22 gt at 0.10); a row that measures its own settle
    /// (the test's 0.10 @ 820: 21) uses its own.
    pub present_settle_worst: i64,
    /// Day-1's confirmation, declared, that the PRESENT -> latch -> AND path is triggered and
    /// unclocked (ironspec 06:08 real): the world-open lead then applies to the settle term only.
    pub present_latch_unclocked: bool,
    /// The outcome bar's positive control (G1 / ironspec 07:31): the release switch on N gt before
    /// each release, so R powers N gt early through the design's own feed path (1 <= N <= k).
    pub early_r: Option<i64>,
    /// The unpowered-release control (G1 (2)): the release switch held off for the first napper
    /// release, so R is never powered after it.
    pub hold_switch_off: bool,
    /// With `hold_switch_off`: a second release 10 gt after the held first one, so the first is
    /// counted at a later release (the mid-run never-powered path; G1's CHANGES on bc25c5cca (4)).
    pub hold_rerelease: bool,
    /// The world's day time at t 0 (ironspec 07:28 / G1 (5): the riders' start state asserted at t
    /// 1): 13,300 by default, a night (the riders' REST); a day start (6000) is the control and must
    /// fail.
    pub day0: i64,
    /// Riders summoned without `farm_rider` under --nap-ai (no HOME): the control of the t 1 HOME
    /// assertion, which must fail.
    pub skip_farm_rider: bool,
    /// The run's seed (`RunOpts::steady(seed, …)`; ironspec 09:21: ≥ 8 seeds before a timing figure
    /// is cited). Default 1.
    pub seed: i64,
    /// The designed approach (ironspec Nov 13 10:18 (2), G1 10:17 / 10:24): every returning cart keeps
    /// its own cart and rider (route history): it is PARKED off-track (an isolated flat rail at y 2)
    /// from its release until its return is due, then set on the split row upstream of J ((x-1,m+1,
    /// mz+2), flat east_west) moving east at v_in, through J (x,m+1,mz+2) north_west onto D and C, as
    /// the composed farm's carts arrive; the napper set on K's cell moving north at --nap-feed-v.
    /// Default off: the returns summoned in J's support onto D, each with a fresh rider.
    pub j_rail: bool,
    /// The lip raised this many levels over the lane (the depth family of the lip fix, ironspec Nov 13
    /// 10:56 (3)(i)); 0 = the design of record.
    pub lip_dy: i32,
    /// The C fix's S-bend (ironspec 10:29 / 10:56): c2 a straight east_west into the drop cell from the
    /// west with an east backstop, no c3, C a straight north_south (no westward exit from C).
    pub sbend_straight: bool,
    /// The forced-V SCENARIO (ironspec Nov 13 10:56 (3)): a ridden cart planted at V at this tick. A
    /// scenario only (G1 14:18 L4): no V-empty bar observes it here (the planted cart is not in the
    /// lane's carts, so lipclass, "V held" and the up-slope loop don't see it), and its PANIC is not
    /// read back; the row's own bars judge what follows.
    pub forced_v: Option<i64>,
    /// With `forced_v`: its rider kept in PANIC (a fresh hurt every 30 gt; not read back).
    pub forced_v_panic: bool,
    /// The lip volume for the descent (ironspec 11:08 (2) / 11:11), design frame: x0, x1, y0, y1, z0,
    /// z1; None = tick-exact's lane-3 swept volume (x 1.345..4.182, y m..m+0.7, z 5.940..8.157).
    pub lip_volume: Option<[f64; 6]>,
    /// The build position (ironspec Nov 13 10:49 (1)): the whole bench laid at the design frame plus
    /// this offset (multiples of 16 keep the chunk phase); every reading taken back to the design
    /// frame. (0,0,0) = the design frame.
    pub pos_offset: (i32, i32, i32),
    /// dfb89499's pick (12:26): R_1's pulse ends as soon as the lane's hold string (x,17,0) is pressed
    /// (the advancing cart at slot 1), at most `--r-pulse`.
    pub r_until_string: bool,
    /// G1's 12:32 edge control for the rollback's fallback bar: at tick T, the lane's highest resting
    /// slope cart moved up-slope by exactly D, at rest (D > 0.1 must count, D <= 0.1 must not).
    pub upslope_ctl: Option<(f64, i64)>,
    /// ironspec 12:52 (d), priced: c2 (x-1,m,mz-1) under --sbend-straight a powered rail east_west on a
    /// redstone block (always on: the napper's launch boosted into the drop cell from the west).
    pub c2_booster: bool,
    /// dfb89499's Events 4 (Nov 13 12:36): R_1 powered from the lane's trapdoor shut + D (the lanes'
    /// all-shut + 3: D = 27 - the lane's offset with offsets 0 / 12 / 24), for --r-pulse. 0 = at the shut.
    pub r_delay: i64,
    /// The module of record's branch (7b90c3f1's split row): D an ascending_south detector, the cells
    /// south of it solid (tick-exact's official S-bend row); default the bench's flat D.
    pub d_slope: bool,
    /// --out: write the bench's world as laid and settled (design format) to this path.
    pub out: Option<String>,
    /// --siding-north: leave out the lane-top walkway (glass (lx-3..lx-1, m-3, mz-2)), the control.
    pub no_walkway: bool,
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
    OFFSET.with(|o| o.set(so.pos_offset));
    let (pdx, pdy, pdz) = so.pos_offset;
    let mzc = |z: i32| if so.mirror_z { -1 - z } else { z };
    let mzf = |z: f64| if so.mirror_z { -z } else { z };
    let wp = |p: BlockPos| BlockPos::new(p.x + pdx, p.y + pdy, mzc(p.z) + pdz);
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
    // (the hold's trapdoor (x,16,0): dfb89499's station part, laid here as its stand-in; iron since
    // ironspec 04:34 (wind bursts flip wooden trapdoors; the same facing, half and signal))
    let trap_shut = b("minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = b("minecraft:iron_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string_ew = b("minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,south=false,west=true,powered=false]")?;
    let hook_e = b("minecraft:tripwire_hook[attached=true,facing=east,powered=false]")?;
    let hook_w = b("minecraft:tripwire_hook[attached=true,facing=west,powered=false]")?;
    let string_ns = b("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_sf = b("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_nf = b("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let det_ns = b("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]")?;
    let det_asc_s = b("minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]")?;
    let curve_sw = b("minecraft:rail[shape=south_west,waterlogged=false]")?;
    let repeater_n = b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    let repeater_e = b("minecraft:repeater[delay=1,facing=east,locked=false,powered=false]")?;
    let dust = b("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, blk: &Block| blocks.push((BlockPos::new(x + pdx, y + pdy, mzc(z) + pdz), blk.clone()));
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
    // (--lip-dy: the lip raised that many levels over the lane (ironspec Nov 13 10:56 (3)(i), the depth
    // family: a seated rider at V reaching 0.37 over the lip's floor in the game): the merge, the
    // branch, the siding, the split row and the drop entry all up, the lane and the hold where they are,
    // the drop column taller)
    let m = 18 + ns + so.drop_entry as i32 + so.lip_dy;
    let mz = ns + 2;
    if so.drop_entry {
        // (the backstop wall: glass, golem-proof (9fc3155f's flood: a stone wall's top is an off-pad
        // spawn spot seen from the lane tops))
        put(lx, m, mz - 2, &glass);
        // (--held-rider-control: the lip bar's positive control; its glass two cells under the lip is
        // laid at the napper's release (see the loop), so the lane runs as usual until then)
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
        // from the south both leave north into the intake), else a straight north_south flat; --d-slope:
        // the module of record's branch (7b90c3f1's split row): D an ascending_south detector rising to
        // the row's junction at level m + 1, the cells south of D solid (glass), as tick-exact's official
        // S-bend row lays it (the returns summoned at (x + 0.5, m + 0.0625, mz + 2.5) ride onto D's slope))
        let blk = if z == mz && so.merge_curve && !so.sbend_straight {
            &curve_nw
        } else if z == mz + so.guard_dz && so.guard_det && so.d_slope {
            &det_asc_s
        } else if z == mz + so.guard_dz && so.guard_det {
            &det_ns
        } else if so.d_slope && z == mz + so.guard_dz + 1 {
            &glass
        } else if so.d_slope && z > mz + so.guard_dz + 1 {
            // (record7's air south of the junction's support (cc767b78's record-match, ironspec 23:12))
            &air
        } else {
            &flat_ns
        };
        put(lx, m, z, blk);
        put(lx, m - 1, z, if so.d_slope && z > mz + so.guard_dz + 1 { &air } else { &glass });
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
            // (--j-rail: the designed approach: J (x,m+1,mz+2) north_west (the lane's routing shape: from
            // the west onto D's high end) on its glass; the split row's cell west of J is NOT laid, since
            // a rail there would make K a three-arm T without its tap lever (the record's K holds its
            // shape by its lever's power): the returns are set on J's west half moving east)
            if so.j_rail {
                put(lx, m + 1, mz + 2, &curve_nw);
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
            if so.sbend_straight {
                // (--sbend-straight (ironspec 10:29 / 10:56, the C fix): c2 a straight east_west into the
                // drop cell from the west, its east backstop (x+1,m,mz-1); no c3, so C is the branch's own
                // straight and no cart can leave C westward); --c2-booster (ironspec 12:52 (d), priced): c2 a
                // powered rail on a redstone block, always on
                if so.c2_booster {
                    put(sx + 1, m, mz - 1, &Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]").map_err(|e| e.to_string())?);
                    put(sx + 1, m - 1, mz - 1, &Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?);
                } else {
                    put(sx + 1, m, mz - 1, &flat_ew);
                    put(sx + 1, m - 1, mz - 1, &glass);
                }
                put(lx + 1, m, mz - 1, &glass);
            } else {
                put(sx + 1, m, mz - 1, &curve_sw);
                put(sx + 1, m - 1, mz - 1, &glass);
                put(sx + 1, m, mz, &curve_ne);
                put(sx + 1, m - 1, mz, &glass);
            }
            // (the lane-top walkway (ironspec 16:12, the late-path fix for lanes 3 and 7): glass
            // (lx-3..lx-1, 16+N, mz-2) (the lane's level, m - 3 without --lip-dy), as the S-bend file lays
            // it, so the bench's cells are the file's; --no-walkway, the control)
            if !so.no_walkway {
                for d in 1..=3 {
                    put(lx - d, 16 + ns, mz - 2, &glass);
                }
            }
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
    // (--j-rail: the parking spots, one isolated flat rail on glass per cart, 2 apart at y 2 east of
    // the lane (x+5, 2, -5 + 2c): a returning cart waits there with its rider from its release until its
    // return is due)
    if so.j_rail {
        for c in 0..o.per_lane as i32 {
            put(lx + 5, 2, -5 + 2 * c, &flat_ns);
            put(lx + 5, 1, -5 + 2 * c, &glass);
        }
    }
    let lo = if so.ai { BlockPos::new((lx - 6).min(BED.0 - 3), (-12).min(BED.1 - 3), -6) } else { BlockPos::new(lx - 6, -12, -6) };
    // (the bench's box in the world's frame: its z range mirrored with --mirror-z)
    let (lo, hi) = {
        let hi = BlockPos::new(lx + 6, 30, 14);
        let (za, zb) = (mzc(lo.z), mzc(hi.z));
        (BlockPos::new(lo.x + pdx, lo.y + pdy, za.min(zb) + pdz), BlockPos::new(hi.x + pdx, hi.y + pdy, za.max(zb) + pdz))
    };
    let spec = FarmSpec { blocks, entities: Vec::new(), output: wp(BlockPos::new(lx + 6, 30, 12)), lo, hi, block_entities: Vec::new(), rock: false, nether: None, land: false };
    // (ironspec's 11:05 audit, nothing silently dropped: the composer's world is the spec's box,
    // and a raw placement outside it would be skipped; every bench cell inside it, or the run fails)
    let outside: Vec<String> = spec.blocks.iter().filter(|(p, _)| p.x < lo.x || p.y < lo.y || p.z < lo.z || p.x > hi.x || p.y > hi.y || p.z > hi.z).map(|(p, b)| format!("{},{},{} {b}", p.x, p.y, p.z)).collect();
    if !outside.is_empty() {
        return Err(format!("siding bench: {} cells outside the world's box {lo:?}..{hi:?}: {}", outside.len(), outside.join("; ")));
    }
    // (ironspec 00:27: every bench schedules every fluid cell's tick and prints the count; this bench
    // lays no fluid and schedules none, so a fluid cell here is an error, never a silently unticked cell)
    let fluid_cells = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::Water { .. } | Block::Lava { .. }) || b.to_string().contains("waterlogged=true")).count();
    if fluid_cells > 0 {
        return Err(format!("siding bench: {fluid_cells} fluid cells laid, and this bench schedules no fluid ticks"));
    }
    let ro = if so.ai { RunOpts { ai: true, day0: so.day0, ..RunOpts::steady(so.seed, 0, WalkMode::Random) } } else { RunOpts { ai: false, day0: so.day0, ..RunOpts::steady(so.seed, 0, WalkMode::Random) } };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    // (--out: the bench's world as laid and settled, every non-air cell of its box in the design
    // format, the lanes file's loop frame (lane x 3, the merge at m 24, mz 7) for `mobfarm
    // record-match` (ironspec 21:43: a bench row counts only when its cells match the design of
    // record's; the flat D (--flat-d) is a positive control: its D and (x,24,9) differ))
    if let Some(path) = so.out.as_deref() {
        let w = &run.world;
        let mut cells: std::collections::BTreeMap<(i32, i32, i32), String> = std::collections::BTreeMap::new();
        for dx in 0..w.size[0] {
            for dy in 0..w.size[1] {
                for dz in 0..w.size[2] {
                    let p = BlockPos::new(w.min.x + dx, w.min.y + dy, w.min.z + dz);
                    let b = w.get(p);
                    if !matches!(b, Block::Air) {
                        // (the design frame: the build offset removed, z mirrored back under --mirror-z)
                        let z = if so.mirror_z { -(p.z - pdz) } else { p.z - pdz };
                        cells.insert((p.x - pdx, p.y - pdy, z), b.to_string());
                    }
                }
            }
        }
        let mut states: Vec<String> = cells.values().cloned().collect();
        states.sort();
        states.dedup();
        let glyphs: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&*+-/:;<=>?@^_~".chars().collect();
        if states.len() > glyphs.len() {
            return Err(format!("--out: {} block states, more than the {} glyphs", states.len(), glyphs.len()));
        }
        let ch = |s: &str| glyphs[states.iter().position(|x| x == s).unwrap_or(0)];
        let (x0, x1) = (cells.keys().map(|k| k.0).min().unwrap_or(0), cells.keys().map(|k| k.0).max().unwrap_or(0));
        let (y0, y1) = (cells.keys().map(|k| k.1).min().unwrap_or(0), cells.keys().map(|k| k.1).max().unwrap_or(0));
        let (z0, z1) = (cells.keys().map(|k| k.2).min().unwrap_or(0), cells.keys().map(|k| k.2).max().unwrap_or(0));
        let mut out = format!(
            "# escape-bench --siding's world as laid and settled (--out; bench, tool unreviewed): the lanes file's loop frame, lane x {lx}, the merge at m {m}, mz {mz}; {}\nname: escape-bench-siding\ncircuit: mob-farm iron_ingot\norigin: {x0} 0 {z0}\npalette:\n",
            if so.d_slope { "D ascending_south, glass south of it (the module's geometry)" } else { "the flat-D control (--flat-d)" }
        );
        for s in &states {
            out.push_str(&format!("  {} {}\n", ch(s), s));
        }
        for y in y0..=y1 {
            out.push_str(&format!("layer {y}:\n"));
            for z in z0..=z1 {
                let row: String = (x0..=x1).map(|x| cells.get(&(x, y, z)).map_or('.', |s| ch(s))).collect();
                let row = row.trim_end_matches('.');
                out.push_str(&format!("  {}\n", if row.is_empty() { "." } else { row }));
            }
        }
        std::fs::write(path, out).map_err(|e| format!("--out {path}: {e}"))?;
    }
    // (poi::take, checked like an override (ironspec 07:28 C8): the bed's free tickets before -> after)
    let mut take_note = String::new();
    if so.ai {
        let before = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed);
        redsim::entity::mob::ai::poi::take(&mut run.world, bed);
        let after = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed);
        take_note = format!("; poi::take's free tickets before -> after: {} -> {}", before.map_or("-".to_string(), |n| n.to_string()), after.map_or("-".to_string(), |n| n.to_string()));
        if before == after {
            return Err(format!("--nap-ai: poi::take left the bed's free tickets at {before:?} (a no-op override, ironspec 04:07)"));
        }
    }
    // (the riders at t 1 (read in the loop) and riders without the bed's HOME there (--nap-ai))
    let mut t1_line = String::from("the riders at t 1 (ironspec 07:28, read from the world): not read (the run ended before t 1)");
    let mut t1_unhomed = 0usize;
    let mut t1_not_rest = 0usize;
    let mut t1_no_riders = false;
    let mut t1_ticket_bad = false;
    // (ironspec 06:42 real, the redsim HOME-ticket gap (redsim's summon with HOME takes the bed's
    // ticket; the game's loaded HOME takes none): the bed's free tickets and holders at the start,
    // and how this bench stands with it)
    let bed_start_line = if so.ai {
        format!(
            "the bed at the start (ironspec 06:42 real, the HOME-ticket gap): START STATE (ii) (G1: the design start (i) leaves the bed's ticket free): the shared bed ({},{},{}) laid, its one ticket taken once by the bench at the start (poi::take, a declared stand-in{take_note}); free tickets after it {}; holders: read from the riders' brains at t 1 (the riders line), their HOME set in the brain at their spawn (farm_rider), no HOME NBT, so no summon takes a ticket (the gap doesn't arise)",
            BED.0,
            BED.1,
            BED.2,
            redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed).map_or("- (no POI)".to_string(), |n| n.to_string())
        )
    } else {
        "the bed at the start (ironspec 06:42 real, the HOME-ticket gap): no bed laid; the riders are summoned with no HOME (Passengers: a villager, PersistenceRequired only), so no ticket exists or is taken (the gap doesn't arise)".to_string()
    };
    // (the scarer posts: a zombie seated in a boat (it stays put), persistent)
    for p in &so.posts {
        spawn_off(&mut run.world, "minecraft:oak_boat", [p.0 as f64 + 0.5, p.1 as f64, mzf(p.2 as f64 + 0.5)], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b,IsBaby:0b}]}"))?;
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
    let drive_delay: i64 = drive_delay_of(so);
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
            let id = spawn_off(&mut run.world, "minecraft:minecart", [zc, y, mzf(z)], Some(&nbt0))?;
            if so.ai && !so.skip_farm_rider {
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
        let id = spawn_off(&mut run.world, "minecraft:minecart", [zc, slope_y(z), mzf(z)], Some(&nbt0))?;
        if so.ai && !so.skip_farm_rider {
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
    let mut r_from: i64 = i64::MIN;
    // (R_1's on and off ticks; the refill's first cart-cart contact in slots 1-2 after each R_1 on (G1
    // 12:26: the detector-ended pulse must end before the second cart reaches the front cart); the
    // up-slope motions of lane carts on the slopes (day-1 12:26's bar: no cart moves up-slope))
    let (mut r_on_at, mut r_off_at, mut refill_contacts): (Vec<i64>, Vec<i64>, Vec<(i64, i64)>) = (Vec::new(), Vec::new(), Vec::new());
    let mut refill_pending: Option<i64> = None;
    // (ironspec 13:04, the calibration against drop-bench --refill 2,10: per R_1 on, the ticks after it
    // at which the next cart first moves (z 0.8..2.6, speed over 0.001), first leaves R_1's cell north
    // (z under 1.0) and first rests in the hold (z under 0.6, speed under 0.001); dfb89499's row: +1 /
    // +4 / +13 from the rail's power)
    #[allow(clippy::type_complexity)]
    let mut refill_timing: Vec<(i64, Option<i64>, Option<i64>, Option<i64>)> = Vec::new();
    let mut refill_on_r1: Vec<Vec<usize>> = Vec::new();
    let (mut upslope_any, mut upslope_big): (usize, usize) = (0, 0);
    let mut upslope_max: (f64, i64, usize) = (0.0, 0, 0);
    // (the napper's launches: (tick, its rest z on R before, its step entering C or the drop cell))
    let mut nap_launches: Vec<(i64, Option<f64>, f64)> = Vec::new();
    let mut launch_rest_z: Option<f64> = None;
    let mut launch_in: Vec<bool> = vec![false; carts.len()];
    let mut prev_now: Vec<Option<(f64, f64, f64, f64)>> = vec![None; carts.len()];
    // (G1 12:32's rollback fallback bar: each lane cart's last rest z on the slopes and whether its
    // current excursion was counted; the counted excursions (tick, cart, displacement); the edge
    // control's plant)
    let mut last_rest_z: Vec<Option<(f64, bool)>> = vec![None; carts.len()];
    let mut rollbacks: Vec<(i64, usize, f64)> = Vec::new();
    let mut upslope_planted: Option<(i64, usize, f64)> = None;
    let mut upslope_plant_note = String::new();
    let mut slope_rest_samples: usize = 0;
    let (mut s_until, mut s_is) = (-1i64, false);
    let mut held = 0i64;
    let mut drops: Vec<(i64, usize)> = Vec::new();
    let mut per_release: Vec<usize> = Vec::new();
    let mut fell = vec![false; n];
    // (--j-rail: each cart's parked id between its release and its return)
    let mut parked: Vec<Option<u32>> = vec![None; n];
    // (G1 10:24: every return's first tick after it is set on the track (the summoned start or the
    // J-rail set): (cart, the tick it was set), read back one tick later)
    let mut set_returns: Vec<(usize, i64)> = Vec::new();
    let mut first_ticks: Vec<(f64, f64, f64, f64)> = Vec::new();
    // (ironspec 10:42 (4), G1 10:29 / 10:36: the drop passes: per cart in the drop region (the drop
    // cell and C's north half: x lx..lx+1, z mz-1.5..mz+0.6, y 22.5..25) its ticks off its rail, its
    // largest lateral displacement from the lane's centre line, whether a cart was queued behind it (at
    // C, D or J) while it was off its rail; and every arrival at C (the first tick in C's cell) for the
    // gaps between arrivals)
    let mut lip_open: Vec<Option<(i64, u32, f64, bool)>> = vec![None; n];
    let mut lip_passes: Vec<(i64, usize, u32, f64, bool)> = Vec::new();
    let mut in_c: Vec<bool> = vec![false; n];
    let mut c_arrivals: Vec<(i64, usize, f64, f64)> = Vec::new();
    // (the leader's descent from C past V (ironspec 11:05, day-1's counting answer): each arrival at C
    // (tick, cart, the queue depth then) until the cart's centre is under z 5.7 below y 23 (its rider's
    // box under z 6.0: out of V); done: (depth, gt); cleared never (still at V or above at the run's end
    // or its next release))
    let mut descents: Vec<(i64, usize, usize, i64, Option<i64>)> = Vec::new();
    let mut descents_done: Vec<(usize, i64, i64)> = Vec::new();
    // (the lip arrivals and their classes: (tick, cart, others in the slots, gap to the previous lip
    // arrival, the occupants of the lip volume with their kind, the class), from the shared
    // `mobfarm::lipclass` tracker of this lane's lip (in the design frame; the lip volume
    // --lip-volume, else tick-exact's lane-3 volume relative to the lane))
    #[allow(clippy::type_complexity)]
    let mut lip_arrivals: Vec<(i64, usize, usize, Option<i64>, Vec<(usize, &'static str)>, String)> = Vec::new();
    let mut lip_lane = crate::lipclass::LipLane::new(lx, m, mz, so.lip_volume);
    // (each lip arrival's entry step (its horizontal move that tick) and source (the napper or a batch
    // cart), G1 12:29 (3): the slow-entry class and the least clearing speed per source)
    let mut lip_steps: Vec<(f64, bool)> = Vec::new();
    let mut last_arrival_of: Vec<Option<usize>> = vec![None; n];
    let mut off_rest_class: Vec<(i64, usize, String)> = Vec::new();
    let mut in_j: Vec<bool> = vec![false; n];
    let mut j_settles: Vec<(i64, usize, usize)> = Vec::new();
    let mut settles_done: Vec<(usize, i64)> = Vec::new();
    let mut rider_top_on_r: Option<(f64, f64, f64, usize, f64)> = None;
    let mut unridden: Vec<(i64, usize)> = Vec::new();
    let mut forced_v_id: Option<u32> = None;
    let lip_trace: Option<(i64, i64)> = std::env::var("MCTAS_LIP_TRACE").ok().and_then(|s| s.split_once("..").and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))));
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
    // (each napper's merge entry, for the follower's margin at the run's end: the next cart into the
    // merge after the napper, its entry minus the napper's clear (ironspec 19:04 (b)))
    let mut nap_merges: Vec<(usize, i64)> = Vec::new();
    // (the lip margin's tracks (ironspec 21:14 (2)): every rider's box near the drop's lip and the
    // napper's centre there, positions after each tick, design frame)
    let mut rider_track: Vec<(i64, usize, f64, f64, f64, f64, f64)> = Vec::new();
    let mut nap_track: Vec<(i64, usize, f64, f64)> = Vec::new();
    // (each napper's lip outcome; Err: no lip sample (no cart ahead of it at the merge, or no merge
    // entry), its own class, never counted as clean (G1's CHANGES 1 on a191f3dfb))
    let mut lip_qs: Vec<Result<Lip, &'static str>> = Vec::new();
    // (the carts' block contacts (ironspec 21:25 (4)), by the shared definition
    // `crate::contacts::cart_contacts` (ironspec 07:47's amendment: an axis the game's collision clipped
    // of the cart's attempted move); a declared stop passes, any other fails the row. Declared (ironspec
    // 04:14, non-blocking: from the design, not a hard-coded list): the lanes file's `# contacts:` cells
    // for this lane (the drop entry's backstop), read STRICTLY from `lanes_design`'s own header (G1's
    // CHANGES 3 (3): a read failure is an error); and the station's cells this bench lays as labelled
    // stand-ins: the bench's end of track (x,16,-1)
    // and (x,17,-1) (not in record7: the batch cart leaving the hold stops there before the bench takes
    // it out). Every declared cell must be hit (G1's CHANGES 3 (2): a never-hit declaration is STALE and
    // fails), unless this row's scope can't reach it (printed as such). The hold's trapdoor (x,16,0) is
    // not declared: the batch cart drops through it and rests on it as a FLOOR (a vertical collision),
    // never a horizontal stop in this bench (the amended rule's run on 0.30 @ 800: 0 hits), so a
    // declaration there would be stale)
    // (the declared contacts come from the design of record's header, whatever support this run
    // lays under R: the `# contacts:` line doesn't depend on it, and a control's support (glass) is no
    // design the lanes file can emit, so the design's own slime support is read when the run's isn't
    // one the file has (the glass control's row errored on 329b5acc3's strict read))
    let design_support = so.r_support.as_deref().filter(|s| s.ends_with("slime_block") || s.ends_with("stone")).or(Some("minecraft:slime_block"));
    let lanes_text = lanes_design(ns, so.siding_north, false, design_support, None, so.sbend_straight).map_err(|e| format!("the lanes file's # contacts: can't be read: {e}"))?;
    let lanes_contacts: Vec<(i32, i32, i32)> = crate::contacts::declared_contacts_strict(&lanes_text)?.into_iter().map(|(p, _)| (p.x, p.y, p.z)).filter(|c| c.0 == lx).collect();
    let stand_in_stops: Vec<(i32, i32, i32)> = vec![(lx, 16, -1), (lx, 17, -1)];
    let declared_stops: Vec<(i32, i32, i32)> = lanes_contacts.iter().chain(stand_in_stops.iter()).copied().collect();
    let mut block_contacts: std::collections::BTreeMap<(i32, i32, i32), (String, usize)> = std::collections::BTreeMap::new();
    let mut early_contacts: std::collections::BTreeMap<(i32, i32, i32), (String, usize)> = std::collections::BTreeMap::new();
    // (each napper's PRESENT margin before its leader's press (ironspec 21:14 (1)): the leader's press
    // tick minus the napper's PRESENT tick (its K tick + --nap-present-k); under 7: outside normal
    // operation)
    let mut present_margins: Vec<i64> = Vec::new();
    let mut touching = 0usize;
    // (two carts over the merge with a napper among them: its leader (while the leader's rider is up,
    // and after it dropped), or another cart)
    let (mut touch_leader, mut touch_leader_late, mut touch_follower) = (0usize, 0usize, 0usize);
    let mut last_pass: Vec<Option<i64>> = vec![None; n];
    let mut lap_wait = false;
    let mut nap_since = 0i64;
    let mut late_naps = 0usize;
    let mut nap_rested_at: Option<i64> = None;
    // (day-1's siding settle, PRESENT's physical basis: each napper's first tick on R's cell to its
    // first tick there at speed < 0.001)
    let (mut nap_on_r, mut nap_settled, mut settles): (Option<i64>, bool, Vec<i64>) = (None, false, Vec::new());
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
    // (--nap-present-k (ironspec 17:58 / 18:06, day-1's PRESENT moved to K + 2): the napper's K tick,
    // and each release's state: (release minus the K tick, at rest on R or still rolling, z, speed))
    let mut nap_k_at: Option<i64> = None;
    // (push-scan's pairs R (x-2,24,7) <- K (x-2,25,9): a cart turning at K into F against a cart on R
    // (ironspec 19:58): the previous napper's departure from R, each next napper's K tick minus it, and
    // the ticks with a cart on R while another turned at K or rode F)
    let mut nap_departed_at: Option<i64> = None;
    let mut nap_gaps: Vec<i64> = Vec::new();
    let mut r_joint = 0usize;
    // (the R <- K cell transition: each cart's cell among K / F / R the previous tick, and the carts
    // that moved out of K into F or R)
    let mut kfr_prev: std::collections::HashMap<u32, char> = std::collections::HashMap::new();
    let mut k_turns = 0usize;
    // (the resting napper across the K row's passes (ironspec 23:15 (2), 23:51: push-scan's pairs R
    // (x-2,24,7) <- K (x-2,25,9) close by measurement): while a napper rests on R, every K-row cart
    // within 1.5 blocks of K's centre in x on the row is a pass; the napper's displacement from where it
    // came to rest and its speed over the passes (redsim's AbstractMinecart.push, its 0.8-alignment
    // rule included))
    let (mut push_passes, mut push_disp_max, mut push_v_max) = (0usize, 0.0f64, 0.0f64);
    let mut push_rest_ref: Option<(f64, f64)> = None;
    let mut push_in_pass = false;
    // (the push bar's computed scope: every K-row pass tick and each napper's rest window (at rest ->
    // release, or the run's end); the injected pass of the push grid)
    let mut k_pass_ticks: Vec<i64> = Vec::new();
    let mut rest_windows: Vec<(i64, i64)> = Vec::new();
    // (the napper's spans in R's cell: (cart, first tick, last tick))
    let mut r_windows: Vec<(usize, i64, i64)> = Vec::new();
    let mut inject_pass_done = false;
    // (the napper's z at the injected pass (ironspec 04:14 (3c): the grid row's rest z))
    let mut inject_rest_z: Option<f64> = None;
    let mut pass_overlap = 0usize;
    // (the design's own timing for the PRESENT bar (ironspec 05:03): the release to R powered (the
    // feed delay), and the leader's press to the napper's release (k_sw), each measured)
    let mut feed_wait: Option<i64> = None;
    let (mut feed_meas, mut ksw_meas): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
    // (the napper at rest when R powers (ironspec 06:15 real): the releases judged, and the ticks R
    // powered with the napper not at rest)
    let mut power_judged = 0usize;
    let mut not_rest_at_power: Vec<i64> = Vec::new();
    // (the napper at rest at the end of the previous tick; --early-r's switch-on ticks (the release
    // switch on N gt before each release, the real path's power shifted))
    let mut nap_rest_prev = false;
    let mut early_switch_at: Vec<i64> = Vec::new();
    // (the release -> R powered measured from the switch-on (the design's own delay) and from the
    // release, and the releases R never powered after (G1 (2)))
    let mut switch_on_at: Option<i64> = None;
    let mut feed_from_switch: Vec<i64> = Vec::new();
    let mut r_never_powered: Vec<i64> = Vec::new();
    // (per release: the napper's rest tick and R's first power tick, for the tie)
    let mut rest_power_ticks: Vec<(Option<i64>, i64)> = Vec::new();
    let mut nap_rest_tick: Option<i64> = None;
    let mut nap_release_n = 0usize;
    let mut rereleased = false;
    // (the bench's run-time overrides, each cell's state before and after (ironspec 04:07))
    let mut overrides = Overrides::default();
    // (r_joint's positive control's cart, once spawned)
    let mut control_cart: Option<u32> = None;
    let mut release_states: Vec<(i64, bool, f64, f64)> = Vec::new();
    let mut transits: Vec<i64> = Vec::new();
    let mut pass_of: Vec<Option<i64>> = vec![None; n];
    let (mut d_to_entry, mut d_to_clear): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
    let mut queue_rest: std::collections::BTreeMap<usize, Vec<Vec<(f64, f64, f64)>>> = Default::default();
    let mut first_snaps: Vec<(i64, Vec<(f64, f64, f64, f64)>)> = Vec::new();
    let (mut occ_max, mut occ8, mut occ_start_max) = (0usize, 0usize, 0usize);
    let mut occ_min = usize::MAX;
    // (releases from the steady state with the lane's occupancy below 6: rotation below 7, the
    // physical floor (ironspec 19:29 (4): a summoner's GOLEM_DETECTED_RECENTLY lasts 600 gt, so with
    // 6 in rotation its next turn comes inside it; the official rows lost 49 of 1,280 summons at 6))
    let mut occ_low = 0usize;
    // (the samples behind the bars ironspec 04:14 (1) named: the releases the occupancy bars judge,
    // the steady-window ticks the V bar judges, the nappers with a curve rule, the followers with a
    // figure)
    let (mut occ_samples, mut v_samples, mut curve_samples, mut follow_samples) = (0usize, 0usize, 0usize, 0usize);
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
            ids.push(spawn_off(&mut run.world, "minecraft:minecart", [f.0 as f64 + 0.5, f.1 as f64 + 0.5, mzf(f.2 as f64 + 0.5)], None)?);
            for _ in 0..10 {
                run.tick();
            }
        }
        ids.push(spawn_off(&mut run.world, "minecraft:minecart", [seat.0 as f64 + 0.5, seat.1 as f64 + 0.5, mzf(seat.2 as f64 + 0.5)], Some(&nbt0))?);
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
        // (ironspec 07:28: the riders' state at t 1, read from the world: the day time, every
        // villager's active activity and HOME (with --nap-ai, the bed's holders counted from the
        // brains, not stated), and the bed's free tickets)
        if t == 1 {
            let mut acts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            let (mut vill, mut homed, mut other_home) = (0usize, 0usize, 0usize);
            for e in run.world.entities.list.iter().filter(|e| !e.removed) {
                if let redsim::entity::EntityKind::Mob(mb) = &e.kind
                    && let Some(b) = mb.brain.as_deref()
                {
                    vill += 1;
                    *acts.entry(format!("{:?}", b.activity)).or_default() += 1;
                    match b.home {
                        Some(h) if h == bed => homed += 1,
                        Some(_) => other_home += 1,
                        None => {}
                    }
                }
            }
            let free_now = if so.ai { redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bed) } else { None };
            t1_line = format!(
                "the riders at t 1 (ironspec 07:28, read from the world): day time {}; villagers {vill}; their active activities {}; HOME at the bed ({},{},{}) {homed}, elsewhere {other_home}, none {}; the bed's free tickets {}",
                run.world.day_time(),
                if acts.is_empty() { "none".to_string() } else { acts.iter().map(|(k, n)| format!("{k} x{n}")).collect::<Vec<_>>().join(", ") },
                BED.0,
                BED.1,
                BED.2,
                vill - homed - other_home,
                if so.ai { free_now.map_or("- (no POI)".to_string(), |n| n.to_string()) } else { "- (no bed laid)".to_string() }
            );
            // (asserted, not only printed (G1's CHANGES on bc25c5cca (5)): every rider at REST (the
            // farm's night start; a day start is the control and fails), riders present when the run
            // has them (0 villagers: NOT JUDGED), and under --nap-ai every rider holding the bed as
            // HOME from its spawn (START STATE (ii)) and the bed's one ticket taken (free 0))
            t1_not_rest = acts.iter().filter(|(k, _)| k.as_str() != "Rest").map(|(_, n)| *n).sum();
            t1_no_riders = o.riders && vill == 0;
            if so.ai {
                t1_unhomed = vill - homed;
                t1_ticket_bad = free_now != Some(0);
            }
        }
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
                // (dated at the tick that pressed it: the state read here, before this tick runs, is
                // the previous tick's, so the press happened in t - 1 (the controller's K counts from
                // the press: SIDING_l at the press + K, R at the press + 12 with the 8-gt feed;
                // tick-exact's official boundary row, ironspec 20:40))
                last_pass[c] = Some(t - 1);
                last_any_pass = Some((t - 1, c));
                pass_of[c] = Some(t - 1);
            }
            // (--nap-late-latch: the lane's window pass with its napper picked but not yet waiting on
            // the siding sets the late latch)
            if so.late_latch
                && so.ring > 0
                && nap.is_some()
                && released_at.is_none()
                && latch_at.is_none()
                && window_open(t - 1, &rel_times).is_some()
                && !(match so.present_k {
                    Some(d) => nap_k_at.is_some_and(|k| k + d <= t - 1),
                    None => nap_rested_at.is_some_and(|r| r <= t - 1),
                })
            {
                latch_at = Some(t - 1);
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
            if so.inject_pass.is_none()
                && !entering
                && frozen32.is_none()
                && let Some(&rt) = rel_times.last()
                && so.k_offsets.contains(&(t - rt))
            {
                // (each through-cart at its own x in the row's west cell (G1, the S-bend's margin): a
                // cart entering K's north_east curve from the west at x fraction xf is projected to z
                // (mz + 2) + xf / 2 (moveAlongTrack's curve projection), so the spawn x steps 0.0371
                // through 0.9 of the cell and the entry fractions cover the whole [0, v) range)
                let x0 = (sx - 2) as f64 + 0.05 + (th_spawned as f64 * 0.0371).rem_euclid(0.9);
                let id = spawn_off(&mut run.world, "minecraft:minecart", [x0, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(nr))?;
                if so.ai && !so.skip_farm_rider {
                    farm_rider(&mut run.world, id, bed, None);
                }
                through.push((id, t));
                th_spawned += 1;
            }
            // (--push-grid-run's injected pass (ironspec 03:00): one K-row cart at the grid's speed and
            // entry x through the row's west cell, `delay` gt after the napper comes to rest, the K
            // row's own spawns off; planted (the grid's positive control THROUGH the K-zone gate,
            // ironspec 04:14 (4)): a cart in K's cell near its north edge, counted as a K-row pass like
            // any other (the grid's speed field is unused for it). Its constants (ironspec 07:28 C6):
            // z frac 0.14, inside K's cell (z > 9: still in K's zone) with its box front 0.49 north of
            // its centre; x frac 0.64 = 0.5 + 0.14, on the north_east curve's own line (the rail's path
            // from the north exit (0.5, 0) to the east exit (1, 0.5), x - 0.5 = z frac), so the snap
            // doesn't move it; rolling north at 0.05 b/gt: above the 0.0316 b/gt a cart's heading needs
            // (a horizontal move squared over 0.001) so it heads north and AbstractMinecart.push's
            // 0.8-alignment admits the napper due north, and slow enough to stay in K's zone 3 gt
            // (0.14 / 0.05) while its push box meets the napper's; where it reaches (the table's
            // planted column) the napper rests past z ~7.86)
            if let Some((iv, frac, delay, planted)) = so.inject_pass
                && !inject_pass_done
                && released_at.is_none()
                && nap_rested_at.is_some_and(|r| t == r + delay)
            {
                inject_rest_z = nap.and_then(|(nc, _)| carts[nc]).and_then(|id| st(&run.world, id)).map(|p| p.2);
                let id = if planted {
                    spawn_off(&mut run.world, "minecraft:minecart", [sx as f64 + 0.64, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.14)], Some(&format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", vz(-0.05))))?
                } else {
                    let x0 = (sx - 2) as f64 + 0.05 + frac.clamp(0.0, 1.0) * 0.9;
                    spawn_off(&mut run.world, "minecraft:minecart", [x0, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(&format!("{{Motion:[{iv:.4}d,0.0d,0.0d]{rider}}}")))?
                };
                if so.ai && !so.skip_farm_rider {
                    farm_rider(&mut run.world, id, bed, None);
                }
                through.push((id, t));
                th_spawned += 1;
                inject_pass_done = true;
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
        // (the napper present for the sample: at rest on R (the bench's default), or with
        // --nap-present-k N from N gt after its K tick whether or not it has come to rest)
        let present = match so.present_k {
            Some(d) => nap_k_at.is_some_and(|k| t >= k + d),
            None => nap_ready,
        };
        let present_at = match so.present_k {
            Some(d) => nap_k_at.map(|k| k + d),
            None => nap_rested_at,
        };
        if so.any_pass || so.ring > 0 {
            // (--inject-release T: the boundary probe (ironspec 19:52): the napper's release forced at
            // tick T, before or after its leader's pass, whatever the window and PRESENT say)
            if let Some(ti) = so.inject_release
                && t == ti
                && nap.is_some()
                && released_at.is_none()
            {
                s_until = t + so.nap_pulse;
                released_at = Some(t);
                if let Some((nc, _)) = nap
                    && let Some(p) = carts[nc].and_then(|id| st(&run.world, id))
                {
                    release_states.push((nap_k_at.map_or(-1, |k| t - k), !nap_ready, p.2, p.3));
                }
                pred_pass = last_any_pass.map(|(pp, _)| pp);
                release_lead = last_any_pass.map(|(_, pc)| pc);
            }
            // (--early-r N (G1 / ironspec 07:31): the release switch on N gt before the release tick,
            // through the release's pulse, so R powers N gt early through the design's own feed path;
            // the release itself (released_at, the bench's bookkeeping) stays at its tick)
            if let Some(en) = so.early_r
                && so.inject_release.is_none()
                && nap.is_some()
                && released_at.is_none()
                && present
                && (so.no_d_gate || !guard_on)
                && let Some((pp, _)) = last_any_pass
                && t == pp + so.k - en
                && present_at.is_some_and(|r| r <= pp)
                && via.is_some()
                && !early_switch_at.contains(&t)
            {
                s_until = pp + so.k + so.nap_pulse;
                early_switch_at.push(t);
            }
            if so.inject_release.is_none()
                && nap.is_some()
                && released_at.is_none()
                && present
                && (so.no_d_gate || !guard_on)
                && let Some((pp, pc)) = last_any_pass
                && t >= pp + so.k
                && present_at.is_some_and(|r| r <= pp)
                && let Some(wj) = via
            {
                s_until = t + so.nap_pulse;
                released_at = Some(t);
                if let Some((nc, _)) = nap
                    && let Some(p) = carts[nc].and_then(|id| st(&run.world, id))
                {
                    release_states.push((nap_k_at.map_or(-1, |k| t - k), !nap_ready, p.2, p.3));
                }
                pred_pass = Some(pp);
                release_lead = Some(pc);
                release_window = (wj != usize::MAX && wj != LATCH).then_some(wj);
                released_by_latch = wj == LATCH;
            }
        } else if let Some((nc, pred)) = nap
            && released_at.is_none()
            && nap_ready
            && (so.no_d_gate || !guard_on)
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
        // (--hold-switch-off-rerelease (G1's CHANGES on bc25c5cca (4)): a second release 10 gt after
        // the first, held one, with the switch working: the first, still waiting for R, is then counted
        // at this release (the mid-run path))
        if so.hold_switch_off
            && so.hold_rerelease
            && !rereleased
            && nap_release_n == 1
            && let Some(r0) = released_at
            && t == r0 + 10
        {
            released_at = Some(t);
            s_until = t + so.nap_pulse;
            rereleased = true;
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
                occ_samples += 1;
                occ_max = occ_max.max(lane_now.len());
                occ_min = occ_min.min(lane_now.len());
                if lane_now.len() < 6 {
                    occ_low += 1;
                }
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
            overrides.set(&mut run.world, wp(BlockPos::new(lx, 16, 0)), if want_trap { &trap_open } else { &trap_shut }, "the hold's trapdoor (x,16,0)", false);
            if !want_trap {
                // (dfb89499's Events 4 (12:36): R_1 powered from the lanes' all-shut + 3 for 10 gt: a lane
                // whose own trapdoor shuts at offset o sees R_1 from its shut + (27 - o) with offsets
                // 0 / 12 / 24; --r-delay D lays that, the pulse --r-pulse)
                r_from = t + so.r_delay;
                r_until = r_from + o.r_pulse;
            }
        }
        let want_r = t >= r_from && t < r_until;
        // (--r-until-string, dfb89499's pick 12:26: R_1 off once the lane's hold string (x,17,0) is
        // pressed, the advancing cart at slot 1; at most the pulse)
        let cut = so.r_until_string && r_is && matches!(run.world.get(wp(BlockPos::new(lx, 17, 0))), Block::Tripwire { powered: true, .. });
        if cut {
            // (G1 14:18 L5: the cut ends this pulse; R_1 can't come back while the string stays
            // pressed)
            r_until = t;
        }
        let want_r = want_r && !cut;
        if !want_r && r_is {
            r_off_at.push(t);
        }
        if want_r && !r_is {
            r_on_at.push(t);
            refill_pending = Some(t);
            refill_timing.push((t, None, None, None));
            // (the carts at rest on R_1's cell (z 1..2) at the power, from the last tick's end: the
            // next cart, whose first move the calibration times)
            refill_on_r1.push(
                prev_now
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.is_some_and(|(x, _, z, v)| x >= lx as f64 && x < (lx + 1) as f64 && (1.0..2.0).contains(&z) && v < 0.001))
                    .map(|(c, _)| c)
                    .collect(),
            );
        }
        if want_r != r_is {
            r_is = want_r;
            overrides.set(&mut run.world, wp(BlockPos::new(lx + 1, 17, 1)), if want_r { &rblock } else { &air }, "R_1's switch (x+1,17,1)", false);
        }
        // (--hold-switch-off (G1 (2)'s control): the switch held off through the first napper release;
        // `nap_release_n` counts that release later in this iteration, hence the tick test)
        let first_release = released_at.is_some() && nap_release_n + (released_at == Some(t)) as usize == 1;
        let want_s = t < s_until && !(so.hold_switch_off && first_release);
        if want_s != s_is {
            s_is = want_s;
            if want_s {
                switch_on_at = Some(t);
            }
            overrides.set(&mut run.world, wp(s_switch), if want_s { &rblock } else { &air }, "the siding's switch", false);
        }
        // (--held-rider-control: at the napper's release, glass two cells under the lip in the drop
        // column (x,m-2,mz-1): the cart ahead of the napper drops one block and stays there, its cart
        // below the napper's floor and its rider's top (the cart's y + 1.95) above it, so the napper
        // behind reaches the rider and the rider never drops: the lip outcome must read STUCK and the
        // row fail. (Glass at (x,m-1,mz-1) holds the cart on the lip instead: the napper then stops
        // against the cart's box, 0.49 behind its centre to the rider's 0.3, and never reaches the
        // rider: a merge queue, not the lip's failure.))
        if so.held_rider_control && released_at == Some(t) {
            overrides.set(&mut run.world, wp(BlockPos::new(lx, m - 2, mz - 1)), &glass, "--held-rider-control's glass", true);
        }
        // (--stray-ctl T: the stray-power check's positive control on the S-bend (ironspec 02:18): a
        // redstone block beside R, west of it (x-3,m,mz), from tick T for 10 gt: R powered while its
        // switch is off)
        if let Some(ts) = so.stray_ctl {
            if t == ts {
                overrides.set(&mut run.world, wp(BlockPos::new(sx - 1, m, mz)), &rblock, "--stray-ctl's redstone block", true);
            } else if t == ts + 10 {
                overrides.set(&mut run.world, wp(BlockPos::new(sx - 1, m, mz)), &air, "--stray-ctl's redstone block", true);
            }
        }
        // (--early-r N: the outcome bar's positive control through the design's own cells (G1 /
        // ironspec 07:31: "shift the MEASURED R power through the real cells"): the release switch goes
        // on N gt before the release tick (see the release logic), so R powers N gt early through the
        // real feed path, in a tick's block phase like the design's own power, and the bar judges it
        // the same way)
        // (the napper's rest window for the push bar's computed scope: at rest -> its release)
        if released_at == Some(t)
            && let Some(r) = nap_rested_at
        {
            rest_windows.push((r, t));
        }
        if s_is {
            switch_last_on = t;
        }
        // (every power reaching the siding rest: the rail powered while its declared switch has been
        // off longer than the drive's delay (R's repeater 2 gt, the feed path's repeater 2 more, or
        // day-1's delay-3 6 more with --feed-slime) plus 2 gt is a stray power (the bar: 0); the
        // guard detector powered in those ticks shows whether it was the guard)
        let rest_powered = matches!(run.world.get(wp(rest_pos)), Block::Rail { powered: true, .. });
        // (the feed delay as the design runs it (ironspec 05:03, hole 7): each release to the first
        // tick R is powered after it)
        if released_at == Some(t) {
            nap_release_n += 1;
            // (a release still waiting for R's power when the next one comes was never powered
            // (G1 (2)): a failure, not a dropped sample)
            if let Some(t0) = feed_wait.take() {
                r_never_powered.push(t0);
            }
            feed_wait = Some(t);
        }
        // (the napper's state at the start of this iteration = at the end of the previous tick; the
        // loop runs `run.tick()` after these readings, so every reading here is the world at the end of
        // tick t - 1, R's power included)
        let nap_rest_now = nap.and_then(|(nc, _)| carts[nc]).and_then(|id| st(&run.world, id)).is_some_and(|p| p.0.floor() as i32 == sx && (p.1 + 1e-6).floor() as i32 == rest_y && p.2.floor() as i32 == rest_z && p.3 < 0.001);
        if nap_rest_now && !nap_rest_prev {
            nap_rest_tick = Some(t);
        }
        if rest_powered && let Some(t0) = feed_wait.take() {
            feed_meas.push(t - t0);
            if let Some(s0) = switch_on_at {
                feed_from_switch.push(t - s0);
            }
            rest_power_ticks.push((nap_rest_tick.take(), t));
            // (ironspec 06:15 real, the outcome the PRESENT bar proxies, judged directly: the napper
            // AT REST on R (its block position R's cell, speed < 0.001) when R first powers after its
            // release. The moment read (G1, the tie): R powers in a tick's BLOCK phase (the feed's
            // repeaters, and --early-r's switch through the same path), before that tick's entities
            // move; it reads powered here, at the end of that tick, so the napper is judged as it stood
            // at the end of the tick BEFORE (the previous iteration's reading): a napper coming to rest
            // in the same tick's entity phase was not at rest when R powered)
            power_judged += 1;
            if !nap_rest_prev {
                not_rest_at_power.push(t);
            }
        }
        nap_rest_prev = nap_rest_now;
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
                // (--j-rail: the napper's own parked cart and rider set on K's cell moving north, as
                // --nap-feed-v summons it; else a fresh napper)
                let kpos = [sx as f64 + 0.5, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5 + so.nap_feed_phase)];
                let reused = match (&nbt_feed, parked[c]) {
                    (Some(_), Some(pid)) if so.j_rail && teleport_cart(&mut run.world, pid, kpos, [0.0, 0.0, vz(-so.nap_feed_v.unwrap_or(0.0))]) => Some(pid),
                    _ => None,
                };
                parked[c] = None;
                let id = match (reused, &nbt_feed) {
                    (Some(pid), _) => pid,
                    // (--nap-feed-v: the napper arrives on the feed cell (K's cell under --k-row) moving north)
                    (None, Some(nf)) => spawn_off(&mut run.world, "minecraft:minecart", kpos, Some(nf))?,
                    (None, None) => spawn_off(&mut run.world, "minecraft:minecart", [sx as f64 + 0.5, rest_y as f64 + 0.5 + 0.0625, mzf(rest_z as f64 + 0.5)], Some(&nbt0))?,
                };
                set_returns.push((c, t));
                nap_spawned_t = Some(t);
                (nap_on_r, nap_settled) = (None, false);
                // (the napper's K tick: with --nap-feed-v it is spawned on K's cell moving north; the
                // PRESENT bit (--nap-present-k) counts from here)
                if nbt_feed.is_some() {
                    nap_k_at = Some(t);
                    if let Some(d) = nap_departed_at {
                        nap_gaps.push(t - d);
                    }
                }
                if so.ai && !so.skip_farm_rider {
                    // (--j-rail's reused napper keeps its brain as it is: HOME set at its first summon)
                    if reused.is_none() {
                        farm_rider(&mut run.world, id, bed, so.stamp_at_arrival);
                    }
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
            if so.j_rail && parked[c].is_some() {
                // (--j-rail: the cart's own parked cart and rider set on J's west half (x+0.15, m+1,
                // mz+2.5) moving east at v_in, through J's curve onto D, once J's cell is clear)
                let jx = lx as f64 + 0.15;
                let clear_j = carts.iter().all(|id| id.and_then(|id| st(&run.world, id)).is_none_or(|(x, y, z, _)| !(y > m as f64 + 0.5 && (z - zs).abs() < 0.9 && (x - jx).abs() < 1.3)));
                if clear_j {
                    let pid = parked[c].take().unwrap_or_default();
                    if teleport_cart(&mut run.world, pid, [jx, (m + 1) as f64 + 0.0625, mzf(zs)], [o.v_in, 0.0, 0.0]) {
                        carts[c] = Some(pid);
                        set_returns.push((c, t));
                    }
                    pending.retain(|p| p.1 != c);
                }
                continue;
            }
            let clear = carts.iter().all(|id| id.and_then(|id| st(&run.world, id)).is_none_or(|(x, y, z, _)| !(y > m as f64 - 0.5 && (z - zs).abs() < 1.3 && (x - zc).abs() < 0.9)));
            if clear {
                let id = spawn_off(&mut run.world, "minecraft:minecart", [zc, m as f64 + 0.0625, mzf(zs)], Some(&nbt_in))?;
                if so.ai && !so.skip_farm_rider {
                    farm_rider(&mut run.world, id, bed, None);
                }
                carts[c] = Some(id);
                set_returns.push((c, t));
                pending.retain(|p| p.1 != c);
            }
        }
        // (a restart between ticks t - 1 and t (cc767b78's helper, 58f6f0084): a save/stop/start in a
        // load order, the carts followed to their new ids; the bench's controller runs on (its timers
        // are game ticks; a world open's N idle ticks advance game time with nothing ticking). The cold start is no restart: it is the fresh
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
        // (--forced-v T (ironspec Nov 13 10:56 (3), the FORCED-V ROW): a ridden cart planted at V (the
        // top slot, (x+0.5, slope, 6.02)) at tick T, its rider in PANIC with --forced-v-panic (a fresh
        // hurt every 30 gt), so the next arrivals meet a ridden cart at V: the fixed design must not
        // stall)
        if so.forced_v == Some(t) {
            let vzz = 6.02;
            let id = spawn_off(&mut run.world, "minecraft:minecart", [zc, slope_y(vzz), mzf(vzz)], Some(&nbt0))?;
            if so.ai && !so.skip_farm_rider {
                farm_rider(&mut run.world, id, bed, None);
            }
            forced_v_id = Some(id);
        }
        if so.forced_v_panic
            && t % 30 == 0
            && let Some(id) = forced_v_id
            && let Some(ri) = rider_index(&run.world, id)
        {
            redsim::entity::mob::mob_mut(&mut run.world, ri).last_damage = Some((run.world.time, redsim::entity::mob::Damage::Cramming));
        }
        run.tick();
        let mut now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|id| id.and_then(|id| st(&run.world, id))).collect();
        // (G1 12:32's edge control (G1 14:18 L1): at T, the lane's highest cart at rest on the slopes is
        // moved up-slope by exactly D (z + D, y + D), at rest, right after the tick; its new position is
        // read back from the world, and only the bar's own loop below judges it: 0.10 must not count,
        // 0.11 must)
        if let Some((d, tc)) = so.upslope_ctl
            && t == tc
        {
            let top = (0..n).filter(|c| now[*c].is_some_and(|(x, y, z, v)| v < 0.001 && x >= lx as f64 && x < (lx + 1) as f64 && z > 1.5 && z < 5.5 && y < 23.0)).max_by(|a, b| now[*a].map(|p| p.2).partial_cmp(&now[*b].map(|p| p.2)).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(c) = top
                && let Some((x, y, z, _)) = now[c]
                && let Some(id) = carts[c]
                && teleport_cart(&mut run.world, id, [x, y + d, mzf(z + d)], [0.0, 0.0, 0.0])
            {
                now[c] = st(&run.world, id);
                upslope_planted = Some((t, c, d));
                upslope_plant_note = format!("lane cart {c} from z {z:.4} to {:.4} (y {y:.4} to {:.4})", now[c].map_or(f64::NAN, |p| p.2), now[c].map_or(f64::NAN, |p| p.1));
            }
        }
        // (the refill's first contact: two lane carts' boxes within 0.01 of touching in slots 1-2
        // (z < 2.6, y 15.5..18.6) after R_1 came on)
        if let Some(rt) = refill_timing.last_mut()
            && t - rt.0 <= 60
        {
            let in_lane = |p: &(f64, f64, f64, f64)| p.0 >= lx as f64 && p.0 < (lx + 1) as f64;
            if rt.1.is_none() && refill_on_r1.last().is_some_and(|ids| ids.iter().any(|c| now[*c].is_some_and(|p| p.3 > 0.001))) {
                rt.1 = Some(t - rt.0);
            }
            if rt.1.is_some() && rt.2.is_none() && now.iter().flatten().any(|p| in_lane(p) && p.2 < 1.0 && p.2 > 0.6 && p.3 > 0.001) {
                rt.2 = Some(t - rt.0);
            }
            if rt.2.is_some() && rt.3.is_none() && now.iter().flatten().any(|p| in_lane(p) && p.2 < 0.6 && p.3 < 0.001 && p.1 < 16.2) {
                rt.3 = Some(t - rt.0);
            }
        }
        if let Some(t_on) = refill_pending {
            let low: Vec<(f64, f64)> = now.iter().filter_map(|p| p.and_then(|(x, y, z, _)| (x >= lx as f64 && x < (lx + 1) as f64 && z < 2.6 && y > 15.5 && y < 18.6).then_some((y, z)))).collect();
            let touching = low.iter().enumerate().any(|(i, a)| low.iter().skip(i + 1).any(|b| (a.1 - b.1).abs() < 0.99 && (a.0 - b.0).abs() < 0.71));
            if touching {
                refill_contacts.push((t_on, t));
                refill_pending = None;
            } else if t - t_on > 60 {
                refill_pending = None;
            }
        }
        // (tick-exact 12:27: the napper's LAUNCH from R: its rest z on R before the launch and its step
        // (the horizontal move that tick) when its centre first enters C's cell (x,m,mz), or with
        // --sbend-straight the drop cell (x,m,mz-1) from the west)
        if let Some((nc, _)) = nap
            && let Some((x, y, z, v)) = now[nc]
        {
            if v < 0.001 && x >= sx as f64 && x < (sx + 1) as f64 && (y + 1e-6).floor() as i32 == m && z >= mz as f64 && z < (mz + 1) as f64 {
                launch_rest_z = Some(z);
            }
            let (cz0, cz1) = if so.sbend_straight { ((mz - 1) as f64, mz as f64) } else { (mz as f64, (mz + 1) as f64) };
            let at = x >= lx as f64 && x < (lx + 1) as f64 && z >= cz0 && z < cz1 && y > m as f64 - 0.2;
            if at && !launch_in[nc] {
                let step = prev_now.get(nc).copied().flatten().map_or(f64::NAN, |(px, _, pz, _)| (x - px).hypot(z - pz));
                nap_launches.push((t, launch_rest_z.take(), step));
            }
            launch_in[nc] = at;
        }
        prev_now.clone_from(&now);
        // (up-slope motion on the slopes: a lane cart at z 1.0..6.6, y 16.5..23.2 in its lane moving
        // south (up the ascending_south slopes) by its stored motion; and its net displacement from its
        // last rest)
        for c in 0..n {
            if let Some((x, y, z, _)) = now[c]
                && x >= lx as f64
                && x < (lx + 1) as f64
                && z > 1.0
                && z < 6.6
                && y > 16.5
                && y < 23.2
                && let Some(id) = carts[c]
                && let Some(i) = redsim::entity::index_of(&run.world, id)
            {
                let up = vz(run.world.entities.list[i].delta_movement.z);
                if up > 0.001 {
                    upslope_any += 1;
                    if up > 0.05 {
                        upslope_big += 1;
                    }
                    if up > upslope_max.0 {
                        upslope_max = (up, t, c);
                    }
                }
                // (G1 12:32, the rollback's fallback bar: a net up-slope displacement over 0.1 block from
                // the cart's last rest (at rest: speed under 0.001 on the slopes), once per excursion)
                let v_h = run.world.entities.list[i].delta_movement.x.hypot(run.world.entities.list[i].delta_movement.z);
                // (the displacement is judged against the rest held BEFORE this tick's rest update, so a
                // cart that jumps up and stops in one tick counts (G1 14:18 L1))
                if let Some((rz, flagged)) = last_rest_z[c]
                    && !flagged
                    && z - rz > 0.1 + 1e-9
                {
                    rollbacks.push((t, c, z - rz));
                    last_rest_z[c] = Some((rz, true));
                }
                if v_h < 0.001 {
                    slope_rest_samples += 1;
                    if let Some((rz, flagged)) = last_rest_z[c]
                        && flagged
                        && z <= rz + 1e-9
                    {
                        // (back at or below the last rest: a new excursion can count)
                        last_rest_z[c] = Some((z, false));
                    } else if last_rest_z[c].is_none_or(|(_, f)| !f) {
                        last_rest_z[c] = Some((z, false));
                    }
                }
            } else {
                // (off the slopes (released, on the lift, at the lip): its next rest starts afresh, so a
                // cart's return to the lane top is no displacement from its old slot)
                last_rest_z[c] = None;
            }
        }
        // (G1 12:32's edge control, judged by the bar's own loop (G1 14:18 L1): planted right after the
        // tick, before the up-slope loop, so that loop sees the planted cart)
        // (G1 10:24: each return's first tick after it was set on the track, its position and motion)
        set_returns.retain(|&(c, ts)| {
            if t > ts {
                if let Some(id) = carts[c]
                    && let Some(i) = redsim::entity::index_of(&run.world, id)
                {
                    let e = &run.world.entities.list[i];
                    let (_, py, pz) = dpos(e.pos);
                    first_ticks.push((py, pz, e.delta_movement.x, vz(e.delta_movement.z)));
                }
                false
            } else {
                true
            }
        });
        // (the drop passes and the arrivals at C (ironspec 10:42 (4), G1 10:29 / 10:36), design frame)
        for c in 0..n {
            let Some((x, y, z, csp)) = now[c] else {
                in_c[c] = false;
                in_j[c] = false;
                if let Some((t0, off, lat, q)) = lip_open[c].take() {
                    lip_passes.push((t0, c, off, lat, q));
                }
                continue;
            };
            let at_c = x >= lx as f64 && x < (lx + 1) as f64 && z >= mz as f64 && z < (mz + 1) as f64 && y > (m as f64) - 0.1 && y < m as f64 + 0.9;
            // (G1 11:23 / day-1: J(l) -> slot settle: from the cart's first tick in J's cell (x, m+1,
            // mz+2) to its first tick at rest (speed < 0.001) in the lane's slots (y under m - 1), with
            // the queue depth at its J tick)
            let at_j = x >= lx as f64 && x < (lx + 1) as f64 && z >= (mz + 2) as f64 && z < (mz + 3) as f64 && y > (m + 1) as f64 - 0.1 && y < (m + 1) as f64 + 0.9;
            if at_j && !in_j[c] {
                let q = now.iter().enumerate().filter(|(j, p)| *j != c && p.is_some_and(|(qx, qy, qz, _)| qx >= lx as f64 && qx < (lx + 1) as f64 && qy > 15.5 && qy < 23.0 && qz > 0.0 && qz < 6.5)).count();
                j_settles.retain(|s| s.1 != c);
                j_settles.push((t, c, q));
            }
            in_j[c] = at_j;
            if let Some(k) = j_settles.iter().position(|s| s.1 == c)
                && csp < 0.001
                && y < m as f64 - 1.0
            {
                let (t0, _, q) = j_settles.remove(k);
                settles_done.push((q, t - t0));
            }
            if at_c && !in_c[c] {
                c_arrivals.push((t, c, csp, z));
                // (day-1 / ironspec 11:05: the queue depth at this arrival: the other carts in the
                // lane's slots (x lx..lx+1, y 15.5..23, z 0..6.5))
                let q = now.iter().enumerate().filter(|(j, p)| *j != c && p.is_some_and(|(qx, qy, qz, _)| qx >= lx as f64 && qx < (lx + 1) as f64 && qy > 15.5 && qy < 23.0 && qz > 0.0 && qz < 6.5)).count();
                descents.retain(|d| d.1 != c);
                descents.push((t, c, q, t, None));
            }
            in_c[c] = at_c;
            let in_region = x >= lx as f64 && x < (lx + 1) as f64 && z > mz as f64 - 1.5 && z < mz as f64 + 0.6 && y > m as f64 - 1.5 && y < m as f64 + 1.0;
            if in_region {
                let bp = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
                let rail_at = |p: BlockPos| run.world.get(wp(p)).to_string().contains("rail");
                let off_rail = !rail_at(bp) && !rail_at(BlockPos::new(bp.x, bp.y - 1, bp.z));
                let queued = off_rail
                    && now.iter().enumerate().any(|(j, q)| j != c && q.is_some_and(|(qx, qy, qz, _)| qx > lx as f64 - 0.6 && qx < lx as f64 + 1.6 && qz > mz as f64 - 0.1 && qz < mz as f64 + 2.8 && qy > m as f64 - 0.2 && qy < m as f64 + 1.9));
                let lat = (x - (lx as f64 + 0.5)).abs();
                let e = lip_open[c].get_or_insert((t, 0, 0.0, false));
                if off_rail {
                    e.1 += 1;
                }
                e.2 = e.2.max(lat);
                e.3 |= queued;
            } else if let Some((t0, off, lat, q)) = lip_open[c].take() {
                lip_passes.push((t0, c, off, lat, q));
            }
        }
        // (the descents past V (ironspec 11:08 (2) / 11:11: the descent ends at the first tick after
        // which neither the cart's box nor its rider's box intersects the LIP VOLUME, `--lip-volume`,
        // default tick-exact's swept volume for lane 3 (x 1.345..4.182, y m..m+0.7, z 5.940..8.157,
        // its 11:18 numbers); the proxy (the centre under z 5.7 below y 23) printed beside it)
        {
            let lv = so.lip_volume.unwrap_or([1.345, 4.182, m as f64, m as f64 + 0.7, 5.940, 8.157]);
            let meets = |bb: &redsim::entity::vec3::Aabb| {
                let b = dbox(bb);
                b.1 > lv[0] && b.0 < lv[1] && b.3 > lv[2] && b.2 < lv[3] && b.5 > lv[4] && b.4 < lv[5]
            };
            descents.retain_mut(|d| {
                let (t0, c, q) = (d.0, d.1, d.2);
                let Some(id) = carts[c] else { return false };
                let Some(ci) = redsim::entity::index_of(&run.world, id) else { return false };
                let mut inside = meets(&run.world.entities.list[ci].bb);
                if let Some(ri) = rider_index(&run.world, id) {
                    inside |= meets(&run.world.entities.list[ri].bb);
                }
                if inside {
                    d.3 = t;
                }
                if d.4.is_none()
                    && let Some((_, y, z, _)) = now[c]
                    && y < 23.0
                    && z < 5.7
                {
                    d.4 = Some(t);
                }
                // (done once out of the volume for 10 ticks after the proxy's end)
                if let Some(pt) = d.4
                    && t - d.3 > 10
                {
                    descents_done.push((q, pt - t0, d.3 + 1 - t0));
                    return false;
                }
                true
            });
            // (day-1 12:09 sub-bar (iv), ironspec 12:08 (3) / G1 12:12: every LIP ARRIVAL (a cart's
            // centre entering the drop cell (x,m,mz-1), from C or from the napper's straight) with the
            // other carts' and riders' boxes inside the lip volume at that tick, each classified by the
            // V-empty sub-bars: a 9th cart (8 others in the slots), a ROLLBACK (an occupant that had
            // come to rest in the slots below V and rose back into V's volume), a gap under Δ*_worst + 10 =
            // 23 (tick-exact 12:18: the C-gap's 13; an occupant that arrived at the lip within 40 gt, or
            // the previous lip arrival within 23),
            // else UNCLASSIFIED (a third mechanism); the off-rail rests take their arrival's class): the
            // ONE definition, `mobfarm::lipclass` (onechunk 12:47), fed the design frame's boxes
            let obs: Vec<crate::lipclass::CartObs> = (0..n)
                .filter_map(|c| {
                    let id = carts[c]?;
                    let ci = redsim::entity::index_of(&run.world, id)?;
                    let e = &run.world.entities.list[ci];
                    let (x, y, z) = dpos(e.pos);
                    let b = dbox(&e.bb);
                    let rider_box = rider_index(&run.world, id).map(|ri| {
                        let r = dbox(&run.world.entities.list[ri].bb);
                        [r.0, r.1, r.2, r.3, r.4, r.5]
                    });
                    Some(crate::lipclass::CartObs { id: c as u32, pos: (x, y, z), speed: e.delta_movement.x.hypot(e.delta_movement.z), cart_box: [b.0, b.1, b.2, b.3, b.4, b.5], rider_box })
                })
                .collect();
            for a in lip_lane.tick(t, &obs) {
                let c = a.cart as usize;
                let is_nap = nap.map(|(nc, _)| nc) == Some(c);
                let occupants: Vec<(usize, &'static str)> = a
                    .occupants
                    .iter()
                    .map(|(j, k)| {
                        (*j as usize, match k {
                            crate::lipclass::OccKind::Rollback => "rollback",
                            crate::lipclass::OccKind::Descending => "descending",
                            crate::lipclass::OccKind::Other => "other",
                        })
                    })
                    .collect();
                lip_arrivals.push((a.tick, c, a.others_in_slots, a.gap, occupants, a.class.name()));
                lip_steps.push((a.step, is_nap));
                last_arrival_of[c] = Some(lip_arrivals.len() - 1);
            }
        }
        // (G1 11:25 gap (2): a lane cart without its rider, the presence string's blind case (it reads
        // "R empty" under an unridden cart): every (cart, tick) a ridden run's lane cart has no passenger;
        // the bar 0)
        if o.riders {
            for (c, id) in carts.iter().enumerate() {
                if let Some(id) = id
                    && let Some(ci) = redsim::entity::index_of(&run.world, *id)
                    && run.world.entities.list[ci].passengers.is_empty()
                {
                    unridden.push((t, c));
                }
            }
        }
        // (the presence tripwire over R (ironspec 10:49 (3)): the napper's rider's box top while its
        // cart rests on R, against the string's box at y 26.0625..26.15625)
        if let Some((nc, _)) = nap
            && let Some((x, y, z, sp)) = now[nc]
            && x.floor() as i32 == sx
            && (y + 1e-6).floor() as i32 == rest_y
            && z.floor() as i32 == rest_z
            && sp < 0.001
            && let Some(id) = carts[nc]
            && let Some(ri) = rider_index(&run.world, id)
        {
            let top = dbox(&run.world.entities.list[ri].bb).3;
            let cart_top = redsim::entity::index_of(&run.world, id).map_or(f64::MIN, |ci| dbox(&run.world.entities.list[ci].bb).3);
            let e = rider_top_on_r.get_or_insert((f64::MAX, f64::MIN, 0.0, 0usize, f64::MIN));
            if top < e.0 {
                e.0 = top;
                e.2 = z;
            }
            e.1 = e.1.max(top);
            e.3 += 1;
            e.4 = e.4.max(cart_top);
        }
        // (MCTAS_LIP_TRACE=t0..t1 (ironspec 10:09 / 10:20 (a): the mechanism first): per tick, every
        // minecart near the lane's top (design frame x lx-3..lx+2, y 21.5..26.5, z 4.5..10) with its
        // position, motion, the block under it, this tick's SELF moves (requested -> returned) and its
        // riders' boxes; and every villager whose box reaches the drop column (x lx..lx+1, z 5.5..7.5, y
        // 22..25.5) with its vehicle, box, activity and walk target)
        if let Some((t0, t1)) = lip_trace
            && t >= t0
            && t <= t1
        {
            let wt = run.world.time;
            let bx = |e: &redsim::entity::Entity| {
                let b = dbox(&e.bb);
                format!("x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3}", b.0, b.1, b.2, b.3, b.4, b.5)
            };
            for e in run.world.entities.list.iter().filter(|e| !e.removed) {
                let (x, y, z) = dpos(e.pos);
                match &e.kind {
                    redsim::entity::EntityKind::Minecart(_) if x > (lx - 3) as f64 && x < (lx + 2) as f64 && y > 15.5 && y < 26.5 && z > -0.5 && z < 10.0 => {
                        let under = run.world.get(wp(BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32)));
                        let moves: Vec<String> = e
                            .self_moves
                            .iter()
                            .filter(|mv| mv.tick == wt)
                            .map(|mv| format!("{:.4},{:.4},{:.4}->{:.4},{:.4},{:.4}", mv.requested.x, mv.requested.y, vz(mv.requested.z), mv.returned.x, mv.returned.y, vz(mv.returned.z)))
                            .collect();
                        let riders: Vec<String> = e
                            .passengers
                            .iter()
                            .filter_map(|r| redsim::entity::index_of(&run.world, *r).map(|ri| format!("{} [{}]", r, bx(&run.world.entities.list[ri]))))
                            .collect();
                        let slot = carts.iter().position(|cid| *cid == Some(e.id)).map_or("-".to_string(), |c| c.to_string());
                        eprintln!(
                            "lip t {t}: cart {} (lane cart {slot}) at {x:.4},{y:.4},{z:.4} v {:.4},{:.4},{:.4} in {} | box [{}] | moves [{}] | riders [{}]",
                            e.id,
                            e.delta_movement.x,
                            e.delta_movement.y,
                            vz(e.delta_movement.z),
                            under.to_string().trim_start_matches("minecraft:").split(",waterlogged").next().unwrap_or("").to_string(),
                            bx(e),
                            moves.join("; "),
                            riders.join("; ")
                        );
                    }
                    redsim::entity::EntityKind::Mob(mb) => {
                        let b = dbox(&e.bb);
                        let reaches = b.1 > lx as f64 && b.0 < (lx + 1) as f64 && b.5 > mz as f64 - 1.5 && b.4 < mz as f64 + 0.5 && b.3 > m as f64 - 2.0 && b.2 < m as f64 + 1.5;
                        if reaches {
                            let act = mb.brain.as_deref().map_or("-".to_string(), |b| format!("{:?}", b.activity));
                            let walk = mb.brain.as_deref().and_then(|b| b.ex.as_deref()).and_then(|ex| ex.mem.walk_target.as_ref()).map_or("-".to_string(), |w| format!("{:?}", w.target));
                            eprintln!("lip t {t}: mob {} vehicle {:?} [{}] activity {act} walk {walk}", e.id, e.vehicle, bx(e));
                        }
                    }
                    _ => {}
                }
            }
        }
        // (the carts' block contacts: the shared definition, `crate::contacts::cart_contacts`
        // (ironspec 02:41 / 02:48 / 02:52, amended 07:47 on G1's CHANGES 3): every minecart in the world
        // (the lane's carts, the K row's through-carts, the control cart), judged from this tick's
        // recorded moves (`Entity.move(SELF, …)`: the box before, the requested and the returned
        // movement), an axis counting only where the game's collision clipped it; each clipping box
        // named by its cell in the design frame; the contacts before t 20 (the summoned start's settle)
        // counted apart and judged against the same stops)
        for (ei, e) in run.world.entities.list.iter().enumerate().filter(|(_, e)| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))) {
            for (p, blk) in crate::contacts::cart_contacts(&run.world, ei) {
                let cell = (p.x - pdx, p.y - pdy, mzc(p.z - pdz));
                let map = if t >= 20 { &mut block_contacts } else { &mut early_contacts };
                map.entry(cell).or_insert_with(|| (blk.clone(), 0)).1 += 1;
                if std::env::var("MCTAS_CONTACT_DEBUG").is_ok() {
                    eprintln!("t {t}: cart entity {} at {:.3},{:.3},{:.3} met {:?} {blk}", e.id, e.pos.x, e.pos.y, mzf(e.pos.z), cell);
                }
            }
        }
        // (the napper's windows on R for the push bar's computed scope (ironspec 03:00): every tick its
        // centre is in R's cell, resting or still rolling onto it, as spans)
        if let Some((nc, _)) = nap
            && let Some(p) = now[nc]
            && p.0.floor() as i32 == sx
            && (p.1 + 1e-6).floor() as i32 == rest_y
            && p.2.floor() as i32 == rest_z
        {
            match r_windows.last_mut() {
                Some(w) if w.0 == nc && t <= w.2 + 2 => w.2 = t,
                _ => r_windows.push((nc, t, t)),
            }
        }
        // (the napper's settle on R: its first tick on R's cell, then its first tick there at speed
        // < 0.001, positions and speeds after the tick)
        if let Some((nc, _)) = nap
            && released_at.is_none()
            && !nap_settled
            && let Some(p) = now[nc]
        {
            // (R's cell: the ascending rest's y runs 24 to 25 across it, so its cell is floor(y), not
            // the rounded y (a napper at rest past z 7.5 rides above 24.5))
            let on_r = p.0.floor() as i32 == sx && (p.1 + 1e-6).floor() as i32 == rest_y && p.2.floor() as i32 == rest_z;
            if on_r && nap_on_r.is_none() {
                nap_on_r = Some(t);
            }
            if on_r
                && p.3 < 0.001
                && let Some(t0) = nap_on_r
            {
                settles.push(t - t0);
                nap_settled = true;
            }
        }
        // (the resting napper across the K row's passes: its displacement and speed while a K-row
        // cart is beside K; every K-row pass tick is kept, resting napper or not, for the push bar's
        // computed scope (ironspec 03:00: the rest windows against the pass ticks, shown))
        {
            let kx = sx as f64 + 0.5;
            let row_z = (mz + 2) as f64;
            let k_row_passing = through.iter().filter_map(|(id, _)| st(&run.world, *id)).any(|q| (q.0 - kx).abs() < 1.5 && q.2 > row_z && q.2 < row_z + 1.0);
            if k_row_passing {
                k_pass_ticks.push(t);
            }
            let resting = nap.and_then(|(nc, _)| now[nc]).filter(|_| nap_rested_at.is_some() && released_at.is_none());
            // (a K-zone cart whose box, grown 0.2 in x and z (the push's reach), meets the resting
            // napper's box: the push's precondition, which the planted control must reach)
            if let Some(p) = resting {
                let met = through.iter().filter_map(|(id, _)| st(&run.world, *id)).any(|q| (q.0 - kx).abs() < 1.5 && q.2 > row_z && q.2 < row_z + 1.0 && (q.0 - p.0).abs() < 0.98 + 0.2 && (q.2 - p.2).abs() < 0.98 + 0.2 && (q.1 - p.1).abs() < 0.7);
                if met {
                    pass_overlap += 1;
                }
            }
            match resting {
                Some(p) => {
                    let (rx, rz) = *push_rest_ref.get_or_insert((p.0, p.2));
                    let passing = k_row_passing
                        // (r_joint's control cart, while it lives, counts as a pass: the turn control,
                        // rolling north at K into F and R, reads the same displacement; the push grid's
                        // controls go through the K-zone gate (planted K-row carts))
                        || control_cart.and_then(|id| st(&run.world, id)).is_some();
                    if passing {
                        if !push_in_pass {
                            push_passes += 1;
                        }
                        push_disp_max = push_disp_max.max(((p.0 - rx).powi(2) + (p.2 - rz).powi(2)).sqrt());
                        push_v_max = push_v_max.max(p.3);
                    }
                    push_in_pass = passing;
                }
                None => {
                    push_rest_ref = None;
                    push_in_pass = false;
                }
            }
        }
        // (the lip margin's tracks: carts from the merge's south face to 3 cells north of it)
        for c in 0..n {
            let (Some(p), Some(id)) = (now[c], carts[c]) else { continue };
            if p.2 > (mz + 1) as f64 || p.2 < (mz - 3) as f64 {
                continue;
            }
            if nap.is_some_and(|(nc, _)| nc == c) {
                nap_track.push((t, c, p.0, p.2));
            }
            if let Some(i) = redsim::entity::index_of(&run.world, id)
                && let Some(&rid) = run.world.entities.list[i].passengers.first()
                && let Some(ri) = redsim::entity::index_of(&run.world, rid)
            {
                let re = &run.world.entities.list[ri];
                let (w, h) = re.kind.dims();
                let (rx, ry, rz) = dpos(re.pos);
                rider_track.push((t, c, rx, rz, ry, w as f64 / 2.0, h as f64));
            }
        }
        // (ironspec 10:02, the lane overflow: the occupancy at which a cart first rests at the
        // lane's top cell V (x,m-2,mz-1), and any cart resting on another cart's top (its floor
        // within the other's box height above it, the boxes overlapping in x and z): a failure)
        {
            let lane: Vec<(f64, f64, f64, f64)> = now.iter().flatten().copied().filter(|p| (p.0 - zc).abs() < 0.6 && p.2 < mz as f64 && p.1 < m as f64 - 0.5 && p.1 > 15.0).collect();
            let top_z = (mz - 1) as f64;
            // (the steady window: from the steady state to the last release; after it the lane
            // refills with no release and holds V by design (the bench's end))
            let steady_now = t >= steady_from && t < rel_end;
            if steady_now {
                v_samples += 1;
            }
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
                // (each cart's position after the tick and its motion (vx, vy, vz: delta_movement,
                // design frame), for a summon that replays it (tick-exact's official rows))
                let xs: Vec<String> = (0..n)
                    .filter_map(|c| {
                        let s = now[c]?;
                        let dmv = carts[c].and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| run.world.entities.list[i].delta_movement);
                        Some(match dmv {
                            Some(d) => format!("{c}:{:.3},{:.3},{:.3}/{:.3} m({:.4},{:.4},{:.4})", s.0, s.1, s.2, s.3, d.x, d.y, if so.mirror_z { -d.z } else { d.z }),
                            None => format!("{c}:{:.2},{:.2},{:.2}/{:.2}", s.0, s.1, s.2, s.3),
                        })
                    })
                    .collect();
                // (the trace's states are read at the tick's start, before the world's tick t: the
                // positions after tick t - 1, and D's (--siding-north) pressed state from tick t - 1)
                eprintln!("t {t}: trap {} R {} S {} hold {} {} {} : {}", trap_is as u8, r_is as u8, s_is as u8, hh as u8, if so.siding_north { "D" } else { "merge" }, mh as u8, xs.join(" "));
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
        // (r_joint's positive control: the second cart spawned at its tick)
        if let Some((kind, ti)) = so.r_joint_control.as_ref()
            && t == *ti
            && control_cart.is_none()
        {
            let id = match kind.as_str() {
                "turn" => {
                    let nf = nbt_feed.as_ref().ok_or("--r-joint-control turn needs --nap-feed-v (the cart turning at K moves north like a napper)")?;
                    spawn_off(&mut run.world, "minecraft:minecart", [sx as f64 + 0.5, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(nf))?
                }
                "pass" => {
                    let nr = nbt_row.as_ref().ok_or("--r-joint-control pass needs --k-row (the K row's eastbound pass)")?;
                    spawn_off(&mut run.world, "minecraft:minecart", [(sx - 2) as f64 + 0.5, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(nr))?
                }
                // (ironspec 04:14 (8): the diagonal turn: a cart entering K's north_east curve from its
                // east cell, rolling west at --k-row's speed: the curve turns it north through K at 45
                // degrees (|vx| = |vz|) into F and R; the cell transition must count it)
                "diag" => {
                    let v = so.k_row.ok_or("--r-joint-control diag needs --k-row (its speed)")?;
                    spawn_off(&mut run.world, "minecraft:minecart", [(sx + 1) as f64 + 0.5, (m + 1) as f64 + 0.0625, mzf((mz + 2) as f64 + 0.5)], Some(&format!("{{Motion:[{:.4}d,0.0d,0.0d]{rider}}}", -v)))?
                }
                other => return Err(format!("--r-joint-control {other}: turn:T, diag:T or pass:T")),
            };
            control_cart = Some(id);
        }
        // (the pass control's cart, once it leaves K's T east, is removed there like the K row's own
        // carts, so it can't stop at the row's end and block the next K-row cart; the diag control's
        // cart starts east of K and is left alone)
        if let Some(id) = control_cart
            && so.r_joint_control.as_ref().is_some_and(|(k, _)| k == "pass")
            && let Some(p) = st(&run.world, id)
            && p.0 > (sx + 1) as f64 + 0.4
        {
            kill_with_rider(&mut run.world, id);
        }
        // (R against K's turn, by CELL TRANSITION (ironspec 04:14 (8)): a cart's centre in R's cell
        // while another's is in F's cell (F (x-2,m+1,mz+1) is only entered by a cart turning north out
        // of K or leaving R south), or while another moves out of K's cell into F's or R's this tick,
        // or while another in K's cell moves north at 45 degrees or steeper (|vz| >= |vx|: a turn that
        // pushes a napper resting near K from inside K never enters F; the K row's passes move south of
        // east on K's T, vz > 0), or two carts in R's cell at once (ironspec 01:37 (2a)); every cart: the
        // lane's, the K row's through-carts and the control's)
        {
            // (a cell by floor(y): R is an ascending rail whose rider's y runs 24 to 25 across it, so
            // the rounded y missed every cart at rest past z 7.5 (the self-judging turn control found
            // it); F and K are flat, their carts at y + 0.0625 at most)
            let cell = |p: &(f64, f64, f64, f64, f64), x: i32, y: i32, z: i32| p.0.floor() as i32 == x && (p.1 + 1e-6).floor() as i32 == y && p.2.floor() as i32 == z;
            // (position and motion in the design frame: x, y, z, motion x, motion z)
            let stv = |id: u32| -> Option<(f64, f64, f64, f64, f64)> {
                redsim::entity::index_of(&run.world, id).map(|i| {
                    let e = &run.world.entities.list[i];
                    {
                        let (px, py, pz) = dpos(e.pos);
                        (px, py, pz, e.delta_movement.x, vz(e.delta_movement.z))
                    }
                })
            };
            let mut ids: Vec<u32> = carts.iter().flatten().copied().filter(|id| stv(*id).is_some()).collect();
            ids.extend(through.iter().map(|(id, _)| *id).filter(|id| stv(*id).is_some()));
            ids.extend(control_cart.filter(|id| stv(*id).is_some()));
            let all: Vec<(f64, f64, f64, f64, f64)> = ids.iter().filter_map(|id| stv(*id)).collect();
            let on_r: Vec<usize> = (0..all.len()).filter(|i| cell(&all[*i], sx, rest_y, rest_z)).collect();
            // (each cart's cell among K / F / R this tick, and the carts that left K's cell into F's or
            // R's since the previous tick: the transition)
            let cell_of = |p: &(f64, f64, f64, f64, f64)| -> char {
                if cell(p, sx, m + 1, mz + 2) {
                    'K'
                } else if cell(p, sx, m + 1, mz + 1) || cell(p, sx, m, mz + 1) {
                    'F'
                } else if cell(p, sx, rest_y, rest_z) {
                    'R'
                } else {
                    '-'
                }
            };
            let from_k: Vec<usize> = (0..all.len()).filter(|i| kfr_prev.get(&ids[*i]) == Some(&'K') && matches!(cell_of(&all[*i]), 'F' | 'R')).collect();
            k_turns += from_k.len();
            // (MCTAS_RJOINT_DEBUG: every tick a cart is on R while another is in F's or K's cell, with
            // that cart's kind, position, motion and cell transition: the per-tick diagnosis ironspec
            // 01:37 (2b) asks for)
            if !on_r.is_empty() && std::env::var("MCTAS_RJOINT_DEBUG").is_ok() {
                let n_lane = carts.iter().flatten().filter(|id| stv(**id).is_some()).count();
                let n_th = through.iter().filter(|(id, _)| stv(*id).is_some()).count();
                for (i, p) in all.iter().enumerate() {
                    if on_r.contains(&i) {
                        continue;
                    }
                    let c = cell_of(p);
                    if c == 'F' || c == 'K' {
                        let kind = if i < n_lane { "lane cart" } else if i < n_lane + n_th { "K-row cart" } else { "control cart" };
                        eprintln!(
                            "rjoint t {t}: {kind} in {c} at {:.3},{:.3},{:.3} motion ({:+.4},{:+.4}); R held by {} cart(s); came from {}; the transition test {}",
                            p.0,
                            p.1,
                            p.2,
                            p.3,
                            p.4,
                            on_r.len(),
                            kfr_prev.get(&ids[i]).copied().unwrap_or('-'),
                            if c == 'F' || from_k.contains(&i) || (c == 'K' && p.4 < -1e-4 && p.4.abs() >= p.3.abs() - 1e-9) { "counts" } else { "no" }
                        );
                    }
                }
            }
            if on_r.len() >= 2 || !on_r.is_empty() && (0..all.len()).any(|i| !on_r.contains(&i) && (cell_of(&all[i]) == 'F' || from_k.contains(&i) || (cell_of(&all[i]) == 'K' && all[i].4 < -1e-4 && all[i].4.abs() >= all[i].3.abs() - 1e-9))) {
                r_joint += 1;
            }
            kfr_prev = ids.iter().zip(all.iter()).map(|(id, p)| (*id, cell_of(p))).collect();
        }
        if on_merge > 1 {
            touching += 1;
            // (who touched whom (ironspec 04:14 (5): the lip evidence from the actual contacts): with a
            // napper in the merge cell, each other cart there is its LEADER (the cart ahead its lip
            // margin measures: the latest other cart into the merge before the napper's entry) or
            // another cart (its follower, or a cart still in the merge from earlier))
            if let Some((nc, _)) = nap
                && now[nc].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))
            {
                let ne = merge_occ.iter().filter(|mo| mo.0 == nc).map(|mo| mo.1).max();
                // (only the napper's first passage after its release: the passage its lip margin
                // measures)
                let first = merge_occ.iter().filter(|mo| mo.0 == nc && released_at.is_some_and(|r| mo.1 >= r)).map(|mo| mo.1).min();
                let ahead = ne.filter(|en| Some(*en) == first).and_then(|en| merge_occ.iter().filter(|mo| mo.0 != nc && mo.1 < en).max_by_key(|mo| mo.1).map(|mo| mo.0));
                for c2 in (0..n).filter(|c| *c != nc && now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))) {
                    if Some(c2) == ahead {
                        // (the lip's contact is the napper against its leader while the leader's rider
                        // is still above the napper's floor (its box top, as tracked this tick or the
                        // last); against the leader's cart after its rider has dropped, it is a leader
                        // stuck in the merge, counted apart)
                        let rider_up = rider_track.iter().rev().find(|k| k.1 == c2 && k.0 <= t).is_some_and(|k| k.0 >= t - 1 && k.4 + k.6 > m as f64);
                        if std::env::var("MCTAS_TOUCH_DEBUG").is_ok() {
                            eprintln!("touch t {t}: napper cart {nc} (entry {ne:?}, first after release {first:?}) beside its leader cart {c2}, the leader's rider up {rider_up}");
                        }
                        if rider_up {
                            touch_leader += 1;
                        } else {
                            touch_leader_late += 1;
                        }
                    } else {
                        touch_follower += 1;
                    }
                }
            }
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
                    off_rest_class.push((t, c, String::new()));
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
                        nap_k_at = None;
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
                    // (--j-rail: the cart and its rider parked at its own spot, at rest, until its
                    // return is due; else both taken out and the return summoned fresh)
                    if so.j_rail && teleport_cart(&mut run.world, id, [lx as f64 + 5.5, 2.0625, mzf(-5.0 + 2.0 * c as f64 + 0.5)], [0.0, 0.0, 0.0]) {
                        parked[c] = Some(id);
                    } else {
                        kill_with_rider(&mut run.world, id);
                    }
                    fell[c] = false;
                }
            }
            // (the napper's launch from R (ironspec 09:01, R on slime: slime's stepOn scales a grounded
            // cart's horizontal motion by 0.4 when |vy| < 0.1): the first tick after its release its
            // centre is past R's north end (below R's cell in z; a napper still rolling onto R at its
            // release, --nap-present-k, counts from there too), its speed and R's power then; not past
            // it 40 gt after its release: a stuck launch)
            if let (Some((nc, _)), Some(ra)) = (nap, released_at)
                && nc == c
                && launch_for != Some(ra)
                && t > ra
            {
                let left = (z.floor() as i32) < rest_z;
                if left {
                    nap_departed_at = Some(t);
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
                let m_nap = merge_occ.iter().rev().find(|mo| mo.0 == c).map(|mo| mo.1);
                // (the cart ahead of the napper at the merge: the latest other cart into the merge
                // before the napper's entry (its leader in fact; the release's lead when the napper is
                // released before its leader reaches the merge would pair it with an older pass); its
                // clear, or the napper's entry when it had not cleared yet (an overlap: negative))
                let m_pred = match m_nap {
                    Some(en) => merge_occ
                        .iter()
                        .filter(|mo| mo.0 != c && mo.1 < en)
                        .max_by_key(|mo| mo.1)
                        .map(|mo| mo.2.unwrap_or(en + 1))
                        .or_else(|| merge_occ.iter().rev().find(|mo| mo.0 == lead && mo.1 <= ra).and_then(|mo| mo.2)),
                    None => merge_occ.iter().rev().find(|mo| mo.0 == lead && mo.1 <= ra).and_then(|mo| mo.2),
                };
                if let Some(en) = m_nap {
                    nap_merges.push((c, en));
                }
                let curve_rule = match (m_pred, m_nap) { (Some(cl), Some(en)) => format!("{}", en - cl), _ => "-".to_string() };
                if m_pred.is_some() && m_nap.is_some() {
                    curve_samples += 1;
                }
                // (the cart ahead at the merge, the lip margin against its rider (ironspec 21:14 (2)),
                // and this napper's PRESENT margin before that cart's press (21:14 (1)))
                let ahead = m_nap.and_then(|en| merge_occ.iter().filter(|mo| mo.0 != c && mo.1 < en).max_by_key(|mo| mo.1).map(|mo| mo.0));
                let lip: Result<Lip, &'static str> = match (m_nap, ahead) {
                    (None, _) => Err("no merge entry"),
                    (Some(_), None) => Err("no cart ahead at the merge"),
                    (Some(_), Some(a)) => {
                        let nt: Vec<(i64, f64, f64)> = nap_track.iter().filter(|k| k.1 == c && k.0 > ra).map(|k| (k.0, k.2, k.3)).collect();
                        let rt: Vec<(i64, f64, f64, f64, f64, f64)> = rider_track.iter().filter(|k| k.1 == a && k.0 > ra - 40).map(|k| (k.0, k.2, k.3, k.4, k.5, k.6)).collect();
                        Ok(lip_margin(&nt, &rt, m as f64))
                    }
                };
                lip_qs.push(lip);
                let lip_txt = match lip {
                    Ok(Lip::Margin(q)) => format!("{q} gt"),
                    Ok(Lip::Never) => "never reaches the rider".to_string(),
                    Ok(Lip::Held(n)) => format!("STUCK (reaches the rider at t {n}, the rider never below the floor)"),
                    Err(why) => format!("no sample ({why})"),
                };
                let pmargin = match (ahead.and_then(|a| last_pass[a]), nap_k_at, so.present_k) {
                    (Some(pa), Some(k), Some(d)) => Some(pa - (k + d)),
                    _ => None,
                };
                if let Some(pm) = pmargin {
                    present_margins.push(pm);
                }
                if let Some(pa) = ahead.and_then(|a| last_pass[a]) {
                    ksw_meas.push(ra - pa);
                }
                let c_nap = contacts.iter().filter(|k| k.0 == c && k.4).map(|k| k.1).fold(0.0f64, f64::max);
                let q_nap = contacts.iter().rev().find(|k| k.0 == c && k.4).map_or("-".to_string(), |k| format!("{}", k.3 as usize));
                // (its wait on the siding: from coming to rest there to its release (day-1's strand
                // margin and the siding's double occupancy))
                let waited = nap_rested_at.map(|r| ra - r);
                if let Some(w) = waited {
                    nap_wait_max = nap_wait_max.max(w);
                }
                nap_rows.push(format!(
                    "napper cart {c} (predecessor {pred}){}: the predecessor over the merge string t {}, at rest on the siding t {}, released t {ra} (waited {} gt on the siding), at the lane's back t {t} (z {z:.2}); the merge's curve rule (its entry minus the predecessor's clear) {curve_rule} gt; the lip margin {}; PRESENT before the leader's press {}; its contact {c_nap:.3} at q {q_nap}; braked creep {creep:.4}",
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
                    waited.map_or("-".to_string(), |w| w.to_string()),
                    lip_txt,
                    pmargin.map_or("-".to_string(), |p| format!("{p} gt"))
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
    // (a napper released but never back at the lane's back by the run's end (held at the merge or
    // the lip: no napper row closed for it) still gets its lip outcome (ironspec 00:02 (1): a rider
    // held at the lip while the napper reaches it is STUCK, never a missing sample))
    let mut inflight_lip: Option<String> = None;
    if let (Some((c, _)), Some(ra)) = (nap, released_at) {
        let m_nap = merge_occ.iter().filter(|mo| mo.0 == c && mo.1 > ra).map(|mo| mo.1).min();
        let ahead = m_nap.and_then(|en| merge_occ.iter().filter(|mo| mo.0 != c && mo.1 < en).max_by_key(|mo| mo.1).map(|mo| mo.0));
        let lip: Result<Lip, &'static str> = match (m_nap, ahead) {
            (None, _) => Err("no merge entry"),
            (Some(_), None) => Err("no cart ahead at the merge"),
            (Some(_), Some(a)) => {
                let nt: Vec<(i64, f64, f64)> = nap_track.iter().filter(|k| k.1 == c && k.0 > ra).map(|k| (k.0, k.2, k.3)).collect();
                let rt: Vec<(i64, f64, f64, f64, f64, f64)> = rider_track.iter().filter(|k| k.1 == a && k.0 > ra - 40).map(|k| (k.0, k.2, k.3, k.4, k.5, k.6)).collect();
                Ok(lip_margin(&nt, &rt, m as f64))
            }
        };
        lip_qs.push(lip);
        inflight_lip = Some(match lip {
            Ok(Lip::Margin(q)) => format!("{q} gt"),
            Ok(Lip::Never) => "never reaches the rider".to_string(),
            Ok(Lip::Held(n)) => format!("STUCK (reaches the rider at t {n}, the rider never below the floor)"),
            Err(why) => format!("no sample ({why})"),
        });
    }
    // (bars NOT JUDGED beyond the samples' zeros (ironspec 04:14 (2), (7)): pushed where they're
    // found, counted in the samples line)
    let mut extra_nj: Vec<String> = Vec::new();
    let mut lines = vec![format!(
        "siding bench: lane x {lx} in the budget's cells (the hold (x,16,0), R_1 (x,17,1), {ns} slopes, the merge (x,{m},{mz}) with its string, the siding (x-1,{m},{}) and its curve{}), {} carts ({}), the R line {} gt, the trapdoor {} gt every {} gt (corner {:+}), returns on the branch after {} gt at {:.2} b/t, a napper every {} drops back on the siding after {} gt, released {} gt after its predecessor's merge pass for {} gt; {} cycles{}",
        mz + 1,
        if so.drop_entry { format!("; the drop entry: the merge one level up, its north end over the drop (x,{m}..{},{}) onto the top slope, the wall (x,{m},{}) — one level and one wall block a lane", m - 1, mz - 1, mz - 2) } else { String::new() },
        o.per_lane, if o.riders { "ridden" } else { "empty" }, o.r_pulse, o.drop_w, o.period, o.corner, o.ret, o.v_in, so.nap_every, so.nap_ret, so.k, so.nap_pulse, o.cycles,
        if o.login.is_empty() && o.freeze32.is_empty() { String::new() } else { format!(", logins {:?} {:?}", o.login, o.freeze32) }
    )];
    // (G1 14:18 L7 / L8: the run's identity on its own line: the seed, the R_1 drive (a STAND-IN: an
    // overridden redstone block beside R_1 at the shut + --r-delay for --r-pulse, not the built R line
    // of Events 4 (all-shut + 2, the repeater's 1 gt)), the build position, the lip's depth and the
    // C variant)
    lines.push(format!(
        "the run (G1 Nov 13 14:18 L7): seed {}; R_1's drive a bench STAND-IN (a redstone block override at (x+1,17,1) from the shut + {} gt for {} gt{}; not Events 4's built R line){}; build position {:?}; lip_dy {}; {}{}{}",
        so.seed,
        so.r_delay,
        o.r_pulse,
        if so.r_until_string { ", cut at the hold string's press" } else { "" },
        if so.r_delay == 0 && o.r_pulse == 30 { " (bench R_1 pulse 30: a stand-in; the station's is 10)" } else { "" },
        so.pos_offset,
        so.lip_dy,
        if so.sbend_straight { "the C fix --sbend-straight" } else { "the S-bend of record" },
        if so.c2_booster { ", the c2 booster (priced, not adopted)" } else { "" },
        if so.j_rail { "; returns set on the rail west of J (--j-rail)" } else { "; returns summoned in J's support onto D" }
    ));
    lines.push(bed_start_line);
    lines.push(format!(
        "{t1_line}; riders without the bed's HOME at t 1 under --nap-ai (the bar: 0): {t1_unhomed}; riders not at REST at t 1 (the bar: 0): {t1_not_rest}; the bed's ticket not taken at t 1 under --nap-ai (the bar: 0): {}{}",
        t1_ticket_bad as usize,
        if t1_no_riders { "; NOT JUDGED: no rider at t 1 in a run with riders" } else { "" }
    ));
    if t1_no_riders {
        extra_nj.push("the riders at t 1 (none present in a run with riders)".to_string());
    }
    lines.push(format!("carts per release (exactly 1): {}; empty releases after the first (golems lost: a batch of 2 can't summon): {empty_releases}; late nappers (a lane-lap wait): {late_naps}", ph.iter().map(|(a, b2)| format!("{a}: {b2}")).collect::<Vec<_>>().join(", ")));
    lines.push(format!("nappers through the siding: {}", if nap_rows.is_empty() { "none".to_string() } else { nap_rows.join(" | ") }));
    {
        // (the lip margin over the nappers (ironspec 21:14 (2); three outcomes, 00:02 (1)) and their
        // PRESENT margins (21:14 (1)))
        let qs: Vec<i64> = lip_qs.iter().filter_map(|q| if let Ok(Lip::Margin(v)) = q { Some(*v) } else { None }).collect();
        let never = lip_qs.iter().filter(|q| matches!(q, Ok(Lip::Never))).count();
        let held = lip_qs.iter().filter(|q| matches!(q, Ok(Lip::Held(_)))).count();
        let no_ahead = lip_qs.iter().filter(|q| matches!(q, Err("no cart ahead at the merge"))).count();
        let no_merge = lip_qs.iter().filter(|q| matches!(q, Err("no merge entry"))).count();
        let samples = qs.len() + never + held;
        lines.push(format!(
            "the lip margin (ironspec 21:14 (2), 00:02 (1): the tick the napper's box front first reaches the leader's rider's box back face as the rider stood after the previous tick (napper first), minus the first tick after which the rider's box top is below the napper's floor y {m}.0, minus 1; -1 or less is contact (stuck)): margins {} over {} nappers; never reaching the rider (clean) {never}; STUCK (reaching the rider, the rider never below the floor; fails) {held}; samples {samples}{}; no sample: no cart ahead at the merge {no_ahead}, no merge entry {no_merge}",
            if qs.is_empty() { "-".to_string() } else { format!("{}..{} gt", qs.iter().min().unwrap_or(&0), qs.iter().max().unwrap_or(&0)) },
            qs.len(),
            if samples == 0 && so.nap_every > 0 { " (NOT JUDGED (0 samples))" } else { "" }
        ));
        lines.push(format!(
            "the nappers' PRESENT margins before their leader's press (ironspec 21:14 (1): the leader's press tick minus the napper's PRESENT tick; under 7 is outside normal operation): {} over {} nappers, {} under 7",
            if present_margins.is_empty() { "-".to_string() } else { format!("{}..{} gt", present_margins.iter().min().unwrap_or(&0), present_margins.iter().max().unwrap_or(&0)) },
            present_margins.len(),
            present_margins.iter().filter(|p| **p < 7).count()
        ));
        lines.push(format!(
            "the napper's settle on R (day-1, PRESENT's physical basis (PRESENT + max(6, settle) <= D - 1): from its first tick on R's cell ({sx},{rest_y},{rest_z}) to its first tick there at speed < 0.001, positions and speeds after the tick; the napper on K's cell at {}): {} over {} nappers",
            so.nap_feed_v.map_or("its spawn speed".to_string(), |v| format!("{v:.3} b/gt (phase {:.4})", so.nap_feed_phase)),
            if settles.is_empty() { "-".to_string() } else { format!("{}..{} gt", settles.iter().min().unwrap_or(&0), settles.iter().max().unwrap_or(&0)) },
            settles.len()
        ));
        // (ironspec 04:14 (9): the contacts before t 20 are judged too, against the same declared stops:
        // the summoned start's carts exist from t 0, so its settle can meet a cell like any later tick)
        let undeclared: Vec<String> = block_contacts
            .iter()
            .filter(|(k, _)| !declared_stops.contains(k))
            .map(|(k, v)| format!("({},{},{}) {} x{}", k.0, k.1, k.2, v.0, v.1))
            .chain(early_contacts.iter().filter(|(k, _)| !declared_stops.contains(k)).map(|(k, v)| format!("({},{},{}) {} x{} before t 20", k.0, k.1, k.2, v.0, v.1)))
            .collect();
        // (G1's CHANGES 3 (2): every declared stop's hits; a declared stop no cart ever met is STALE
        // and fails the row (a declaration that never applies hides nothing but names nothing either))
        {
            // (the returns' first ticks (G1 10:24), the drop passes (ironspec 10:42 (4)), the arrivals
            // at C with their gaps (onechunk 10:48: the night pair), the napper's rider box on R (the
            // presence tripwire, ironspec 10:49 (3)): printed, judged later against the game's envelope)
            for c in 0..n {
                if let Some((t0, off, lat, q)) = lip_open[c].take() {
                    lip_passes.push((t0, c, off, lat, q));
                }
            }
            let mm = |v: &mut Vec<f64>| -> String {
                if v.is_empty() {
                    return "-".to_string();
                }
                v.sort_by(|a, b| a.total_cmp(b));
                format!("{:.4}..{:.4} (median {:.4})", v[0], v[v.len() - 1], v[v.len() / 2])
            };
            let (mut fy, mut fz, mut fvx, mut fvz): (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for (y, z, vx, vzz) in &first_ticks {
                fy.push(*y);
                fz.push(*z);
                fvx.push(*vx);
                fvz.push(*vzz);
            }
            lines.push(format!(
                "the returns' first tick after being set on the track (G1 Nov 13 10:24; {}): {} returns; y {}; z {}; motion x {}; motion z {}",
                if so.j_rail { "--j-rail: each cart's own parked cart and rider set west of J moving east, the napper on K's cell" } else { "summoned in J's support (x+0.5, m+0.0625, mz+2.5) moving north, a fresh rider each" },
                first_ticks.len(),
                mm(&mut fy),
                mm(&mut fz),
                mm(&mut fvx),
                mm(&mut fvz)
            ));
            let mut offs: Vec<u32> = lip_passes.iter().map(|p| p.2).collect();
            offs.sort();
            let hist = {
                let mut h: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
                for o in &offs {
                    *h.entry(*o).or_default() += 1;
                }
                h.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
            };
            let max_lat = lip_passes.iter().map(|p| p.3).fold(0.0f64, f64::max);
            let mut worst: Vec<&(i64, usize, u32, f64, bool)> = lip_passes.iter().collect();
            worst.sort_by(|a, b| b.2.cmp(&a.2).then(b.3.total_cmp(&a.3)));
            lines.push(format!(
                "the drop passes (ironspec Nov 13 10:42 (4), G1 10:29 / 10:36; the envelope to compare with the game's normal passes): passes {}; ticks off the rail per pass (ticks:passes) [{hist}]; the largest lateral displacement from the lane's centre line {max_lat:.4}; passes off the rail with a cart queued behind (at C, D or J) {}; the longest 5 (t, lane cart, ticks off, lateral, queued) [{}]",
                lip_passes.len(),
                lip_passes.iter().filter(|p| p.4).count(),
                worst.iter().take(5).map(|p| format!("t {} c{} {} {:.3}{}", p.0, p.1, p.2, p.3, if p.4 { " Q" } else { "" })).collect::<Vec<_>>().join(", ")
            ));
            let gaps: Vec<i64> = c_arrivals.windows(2).map(|w| w[1].0 - w[0].0).collect();
            let mut gs = gaps.clone();
            gs.sort();
            let mut sp: Vec<f64> = c_arrivals.iter().map(|a| a.2).collect();
            lines.push(format!(
                "the arrivals at C (onechunk Nov 13 10:48: speed, z phase, spacing; the night pair): {} arrivals; speed {}; gaps between arrivals {}; gaps under 30 gt {}",
                c_arrivals.len(),
                mm(&mut sp),
                if gs.is_empty() { "-".to_string() } else { format!("{}..{} (median {})", gs[0], gs[gs.len() - 1], gs[gs.len() / 2]) },
                gs.iter().filter(|g| **g < 30).count()
            ));
            lines.push({
                let mut by_q: std::collections::BTreeMap<usize, (Vec<i64>, Vec<i64>)> = std::collections::BTreeMap::new();
                for (q, proxy, vol) in &descents_done {
                    let e = by_q.entry(*q).or_default();
                    e.0.push(*vol);
                    e.1.push(*proxy);
                }
                let never: std::collections::BTreeMap<usize, usize> = descents.iter().fold(std::collections::BTreeMap::new(), |mut m, d| {
                    *m.entry(d.2).or_default() += 1;
                    m
                });
                let lv = so.lip_volume.unwrap_or([1.345, 4.182, m as f64, m as f64 + 0.7, 5.940, 8.157]);
                format!(
                    "the descents from C past V (ironspec Nov 13 11:08 (2) / 11:11, day-1's sub-bar (3): from each arrival at C to the first tick after which neither the cart's box nor its rider's box meets the lip volume x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3}; beside it the proxy: the centre under z 5.7 below y 23), per queue depth at the arrival (the other carts in the slots): [{}]; not cleared by the run's end, per depth: [{}]",
                    lv[0],
                    lv[1],
                    lv[2],
                    lv[3],
                    lv[4],
                    lv[5],
                    by_q.iter_mut()
                        .map(|(q, (v, p))| {
                            v.sort();
                            p.sort();
                            format!("q {q}: {} arrivals, the lip volume left {}..{} gt (median {}), the proxy {}..{} gt", v.len(), v[0], v[v.len() - 1], v[v.len() / 2], p[0], p[p.len() - 1])
                        })
                        .collect::<Vec<_>>()
                        .join("; "),
                    never.iter().map(|(q, n)| format!("q {q}: {n}")).collect::<Vec<_>>().join(", ")
                )
            });
            lines.push({
                let mut by_q: std::collections::BTreeMap<usize, Vec<i64>> = std::collections::BTreeMap::new();
                for (q, dt) in &settles_done {
                    by_q.entry(*q).or_default().push(*dt);
                }
                format!(
                    "the settles from J to rest in the slots (G1 Nov 13 11:23, day-1's least-acceptance gap: from a cart's first tick in J's cell to its first tick at rest, speed < 0.001, in the lane's slots under y m - 1), per queue depth at its J tick: [{}]; not settled by the run's end {}",
                    by_q.iter_mut()
                        .map(|(q, v)| {
                            v.sort();
                            format!("q {q}: {} carts, {}..{} gt (median {})", v.len(), v[0], v[v.len() - 1], v[v.len() / 2])
                        })
                        .collect::<Vec<_>>()
                        .join("; "),
                    j_settles.len()
                )
            });
            lines.push(format!(
                "the lane carts without their rider (G1 Nov 13 11:25 gap (2), the presence string's blind case; printed, not a bar): {} cart-ticks{}",
                unridden.len(),
                unridden.first().map_or(String::new(), |(t, c)| format!(" (the first: t {t}, lane cart {c})"))
            ));
            lines.push(match rider_top_on_r {
                Some((lo, hi, zlo, k, ctop)) => format!(
                    "the napper's rider on R (the presence tripwire over R, ironspec Nov 13 10:49 (3) / 10:52 (a): the string's box at y 26.0625..26.15625; the bar: the rider's top over it by >= 0.05, the cart's top under it): the rider's box top {lo:.4}..{hi:.4} over {k} resting ticks (the lowest at the cart's z {zlo:.4}): margin {:.4}; the cart's box top at most {ctop:.4}: margin {:.4}",
                    lo - 26.0625,
                    26.0625 - ctop
                ),
                None => "the napper's rider on R (the presence tripwire over R, ironspec Nov 13 10:49 (3)): no resting tick with a rider".to_string(),
            });
        }
        let hits = |k: &(i32, i32, i32)| block_contacts.get(k).map_or(0, |v| v.1) + early_contacts.get(k).map_or(0, |v| v.1);
        let stale: Vec<String> = declared_stops.iter().filter(|k| hits(k) == 0).map(|k| format!("({},{},{})", k.0, k.1, k.2)).collect();
        lines.push(format!(
            "the carts' block contacts (ironspec 21:25 (4), by the shared mobfarm::contacts::cart_contacts (ironspec 02:41 / 02:48 / 02:52, amended 07:47): every cart's recorded move this tick (Entity.move(SELF, ...): the box before, the requested and the returned movement; the lane's carts, the K row's through-carts, the control cart), replayed in the game's collide order (y, then the larger horizontal axis of the request, then the other), a box counting only on an axis the collision clipped, within 0.011 of its face on the side the request goes and overlapping more than 0.01 on both other axes at that step; declared stops: the lanes file's # contacts: {} and the station's stand-ins {}): from t 20 {} cells ({}); before t 20 (the summoned start's settle, judged by the same stops): {} contacts at {} cells ({}); every declared stop's hits: {}; stale declared stops (never hit; the bar: 0): {}{}; undeclared (the bar: 0): {}{}",
            if lanes_contacts.is_empty() { "none".to_string() } else { lanes_contacts.iter().map(|k| format!("({},{},{})", k.0, k.1, k.2)).collect::<Vec<_>>().join(" ") },
            stand_in_stops.iter().map(|k| format!("({},{},{})", k.0, k.1, k.2)).collect::<Vec<_>>().join(" "),
            block_contacts.len(),
            block_contacts.iter().map(|(k, v)| format!("({},{},{}) x{}", k.0, k.1, k.2, v.1)).collect::<Vec<_>>().join(" "),
            early_contacts.values().map(|v| v.1).sum::<usize>(),
            early_contacts.len(),
            early_contacts.iter().map(|(k, v)| format!("({},{},{}) {} x{}", k.0, k.1, k.2, v.0, v.1)).collect::<Vec<_>>().join(" "),
            declared_stops.iter().map(|k| format!("({},{},{}) x{}", k.0, k.1, k.2, hits(k))).collect::<Vec<_>>().join(" "),
            stale.len(),
            if stale.is_empty() { String::new() } else { format!(" [{}]", stale.join(" ")) },
            undeclared.len(),
            if undeclared.is_empty() { String::new() } else { format!(" [{}]", undeclared.join("; ")) }
        ));
    }
    {
        // (the follower's margin behind each napper at the merge: the next cart's entry minus the
        // napper's clear; a napper without a figure is counted: no merge clear by the run's end (it
        // entered in the run's last 100 gt) or never cleared (entered earlier), or cleared with no
        // cart after it before the run's end)
        let mut no_clear_end = 0usize;
        let mut never_cleared = 0usize;
        let mut no_next = 0usize;
        let follow: Vec<i64> = nap_merges
            .iter()
            .filter_map(|(c, en)| {
                let Some(clear) = merge_occ.iter().find(|mo| mo.0 == *c && mo.1 == *en).and_then(|mo| mo.2) else {
                    if *en >= ticks - 100 {
                        no_clear_end += 1;
                    } else {
                        never_cleared += 1;
                    }
                    return None;
                };
                let Some(next) = merge_occ.iter().filter(|mo| mo.1 > *en && mo.0 != *c).map(|mo| mo.1).min() else {
                    no_next += 1;
                    return None;
                };
                Some(next - clear)
            })
            .collect();
        follow_samples = follow.len();
        lines.push(format!(
            "the cart after each napper into the merge (its entry minus the napper's clear): {} over {} nappers; without a figure: {no_clear_end} with no merge clear at the run's end (entered in its last 100 gt), {never_cleared} never cleared (entered earlier), {no_next} cleared with no cart after it before the run's end",
            if follow.is_empty() { "-".to_string() } else { format!("{}..{} gt", follow.iter().min().unwrap_or(&0), follow.iter().max().unwrap_or(&0)) },
            follow.len()
        ));
    }
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
    lines.push(format!("the lane's occupancy at the releases from the steady state (t {steady_from}): the largest {occ_max}, releases at 8 (the 8th slot is the top cell; the bar: 0): {occ8}; before it (the start's full lane) up to {occ_start_max}; the smallest at a release from the steady state {}; releases below 6 (rotation below 7: a summoner's GOLEM_DETECTED_RECENTLY lasts 600 gt, ironspec 19:29 (4); the bar: 0): {occ_low}", if occ_min == usize::MAX { "-".to_string() } else { occ_min.to_string() }));
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
        "the nappers' launches from the rest (its centre past R's north end after the release; the rest's support {}): {} launches, departure {} gt after the release, speed then {}; stuck (still on the rest 40 gt after its release; the bar: 0): {launch_stuck}; the launch threshold (the latest departure, counted from the switch; R powers {} gt after it) {} gt against the pulse {} gt; launches after the pulse ended (R unpowered at the departure tick; the bar: 0): {}",
        if so.siding_north { so.r_support.as_deref().unwrap_or("minecraft:stone") } else { "-" },
        launches.len(),
        if launches.is_empty() { "-".to_string() } else { format!("{}..{}", launches.iter().map(|l| l.0).min().unwrap_or(0), launches.iter().map(|l| l.0).max().unwrap_or(0)) },
        if launches.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", launches.iter().map(|l| l.1).fold(f64::MAX, f64::min), launches.iter().map(|l| l.1).fold(0.0f64, f64::max)) },
        drive_delay,
        launches.iter().map(|l| l.0).max().map_or("-".to_string(), |d| d.to_string()),
        so.nap_pulse,
        launches.iter().filter(|l| !l.2).count()
    ));
    {
        let rolling: Vec<&(i64, bool, f64, f64)> = release_states.iter().filter(|r| r.1).collect();
        let ks: Vec<i64> = release_states.iter().map(|r| r.0).filter(|k| *k >= 0).collect();
        lines.push(format!(
            "the nappers at their release (the PRESENT bit: {}): {} releases, {} while still rolling onto R (not yet at rest; their z {} and speed {} at the release), the release {} gt after the napper's K tick; R powers {drive_delay} gt after the release; the napper's K tick to its rest on R {} gt",
            match so.present_k { Some(d) => format!("{d} gt after the napper's K tick"), None => "the napper at rest on R".to_string() },
            release_states.len(),
            rolling.len(),
            if rolling.is_empty() { "-".to_string() } else { format!("{:.2}..{:.2}", rolling.iter().map(|r| r.2).fold(f64::MAX, f64::min), rolling.iter().map(|r| r.2).fold(f64::MIN, f64::max)) },
            if rolling.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", rolling.iter().map(|r| r.3).fold(f64::MAX, f64::min), rolling.iter().map(|r| r.3).fold(f64::MIN, f64::max)) },
            if ks.is_empty() { "-".to_string() } else { format!("{}..{}", ks.iter().min().unwrap_or(&0), ks.iter().max().unwrap_or(&0)) },
            if transits.is_empty() { "-".to_string() } else { format!("{}..{}", transits.iter().min().unwrap_or(&0), transits.iter().max().unwrap_or(&0)) }
        ));
    }
    {
        // (ironspec 04:14 (7), 05:03: the PRESENT bar every row enforces is the one its settle implies:
        // m >= max(L_latch + 1, settle - (k_sw + the feed delay)) + 2 (ironspec 01:33 (1); day-1's
        // derivation, accepted 05:03: k_sw + the feed delay = D press -> R powered, 7 = L_latch + 1,
        // + 2 the world-open lead), with k_sw (the leader's press -> the napper's release) and the feed
        // delay (the release -> R powered) READ FROM THE DESIGN AS IT RUNS in this row, each its least
        // (the larger bar); settle = PRESENT -> at rest = the napper's K tick -> rest on R minus 2
        // (PRESENT = K + 2), this row's worst; a row whose nappers never came to rest takes the rest
        // sweep's worst (--present-settle-worst, 22 gt at arrival 0.10); a row with no release to
        // measure k_sw or the feed delay on has no PRESENT margin to judge)
        let own = transits.iter().max().map(|k| k - 2);
        let settle = own.unwrap_or(so.present_settle_worst);
        let (ksw, feed) = (ksw_meas.iter().min().copied(), feed_meas.iter().min().copied());
        let lead = ksw.zip(feed).map(|(a, b)| a + b);
        // (ironspec 06:08 real: the world-open lead + 2 applies to the settle term always (R powering,
        // redstone from D, must not beat the napper coming to rest); to the latch term only when the
        // PRESENT -> latch -> AND path is not confirmed triggered and unclocked: with
        // --present-latch-unclocked (day-1's confirmation, declared) the bar is max(7, settle - lead + 2),
        // without it max(7, settle - lead) + 2, the floor 9)
        let bar = lead.map(|l| if so.present_latch_unclocked { 7.max(settle - l + 2) } else { 7.max(settle - l) + 2 });
        let under: Vec<i64> = bar.map_or(Vec::new(), |b| present_margins.iter().copied().filter(|p| *p < b).collect());
        lines.push(format!(
            "the PRESENT proxy bar (information, ironspec 06:15 real: the bar is the napper at rest when R powers, judged on the next line; ironspec 04:14 (7), 05:03, 06:08 real; ironspec 01:33 (1) with day-1's derivation; {}): k_sw (the leader's press -> the release, read from this run) {} gt, the feed delay (the release -> R powered, read from this run) {} gt, their sum {}; the settle (PRESENT -> at rest) {} gt ({}) -> the proxy bar {} gt; PRESENT margins {} over {} nappers; under the proxy bar (information): {}",
            if so.present_latch_unclocked {
                "m >= max(L_latch + 1 = 7, settle - (k_sw + the feed delay) + 2): the world-open lead on the settle term only, --present-latch-unclocked declared (day-1's confirmation that the PRESENT -> latch -> AND path is triggered and unclocked)"
            } else {
                "m >= max(L_latch + 1 = 7, settle - (k_sw + the feed delay)) + 2: the world-open lead on both terms, the floor 9 (--present-latch-unclocked not declared: day-1's confirmation of an unclocked latch path pending)"
            },
            if ksw_meas.is_empty() { "-".to_string() } else { format!("{}..{}", ksw_meas.iter().min().unwrap_or(&0), ksw_meas.iter().max().unwrap_or(&0)) },
            if feed_meas.is_empty() { "-".to_string() } else { format!("{}..{}", feed_meas.iter().min().unwrap_or(&0), feed_meas.iter().max().unwrap_or(&0)) },
            lead.map_or("-".to_string(), |l| format!("{l} gt (the least)")),
            settle,
            if own.is_some() { "this row's worst napper" } else { "no napper came to rest in this row: the rest sweep's worst, --present-settle-worst" },
            bar.map_or("-".to_string(), |b| b.to_string()),
            if present_margins.is_empty() { "-".to_string() } else { format!("{}..{} gt", present_margins.iter().min().unwrap_or(&0), present_margins.iter().max().unwrap_or(&0)) },
            present_margins.len(),
            under.len()
        ));
        // (G1 (2): every napper release is judged once: R powered after it (the outcome), or R never
        // powered after it (a failure); a release in the run's last drive delay + 2 gt is printed apart)
        let mut never = r_never_powered.clone();
        let mut ended_waiting = 0usize;
        if let Some(t0) = feed_wait {
            if ticks - t0 > drive_delay + 2 {
                never.push(t0);
            } else {
                ended_waiting += 1;
            }
        }
        lines.push(format!(
            "the napper at rest when R powers (ironspec 06:15 real: the outcome the PRESENT bar proxies, judged directly; R's power read from the world (its block's powered state); at rest = the cart's block position R's cell and speed < 0.001; the moment read (G1, the tie): the napper as it stood at the end of the tick BEFORE the one whose block phase powered R): napper releases {nap_release_n}; releases judged {power_judged}; R powered with the napper not at rest (the bar: 0): {}{}; R never powered after a release (the bar: 0): {}{}; a release in the run's last {} gt (not judged) {ended_waiting}; per release the napper's first tick at rest -> R's first power reading (the tie at 0: R powered in the block phase of the tick the napper came to rest) [{}]; the switch on -> R powered {} gt (the design's own feed), the release -> R powered {} gt",
            not_rest_at_power.len(),
            if not_rest_at_power.is_empty() { String::new() } else { format!(" (t {})", not_rest_at_power.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")) },
            never.len(),
            if never.is_empty() { String::new() } else { format!(" (released t {})", never.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")) },
            drive_delay + 2,
            rest_power_ticks.iter().map(|(q, p)| match q { Some(q) => format!("{q} -> {p} ({})", p - q), None => format!("never at rest -> {p}") }).collect::<Vec<_>>().join(", "),
            if feed_from_switch.is_empty() { "-".to_string() } else { format!("{}..{}", feed_from_switch.iter().min().unwrap_or(&0), feed_from_switch.iter().max().unwrap_or(&0)) },
            if feed_meas.is_empty() { "-".to_string() } else { format!("{}..{}", feed_meas.iter().min().unwrap_or(&0), feed_meas.iter().max().unwrap_or(&0)) }
        ));
        // (G1's CHANGES on bc25c5cca (2) / (8): the drive delay the bench builds the feed for is
        // CHECKED, not assumed: every release's measured switch on -> R powered must equal it, and its
        // release -> R powered must equal it less --early-r's shift (0 without); a release whose feed
        // differs fails the row)
        let shift = so.early_r.unwrap_or(0);
        let mut feed_off: Vec<String> = feed_from_switch.iter().zip(feed_meas.iter()).filter(|(a, b)| **a != drive_delay || **b != drive_delay - shift).map(|(a, b)| format!("{a}/{b}")).collect();
        // (ironspec 10:54 (2)(c) on 1788d4f65: a judged release with one of its two measurements missing
        // fails, never truncated away by the pairing)
        for _ in 0..feed_meas.len().abs_diff(feed_from_switch.len()) {
            feed_off.push("a release with one measurement missing".to_string());
        }
        lines.push(format!(
            "the feed's measured delays against the design (G1's CHANGES on bc25c5cca (2), (8): the switch on -> R powered must be the drive delay {drive_delay} gt, the release -> R powered {} gt): releases measured {}; releases whose feed differs (the bar: 0): {}{}",
            drive_delay - shift,
            feed_meas.len().min(feed_from_switch.len()),
            feed_off.len(),
            if feed_off.is_empty() { String::new() } else { format!(" (switch/release -> R: {})", feed_off.join(", ")) }
        ));
    }
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
    lines.push(format!("ticks with two carts over the merge (the curve rule's touching; the bar: 0): {touching}; with a napper in the merge cell: beside its leader {touch_leader} (the cart ahead its lip margin measures, its rider still above the napper's floor: the lip's contact), beside its leader after its rider dropped {touch_leader_late} (a leader stuck in the merge), beside another cart {touch_follower} (its follower, or a cart still in the merge){}", if touch_note.is_empty() { String::new() } else { format!(" ({})", touch_note.join("; ")) }));
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
    {
        // (tick-exact 12:27: the napper's launch from R: the rest z before, the step entering C (or the
        // drop cell with --sbend-straight))
        lines.push(format!(
            "the napper's launches from R (tick-exact Nov 13 12:27: a napper rested low on R stalls at the lip at stored 0.62 from z 7.189, entering C at a step of ~0.11; 0.30 from z 7.574 enters at ~0.19 and falls; the launch's own rest bar is pending): {} launches; (the rest z on R before -> the step entering {}): [{}]",
            nap_launches.len(),
            if so.sbend_straight { "the drop cell from the west" } else { "C" },
            nap_launches.iter().map(|(t, rz, s)| format!("t {t}: {} -> {s:.4}", rz.map_or("-".to_string(), |z| format!("{z:.4}")))).collect::<Vec<_>>().join(", ")
        ));
    }
    {
        // (dfb89499 / G1 12:26: R_1's pulse against the refill's first contact, and the up-slope bar)
        let durs: Vec<i64> = r_on_at.iter().filter_map(|on| r_off_at.iter().find(|off| **off > *on).map(|off| off - on)).collect();
        let margins: Vec<(i64, i64)> = refill_contacts.iter().filter_map(|(on, ct)| r_off_at.iter().find(|off| **off > *on).map(|off| (*ct, off - ct))).collect();
        let late: Vec<String> = margins.iter().filter(|m| m.1 >= 0).take(6).map(|m| format!("t {} +{}", m.0, m.1)).collect();
        let span = |v: &[i64]| if v.is_empty() { "-".to_string() } else { format!("{}..{}", v.iter().min().unwrap_or(&0), v.iter().max().unwrap_or(&0)) };
        lines.push(format!(
            "R_1's pulse against the refill (dfb89499 Nov 13 12:26: the station's R_1 powered 10 gt; G1 12:26: the pulse must end BEFORE the second cart reaches the front cart; this run: --r-pulse {}{}): R_1 on {} times, for {} gt; the refills' first cart-cart contact in slots 1-2 after R_1 on: {} refills, R_1's off minus the contact {} gt (negative: off before the contact); refills with R_1 still powered at the contact (printed, not a bar): {}{}; lane carts moving up-slope on the slopes (day-1 Nov 13 12:26, stored motion south on z 1.0..6.6; printed, not a bar: the bar is the displacement below): cart-ticks over 0.001 b/t {upslope_any}, over 0.05 b/t {upslope_big}; the fastest {:.4} b/t at t {} (lane cart {})",
            o.r_pulse,
            if so.r_until_string { ", --r-until-string: off once the hold string (x,17,0) presses" } else { "" },
            r_on_at.len(),
            span(&durs),
            refill_contacts.len(),
            span(&margins.iter().map(|m| m.1).collect::<Vec<_>>()),
            margins.iter().filter(|m| m.1 >= 0).count(),
            if late.is_empty() { String::new() } else { format!(" [{}]", late.join(", ")) },
            upslope_max.0,
            upslope_max.1,
            upslope_max.2
        ));
        lines.push(format!(
            "the queue's net up-slope displacements (G1 Nov 13 12:32, the rollback's fallback bar: a lane cart's move up the slopes more than 0.1 block from its last rest, once per excursion; the bar: 0): {} [{}]{}",
            rollbacks.len(),
            rollbacks.iter().take(6).map(|(t, c, d)| format!("t {t} lane cart {c} +{d:.4}")).collect::<Vec<_>>().join(", "),
            upslope_planted.map_or(String::new(), |(t, c, d)| format!("; the edge control's plant: lane cart {c} moved up-slope by {d:.4} at t {t} ({upslope_plant_note}, read back from the world)"))
        ));
        let col = |k: usize| -> String {
            let v: Vec<i64> = refill_timing.iter().filter_map(|r| match k {
                1 => r.1,
                2 => r.2,
                _ => r.3,
            }).collect();
            if v.is_empty() { "-".to_string() } else { format!("+{}..+{} ({})", v.iter().min().unwrap_or(&0), v.iter().max().unwrap_or(&0), v.len()) }
        };
        lines.push(format!(
            "the refills' timing from R_1 on (ironspec Nov 13 13:04, the calibration against dfb89499's drop-bench --refill 2,10 (its row: the next cart moves +1, leaves R_1's cell +4, rests in the hold +13 from the rail's power)): {} refills; the next cart moves {}; leaves R_1's cell north {}; rests in the hold {}; refills not completed in 60 gt (printed, not a bar): {}",
            refill_timing.len(),
            col(1),
            col(2),
            col(3),
            refill_timing.iter().filter(|r| r.3.is_none()).count()
        ));
        lines.push(format!(
            "the refills with no cart at rest on R_1's cell at the power (their 'moves' is not timed): {} of {}",
            refill_on_r1.iter().filter(|v| v.is_empty()).count(),
            refill_on_r1.len()
        ));
    }
    {
        // (day-1 12:09 sub-bar (iv), ironspec 12:08 (3), G1 12:12: the lip arrivals by class, and each
        // off-rail rest classified by its arrival's class)
        // (the pass windows still open at the run's end close here; each off-rail rest takes the class
        // of its cart's latest lip arrival at or before the rest)
        for a in lip_lane.finish() {
            let c = a.cart as usize;
            let occupants: Vec<(usize, &'static str)> = a
                .occupants
                .iter()
                .map(|(j, k)| {
                    (*j as usize, match k {
                        crate::lipclass::OccKind::Rollback => "rollback",
                        crate::lipclass::OccKind::Descending => "descending",
                        crate::lipclass::OccKind::Other => "other",
                    })
                })
                .collect();
            lip_arrivals.push((a.tick, c, a.others_in_slots, a.gap, occupants, a.class.name()));
            lip_steps.push((a.step, nap.map(|(nc, _)| nc) == Some(c)));
        }
        let arrival_before = |c: usize, t: i64| -> Option<usize> { lip_arrivals.iter().enumerate().filter(|(_, a)| a.1 == c && a.0 <= t).max_by_key(|(_, a)| a.0).map(|(k, _)| k) };
        for r in off_rest_class.iter_mut() {
            r.2 = arrival_before(r.1, r.0).map_or("no lip arrival".to_string(), |k| lip_arrivals[k].5.clone());
        }
        let mut by: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for a in &lip_arrivals {
            *by.entry(a.5.as_str()).or_default() += 1;
        }
        let occupied: Vec<String> = lip_arrivals
            .iter()
            .filter(|a| !a.4.is_empty() || a.5 != "clear")
            .take(6)
            .map(|a| format!("t {} c{} {} (others in the slots {}, gap {}, in the lip volume [{}])", a.0, a.1, a.5, a.2, a.3.map_or("-".to_string(), |g| g.to_string()), a.4.iter().map(|o| format!("c{} {}", o.0, o.1)).collect::<Vec<_>>().join(", ")))
            .collect();
        let mut rest_by: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for r in &off_rest_class {
            *rest_by.entry(r.2.as_str()).or_default() += 1;
        }
        let unclassified = off_rest_class.iter().filter(|r| r.2 == "UNCLASSIFIED" || r.2 == "clear" || r.2 == "no lip arrival").count();
        // (G1 12:29 (3): the entry step per source; the least that cleared and the steps of the arrivals
        // that rested off-rail)
        let src_steps = |nap_src: bool| -> String {
            let v: Vec<f64> = lip_steps.iter().filter(|s| s.1 == nap_src && s.0.is_finite()).map(|s| s.0).collect();
            if v.is_empty() { "none".to_string() } else { format!("{:.4}..{:.4} ({})", v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max), v.len()) }
        };
        let rest_steps: Vec<String> = off_rest_class
            .iter()
            .filter_map(|r| arrival_before(r.1, r.0).map(|k| format!("t {} c{} {:.4}{}", r.0, r.1, lip_steps[k].0, if lip_steps[k].1 { " (napper)" } else { "" })))
            .take(8)
            .collect();
        lines.push(format!(
            "the lip arrivals' entry steps (G1 Nov 13 12:29 (3): the horizontal move the tick a cart's centre enters the drop cell; tick-exact: 0.19 onto C clears, 0.11 stalls; the class 'slow entry' under 0.19): batch carts {}; nappers {}; the arrivals that rested off-rail: [{}]",
            src_steps(false),
            src_steps(true),
            rest_steps.join(", ")
        ));
        lines.push(format!(
            "the lip arrivals (day-1 Nov 13 12:09 sub-bar (iv), ironspec 12:08 (3), G1 12:12: a cart's centre entering the drop cell, with every other cart's and rider's box in the lip volume at that tick; classes: a 9th cart (8 others in the slots), rollback (an occupant that had rested in the slots and rose back into V's volume), a gap under 23 (ironspec 12:39: Δ*_worst 13 + 10; an occupant that reached the lip within 40 gt, or the previous lip arrival within 23 gt), UNCLASSIFIED (occupied otherwise); the bar for (iv) is the windowed V-empty check, not this diagnostic: printed, not a bar): {} arrivals, by class [{}]; occupied or classed (first 6): [{}]; the off-rail rests by their arrival's class [{}]; rests not explained by the V-empty sub-bars (a third mechanism; printed, not a bar): {unclassified}",
            lip_arrivals.len(),
            by.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", "),
            occupied.join("; "),
            rest_by.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")
        ));
    }
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
            "a napper still waiting at the end: cart {c} (predecessor pass {:?}, released {:?}), {waited} gt since its drop{}{}",
            pred_pass,
            released_at,
            if waited > lap + so.nap_ret { format!(" — stuck (over a lane-lap, {lap} gt, past its return)") } else { format!(" — within a lane-lap ({lap} gt) of its return: the run's end cut its wait") },
            inflight_lip.as_ref().map_or(String::new(), |l| format!("; released but not back at the lane's back: its lip outcome {l}"))
        ));
    }
    lines.push(format!(
        "the siding's single occupancy (push-scan's pairs R ({sx},{rest_y},{rest_z}) <- K ({sx},{},{}), a cart turning at K into F, by cell transition (ironspec 04:14 (8)); ironspec 19:58): ticks with a cart on R while another rode F, moved out of K into F or R, or moved north in K at 45 degrees or steeper, or two carts on R (the bar: 0): {r_joint}; carts moving out of K's cell into F's or R's over the run {k_turns}; each next napper's K tick minus the previous napper's departure from R: {} over {} pairs (a reported margin, no bar; its contact boundary, about 12 gt (the K -> R push-range travel), is ESTIMATED; the pair's gate is the joint count above)",
        m + 1,
        mz + 2,
        if nap_gaps.is_empty() { "-".to_string() } else { format!("{}..{} gt", nap_gaps.iter().min().unwrap_or(&0), nap_gaps.iter().max().unwrap_or(&0)) },
        nap_gaps.len()
    ));
    lines.push(format!(
        "the resting napper across the K row's passes (ironspec 23:15 (2): push-scan's pairs R ({sx},{rest_y},{rest_z}) <- K ({sx},{},{}); redsim's AbstractMinecart.push, its 0.8-alignment rule included): {push_passes} passes while a napper rested on R (the samples); its largest displacement from where it came to rest during them {push_disp_max:.6}, its largest speed {push_v_max:.6} (the bar: 0); ticks a K-zone cart's push box (its box grown 0.2) met the resting napper's box {pass_overlap}; the least box gap between a passing cart's box and the napper's push box {} (positive is clear){}",
        m + 1,
        mz + 2,
        if th_zgap == f64::MAX { "none".to_string() } else { format!("{th_zgap:.3}") },
        match (so.inject_pass, inject_rest_z) {
            (Some(_), Some(z)) => format!("; the push grid's injected pass (ironspec 04:14 (3c)): the napper at rest at z {z:.4} at the pass"),
            (Some(_), None) => "; the push grid's injected pass: never injected (the napper never came to rest)".to_string(),
            _ => String::new(),
        }
    ));
    {
        // (the push bar's computed scope (ironspec 03:00): the napper's windows on R (every span its
        // centre is in R's cell, resting or still rolling onto it: a superset of its rest windows)
        // against every K-row pass tick, both printed, so an empty intersection is shown, not
        // asserted; a row with no pass in a window is out of scope only while its K row is alive
        // (passes at other times, G1's guard), and each of its rest positions names the push-grid row
        // covering it (a rest no grid row covers fails the row; a napper released before it came to
        // rest has no rest position to cover))
        let mut windows: Vec<(i64, i64)> = r_windows.iter().map(|w| (w.1, w.2)).collect();
        let rest_ws: Vec<(i64, i64)> = rest_windows.iter().copied().chain(nap_rested_at.filter(|_| released_at.is_none()).map(|r| (r, ticks))).collect();
        for w in rest_ws.iter().copied() {
            if !windows.iter().any(|v| w.0 >= v.0 && w.1 <= v.1) {
                windows.push(w);
            }
        }
        windows.sort();
        let meet = k_pass_ticks.iter().filter(|pt| windows.iter().any(|w| **pt >= w.0 && **pt <= w.1)).count();
        // (ironspec 04:14 (2): the push is measured only while the napper rests; a pass meeting it
        // while it rolls on R or launches measures nothing, so those ticks are NOT JUDGED (a failure))
        let meet_rolling = k_pass_ticks.iter().filter(|pt| windows.iter().any(|w| **pt >= w.0 && **pt <= w.1) && !rest_ws.iter().any(|w| **pt >= w.0 && **pt <= w.1)).count();
        let ranges = {
            let mut v: Vec<(i64, i64)> = Vec::new();
            for &pt in &k_pass_ticks {
                match v.last_mut() {
                    Some(l) if pt == l.1 + 1 => l.1 = pt,
                    _ => v.push((pt, pt)),
                }
            }
            v.iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}..{b}") }).collect::<Vec<_>>().join(" ")
        };
        let mut uncovered = 0usize;
        if meet_rolling > 0 {
            extra_nj.push(format!("the K-row push bar: {meet_rolling} pass ticks met the napper while it rolled on R or launched, where the push is measured only at rest"));
        }
        let verdict = if so.k_row.is_none() {
            "no K row in this run".to_string()
        } else if k_pass_ticks.is_empty() {
            // (counted NOT JUDGED in the samples line: the K row's passes at K 0)
            "NOT JUDGED: no K-row cart passed K in the run (a dead K row)".to_string()
        } else if meet > 0 {
            format!("judged where they meet at rest ({} ticks; the passes above){}", meet - meet_rolling, if meet_rolling > 0 { format!("; NOT JUDGED where they meet the rolling napper ({meet_rolling} ticks)") } else { String::new() })
        } else {
            // (each rest z must be covered by a grid row that met passes and read no displacement
            // (ironspec 04:14 (3a): the covering row's content, not only its z))
            let cover: Vec<String> = rest_zs
                .iter()
                .map(|z| match so.push_grid.as_ref().and_then(|g| g.iter().filter(|r| (r.z - z).abs() <= 0.0015).min_by(|a, b| (a.z - z).abs().total_cmp(&(b.z - z).abs()))) {
                    Some(r) if r.passes > 0 && r.disp <= 1e-9 && r.speed <= 1e-9 => format!("rest z {z:.4}: the grid row arrival {:.3} phase {:.4} (rest z {:.4}, {} passes, displacement {:.6})", r.arrival, r.phase, r.z, r.passes, r.disp),
                    Some(r) => {
                        uncovered += 1;
                        format!("rest z {z:.4}: NOT COVERED: the nearest grid row (rest z {:.4}) has {} passes, displacement {:.6}, speed {:.6} (a cover needs passes > 0 and 0 displacement)", r.z, r.passes, r.disp, r.speed)
                    }
                    None => {
                        uncovered += 1;
                        format!("rest z {z:.4}: NOT COVERED by the push grid{}", if so.push_grid.is_none() { " (no --push-grid given)" } else { " (no row within 0.0015)" })
                    }
                })
                .collect();
            format!(
                "push: out of scope (computed: no K-row pass falls in a window on R; the K row alive); {}",
                if cover.is_empty() { "no rest position (each napper released before it came to rest on R)".to_string() } else { cover.join("; ") }
            )
        };
        lines.push(format!(
            "the K-row push bar's scope (ironspec 03:00): the napper's windows on R (in R's cell, resting or rolling onto it) {}; the K-row pass ticks [{ranges}]; ticks where they meet {meet}; {verdict}; rests the push grid doesn't cover (the bar: 0): {uncovered}",
            if windows.is_empty() { "none".to_string() } else { windows.iter().map(|w| format!("[{}, {}]", w.0, w.1)).collect::<Vec<_>>().join(" ") }
        ));
    }
    // (ironspec 19:07, the bar audit: every inherited number this bench enforces, printed "inherited",
    // with its physical basis first where one exists; the other bars are physical: game rules or
    // measured boundaries, see designs/circuits/mobfarm/tracks/ironfarm/NOTES.md)
    lines.push(
        "the bars' numbers that are inherited (no failure measured at them; physical basis first): the arrivals at the lanes' backs and the landings <= 0.46 \
         (inherited: the brief's gravity-run rule; physical: the bump bound 0.508, official); the calm-REST stamp <= 1,000 gt (inherited margin; physical: vanilla's \
         tooLongUnreachableDuration 1,200); releases at occupancy 8 and V held 0 (inherited: ironspec's slot-8 rulings); lane carts on the siding's cells 0 \
         (inherited: the S-bend ruling); exactly 1 cart per release (inherited: the brief's 3 a slot from 3 lanes); windows cut short 0 (inherited: day-1's ring); \
         this bench's windows (inherited: mine): at rest off the rails 20 gt, V held 20 gt, the napper on R 40 gt after its release, a stray power past the drive delay + 2 gt"
            .to_string(),
    );
    {
        // (ironspec 04:07: every run-time override prints its cell's state before and after it is laid;
        // one that leaves its cell unchanged is a silent no-op and fails the row)
        lines.push(format!(
            "the bench's overrides (ironspec 04:07: an override that leaves its cell unchanged fails): {}; every override's cell before -> after: {}; the controls' cells: {}; no-op overrides (the bar: 0): {}",
            if overrides.counts.is_empty() { "none".to_string() } else { overrides.counts.iter().map(|(k, (n, z))| format!("{k} {n} sets, {z} no-ops")).collect::<Vec<_>>().join("; ") },
            overrides.transitions(),
            if overrides.notes.is_empty() { "none".to_string() } else { overrides.notes.join("; ") },
            overrides.noops()
        ));
    }
    {
        // (ironspec 00:02 (3), G1's note (d): every control in this run asserts its own expected
        // failure: a control that does not bite fails the run)
        let mut ctl: Vec<String> = Vec::new();
        let mut ctl_notes: Vec<String> = Vec::new();
        let mut ctl_fail = 0usize;
        let mut ctl_n = 0usize;
        // (each control's kind (G1's CHANGES on bc25c5cca (1)): a POSITIVE control names its own bar,
        // which must fail in the row for the control to be caught; a NEGATIVE control (expected 0)
        // names none, and its run follows the plain-row rule)
        let mut ctl_keys: Vec<String> = Vec::new();
        let mut judge = |own: Option<&'static str>, what: String, ok: bool, seen: String, pass_txt: &str, fail_txt: &str| {
            ctl_n += 1;
            if !ok {
                ctl_fail += 1;
            }
            ctl_keys.push(match own { Some(b) => format!("P({b})={}", ok as u8), None => format!("N={}", ok as u8) });
            ctl.push(format!("{what}: {seen}: {}", if ok { pass_txt } else { fail_txt }));
        };
        // (each control tick's distance from the K row's spawns (release + k_offsets): a control placed
        // on a K-row pass meets it (ironspec 02:18: chosen clear of them, printed))
        let spawn_gap = |tc: i64| -> String {
            rel_times.iter().flat_map(|r| so.k_offsets.iter().map(move |k| r + k)).min_by_key(|s| (s - tc).abs()).map_or("no K-row spawn".to_string(), |s| format!("{} gt from the nearest K-row spawn (t {s})", (tc - s).abs()))
        };
        // (G1 12:32's edge control on the rollback's fallback bar: a plant of D > 0.1 is a POSITIVE
        // control (its own bar must count it), D <= 0.1 a NEGATIVE one (the plant must not count))
        if let Some((d, tc)) = so.upslope_ctl {
            let counted = upslope_planted.is_some_and(|(tp, c, _)| rollbacks.iter().any(|r| r.0 >= tp && r.1 == c));
            let planted = upslope_planted.is_some();
            let seen = format!("planted {} at t {tc} (D {d:.4}); counted by the bar: {counted}", if planted { "yes" } else { "NO (no cart at rest on the slopes)" });
            if d > 0.1 + 1e-9 {
                judge(Some("net up-slope displacement over 0.1 block"), "the rollback bar's edge control above the edge (G1 12:32: D > 0.1 must count)".to_string(), planted && counted, seen, "counted, as it must be", "NOT counted: the bar misses it");
            } else {
                judge(None, "the rollback bar's edge control at or under the edge (G1 12:32: D <= 0.1 must not count)".to_string(), planted && !counted, seen, "not counted, as it must be", "counted (or not planted): the bar's edge is wrong");
            }
        }
        match so.r_joint_control.as_ref() {
            Some((kind, ti)) if kind == "turn" => {
                judge(Some("R <- K joint ticks"), format!("r_joint's positive control (--r-joint-control turn:{ti}, a cart turning north at K while a napper rests on R; {})", spawn_gap(*ti)), r_joint > 0, format!("{r_joint} joint ticks (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
                judge(Some("the resting napper pushed"), format!("the push displacement's control through the control cart (the same cart, counted a pass while it lives: it does not go through the K-zone gate; the gate's own control is the push grid's planted pass)"), push_disp_max > 1e-6 || push_v_max > 1e-6, format!("the resting napper's displacement {push_disp_max:.6}, speed {push_v_max:.6} (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
            }
            Some((kind, ti)) if kind == "diag" => {
                judge(Some("R <- K joint ticks"), format!("r_joint's diagonal control (ironspec 04:14 (8): --r-joint-control diag:{ti}, a cart entering K's curve from its east cell, turned north through K at 45 degrees while a napper rests on R; {})", spawn_gap(*ti)), r_joint > 0 && k_turns > 0, format!("{r_joint} joint ticks, {k_turns} carts out of K into F or R (expected > 0 each)"), "the control bites", "FAIL, the control does NOT bite");
            }
            Some((kind, ti)) => judge(None, format!("r_joint's negative control (--r-joint-control {kind}:{ti}, a cart through K's cell eastbound; {})", spawn_gap(*ti)), r_joint == 0, format!("{r_joint} joint ticks (expected 0)"), "0 as expected", "FAIL, expected 0"),
            None => {}
        }
        if let Some(ts) = so.stray_ctl {
            judge(Some("stray power"), format!("the stray-power control (--stray-ctl {ts}: a redstone block beside R ({},{m},{mz}) for 10 gt, R powered with its switch off; the block is laid between ticks, so this run's outcome readings near it carry a one-reading offset: its own bar is the stray power, not the outcome)", sx - 1), stray_power > 0, format!("{stray_power} stray-power ticks (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
        }
        if let Some(kind) = so.side_ctl.as_deref() {
            judge(Some("stray power"), format!("the stray-power control (--side-ctl {kind}: a driver the guard detector can reach)"), stray_power > 0, format!("{stray_power} stray-power ticks (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
        }
        // (the rest-support control (ironspec 04:14 (9)): only a NON-CONDUCTING support is a control: R
        // is a powered rail whose switched power reaches it through its support, so glass leaves R
        // unpowered and its launches must fail; a conducting support (stone, a slime block) powers R
        // and launches: no control, judged as a variant)
        if so.siding_north && let Some(id) = so.r_support.as_deref().filter(|id| !id.ends_with("slime_block")) {
            let failed = launch_stuck + launches.iter().filter(|l| !l.2).count();
            if id.ends_with("glass") {
                judge(Some("launches stuck or late"), format!("the rest-support control (--r-support {id}: a non-conductor, so R's switched power can't reach R through it; expected: every launch fails)"), failed > 0, format!("{failed} launches stuck or after the pulse (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
            } else {
                ctl_notes.push(format!("--r-support {id}: not a control (a conductor: R's switched power reaches R through it, so its launches are expected to run; {failed} failed, judged by the launch bars)"));
            }
        }
        if let Some(en) = so.early_r {
            // (the outcome bar's positive control through the design's own cells: the release switch on
            // N gt before each release, R's power shifted by the real feed path. G1's CHANGES on
            // bc25c5cca (2): the shift is checked against the design, not by construction: every
            // release's measured switch on -> R powered must equal the drive delay, and its release -> R
            // powered the drive delay - N (R's power reading moved by exactly N); and a release must fail
            // the outcome)
            let shift_ok = !early_switch_at.is_empty() && !feed_meas.is_empty() && feed_from_switch.len() == feed_meas.len() && feed_from_switch.iter().all(|a| *a == drive_delay) && feed_meas.iter().all(|b| *b == drive_delay - en);
            judge(
                Some("the napper not at rest when R powered"),
                format!(
                    "the outcome bar's positive control (G1 / ironspec 07:31: --early-r {en}: the release switch on {en} gt before each release (t {}), so R powers {en} gt early through the design's own feed path; measured: the switch on -> R powered {} gt (the design's drive delay {drive_delay}), the release -> R powered {} gt (expected {}))",
                    if early_switch_at.is_empty() { "never".to_string() } else { early_switch_at.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ") },
                    if feed_from_switch.is_empty() { "-".to_string() } else { format!("{}..{}", feed_from_switch.iter().min().unwrap_or(&0), feed_from_switch.iter().max().unwrap_or(&0)) },
                    if feed_meas.is_empty() { "-".to_string() } else { format!("{}..{}", feed_meas.iter().min().unwrap_or(&0), feed_meas.iter().max().unwrap_or(&0)) },
                    drive_delay - en
                ),
                shift_ok && !not_rest_at_power.is_empty(),
                format!("{} releases with R powered and the napper not at rest (expected > 0); the shift measured {} (expected R's power reading moved by exactly {en})", not_rest_at_power.len(), if shift_ok { "as expected" } else { "WRONG or not measured" }),
                "the control bites",
                "FAIL, the control does NOT bite",
            );
        }
        if so.held_rider_control {
            let held = lip_qs.iter().filter(|q| matches!(q, Ok(Lip::Held(_)))).count();
            judge(Some("STUCK at the lip"), "the lip bar's positive control (--held-rider-control: glass two cells under the lip, the leader held one block down with its rider above the floor)".to_string(), held > 0, format!("{held} nappers STUCK at the held rider (expected > 0)"), "the control bites", "FAIL, the control does NOT bite");
        }
        // (G1 (2): the unpowered-release controls: --hold-switch-off holds the release switch off for
        // the run's first napper release, so R is never powered after it (counted at the run's end);
        // --hold-switch-off-rerelease also releases again 10 gt later with the switch working, so the
        // first, still waiting, is counted at the SECOND release (the mid-run path, G1's CHANGES on
        // bc25c5cca (4))
        if so.hold_switch_off {
            let mid = r_never_powered.len();
            let end = feed_wait.is_some_and(|t0| ticks - t0 > drive_delay + 2) as usize;
            let (ok, what) = if so.hold_rerelease {
                (mid > 0, "the mid-run unpowered-release control (G1's CHANGES on bc25c5cca (4): --hold-switch-off-rerelease, the release switch held off for the first napper release, a second release 10 gt later)")
            } else {
                (mid + end > 0, "the unpowered-release control (G1 (2): --hold-switch-off, the release switch held off for the first napper release)")
            };
            judge(Some("R never powered after a release"), what.to_string(), ok, format!("releases R never powered after: {mid} at a later release, {end} at the run's end (expected > 0{})", if so.hold_rerelease { " at a later release" } else { "" }), "the control bites", "FAIL, the control does NOT bite");
        }
        drop(judge);
        // (ironspec 10:54 (2) on 1788d4f65: (a) a run mixing negative and positive controls is refused
        // (the single row's verdict follows one kind); (b) a control's own bar name may not contain ", "
        // (the kinds line splits on it))
        if ctl_keys.iter().any(|k| k.starts_with("N=")) && ctl_keys.iter().any(|k| k.starts_with("P(")) {
            return Err(format!("a run mixing negative and positive controls is refused (ironspec 10:54 (2)(a)): [{}]", ctl_keys.join(", ")));
        }
        if let Some(k) = ctl_keys.iter().find(|k| k.strip_prefix("P(").and_then(|r| r.rsplit_once(")=")).is_some_and(|(b, _)| b.contains(", "))) {
            return Err(format!("a control's own bar name holds \", \" (ironspec 10:54 (2)(b): the kinds line splits on it): {k}"));
        }
        ctl.extend(ctl_notes);
        lines.push(format!(
            "the controls (ironspec 00:02 (3): each asserts its own expected failure): {}; controls judged {ctl_n}; the controls' kinds (P: positive, its own bar named; N: negative, the plain-row rule; =1 bit) [{}]; controls failing (the bar: 0): {ctl_fail}",
            if ctl.is_empty() { "none in this run".to_string() } else { ctl.join("; ") },
            ctl_keys.join(", ")
        ));
    }
    {
        // (ironspec 23:56, no vacuous passes: every bar's samples; a bar in this run's scope with 0
        // samples is NOT JUDGED and fails the row; the K-row pass bar is judged over a matrix's rows;
        // ironspec 04:14 (1): the occupancy bars, the V bar, the curve rule, the follower and the settle
        // count their samples too; the settle's scope is computed: the nappers that came to rest (a
        // napper released while still rolling onto R has no settle, and is counted))
        let arrivals_n = contacts.iter().filter(|k| k.4).count();
        let rolling_n = release_states.iter().filter(|r| r.1).count();
        let at_rest_n = release_states.len() - rolling_n;
        let samples: Vec<(&str, usize, bool)> = vec![
            ("releases", rel_times.len(), true),
            ("merge entries (touching, the curve rule's leaders)", merge_occ.len(), true),
            ("arrivals at the lanes' backs (the 0.46 bar)", arrivals_n, true),
            ("the releases from the steady state (the occupancy bars: under 6, at 8)", occ_samples, true),
            ("the steady window's ticks (the V bar)", v_samples, true),
            ("the lip margin's samples (margins, never reaching, stuck)", lip_qs.iter().filter(|q| q.is_ok()).count(), so.nap_every > 0),
            ("the nappers' curve rules at the merge", curve_samples, so.nap_every > 0),
            ("the followers' margins behind a napper", follow_samples, so.nap_every > 0),
            ("the nappers' PRESENT margins", present_margins.len(), so.nap_every > 0 && so.present_k.is_some()),
            ("the releases judged at R's first power (the napper at rest)", power_judged, so.nap_every > 0),
            ("the napper's settles on R (scope: the nappers released at rest)", settles.len(), so.nap_every > 0 && at_rest_n > 0),
            ("napper launches (stuck, after the pulse)", launches.len(), so.nap_every > 0),
            ("the K row's through-carts (push range)", th_spawned, so.k_row.is_some()),
            ("the K row's passes at K (the K row alive)", k_pass_ticks.len(), so.k_row.is_some()),
            // (G1 14:18 L3: the up-slope bar's samples: lane-cart rests seen on the slopes)
            ("the queue's rests on the slopes (the up-slope displacement bar)", slope_rest_samples, true),
        ];
        let mut not_judged: Vec<String> = samples.iter().filter(|s| s.2 && s.1 == 0).map(|s| s.0.to_string()).collect();
        not_judged.extend(extra_nj.iter().cloned());
        lines.push(format!(
            "the bars' samples (ironspec 23:56: a bar in scope with 0 samples is NOT JUDGED and fails): {}; the settle's scope: nappers released at rest {at_rest_n}, released while still rolling onto R {rolling_n}; the resting napper's K-row passes {push_passes} (judged over a matrix's rows); the fluid cells laid {fluid_cells}, their ticks scheduled 0 (ironspec 00:27: this bench lays no fluid); NOT JUDGED (0 samples): {}{}",
            samples.iter().map(|s| format!("{} {}{}", s.0, s.1, if s.2 { "" } else { " (out of this run's scope)" })).collect::<Vec<_>>().join(", "),
            not_judged.len(),
            if not_judged.is_empty() { String::new() } else { format!(" [{}]", not_judged.join("; ")) }
        ));
    }
    Ok(lines.join("\n"))
}

/// The three lanes (x -1 / 3 / 7) as a design file in the loop frame, the cells the bench lays with
/// `--merge-curve --siding-dx 2 --guard-det --drop-entry` (onechunk's F1 + Q2 module; farmopt
/// 2026-11-12 01:30): R_1 and the hold string, the N slopes on glass, the drop entry (the merge one
/// level up, the empty drop column, the wall, the pass string), the 2-cell branch with its detector
/// guard, the x-2 siding. The switched cells are left air and named in the header.
pub fn lanes_design(ns: i32, sbend: bool, row: bool, r_support: Option<&str>, rest_range: Option<&str>, straight: bool) -> Result<String, String> {
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
        // (the drop entry; with `straight` (the C fix, ironspec Nov 13 12:08 (4), provisionally adopted)
        // C is the branch's own straight north_south)
        put(x, m, mz, if straight && sbend { flat_ns } else { curve_nw });
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
            if straight {
                // (the C fix: c2 a straight east_west into the drop cell from the west, its east
                // backstop (x+1,m,mz-1); no c3: no cart can leave C westward, and a blocked napper waits
                // on its own straight, on rails)
                put(x - 1, m, mz - 1, flat_ew);
                put(x - 1, m - 1, mz - 1, glass);
                put(x + 1, m, mz - 1, glass);
            } else {
                put(x - 1, m, mz - 1, curve_sw);
                put(x - 1, m - 1, mz - 1, glass);
                put(x - 1, m, mz, curve_ne);
                put(x - 1, m - 1, mz, glass);
            }
            // (the lane-top walkway (lanes 3 and 7; ironspec 16:12, the late-path FAIL of 16:02): glass
            // (x-3..x-1, m-3, mz-2) under (x-3..x-1, m-2, mz-2), joining the lane top's drop (its start
            // node (x,m-2,mz-1), then (x,m-2,mz-2) over slope k=3) to F3's west seat candidates
            // (x-3,m-2,mz-1..mz+2); lane -1 has the same join through F3's glowstone (-4..-2,21,5))
            if x == 3 || x == 7 {
                for d in 1..=3 {
                    put(x - d, m - 3, mz - 2, glass);
                }
            }
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
    if sbend && straight {
        out.push_str("# THIS FILE IS THE C FIX `--sbend-straight` (ironspec Nov 13 12:08 (4), provisionally adopted; bench, tool unreviewed): per lane x, C (x,24,7) is a straight north_south (the batch carts' own, from D), c2 (x-1,24,6) a straight east_west into the drop cell from the WEST, no c3 (x-1,24,7) and no c3 support (x-1,23,7), and the napper's east backstop (x+1,24,6) glass; the napper's path R -> c1 -> c2 -> the drop cell; no cart can leave C westward. Where the prose below names C's north_west curve, c3 or the S-bend's west arm, THIS LINE rules for those cells; every other cell is the S-bend file's.\n");
    }
    if sbend {
        out.push_str(&format!(
            "# F1 + Q2 lanes, IN-CHUNK SPLIT with the S-bend siding (ironfarm; ironspec 2026-11-12 06:24 option (a), provisional pick; bench, tool unreviewed): three lanes x -1 / 3 / 7, loop frame (paste with no move), {ns} slopes behind R_1, the merge at m = {m}, mz = {mz}. Benched as `mctas mobfarm escape-bench --siding --merge-curve --siding-dx 2 --guard-det --drop-entry --slopes {ns} --r-pulse 30 --drop-w 20 --per-lane 8 --siding-north --k-row 0.30 --k-offsets 0,12,24 --nap-feed-v 0.30` (explore/ironfarm-go): one lane, these cells at x 3, the split's K and row laid by the bench for the through-carts.\n"
        ));
        out.push_str("# Per lane x: the hold string (x-2..x+2,17,0) and R_1 (x,17,1): the station's file (dfb89499). The split's file (7b90c3f1) owns the row along z 9 at y 25, K (x-2,25,9) (a power-forced T: north_east through, north_west into the siding; driver from day-1), J (x,25,9), their glass (x-2,24,9) and (x,24,9), and the branch's detector slope D (x,24,8) (the guard and the pass). Not laid here.");
        out.push_str(&format!(" Here: the slopes (x,18..{},2..{}) plain, ascending south, on glass, the top one's high side (x,{},{}) glass; the merge C (x,{m},{mz}) a north_west curve on glass (x,{},{mz}), its north end the rail end over the drop column (x,{m}..{},{}) (empty) onto the top slope, the wall (x,{m},{}) glass (golem-proof);", 17 + ns, 1 + ns, 17 + ns, 2 + ns, m - 1, m - 1, mz - 1, mz - 2));
        out.push_str(&format!(" the siding (the S-bend): the feed F (x-2,{},{}) north_south on glass (x-2,{m},{}), the rest R (x-2,{m},{mz}) a powered rail ascending south (braked unpowered) on {} (x-2,{},{mz}) (its switched power: strongly powered by the repeater (x-2,{},{}) outputting north into R's support (state facing=south), on glass (x-2,{},{})), c1 (x-2,{m},{}) south_east, c2 (x-1,{m},{}) south_west, c3 (x-1,{m},{mz}) north_east into C, the curves on glass. No pass string: the pass is D's rising edge (D -> the same cart's merge-cell entry 2 gt, clear 9 gt).{}\n", m + 1, mz + 1, mz + 1, support.trim_start_matches("minecraft:"), m - 1, m - 1, mz + 1, m - 2, mz + 1, mz - 1, mz - 1, if support == "minecraft:slime_block" { " R's support is a slime block (ironspec 08:46, the row partials: a conductor that is not solid-render, so R's cell is no stable destination for a panicking rider; the same repeater powers it)." } else { "" }));
        out.push_str(&format!("# Switched cells (left air here): R_1's side power (x+1,17,1) (the station's R line); the siding's switch: lane 7: the feed repeater's input (7,{},{}) (the feed path laid here: dust at R's input (5,{},{}), the delay-1 feed repeater (6,{},{}) outputting west into R's input dust (state facing=east), both on glass); lanes -1 and 3 (day-1's feed from below, ironspec 11:22, option (a), the seat candidates' columns (x,22..23,9) kept clear): R's input dust (x-2,{},{}) laid here on a slime block (x-2,{},{}) that day-1's controller cells strongly power (day-1's cells, not this file: lane -1 a delay-3 repeater (-4,22,9) outputting east into it (state facing=west); lane 3 a repeater (2,22,9) outputting west into it (state facing=east)); the siding pulse: on 30 gt (the launch threshold 8) at the napper's release, K gt after the window's pass (D's rising edge), with no wait for D off (day-1's release rule, v6 §7: k_sw = 6 since ironspec 21:35, R at D's press + 14 through the 8-gt feed; ironspec 19:29 / 22:03); no other powered rail within 8 along R's line). No siding sensor: the napper-present bit is K's routing, set 2 gt after the napper's K tick (day-1's PRESENT at K + 2, ironspec 19:29; the K(n) reset stays at K + 16; K -> rest 12..15 gt measured at arrival 0.30, 23 at 0.10). The holds' trapdoors (x,16,0) are the station's. Stops and buffers (ironspec 21:25 (4): a cart's move stopped by a block face must be a declared stop): the drop entry's backstop (x,24,5), every cart into the lane stops against it, then falls into the drop column (x,24..23,6).\n", m - 1, mz + 2, m - 1, mz + 2, m - 1, mz + 2, m - 1, mz + 2, m - 2, mz + 2));
        let (c_sup_y, c_help_z, c_pow_y) = (m - 1, mz - 1, m + 1);
        out.push_str(&format!(
            "# Placement order (a rail placed last among its neighbours takes its natural shape; these differ from the design and need a temporary helper rail while placed, removed after: a removed rail reshapes nothing): {}. Lane x 3 shown; the same cells at x -1 and 7. c3 (x-1,{m},{mz}) sees three rails (c2, C, R): lay it before R. THE MERGE C (x,{m},{mz}) NEEDS A POWERED PLACEMENT (official: tick-exact's replay on record7e, artifact 1672965364365960, the `# shape:` line's build step; every unpowered order ends south_west): with C's support glass (x,{c_sup_y},{mz}) laid, lay c3 and D (the split's); then a temporary support (x,{c_sup_y},{c_help_z}) and a helper rail on it (x,{m},{c_help_z}), and a temporary redstone block over C (x,{c_pow_y},{mz}); then C, which takes north_west; then remove the helper rail, then the redstone block, then the support. THE TOP SLOPE S (x,{},{}) (official, tick-exact, `mctas mobai rail-slope`): the glass (x,{},{}) first, a helper rail (x,{},{}) on it, the slope below at its low end; then S comes out ascending_south and keeps it; with no helper it settles north_south.\n",
            if natural_mismatch.is_empty() { "none".to_string() } else { natural_mismatch.join("; ") },
            17 + ns,
            1 + ns,
            17 + ns,
            2 + ns,
            18 + ns,
            2 + ns
        ));
        out.push_str("# Bench (the S-bend, wt-go; tool unreviewed): napper arrivals 0.10..0.44 on K's cell (the brief's band) rest on R at z 7.343..7.922 over their entry phases (the rest sweep, `--rest-sweep lo,hi,step --rest-phases N` with K laid: the ceiling 7.922 at arrival 0.102 over 64 phases, 0.10 at phase 0 7.895; rest z is not monotone in the arrival, its maxima fall with speed) and leave R 6..4 gt after R powers (lane 7: 10..8 gt after its switch, the feed repeater's input (7,23,9), R powering 4 gt later through the feed repeater and R's own; lanes -1 and 3 under day-1's feed from below: 14..12 gt after day-1's delay-3 repeater's input, R powering 8 gt later; without the feed path 8..6 gt, where tick-exact's official release, counted from the input pulse at R's repeater, reads one tick later: +9/+8/+7) (the launch threshold: the switch's pulse must be at least 8 gt, at the 0.10 rest and at the ceiling rest 7.922 alike: at 7 the napper leaves after R's power ended, at 0.231..0.233; 6 gt crawls or sticks at the high rests; the design's pulse 30); push contact with every passing cart 0, the least gap +0.084 at a 0.10 arrival and +0.239 at 0.20 against the split's ridden through-carts at 0.30, their entries into K swept (0.30 is the conservative end: the row's measured 0.35..0.39 moves a cart further along K's chord before the push search; tick-exact's official crossing on slime 1491498096433479: +0.0795 against 0.29 through-carts, and its first crossing (the napper placed static at its rest): +0.0838 against 0.30 (553dcf430, correcting f9d148591's +0.111), slime and stone alike; the final composition's crossing rows (0.29..0.44, their hashes printed) supersede both when they land); lane carts on the siding's cells 0; stray power on R 0. On the slime support with the feed path (lane 7's; day-1's feed from below for lanes -1 and 3 benched the same, `--feed-slime`): matrix (ii) and (A) 200 + 200 rows 0 failing (rets 132..400, merge entry 0.283..0.476, nap_rets 300..1700, +-2, the split row's through-carts at 0.30; occupancy 8, stamps over 1,000, cut windows and carts resting on another cart's top counted; the lane's largest occupancy at a release 7), restarts inside the napper's wait 1000/0 + 1000/0, the start-up from the steady state (7 riders, the 8th in the pocket) 3 cycles clean, the launch identical to stone's at every arrival and pulse; the control (glass under R) never launches. The lane overflow (ironspec 10:02): 0 carts resting on another cart's top in every row; V (x,22,6), slot 8, never held from the steady state to the last release (the bar: 0; every row of (ii) and (A), from both starts), held only in the full-lane start's transient (8 carts laid in the lane at t 0) and after the last release (the lane refilling with no release, the bench's end); an overfilled lane (9 carts, the control) leaves its extra cart at rest off the rails on the merge support's edge above V (a counted failure) with 14 empty releases of 40. The delivery floor: first failure stored 0.22, 0.23..1.00 clean.\n");
        if support == "minecraft:slime_block" {
            out.push_str("# R's slime support: lay it only together with F3's z 9 row (9fc3155f's F3 part f17304792: the floors (-1,21,9), (0,21,9), (4,21,9); lane -1's (-3,21,9) and (-2,21,9) dropped, their candidate cells being the feed's cells): lay with it, not before. On it (9fc3155f's seat rows, bench: in f17304792's commit message, measured on rev 3 e9eeeee52 with R's slime and the feeds as ruled, and on the module of record 89f8d159e with lane -1's feed stack and that z 9 row, artifact 1011740584548583, the same candidates and P_null): no seat's own cell is a stroll candidate; each seat's candidates, every one reached at the 0.10 / 0.20 / 0.40 arrivals (rests z 7.895 / 7.740 / 7.435, all in R's cell, so each lane's box is the same at every arrival): lane -1 three ((-3,22,5), (-2,22,5), (-1,22,9); (-4,22,5), over F3's glowstone (-4,21,5) inside its box, is none: WATER_BORDER malus from the lift exit's water source (-5,23,6)), lane 3 five ((0,22,6..9), (-1,22,9)), lane 7 four ((4,22,6..9)); model P_null lane -1 / 3 / 7: 0.784 / 0.665 / 0.722 ((1 - c/125)^10); 0 candidates in or lifted into the feed and switched cells. Cited: tick-exact's napper release on the official server (`mctas mobai napper-release`, de0bcdeae; the stone baseline artifact 1012303384495744, the slime run matching it: the cart leaves R +9/+8/+7 gt from the input pulse at 0.37..0.52 b/gt from the rests 7.895 / 7.740 / 7.435; a cart on rails is never onGround, so slime's stepOn does not run). Pending: the seats' official P_null re-measured on slime.\n");
        }
        out.push_str(&format!("# The lane-top walkway (lanes 3 and 7; ironspec 16:12, the late-path FAIL of 16:02): glass (x-3..x-1,{},{}) under the walkway (x-3..x-1,{},{}) joins the lane top to F3's west seat candidates (x-3,{},{}..{}). A PANIC draw made in the S-bend (R, c1, c2, c3) is pathed up to 39 gt later (MoveToTargetSink's cooldown), possibly from the airborne drop off C's north end, whose start node is the top slope (x,{},{}) with rail nodes blocked; from there the route is (x,{},{}) over slope k=3, the walkway, the candidate, crossing no rail. 9fc3155f's --path-probe on the module of record 89f8d159e: without the walkway, partial from the drop's top (y 24.0); with it, reached in 6..10 nodes from y 24.0 / 23.5 / 23.0 / 22.7. Lane -1 has the same join through F3's glowstone (-4..-2,21,5). Glass is not solid-render: no new stroll candidate.\n", m - 3, mz - 2, m - 2, mz - 2, m - 2, mz - 1, mz + 2, m - 2, mz - 1, m - 2, mz - 2));
        if let Some(rr) = rest_range {
            out.push_str("# The speeds line's nap-rest-z: the rest sweep (escape-bench --siding-north --k-row 0.30 --r-support slime_block --rest-sweep): 0.10..0.44 by 0.01 at 8 entry phases 7.343..7.912; 0.10..0.14 by 0.005 at 32 phases up to 7.920; 0.100..0.112 by 0.002 at 64 phases the ceiling 7.922 (arrival 0.102, phase 0.0606); no run without a napper at rest.\n");
            out.push_str(&format!("# speeds: nap-rest-z={rr}\n"));
        }
        // (c3 is a junction only through R beside it: ironspec's ruling (07:52), declared for push-scan;
        // with `straight` there is no c3)
        let why = "c3 (ironspec 07:52): its third arm is the siding rest R itself and it is never switched, and no other route's cart enters it (ironfarm escape-bench --siding-north: the westmost branch-cart centre in the merge cell x+0.24 over the matrix and restarts, lane carts on the siding's cells 0, push contact 0, one napper per siding at a time)";
        if !straight {
            out.push_str(&format!("# own-route-junctions: {}\n", [-1, 3, 7].iter().map(|x| format!("{},{m},{mz}={why}", x - 1)).collect::<Vec<_>>().join("; ")));
        }
        // (the declared contacts (ironspec 21:25 (4), 22:05: a stop with the evidence of the ride that
        // shows it; a wall is no stop): the drop entry's backstop in each lane)
        let why_c = "the drop entry's backstop, a stop: every cart into the lane stops against its south face (the cart's centre at z 6.490) and falls into the drop column (x,24..23,6) two ticks later (evidence: tick-exact's official S-bend rows, artifact 1009706688854741: the leader stops at (3.655,24.000,6.490) at P + 8 and falls at P + 10; ironfarm escape-bench --siding's block-contact line, every cart once a pass)";
        out.push_str(&format!("# contacts: {}={why_c}\n", [-1, 3, 7].iter().map(|x| format!("{x},{m},{}", mz - 2)).collect::<Vec<_>>().join("+")));
        if straight {
            // (the C fix's east backstop (ironspec Nov 13 12:08 (4): declared; the bounded form's
            // class=design with its reference run comes with the bench's move to the shared judge))
            let why_e = "the napper's east backstop, a stop (ironspec Nov 13 12:08 (4), the C fix): the napper enters the drop cell from the west off its straight c2 and stops against this block's west face, then falls into the drop column (x,24..23,6); evidence: escape-bench --siding --sbend-straight (the drop passes, lateral 0.29 at most)";
            out.push_str(&format!("# contacts: {}={why_e}\n", [-1, 3, 7].iter().map(|x| format!("{},{m},{}", x + 1, mz - 1)).collect::<Vec<_>>().join("+")));
        }
        // (the siding feed's drives (ironspec 23:15 (3)): each lane's last feed repeater, the switch
        // that powers R through its support; the precondition table's line 14 (k_sw -> SIDING_l -> R
        // at D + k_sw + 8) and line 11 (the siding feeds))
        out.push_str(&format!(
            "# switch: {}\n",
            [-1, 3, 7]
                .iter()
                .map(|x| format!("{},{},{}=the siding feed's last repeater (delay 1, facing south): powers R ({},{m},{}) through its support slime ({},{},{}); the precondition table's lines 11 and 14 (R at D's press + k_sw + 8)", x - 2, m - 1, mz + 1, x - 2, mz, x - 2, m - 1, mz))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        // (the lanes' z-9 feed dusts (tick-exact Nov 13 10:41, via farmopt): each lane's dust two below its
        // K T toggles at its siding's release; its update reaches K(l) (x-2,m+1,mz+2), a switched T (its tap
        // lever, the split's file), which re-settles to its own power's shape: declared update-only, judged
        // by 9fc3155f's x26 with a coincidence row)
        out.push_str(&format!(
            "# update-only: {}\n",
            [-1, 3, 7]
                .iter()
                .map(|x| format!("{},{},{}=lane {x}'s siding feed dust (toggles at its siding's release): its update reaches K({x})'s T ({},{},{}) two above it, a switched T (its tap lever, the split's file), which re-settles to its own power's shape (tick-exact Nov 13 10:41)", x - 2, m - 1, mz + 2, x - 2, m + 1, mz + 2))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        // (the integrity entries (ironspec Nov 13 11:02, 9fc3155f's part 6): R (x-2,m,mz) holding c3's
        // north_east: R's two real connections (its own straight: north (x-2,m,mz-1) on the level, south one
        // up (x-2,m+1,mz+1), F), so c3's third potential arm (R) is blocked and c3 stays north_east; with
        // `straight` there is no c3, and the entry is retired (ironspec Nov 13 12:08 (4)))
        if !straight {
        out.push_str(&format!(
            "# integrity: {}\n",
            [-1, 3, 7]
                .iter()
                .map(|x| format!("{},{m},{mz}=connections {},{m},{}+{},{},{}; holds {},{m},{mz}=north_east", x - 2, x - 2, mz - 1, x - 2, m + 1, mz + 1, x - 1))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        }
        // (the push pairs closed by measurement (ironspec 23:15 (2), 23:51): each siding rest R against
        // the split row's K T it rests beside, with the measured bound and the evidence row)
        out.push_str(&format!(
            "# push-pair: {}\n",
            [-1, 3, 7]
                .iter()
                .map(|x| format!("{},{m},{}+{},{},{}=the napper resting on R against the K row's carts passing K: the measured bound is 0, no displacement and no speed of the resting napper in any pass (the push grid, ironspec 03:00: every rest position the napper's arrival on K produces, 0.10..0.44 by 0.01 x 8 entry phases, rest z 7.343..7.912, each with injected K-row passes at 0.28..0.44 b/gt by 0.04 across 8 entry x's, 1 and 20 gt after the rest, 22,400 passes, displacement 0.000000; the table designs/circuits/mobfarm/tracks/ironfarm/push-grid-sbend.txt, by ironfarm escape-bench --siding --push-grid-run; with the k_sw = 6 matrix's own passes, 8 arrivals x 16 qualifying phases x 4 variants, and its 1-4-gt freezes; its positive control --r-joint-control turn:T bites; a push beyond 0 in any run fails that run)", x - 2, mz, x - 2, m + 1, mz + 2))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        // (the stored rail shapes the placement pass would not give (cc767b78's settle-check; ironspec
        // 01:46 (1), G1's `shape:` key at 01:49): the merge C (north_west: the S-bend's west arm and
        // the lip; placement computes south_west from D and c3) and the top slope (ascending_south with
        // no rail one up to the south; placement computes north_south), each with its reason, evidence
        // and build step (the replay decides: tick-exact's bot on the official server, or redsim once it
        // reproduces the official controls))
        out.push_str(&format!(
            "# shape: {}\n",
            [-1, 3, 7]
                .iter()
                .flat_map(|x| [
                    format!("{x},{m},{mz}=north_west (load-bearing: it joins the S-bend's west arm c3 ({},{m},{mz}) to the lip and snaps the leader from D on to the lip, while south_west would send the napper into D and every leader into the S-bend; evidence: tick-exact's official S-bend rows ran on it, artifacts 1009706688854741 and 1032428046539821; build step (ironspec 01:46 (1), replayed officially by tick-exact on record7e, lanes -1 / 3 / 7, artifact 1672965364365960: C north_west and nothing temporary left, while the two failing orders end south_west at D's placement): with C's support glass laid, lay c3 and D ({x},{m},{}), then a temporary block ({x},{},{}) and a temporary rail on it ({x},{m},{}) and a temporary redstone block over C ({x},{},{mz}), place C (the powered T resolves to north_west), remove the temporary rail, then the redstone block, then the temporary block)", x - 1, mz + 1, m - 1, mz - 1, mz - 1, m + 1),
                    format!("{x},{},{}=ascending_south (load-bearing: the landing from the lip rolls down it to the lane's back, where a flat cell would change every landing's descent and speed; evidence: tick-exact's official S-bend rows, the leader landing from the lip and riding the slopes down, artifact 1032428046539821, and the official build replay, artifact 1672965364365960; build step: lay the glass ({x},{},{}), a temporary rail on it at ({x},{},{}) (C's support cell, so before that support is laid), place ({x},{},{}) (it ascends south), remove the temporary rail, then lay C's support glass ({x},{},{}))", m - 2, mz - 1, m - 2, mz, m - 1, mz, m - 2, mz - 1, m - 1, mz),
                ])
                // (with `straight` C is the branch's own straight north_south, which placement gives:
                // only the top slope's entry stays)
                .enumerate()
                .filter(|(i, _)| !straight || i % 2 == 1)
                .map(|(_, s)| s)
                .collect::<Vec<_>>()
                .join("; ")
        ));
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
        "# Placement order (a rail placed last among its neighbours takes its natural shape; these differ from the design and need a temporary helper rail while placed, removed after: a removed rail reshapes nothing): {}. Lane x 3 shown; the same cells at x -1 and 7. THE MERGE C (x,24,7) NEEDS A POWERED PLACEMENT (official: tick-exact's replay on record7e, artifact 1672965364365960; every unpowered order ends south_west): with C's support glass (x,23,7) laid, place the guard detector D (x,24,8); then a temporary support block (x,23,6) and a helper rail on it (x,24,6), and a temporary redstone block over C (x,25,7); then C, which takes north_west; then remove the helper rail, then the redstone block, then the support. Once north_west, C holds through slope carts, siding carts and a restart. THE TOP SLOPE S (x,22,6) (official, tick-exact, `mctas mobai rail-slope`): the glass G (x,22,7) first, a helper rail H (x,23,7) on G, with the slope below at its low end; S then comes out ascending_south and keeps it through H's removal (G holds its high side), a neighbour update, glass placed where H stood (the merge's support), ridden carts, and a restart; with no H it settles north_south. The guard detector's raw connections (C and (x,24,9)) have 2 potential connections each (no T). R_1's side power (x+1,17,1) is the station's driver: it is face-adjacent to the hold string's hook (x+1,17,0), so no dust and no repeater or comparator reading from the north there.\n",
        if natural_mismatch.is_empty() { "none".to_string() } else { natural_mismatch.join("; ") }
    ));
    out.push_str("# The split's interface (the bench's v_in is the STORED speed, the cart's Motion at (x,24,9); a ridden cart moves 0.75 of it a tick): stored 0.30..0.48 benched clean (8 a lane, 5 slopes, ret 130..600, nap_ret 60/150/300, +-2: arrivals q0/q1 0.459, q2 0.392, q3 <= 0.332, nappers <= 0.459, landings meeting a cart 0); stored 0.26 fails the late napper with the guard at (x,24,8) (napper and successor land together); 0.22 rests on the merge's glass. Restarts (10 load orders at every phase) and the fresh-design cold start at every phase: 0 failing.\n");
    }
    out.push_str(if sbend && straight { "name: f1q2-lanes-sbend-straight\ncircuit: mob-farm iron_ingot\n" } else if sbend { "name: f1q2-lanes-sbend\ncircuit: mob-farm iron_ingot\n" } else { "name: f1q2-lanes\ncircuit: mob-farm iron_ingot\n" });
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
    /// Nappers whose curve rule printed "-" (no merge entry or no predecessor's clear; G1 (6)).
    pub nap_no_curve: usize,
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
    /// V holds in the start's transient (before the steady state; the full-lane start's only).
    pub v_holds_start: usize,
    /// Releases while the napper was still rolling onto R (--nap-present-k).
    pub rolling_releases: usize,
    /// The least margin of the cart after a napper into the merge (its entry minus the napper's
    /// clear), gt.
    pub nap_follow_min: Option<i64>,
    /// The least lip margin over the row's nappers with a margin (ironspec 21:14 (2)); None when no
    /// napper has one.
    pub lip_min: Option<i64>,
    /// Nappers that never reached their leader's rider (clean).
    pub lip_never: usize,
    /// Nappers that reached the rider while the rider never dropped below the floor (STUCK, ironspec
    /// 00:02 (1)): failures.
    pub lip_stuck: usize,
    /// The lip margin's samples: margins + never + stuck.
    pub lip_samples: usize,
    /// Nappers with no lip sample: no cart ahead of them at the merge, or no merge entry.
    pub lip_no_ahead: usize,
    pub lip_no_merge: usize,
    /// The least PRESENT margin before the leader's press (ironspec 21:14 (1)).
    pub present_margin_min: Option<i64>,
    /// The carts' undeclared block contacts from t 20 (ironspec 21:25 (4)): cells a cart's box met
    /// when its move collided horizontally, outside the declared stops.
    pub contacts_undeclared: usize,
    /// G1's 12:32 rollback fallback bar: the queue's net up-slope displacements over 0.1 block from a
    /// cart's last rest (the bar: 0).
    pub upslope_disp: usize,
    /// Declared stops no cart met (G1's CHANGES 3 (2)): failures.
    pub contacts_stale: usize,
    /// The contacts before t 20 (the summoned start's settle), counted apart.
    pub contacts_early: usize,
    /// The resting napper across the K row's passes (ironspec 23:15 (2)): the passes (samples), its
    /// largest displacement from its rest and largest speed during them.
    pub push_passes: usize,
    pub push_disp: f64,
    pub push_speed: f64,
    /// Bars in the run's scope with 0 samples (ironspec 23:56: NOT JUDGED, a failure).
    pub not_judged: usize,
    /// Controls in the run that did not bite (ironspec 00:02 (3)).
    pub control_fail: usize,
    /// The controls this run judged (0: a plain row).
    pub controls_n: usize,
    /// Each control's own bar (None: a negative control) and whether it bit (G1's CHANGES on
    /// bc25c5cca (1)).
    pub controls: Vec<(Option<String>, bool)>,
    /// The nappers' settle on R (day-1): the first tick on R's cell to speed < 0.001, (least, most).
    pub settle: Option<(i64, i64)>,
    /// The nappers' K tick to their rest on R, (least, most): the settle PRESENT's bar uses is this
    /// minus 2 (PRESENT = K + 2; ironspec 01:33 (1)).
    pub k_to_rest: Option<(i64, i64)>,
    /// Ticks with a cart on R while another turned at K or rode F (push-scan's R <- K pairs).
    pub r_joint: usize,
    /// The least gap from a napper's departure off R to the next napper's K tick, gt.
    pub nap_gap_min: Option<i64>,
    pub drop_stamp_max: i64,
    /// The branch returns' merge-entry stored speed (min, max; -1 none); the nappers' rest z (min,
    /// max; -1 none); --k-row: through-carts slowed, lost, ticks in push range of the waiting napper,
    /// box overlaps; --flat-detector: false trips.
    pub merge_v_min: f64,
    pub merge_v_max: f64,
    pub rest_z_min: f64,
    pub rest_z_max: f64,
    pub th_slowed: usize,
    /// The K row's through-carts that passed K in the run (the K row alive: G1's guard on
    /// --push-out-of-scope).
    pub th_passed: usize,
    /// The push bar computed out of scope in this row (ironspec 03:00: no K-row pass in a rest, the K
    /// row alive), and its rests no push-grid row covers (failures).
    pub push_excluded: bool,
    pub push_uncovered: usize,
    /// Run-time overrides that left their cell unchanged (ironspec 04:07): failures.
    pub override_noops: usize,
    /// The releases the carts-per-release pairs add up to, and the samples line's release count
    /// (ironspec 04:14 (6): they must agree).
    pub releases_counted: usize,
    pub releases_n: usize,
    /// PRESENT margins under the row's proxy bar (ironspec 04:14 (7); information since 06:15 real).
    pub present_under: usize,
    /// The PRESENT proxy's bar this row computed (information; None when not computed).
    pub present_bar: Option<i64>,
    /// The releases judged at R's first power after them, and those with the napper not at rest
    /// then (ironspec 06:15 real: the outcome the PRESENT bar proxies; failures).
    pub power_judged: usize,
    pub not_rest_at_power: usize,
    /// Riders without the bed's HOME at t 1 under --nap-ai (ironspec 07:28): failures.
    pub t1_unhomed: usize,
    /// Riders not at REST at t 1 (G1's CHANGES on bc25c5cca (5)): failures.
    pub t1_not_rest: usize,
    /// Releases whose measured feed differs from the design's drive delay (G1's CHANGES on bc25c5cca
    /// (2), (8)): failures.
    pub feed_off: usize,
    /// The bed's ticket not taken at t 1 under --nap-ai (free tickets not 0): a failure.
    pub t1_ticket_bad: usize,
    /// Napper releases R never powered after (G1 (2)): failures.
    pub r_never_powered: usize,
    /// The napper's z at the push grid's injected pass (ironspec 04:14 (3c)).
    pub inject_rest_z: Option<f64>,
    /// Ticks a K-zone cart's push box met the resting napper's box (the planted control's
    /// precondition), and the carts that moved out of K into F or R.
    pub pass_overlap: usize,
    pub k_turns: usize,
    /// Ticks two carts were over the merge with a napper among them: beside its leader (the lip's
    /// contact), beside its follower.
    pub touch_leader: usize,
    pub touch_leader_late: usize,
    pub touch_follower: usize,
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
    /// The lane's smallest occupancy at a release from the steady state.
    pub occ_min: Option<usize>,
    /// Releases from the steady state with the lane's occupancy below 6 (rotation below 7).
    pub occ_low: usize,
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

/// One row of the siding bench, read from its printed lines by [`siding_row`].
impl SidingRow {
    /// Every bar this row fails, by name and count (ironspec 04:14 (1): the one list behind every
    /// matrix's "failing" count and the run's exit code); `lip_bar` and `curve_bar` are the matrix's.
    /// A single row's verdict (G1's CHANGES on bc25c5cca (1)): Ok with the line to print, Err with
    /// the failure.
    /// - A plain row (no control) passes only with no failure.
    /// - A run with a NEGATIVE control (expected 0, "bit" its passing state) follows the plain-row
    ///   rule: any failure, or the control not holding, fails it.
    /// - A run of POSITIVE controls is CAUGHT only when every control bit AND its own bar is among the
    ///   row's failures; the other failures are the controls' side effects, printed.
    pub fn single_row_verdict(&self, lip_bar: Option<i64>, curve_bar: Option<i64>) -> Result<String, String> {
        let reasons = self.failures(lip_bar, curve_bar);
        let fails = reasons.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ");
        if self.controls.is_empty() {
            return if reasons.is_empty() { Ok("the row passes".to_string()) } else { Err(format!("the row FAILS: {fails}")) };
        }
        let negatives = self.controls.iter().filter(|c| c.0.is_none()).count();
        if negatives > 0 {
            // (a negative control's run: the plain-row rule)
            if self.control_fail > 0 || !reasons.is_empty() {
                return Err(format!("the row FAILS (a negative control's run follows the plain-row rule): {}", if fails.is_empty() { "a control that did not hold".to_string() } else { fails }));
            }
            return Ok(format!("CONTROL HOLDS: {negatives} negative control(s) held, no failure"));
        }
        let missing: Vec<&str> = self.controls.iter().filter_map(|c| c.0.as_deref().filter(|b| !c.1 || !reasons.iter().any(|r| r.0 == *b))).collect();
        if !missing.is_empty() {
            return Err(format!("CONTROL NOT CAUGHT: {} of {} positive controls did not fail their own bar ({}); the row's failures: {}", missing.len(), self.controls.len(), missing.join(", "), if fails.is_empty() { "none".to_string() } else { fails }));
        }
        Ok(format!("CONTROL CAUGHT: {} of {} positive controls bit and failed their own bar ({}); the row's failures: {fails}", self.controls.len(), self.controls.len(), self.controls.iter().filter_map(|c| c.0.clone()).collect::<Vec<_>>().join(", ")))
    }

    pub fn failures(&self, lip_bar: Option<i64>, curve_bar: Option<i64>) -> Vec<(&'static str, usize)> {
        let all: Vec<(&'static str, usize)> = vec![
            ("lost (empty releases)", self.empty_releases),
            ("not exactly 1 cart a release", self.not_one),
            ("the napper heading the wrong way", self.wrong),
            ("knocked loose", self.knocked),
            ("two carts over the merge", self.touching),
            ("order broken", self.order_broken),
            ("off the rails", self.off_rail),
            ("eyes", self.eyes),
            ("landings over 0.46", self.landing_over),
            ("popped", self.popped),
            ("stray power", self.stray_power),
            ("a napper still waiting (stuck)", self.stuck as usize),
            ("K-row carts in push range of the waiting napper", self.th_push),
            ("K-row carts lost", self.th_lost),
            ("lane carts in push contact", self.lane_push),
            ("lane carts on the siding", self.lane_on_siding),
            ("the flat detector's false detections", self.flat_false),
            ("releases at occupancy 8", self.occ8),
            ("home lost", self.home_lost),
            ("stamps over 1,000", self.stamp_over_1000),
            ("windows cut short", self.win_cut),
            ("launches stuck or late", self.launch_stuck),
            ("carts stacked", self.stacked),
            ("V held", self.v_holds),
            ("R <- K joint ticks", self.r_joint),
            ("releases at occupancy under 6", self.occ_low),
            ("undeclared contacts", self.contacts_undeclared),
            ("net up-slope displacement over 0.1 block", self.upslope_disp),
            ("stale declared stops (never hit)", self.contacts_stale),
            ("bars NOT JUDGED", self.not_judged),
            ("STUCK at the lip", self.lip_stuck),
            ("controls failing", self.control_fail),
            ("rests the push grid doesn't cover", self.push_uncovered),
            ("no-op overrides", self.override_noops),
            ("the napper not at rest when R powered", self.not_rest_at_power),
            ("R never powered after a release", self.r_never_powered),
            ("riders without the bed's HOME at t 1 (--nap-ai)", self.t1_unhomed),
            ("riders not at REST at t 1", self.t1_not_rest),
            ("releases whose feed differs from the design's drive delay", self.feed_off),
            ("the bed's ticket not taken at t 1 (--nap-ai)", self.t1_ticket_bad),
            ("nappers without a curve rule", self.nap_no_curve),
            ("arrivals over 0.46", self.arrivals_over),
            ("the resting napper pushed", (self.push_disp > 1e-6 || self.push_speed > 1e-6) as usize),
            ("the lip margin under the bar", lip_bar.map_or(0, |b| self.lip_min.is_some_and(|q| q < b) as usize)),
            ("curve rules under the curve bar", curve_bar.map_or(0, |b| self.nap_cr.iter().filter(|c| **c < b).count())),
            ("followers under the curve bar", curve_bar.map_or(0, |b| self.nap_follow_min.is_some_and(|f| f < b) as usize)),
        ];
        all.into_iter().filter(|x| x.1 > 0).collect()
    }
}

/// The bench's numbers read back from its own text (one source of truth: the printed lines).
pub fn siding_row(o: &DropOpts, so: &SidingOpts) -> Result<SidingRow, String> {
    let text = siding_bench(o, so)?;
    parse_siding_row(text, so)
}

/// `siding_row`'s strict reader of a bench text (every bar's line required; a missing or unparsable
/// count, or counts that don't reconcile, an error).
pub fn parse_siding_row(text: String, so: &SidingOpts) -> Result<SidingRow, String> {
    let mut r = SidingRow { min_health: f32::MAX, stamp_calm_max: -1, stamp_siding_max: -1, stamp_lane_max: -1, drop_stamp_max: -1, merge_v_min: -1.0, merge_v_max: -1.0, rest_z_min: -1.0, rest_z_max: -1.0, th_min_exit: -1.0, th_zgap: f64::MAX, ..Default::default() };
    let num_after = |s: &str, key: &str| -> Option<f64> {
        let i = s.find(key)? + key.len();
        let t: String = s[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
        t.parse().ok()
    };
    let req = |l: &str, key: &str| -> Result<f64, String> { num_after(l, key).ok_or(format!("siding_row: no number after \"{key}\" in: {l}")) };
    for l in text.lines() {
        if l.starts_with("carts per release (exactly 1): ") {
            // (ironspec 04:14 (6): every "carts: releases" pair parses, and their releases add up to the
            // samples line's release count (checked after the loop): no silent 0)
            let body = l.trim_start_matches("carts per release (exactly 1): ");
            let counts = body.split(';').next().unwrap_or("").trim();
            if !counts.is_empty() {
                for part in counts.split(", ") {
                    let (a, b2) = part.split_once(": ").ok_or(format!("siding_row: carts-per-release pair {part:?} in: {l}"))?;
                    let a: usize = a.trim().parse().map_err(|_| format!("siding_row: carts-per-release count {part:?} in: {l}"))?;
                    let n: usize = b2.trim().parse().map_err(|_| format!("siding_row: carts-per-release count {part:?} in: {l}"))?;
                    r.releases_counted += n;
                    if a != 1 {
                        r.not_one += n;
                    }
                }
            }
            r.empty_releases = req(l, "golems lost: a batch of 2 can't summon): ")? as usize;
            r.late_naps = req(l, "late nappers (a lane-lap wait): ")? as usize;
        } else if l.starts_with("nappers through the siding: ") {
            // (every napper's segment carries its contact figure (ironspec 04:14 (6)); its curve rule
            // and its q are a number or an explicit "-" (no merge entry; no contact sample), anything
            // else an error (G1 (6)))
            let body = l.trim_start_matches("nappers through the siding: ");
            if body != "none" {
                for seg in body.split(" | ") {
                    let v = num_after(seg, "its contact ").ok_or(format!("siding_row: a napper with no contact figure in: {seg}"))?;
                    let after = |key: &str| seg.find(key).map(|i| &seg[i + key.len()..]);
                    match after("minus the predecessor's clear) ") {
                        Some(s) if s.starts_with("- ") => r.nap_no_curve += 1,
                        Some(_) => r.nap_cr.push(num_after(seg, "minus the predecessor's clear) ").ok_or(format!("siding_row: a napper's curve rule neither a number nor \"-\" in: {seg}"))? as i64),
                        None => return Err(format!("siding_row: a napper with no curve rule in: {seg}")),
                    }
                    r.nap_contacts.push(v);
                    match after(" at q ") {
                        Some(s) if s.starts_with('-') && !s[1..].starts_with(|c: char| c.is_ascii_digit()) => r.nap_q.push(-1),
                        Some(_) => r.nap_q.push(num_after(seg, " at q ").ok_or(format!("siding_row: a napper's q neither a number nor \"-\" in: {seg}"))? as i64),
                        None => return Err(format!("siding_row: a napper with no q in: {seg}")),
                    }
                }
            }
        } else if l.starts_with("the bench's overrides (ironspec 04:07") {
            r.override_noops = req(l, "no-op overrides (the bar: 0): ")? as usize;
        } else if l.starts_with("the riders at t 1 (ironspec 07:28") {
            r.t1_unhomed = req(l, "riders without the bed's HOME at t 1 under --nap-ai (the bar: 0): ")? as usize;
            r.t1_not_rest = req(l, "riders not at REST at t 1 (the bar: 0): ")? as usize;
            r.t1_ticket_bad = req(l, "the bed's ticket not taken at t 1 under --nap-ai (the bar: 0): ")? as usize;
        } else if l.starts_with("the K-row push bar's scope (ironspec 03:00)") {
            r.push_excluded = l.contains("push: out of scope (computed");
            r.push_uncovered = req(l, "rests the push grid doesn't cover (the bar: 0): ")? as usize;
        } else if l.starts_with("the controls (ironspec 00:02 (3)") {
            r.control_fail = req(l, "controls failing (the bar: 0): ")? as usize;
            r.controls_n = req(l, "controls judged ")? as usize;
            // (each control's kind and own bar (G1's CHANGES on bc25c5cca (1)): "P(bar)=b" or "N=b")
            let key = "the controls' kinds (P: positive, its own bar named; N: negative, the plain-row rule; =1 bit) [";
            let i = l.find(key).ok_or(format!("siding_row: no controls' kinds in: {l}"))? + key.len();
            let body = &l[i..l[i..].find(']').map(|j| i + j).ok_or(format!("siding_row: unclosed controls' kinds in: {l}"))?];
            for k in body.split(", ").filter(|k| !k.is_empty()) {
                let (head, bit) = k.rsplit_once('=').ok_or(format!("siding_row: a control kind with no '=': {k}"))?;
                let bit = match bit { "1" => true, "0" => false, _ => return Err(format!("siding_row: a control's bit neither 0 nor 1: {k}")) };
                if head == "N" {
                    r.controls.push((None, bit));
                } else if let Some(b) = head.strip_prefix("P(").and_then(|s| s.strip_suffix(')')) {
                    r.controls.push((Some(b.to_string()), bit));
                } else {
                    return Err(format!("siding_row: a control kind neither P(bar) nor N: {k}"));
                }
            }
            if r.controls.len() != r.controls_n {
                return Err(format!("siding_row: {} controls judged but {} kinds in: {l}", r.controls_n, r.controls.len()));
            }
        } else if l.starts_with("the bars' samples (ironspec 23:56") {
            r.not_judged = req(l, "NOT JUDGED (0 samples): ")? as usize;
            r.releases_n = req(l, "fails): releases ")? as usize;
        } else if l.starts_with("the resting napper across the K row's passes") {
            r.push_passes = req(l, "included): ")? as usize;
            r.push_disp = req(l, "came to rest during them ")?;
            r.push_speed = req(l, "its largest speed ")?;
            r.pass_overlap = req(l, "met the resting napper's box ")? as usize;
            r.inject_rest_z = num_after(l, "the napper at rest at z ");
        } else if l.starts_with("the carts' block contacts (ironspec 21:25 (4)") {
            r.contacts_undeclared = req(l, "undeclared (the bar: 0): ")? as usize;
            r.contacts_early = req(l, "judged by the same stops): ")? as usize;
            r.contacts_stale = req(l, "stale declared stops (never hit; the bar: 0): ")? as usize;
        } else if l.starts_with("the queue's net up-slope displacements (G1 Nov 13 12:32") {
            r.upslope_disp = req(l, "once per excursion; the bar: 0): ")? as usize;
        } else if l.starts_with("the lip margin (ironspec 21:14 (2)") {
            let s = l.split("(stuck)): margins ").nth(1).ok_or(format!("siding_row: no lip margins in: {l}"))?;
            r.lip_min = if s.starts_with("- ") {
                None
            } else {
                Some(s.split_once("..").and_then(|(a, _)| a.trim().parse().ok()).ok_or(format!("siding_row: lip margins {s:?}"))?)
            };
            r.lip_never = req(l, "never reaching the rider (clean) ")? as usize;
            r.lip_stuck = req(l, "the rider never below the floor; fails) ")? as usize;
            r.lip_samples = req(l, "; samples ")? as usize;
            r.lip_no_ahead = req(l, "no cart ahead at the merge ")? as usize;
            r.lip_no_merge = req(l, "no merge entry ")? as usize;
        } else if l.starts_with("the nappers' PRESENT margins before their leader's press") {
            let s = l.split("outside normal operation): ").nth(1).ok_or(format!("siding_row: no PRESENT margins in: {l}"))?;
            r.present_margin_min = if s.starts_with("- ") { None } else { Some(s.split_once("..").and_then(|(a, _)| a.trim().parse().ok()).ok_or(format!("siding_row: PRESENT margins {s:?}"))?) };
        } else if l.starts_with("the napper's settle on R (day-1") {
            let s = l.rsplit("): ").next().ok_or(format!("siding_row: no settle in: {l}"))?;
            if !s.starts_with("- ") {
                let (a, b) = s.split_once("..").ok_or(format!("siding_row: settle {s:?}"))?;
                r.settle = Some((a.trim().parse().map_err(|_| format!("siding_row: settle {s:?}"))?, b.split(' ').next().unwrap_or("").parse().map_err(|_| format!("siding_row: settle {s:?}"))?));
            }
        } else if l.starts_with("the cart after each napper into the merge") {
            let s = l.split("napper's clear): ").nth(1).ok_or(format!("siding_row: no follower figures in: {l}"))?;
            r.nap_follow_min = if s.starts_with("- ") { None } else { Some(s.split_once("..").and_then(|(a, _)| a.trim().parse().ok()).ok_or(format!("siding_row: follower figures {s:?}"))?) };
        } else if l.starts_with("the siding's single occupancy") {
            r.r_joint = req(l, "or two carts on R (the bar: 0): ")? as usize;
            r.k_turns = req(l, "carts moving out of K's cell into F's or R's over the run ")? as usize;
            r.nap_gap_min = l.split("departure from R: ").nth(1).and_then(|s| s.split_once("..")).and_then(|(a, _)| a.trim().parse().ok());
        } else if l.starts_with("the PRESENT proxy bar") {
            r.present_under = req(l, "under the proxy bar (information): ")? as usize;
            r.present_bar = num_after(l, "-> the proxy bar ").map(|b| b as i64);
        } else if l.starts_with("the feed's measured delays against the design") {
            r.feed_off = req(l, "releases whose feed differs (the bar: 0): ")? as usize;
        } else if l.starts_with("the napper at rest when R powers") {
            r.power_judged = req(l, "releases judged ")? as usize;
            r.not_rest_at_power = req(l, "R powered with the napper not at rest (the bar: 0): ")? as usize;
            r.r_never_powered = req(l, "R never powered after a release (the bar: 0): ")? as usize;
            let releases = req(l, "napper releases ")? as usize;
            let ended = req(l, "gt (not judged) ")? as usize;
            // (G1 (2): every napper release judged once: R powered after it, never powered, or the
            // run's end; a mismatch is an error, not a silent pass)
            if r.power_judged + r.r_never_powered + ended != releases {
                return Err(format!("siding_row: {releases} napper releases but {} judged + {} never powered + {ended} at the run's end in: {l}", r.power_judged, r.r_never_powered));
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
            r.v_holds_start = req(l, "V held before the steady state (the start's transient) ")? as usize;
            r.v_rest_occ = l.split(", the first at the lane's occupancy ").nth(1).and_then(|s| s.split(';').next()).and_then(|s| s.trim().parse().ok());
        } else if l.starts_with("the nappers at their release") {
            r.rolling_releases = l.split(" releases, ").nth(1).and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok()).ok_or(format!("siding_row: no rolling-release count in: {l}"))?;
            let s = l.split("the napper's K tick to its rest on R ").nth(1).ok_or(format!("siding_row: no K tick -> rest in: {l}"))?;
            if !s.starts_with('-') {
                let (a, b) = s.split_once("..").ok_or(format!("siding_row: K tick -> rest {s:?}"))?;
                r.k_to_rest = Some((a.trim().parse().map_err(|_| format!("siding_row: K tick -> rest {s:?}"))?, b.split(' ').next().unwrap_or("").parse().map_err(|_| format!("siding_row: K tick -> rest {s:?}"))?));
            }
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
            r.occ_min = l.split("the smallest at a release from the steady state ").nth(1).and_then(|s| s.split(';').next()).and_then(|s| s.trim().parse().ok());
            r.occ_low = req(l, "ironspec 19:29 (4); the bar: 0): ")? as usize;
            r.occ8 = req(l, "releases at 8 (the 8th slot is the top cell; the bar: 0): ")? as usize;
        } else if l.starts_with("lane carts on the siding's cells") {
            r.lane_on_siding = req(l, "or the old siding's cells; the bar: 0): ")? as usize;
        } else if l.starts_with("lane carts in push contact with the waiting napper") {
            r.lane_push = req(l, "y ranges overlapping; the bar: 0): ")? as usize;
        } else if l.starts_with("the split row's through-carts past K") {
            r.th_passed = req(l, "passed ")? as usize;
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
            r.touch_leader = req(l, "beside its leader ")? as usize;
            r.touch_leader_late = req(l, "beside its leader after its rider dropped ")? as usize;
            r.touch_follower = req(l, "beside another cart ")? as usize;
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
    // (the carts-per-release pairs add up to the releases (ironspec 04:14 (6)))
    if r.releases_counted != r.releases_n {
        return Err(format!("siding_row: carts per release counts {} releases, the samples line {}", r.releases_counted, r.releases_n));
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
    for key in ["the branch returns' stored speed entering the merge cell", "nappers came to rest on the siding at z ", "lane carts in push contact with the waiting napper", "lane carts on the siding's cells", "the nappers' launches from the rest", "the lane's top cell V", "the lane's occupancy at the releases from the steady state", "the carts' block contacts (ironspec 21:25 (4)", "the controls (ironspec 00:02 (3)", "the bench's overrides (ironspec 04:07", "the bed at the start (ironspec 06:42 real", "the riders at t 1 (ironspec 07:28", "the PRESENT proxy bar", "the napper at rest when R powers", "the feed's measured delays against the design", "the K-row push bar's scope (ironspec 03:00)", "the bars' samples (ironspec 23:56", "the lip margin (ironspec 21:14 (2)", "the nappers' PRESENT margins before their leader's press", "the napper's settle on R (day-1", "the cart after each napper into the merge", "the siding's single occupancy", "the resting napper across the K row's passes", "the nappers at their release"] {
        if !text.lines().any(|l| l.starts_with(key)) {
            return Err(format!("siding_row: the bench printed no \"{key}\" line"));
        }
    }
    r.text = text;
    Ok(r)
}

/// The S-bend gate over a matrix's rows (ironspec 00:02 (1)-(2), 23:56): the lip outcome classes and
/// the resting napper's push measurement, each with its sample count and where its worst row is;
/// NOT JUDGED (a failure of the matrix) at 0 samples.
#[derive(Default)]
pub struct GateAgg {
    pub lip_min: Option<(i64, String)>,
    pub never: usize,
    pub stuck: usize,
    pub stuck_at: Vec<String>,
    pub samples: usize,
    pub no_ahead: usize,
    pub no_merge: usize,
    pub push_passes: usize,
    pub push_worst: Option<(f64, f64, String)>,
    pub contacts_early: usize,
    /// The K row's through-carts that passed K over the rows (the K row alive).
    pub k_row_passed: usize,
    /// The rows, and those whose push bar was computed out of scope (ironspec 03:00).
    pub rows: usize,
    pub push_excluded: usize,
    /// Rests no push-grid row covers, over the rows (each fails its row).
    pub push_uncovered: usize,
}

impl GateAgg {
    /// Add one row, `at` naming it.
    pub fn add(&mut self, r: &SidingRow, at: &str) {
        if let Some(q) = r.lip_min
            && self.lip_min.as_ref().is_none_or(|(m, _)| q < *m)
        {
            self.lip_min = Some((q, at.to_string()));
        }
        self.never += r.lip_never;
        self.stuck += r.lip_stuck;
        if r.lip_stuck > 0 && self.stuck_at.len() < 6 {
            self.stuck_at.push(at.to_string());
        }
        self.samples += r.lip_samples;
        self.no_ahead += r.lip_no_ahead;
        self.no_merge += r.lip_no_merge;
        self.push_passes += r.push_passes;
        if self.push_worst.as_ref().is_none_or(|(d, v, _)| r.push_disp > *d || (r.push_disp == *d && r.push_speed > *v)) {
            self.push_worst = Some((r.push_disp, r.push_speed, at.to_string()));
        }
        self.contacts_early += r.contacts_early;
        self.k_row_passed += r.th_passed;
        self.rows += 1;
        self.push_excluded += r.push_excluded as usize;
        self.push_uncovered += r.push_uncovered;
    }

    /// The gate line and the matrix-level failures it adds (NOT JUDGED bars). The K-row push bar with
    /// no pass in any rest stands only as the rows' COMPUTED exclusion (ironspec 03:00): every row out
    /// of scope by its own printed rest windows and pass ticks, its K row alive (G1's guard: a dead K
    /// row is NOT JUDGED), each rest covered by a push-grid row (an uncovered rest fails its row);
    /// otherwise NOT JUDGED, a failure. Passes that do occur are judged.
    pub fn line(&self, lip_bar: Option<i64>, k_row: bool) -> (String, usize) {
        let mut nj = 0usize;
        let lip_nj = if self.samples == 0 {
            nj += 1;
            "; NOT JUDGED (0 samples): FAIL".to_string()
        } else {
            String::new()
        };
        let push_nj = if k_row && self.push_passes == 0 && self.rows > 0 && self.push_excluded == self.rows {
            format!(
                "; OUT OF SCOPE (computed in all {} rows: no K-row pass falls in a napper's window on R, each row's windows and pass ticks printed; the K row alive, {} carts passed K; rests the push grid doesn't cover {})",
                self.rows, self.k_row_passed, self.push_uncovered
            )
        } else if k_row && self.push_passes == 0 {
            nj += 1;
            format!("; NOT JUDGED (0 samples; computed out of scope in {} of {} rows): FAIL", self.push_excluded, self.rows)
        } else {
            String::new()
        };
        let (pd, pv, pat) = self.push_worst.clone().unwrap_or((0.0, 0.0, "-".to_string()));
        (
            format!(
                "the S-bend gate over the rows (ironspec 00:02): the lip margin's samples {} (the least margin {}; never reaching the rider {}; STUCK {}{}; the bar {}){lip_nj}; no sample: no cart ahead at the merge {}, no merge entry {}; the resting napper across the K row's passes: {} passes, its largest displacement {pd:.6} and speed {pv:.6} (at {pat}; the bar: 0){push_nj}; contacts before t 20 (counted apart) {}",
                self.samples,
                self.lip_min.as_ref().map_or("-".to_string(), |(q, at)| format!("{q} gt at {at}")),
                self.never,
                self.stuck,
                if self.stuck_at.is_empty() { String::new() } else { format!(" at [{}]", self.stuck_at.join("; ")) },
                lip_bar.map_or("none".to_string(), |b| b.to_string()),
                self.no_ahead,
                self.no_merge,
                self.push_passes,
                self.contacts_early
            ),
            nj,
        )
    }
}

#[cfg(test)]
mod seed_list_tests {
    use super::*;

    /// `--seeds-matrix`: ranges and lists, and the refusals (a reversed range, a repeat, a non-number).
    #[test]
    fn the_seed_list() {
        assert_eq!(parse_seed_list("1..8"), Ok((1..=8).collect::<Vec<i64>>()));
        assert_eq!(parse_seed_list("1, 3, 5..7"), Ok(vec![1, 3, 5, 6, 7]));
        assert_eq!(parse_seed_list("4"), Ok(vec![4]));
        assert!(parse_seed_list("8..1").is_err());
        assert!(parse_seed_list("1..3,2").is_err());
        assert!(parse_seed_list("one").is_err());
        assert!(parse_seed_list("").is_err());
    }
}

#[cfg(test)]
mod official_row_tests {
    use super::*;

    /// tick-exact's official S-bend boundary row (artifact 1009706688854741; ironspec 21:07 (2)):
    /// lane x 3 on the module's geometry (D ascending_south, glass at (3,24,9)), arrival 0.30 at
    /// rest, the leader summoned at (3.5, 24.0625, 9.5) at 0.36 north, D pressed in world tick 2143
    /// (official t10, the leader's first tick), the switch a redstone block at (-1,22,9) set before
    /// its world tick (official setblock at t = world tick - 2133).
    fn official() -> (crate::dropbench::DropOpts, SidingOpts) {
        let o = crate::dropbench::DropOpts {
            lanes: 3,
            spacing: 4,
            run: 10,
            per_lane: 8,
            period: 100,
            cycles: 30,
            ret: 132,
            corner: 2,
            w0: 13,
            wp: 12,
            drop_w: 20,
            entry: 0,
            hold_n: 20,
            front_boost: false,
            pockets: 0,
            string_dy: 2,
            r_pulse: 30,
            retry: 40,
            login: Vec::new(),
            intake_flat: false,
            v_in: 0.36,
            r_boost: false,
            floor_bottom: false,
            riders: true,
            freeze32: Vec::new(),
            restart: Vec::new(),
        };
        let so = SidingOpts {
            k: 4,
            nap_every: 3,
            nap_ret: 760,
            nap_pulse: 30,
            merge_curve: true,
            slopes: 5,
            guard: false,
            early_only: false,
            any_pass: false,
            ring: 10,
            pick_u: 0,
            win_du: 8,
            win_tap: 0,
            ai: false,
            stamp_at_arrival: None,
            late_latch: false,
            posts: Vec::new(),
            k_row: Some(0.30),
            k_offsets: vec![0, 12, 24],
            nap_feed_v: Some(0.30),
            nap_feed_phase: 0.0,
            feed_path: true,
            feed_z8: false,
            feed_slime: true,
            present_k: Some(2),
            no_d_gate: true,
            inject_release: None,
            r_joint_control: None,
            held_rider_control: false,
            stray_ctl: None,
            inject_pass: None,
            push_grid: None,
            present_settle_worst: 22,
            present_latch_unclocked: false,
            early_r: None,
            hold_switch_off: false,
            hold_rerelease: false,
            day0: 13_300,
            skip_farm_rider: false,
            seed: 1,
            j_rail: false,
            lip_dy: 0,
            sbend_straight: false,
            forced_v: None,
            forced_v_panic: false,
            lip_volume: None,
            pos_offset: (0, 0, 0),
            r_until_string: false,
            upslope_ctl: None,
            c2_booster: false,
            r_delay: 0,
            d_slope: true,
            out: None,
            no_walkway: false,
            flat_detector: false,
            siding_north: true,
            fill_scene: None,
            top_flat: false,
            mirror_z: false,
            start_steady: false,
            r_support: Some("slime_block".to_string()),
            siding_dx: 2,
            siding_dy: 0,
            ns_merge: false,
            guard_det: true,
            guard_dz: 1,
            drop_entry: true,
            no_top_support: false,
            side_real: false,
            side_ctl: None,
        };
        (o, so)
    }

    fn clean(r: &SidingRow) -> bool {
        r.touching == 0 && r.off_rail == 0 && r.empty_releases == 0 && r.not_one == 0 && r.order_broken == 0 && r.knocked == 0 && r.lip_stuck == 0
    }

    /// The known answer, official tick numbering (ironspec 21:07 (2)): the switch at D - 5 is
    /// stuck at curve rule 5, at D - 4 clean at 6, and day-1's K = 4 (the switch at the press + 4,
    /// the pass dated at the press tick) gives 14; the lip margin (21:14 (2)) reads -1 and 0 there.
    #[test]
    fn official_sbend_boundary_and_k4() {
        let (o, so) = official();
        let at = |t: i64| {
            let mut s = so.clone();
            s.inject_release = Some(t);
            siding_row(&o, &s).unwrap()
        };
        let d5 = at(2138);
        assert_eq!(d5.nap_cr, vec![5], "D - 5: {:?}", d5.nap_cr);
        assert!(!clean(&d5), "D - 5 must be stuck: touching {}, off-rail {}", d5.touching, d5.off_rail);
        assert_eq!(d5.lip_min, Some(-1), "D - 5's lip margin");
        assert_eq!(d5.lip_stuck, 0, "D - 5: the rider drops (a margin of -1, not a held rider)");
        let d4 = at(2139);
        assert_eq!(d4.nap_cr, vec![6], "D - 4: {:?}", d4.nap_cr);
        assert!(clean(&d4), "D - 4 must be clean: touching {}, off-rail {}", d4.touching, d4.off_rail);
        assert_eq!(d4.lip_min, Some(0), "D - 4's lip margin");
        assert_eq!((d4.lip_samples, d4.not_judged, d4.control_fail), (1, 0, 0), "D - 4: one lip sample, nothing NOT JUDGED, no control");
        let k4 = siding_row(&o, &so).unwrap();
        assert!(!k4.nap_cr.is_empty() && k4.nap_cr.iter().all(|c| *c == 14), "K = 4: {:?}", k4.nap_cr);
        assert!(clean(&k4));
    }

    /// ironspec 04:14 (1): one failure list behind every count; each failing bar is named, and a clean
    /// row lists none.
    #[test]
    fn failures_list_every_failing_bar() {
        let r = SidingRow { not_rest_at_power: 1, override_noops: 2, push_uncovered: 1, lip_min: Some(1), ..Default::default() };
        let f = r.failures(Some(2), None);
        for k in ["the napper not at rest when R powered", "no-op overrides", "rests the push grid doesn't cover", "the lip margin under the bar"] {
            assert!(f.iter().any(|(n, _)| *n == k), "{k} missing from {f:?}");
        }
        assert!(SidingRow::default().failures(Some(2), None).is_empty());
    }

    /// ironspec 04:14 (8): the diagonal turn (a cart into K's curve from its east cell, turned north
    /// at 45 degrees) counts by the cell transition, and its control asserts it through the row (the
    /// k_sw = 6 row at nap_ret 800, the napper resting on R from t 2117; the cart at t 2139, 13 gt
    /// from the nearest K-row spawn, while K is north_east).
    #[test]
    fn diagonal_turn_counts_by_cell_transition() {
        let (o, mut so) = official();
        so.k = 6;
        so.nap_ret = 800;
        so.r_joint_control = Some(("diag".to_string(), 2139));
        let r = siding_row(&o, &so).unwrap();
        assert!(r.r_joint > 0 && r.k_turns > 0, "{}", r.text.lines().find(|l| l.starts_with("the siding's single occupancy")).unwrap_or("-"));
        assert_eq!(r.control_fail, 0, "{}", r.text.lines().find(|l| l.starts_with("the controls")).unwrap_or("-"));
    }

    /// ironspec 04:14 (7), 05:03, 06:15 real: the bar is the outcome, the napper at rest when R powers,
    /// judged on every release; the PRESENT proxy prints as information, reading k_sw (the press to
    /// the release) and the feed delay (the release to R powered) from the run (the official row's k
    /// is 4, its feed 8), its world-open lead on both terms by default (the floor 9) or on the settle
    /// term only with day-1's declared confirmation (--present-latch-unclocked: the floor 7).
    #[test]
    fn present_bar_reads_the_design() {
        let (o, mut so) = official();
        let r = siding_row(&o, &so).unwrap();
        let l = r.text.lines().find(|l| l.starts_with("the PRESENT proxy bar")).unwrap();
        assert!(l.contains("read from this run) 4..4 gt") && l.contains("read from this run) 8..8 gt"), "{l}");
        assert!(l.contains("-> the proxy bar 9 gt"), "{l}");
        assert!(r.power_judged > 0 && r.not_rest_at_power == 0, "{}", r.text.lines().find(|l| l.starts_with("the napper at rest when R powers")).unwrap_or("-"));
        so.present_latch_unclocked = true;
        let r = siding_row(&o, &so).unwrap();
        let l = r.text.lines().find(|l| l.starts_with("the PRESENT proxy bar")).unwrap();
        assert!(l.contains("-> the proxy bar 7 gt") && l.contains("--present-latch-unclocked declared"), "{l}");
    }

    /// The outcome bar's positive control through the design's own cells (G1 / ironspec 07:31): the
    /// release switch on N gt early shifts R's power N gt through the real feed path, measured in the
    /// same run. THE TIE TEST: at a phase where the plain row finds the napper first at rest exactly one
    /// reading before R's power (the official k 4 row at 0.10, searched over nap_ret 815..823), the
    /// plain row passes; --early-r 1 moves R's power reading onto the very reading at which the plain
    /// napper first shows at rest (R powered in the block phase of the tick whose entity phase would
    /// bring it to rest: the tie), and the bar must fail it (the napper, boosted, never rests);
    /// --early-r 2 fails too. Both controls are caught, the shift measured as N.
    #[test]
    fn early_r_control_fails_the_outcome() {
        let (o, mut so) = official();
        so.nap_feed_v = Some(0.10);
        let line = |r: &SidingRow| r.text.lines().find(|l| l.starts_with("the napper at rest when R powers")).unwrap_or("-").to_string();
        // (per release: the napper's first reading at rest (None: never at rest before R) and R's
        // first power reading)
        let ticks = |r: &SidingRow| -> Vec<(Option<i64>, i64)> {
            let l = line(r);
            let Some(i) = l.find("(the tie at 0: R powered in the block phase of the tick the napper came to rest) [") else { return Vec::new() };
            let body = &l[i..];
            let body = &body[body.find('[').unwrap_or(0) + 1..body.find(']').unwrap_or(body.len())];
            body.split(", ")
                .filter_map(|e| {
                    let (q, p) = e.split_once(" -> ")?;
                    let p: i64 = p.split_whitespace().next()?.parse().ok()?;
                    Some((q.trim().parse::<i64>().ok(), p))
                })
                .collect()
        };
        let mut found = None;
        for nr in [820, 819, 821, 818, 822, 817, 816, 815, 823] {
            so.nap_ret = nr;
            so.early_r = None;
            let plain = siding_row(&o, &so).unwrap();
            let t = ticks(&plain);
            if plain.power_judged > 0 && plain.not_rest_at_power == 0 && plain.r_never_powered == 0 && !t.is_empty() && t.iter().all(|(q, p)| q.is_some_and(|q| p - q >= 1)) && t.iter().any(|(q, p)| q.is_some_and(|q| p - q == 1)) {
                found = Some((nr, t));
                break;
            }
        }
        let (nr, plain_t) = found.expect("a plain phase at 0.10 whose napper first rests exactly one reading before R's power");
        so.nap_ret = nr;
        so.early_r = Some(1);
        let tie = siding_row(&o, &so).unwrap();
        let tie_t = ticks(&tie);
        // (the tie: for every release whose plain napper rested one reading before R, the control's R
        // power reading is that very reading; the bar fails it)
        let tied: Vec<(i64, i64)> = plain_t.iter().filter_map(|(q, p)| q.filter(|q| p - q == 1).map(|q| (q, *p))).collect();
        assert!(!tied.is_empty() && tied.iter().all(|(q, _)| tie_t.iter().any(|(_, p1)| p1 == q)), "nap_ret {nr}: plain {plain_t:?}, --early-r 1 {tie_t:?}");
        assert!(tie.not_rest_at_power > 0, "--early-r 1 at nap_ret {nr} (the tie): {}", line(&tie));
        let ctl = |r: &SidingRow| r.text.lines().find(|l| l.starts_with("the controls")).unwrap_or("-").to_string();
        assert!(tie.control_fail == 0 && tie.controls_n == 1 && ctl(&tie).contains("the shift measured as expected"), "{}", ctl(&tie));
        assert!(tie.single_row_verdict(None, None).is_ok(), "{:?}", tie.single_row_verdict(None, None));
        so.early_r = Some(2);
        let early = siding_row(&o, &so).unwrap();
        assert!(early.not_rest_at_power > 0, "--early-r 2: {}", line(&early));
        assert!(early.control_fail == 0 && early.controls_n == 1, "{}", ctl(&early));
    }

    /// G1 (2): a release R never powers after is a failure, not a dropped sample; the control holds
    /// the release switch off for the first napper release, and the row counts it.
    #[test]
    fn hold_switch_off_counts_an_unpowered_release() {
        let (o, mut so) = official();
        so.hold_switch_off = true;
        let r = siding_row(&o, &so).unwrap();
        let line = r.text.lines().find(|l| l.starts_with("the napper at rest when R powers")).unwrap_or("-").to_string();
        assert!(r.r_never_powered > 0, "{line}");
        assert!(r.failures(None, None).iter().any(|f| f.0 == "R never powered after a release"), "{line}");
        assert_eq!((r.control_fail, r.controls_n), (0, 1), "{}", r.text.lines().find(|l| l.starts_with("the controls")).unwrap_or("-"));
        assert!(r.single_row_verdict(None, None).is_ok(), "{:?}", r.single_row_verdict(None, None));
    }

    /// G1's CHANGES on bc25c5cca (4): the MID-RUN never-powered path. With the switch held off for
    /// the first napper release and a second release 10 gt later (the switch working), the first,
    /// still waiting for R, is counted at the second release, not at the run's end.
    #[test]
    fn a_release_still_waiting_at_the_next_is_counted() {
        let (o, mut so) = official();
        so.hold_switch_off = true;
        so.hold_rerelease = true;
        let r = siding_row(&o, &so).unwrap();
        let line = r.text.lines().find(|l| l.starts_with("the napper at rest when R powers")).unwrap_or("-").to_string();
        let ctl = r.text.lines().find(|l| l.starts_with("the controls")).unwrap_or("-").to_string();
        let mid: usize = ctl.split("releases R never powered after: ").nth(1).and_then(|s| s.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
        assert!(r.r_never_powered > 0 && mid > 0, "{line} | {ctl}");
        assert!(r.single_row_verdict(None, None).is_ok(), "{:?}", r.single_row_verdict(None, None));
    }

    /// G1's CHANGES on bc25c5cca (1): the single row's verdict. A plain failing row fails; a
    /// NEGATIVE control's run with another failure fails (the plain-row rule); a positive control
    /// that bit but whose own bar isn't among the failures is NOT CAUGHT; a biting positive control
    /// whose own bar fails is CAUGHT.
    #[test]
    fn the_single_row_verdict_follows_each_controls_kind() {
        let plain_fail = SidingRow { stray_power: 3, ..Default::default() };
        assert!(plain_fail.single_row_verdict(None, None).is_err());
        assert!(SidingRow::default().single_row_verdict(None, None).is_ok());
        let neg_with_failure = SidingRow { off_rail: 1, controls_n: 1, controls: vec![(None, true)], ..Default::default() };
        assert!(neg_with_failure.single_row_verdict(None, None).is_err());
        let neg_clean = SidingRow { controls_n: 1, controls: vec![(None, true)], ..Default::default() };
        assert!(neg_clean.single_row_verdict(None, None).is_ok());
        let pos_without_own_bar = SidingRow { off_rail: 1, controls_n: 1, controls: vec![(Some("stray power".to_string()), true)], ..Default::default() };
        assert!(pos_without_own_bar.single_row_verdict(None, None).is_err());
        let pos_caught = SidingRow { stray_power: 10, controls_n: 1, controls: vec![(Some("stray power".to_string()), true)], ..Default::default() };
        assert!(pos_caught.single_row_verdict(None, None).is_ok());
        let pos_not_bit = SidingRow { stray_power: 10, control_fail: 1, controls_n: 1, controls: vec![(Some("stray power".to_string()), false)], ..Default::default() };
        assert!(pos_not_bit.single_row_verdict(None, None).is_err());
    }

    /// The rest-support control through the rows' own code path (ironspec 04:14 (9)): glass under R
    /// (a non-conductor) leaves R unpowered, so the launch fails and the control is CAUGHT on its own
    /// bar. (329b5acc3's strict `# contacts:` read errored on this row: the design's support is read
    /// now, whatever a control lays.)
    #[test]
    fn the_glass_support_control_is_caught() {
        let (o, mut so) = official();
        so.r_support = Some("minecraft:glass".to_string());
        let r = siding_row(&o, &so).unwrap();
        let v = r.single_row_verdict(None, None);
        assert!(v.as_ref().is_ok_and(|s| s.contains("launches stuck or late")), "{v:?}");
    }

    /// G1's CHANGES on bc25c5cca (5): the riders' state at t 1 asserted. The official row's riders
    /// are at REST at t 1 (a night start, day time 13,300); a day start (6000) is the control and
    /// fails "riders not at REST at t 1".
    #[test]
    fn a_day_start_fails_the_t1_assertion() {
        let (o, mut so) = official();
        let r = siding_row(&o, &so).unwrap();
        let t1 = r.text.lines().find(|l| l.starts_with("the riders at t 1")).unwrap_or("-").to_string();
        assert_eq!(r.t1_not_rest, 0, "{t1}");
        so.day0 = 6000;
        let d = siding_row(&o, &so).unwrap();
        let t1d = d.text.lines().find(|l| l.starts_with("the riders at t 1")).unwrap_or("-").to_string();
        assert!(d.t1_not_rest > 0 && d.failures(None, None).iter().any(|f| f.0 == "riders not at REST at t 1"), "{t1d}");
    }

    /// G1 (2), the parse: every napper release judged once; a text whose counts don't reconcile is an
    /// error (here the official row's text with one release more than it judged).
    #[test]
    fn unreconciled_releases_are_an_error() {
        let (o, so) = official();
        let text = siding_bench(&o, &so).unwrap();
        assert!(parse_siding_row(text.clone(), &so).is_ok());
        let l = text.lines().find(|l| l.starts_with("the napper at rest when R powers")).unwrap().to_string();
        let i = l.find("napper releases ").unwrap() + "napper releases ".len();
        let n: usize = l[i..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
        let bad = text.replace(&l, &l.replacen(&format!("napper releases {n}"), &format!("napper releases {}", n + 1), 1));
        assert!(parse_siding_row(bad, &so).is_err());
    }

    /// ironspec 04:07 / 07:28 C8: an override that leaves its cell unchanged is counted as a no-op,
    /// and every override prints its cell's before -> after.
    #[test]
    fn an_override_that_changes_nothing_is_a_noop() {
        let mut w = redsim::world::World::new(BlockPos::new(0, 0, 0), [4, 4, 4]);
        let glass = Block::parse("minecraft:glass").unwrap();
        let mut ov = Overrides::default();
        ov.set(&mut w, BlockPos::new(1, 1, 1), &glass, "test glass", false);
        assert_eq!(ov.noops(), 0);
        ov.set(&mut w, BlockPos::new(1, 1, 1), &glass, "test glass", false);
        assert_eq!(ov.noops(), 1);
        let t = ov.transitions();
        assert!(t.contains("minecraft:air -> minecraft:glass x1") && t.contains("minecraft:glass -> minecraft:glass x1"), "{t}");
    }

    /// The grid's "missed" count (G1 / ironspec 07:28 C7): an injected pass that comes after the
    /// napper has left its rest meets nothing (push passes 0), which the grid counts as a missed pass.
    #[test]
    fn a_pass_after_the_rest_is_missed() {
        let (o, mut so) = official();
        so.inject_pass = Some((0.30, 0.5, 400, false));
        let r = siding_row(&o, &so).unwrap();
        assert_eq!(r.push_passes, 0, "{}", r.text.lines().find(|l| l.starts_with("the resting napper across the K row's passes")).unwrap_or("-"));
    }

    /// ironspec 07:28 C7, the push scope's NOT JUDGED: K-row passes meeting the napper while it rolls
    /// on R (the turn control at 0.10, nap_ret 760, t 2108: 9 pass ticks) are NOT JUDGED (the push
    /// is measured only at rest) and counted in the samples line.
    #[test]
    fn a_pass_meeting_the_rolling_napper_is_not_judged() {
        let (o, mut so) = official();
        so.nap_feed_v = Some(0.10);
        so.nap_ret = 760;
        so.r_joint_control = Some(("turn".to_string(), 2108));
        let r = siding_row(&o, &so).unwrap();
        let s = r.text.lines().find(|l| l.starts_with("the bars' samples")).unwrap_or("-").to_string();
        assert!(r.not_judged >= 1 && s.contains("met the napper while it rolled on R or launched"), "{s}");
    }

    /// ironspec 07:28 C7 / G1 (2), a dead K row: with the K row on but no pass ever at K (its spawns
    /// past the run's end), the K row's bars are NOT JUDGED and the row fails.
    #[test]
    fn a_dead_k_row_is_not_judged() {
        let (o, mut so) = official();
        so.k_offsets = vec![100_000];
        let r = siding_row(&o, &so).unwrap();
        let s = r.text.lines().find(|l| l.starts_with("the bars' samples")).unwrap_or("-").to_string();
        assert!(r.not_judged >= 1 && s.contains("the K row's passes at K (the K row alive)"), "{s}");
        assert!(r.failures(None, None).iter().any(|f| f.0 == "bars NOT JUDGED"), "{s}");
    }

    /// ironspec 07:28 C7, the new 0-sample counts: a run too short for a steady window (2 cycles)
    /// has no occupancy, V, lip, curve, follower, PRESENT or R-power samples: each NOT JUDGED.
    #[test]
    fn an_empty_steady_window_is_not_judged() {
        let (mut o, so) = official();
        o.cycles = 2;
        let r = siding_row(&o, &so).unwrap();
        let s = r.text.lines().find(|l| l.starts_with("the bars' samples")).unwrap_or("-").to_string();
        for k in ["the releases from the steady state (the occupancy bars", "the steady window's ticks (the V bar)", "the nappers' curve rules at the merge", "the followers' margins behind a napper", "the releases judged at R's first power (the napper at rest)"] {
            assert!(s.contains(k), "{k} not NOT JUDGED in: {s}");
        }
        assert!(r.failures(None, None).iter().any(|f| f.0 == "bars NOT JUDGED"));
    }

    /// ironspec 07:28 C7, the lip classes: a forced release (k_sw 6, arrival 0.25, nap_ret 822,
    /// released at t 2128) whose napper meets its leader in the merge cell while the leader's rider is
    /// still above its floor: counted as the lip's contact (beside its leader), not as a stuck leader.
    #[test]
    fn the_lip_contact_with_the_leader_is_classed() {
        let (o, mut so) = official();
        so.k = 6;
        so.nap_feed_v = Some(0.25);
        so.nap_ret = 822;
        so.inject_release = Some(2128);
        let r = siding_row(&o, &so).unwrap();
        let l = r.text.lines().find(|l| l.starts_with("ticks with two carts over the merge")).unwrap_or("-").to_string();
        assert!(r.touch_leader > 0 && r.touch_leader_late == 0, "{l}");
        assert!(r.lip_min.is_some_and(|q| q <= -1), "the lip margin {:?}", r.lip_min);
    }

    /// G1 (1): arrivals over 0.46 are in the failure list, so a single row with them fails alone as it
    /// does in a matrix. The drop entry fixes the lane-back arrival at 0.260 whatever the stored speed
    /// (v_in 0.26 .. 0.80 all arrive at 0.260: no row of the design trips it), so the control is the
    /// old curve entry (no drop entry) at a stored 0.60, whose arrivals reach 0.520.
    #[test]
    fn arrivals_over_the_bar_fail_a_single_row() {
        let (mut o, mut so) = official();
        o.v_in = 0.60;
        so.drop_entry = false;
        let r = siding_row(&o, &so).unwrap();
        let f = r.failures(None, None);
        assert!(r.arrivals_over > 0 && f.iter().any(|x| x.0 == "arrivals over 0.46" && x.1 == r.arrivals_over), "no drop entry, v_in 0.60: {f:?}");
        // (the design's own drop entry at a stored 0.26: every arrival at 0.260, none over)
        let (mut o2, so2) = official();
        o2.v_in = 0.26;
        let r2 = siding_row(&o2, &so2).unwrap();
        assert_eq!(r2.arrivals_over, 0);
    }

    /// The lip bar's positive control through the rows' own code path (ironspec 00:27): the official
    /// row at its clean phase (D - 4) with glass two cells under the lip from the napper's release: the
    /// leader is held one block down with its rider above the floor, the napper reaches the rider, the
    /// lip outcome is STUCK and the row fails; the controls line says it bites.
    #[test]
    fn held_rider_control_fails_through_the_row() {
        let (o, mut so) = official();
        so.inject_release = Some(2139);
        so.held_rider_control = true;
        let r = siding_row(&o, &so).unwrap();
        assert!(r.lip_stuck >= 1, "the held rider must read STUCK: {}", r.text.lines().find(|l| l.starts_with("the lip margin")).unwrap_or("-"));
        assert!(!clean(&r), "the row must fail");
        assert_eq!(r.control_fail, 0, "{}", r.text.lines().find(|l| l.starts_with("the controls")).unwrap_or("-"));
    }

    /// The lip margin's definition pinned on tick-exact's official per-tick positions (artifact
    /// 1009706688854741, t28..t33; the rider at the leader's position, its box 0.6 x 1.95): the
    /// switch at D - 5 is cut at t31 (-1), at D - 4 its move passes the rider's plane at t32 with the
    /// rider's top below 24.0 since t31 (0).
    #[test]
    fn lip_margin_on_official_rows() {
        let leader = [(28, 22.263, 6.201), (29, 22.194, 6.132), (30, 22.118, 6.055), (31, 22.032, 5.970), (32, 21.938, 5.876), (33, 21.835, 5.773)];
        let rider: Vec<(i64, f64, f64, f64, f64, f64)> = leader.iter().map(|&(t, y, z)| (t, 3.5, z, y, 0.3, 1.95)).collect();
        let d5 = [(28, 3.272, 7.228), (29, 3.407, 7.093), (30, 3.542, 6.958), (31, 3.542, 6.845), (32, 3.542, 6.845), (33, 3.542, 6.845)];
        let d4 = [(28, 3.136, 7.364), (29, 3.272, 7.228), (30, 3.407, 7.093), (31, 3.542, 6.958), (32, 3.542, 6.704), (33, 3.542, 6.577)];
        assert_eq!(lip_margin(&d5, &rider, 24.0), Lip::Margin(-1));
        assert_eq!(lip_margin(&d4, &rider, 24.0), Lip::Margin(0));
        // (a napper that never reaches the rider: no contact window)
        let far = [(31, 3.542, 7.5), (32, 3.542, 7.45), (33, 3.542, 7.4)];
        assert_eq!(lip_margin(&far, &rider, 24.0), Lip::Never);
    }

    /// The lip bar's positive control (ironspec 00:02 (1), G1's CHANGES 1 on a191f3dfb): a rider held
    /// at the lip, its box top never below the floor, while the napper reaches it, is STUCK (not
    /// "never"); and a row whose only napper had no cart ahead of it at the merge has no lip sample:
    /// NOT JUDGED, a failure of the matrix, never a clean pass.
    #[test]
    fn lip_held_rider_and_no_cart_ahead_fail() {
        // (the D - 5 napper of the official row against a rider that stays on the lip at y 24.0)
        let held: Vec<(i64, f64, f64, f64, f64, f64)> = (28..=33).map(|t| (t, 3.5, 6.490, 24.0, 0.3, 1.95)).collect();
        let d5 = [(28, 3.272, 7.228), (29, 3.407, 7.093), (30, 3.542, 6.958), (31, 3.542, 6.845), (32, 3.542, 6.845), (33, 3.542, 6.845)];
        assert_eq!(lip_margin(&d5, &held, 24.0), Lip::Held(29));
        let row = SidingRow { lip_stuck: 1, lip_samples: 1, ..Default::default() };
        let mut g = GateAgg::default();
        g.add(&row, "held");
        let (line, nj) = g.line(Some(2), false);
        assert!(line.contains("STUCK 1 at [held]"), "{line}");
        assert_eq!(nj, 0);
        let lone = SidingRow { lip_no_ahead: 1, ..Default::default() };
        let mut g = GateAgg::default();
        g.add(&lone, "lone");
        let (line, nj) = g.line(Some(2), false);
        assert_eq!(nj, 1, "{line}");
        assert!(line.contains("NOT JUDGED (0 samples): FAIL"), "{line}");
        // (the K-row pass bar with no pass in a --k-row matrix and no row computed out of scope (a
        // dead K row is never computed out of scope): NOT JUDGED too)
        let mut g = GateAgg::default();
        g.add(&row, "held");
        let (line, nj) = g.line(Some(2), true);
        assert_eq!(nj, 1, "{line}");
        assert!(line.contains("computed out of scope in 0 of 1 rows"), "{line}");
        // (every row computed out of scope (ironspec 03:00), its K row alive: printed so, not a
        // failure and never a pass; one row not out of scope keeps the matrix NOT JUDGED)
        let out = SidingRow { lip_stuck: 1, lip_samples: 1, th_passed: 9, push_excluded: true, ..Default::default() };
        let mut g = GateAgg::default();
        g.add(&out, "out");
        let (line, nj) = g.line(Some(2), true);
        assert_eq!(nj, 0, "{line}");
        assert!(line.contains("OUT OF SCOPE (computed in all 1 rows") && line.contains("9 carts passed K"), "{line}");
        g.add(&row, "held");
        assert_eq!(g.line(Some(2), true).1, 1);
    }

    /// The push grid's table round-trips (--push-grid-out's lines, '#' comments skipped), and a bad
    /// line is an error, not a skipped row.
    #[test]
    fn push_grid_table_parses() {
        // (G1 (5): only a passing grid's table, its provenance header and verdict line present, every
        // row 8 columns)
        let head = "# push grid (ironfarm escape-bench --siding --push-grid-run 0.10,0.44,0.01 ...)\n";
        let pass = "# verdict: PASS (2 rests ...)\n";
        let rows = "7.8950 0.100 0.0000 80 0.000000 0.000000 0.092 0.181000\n7.5700 0.300 0.0375 80 0.000000 0.000000 0.130 0.000000\n";
        let g = parse_push_grid(&format!("{head}{rows}{pass}")).unwrap();
        assert_eq!(g.len(), 2);
        assert_eq!(g[1], GridRow { z: 7.57, arrival: 0.3, phase: 0.0375, passes: 80, disp: 0.0, speed: 0.0, gap: 0.13, planted: 0.0 });
        assert_eq!((g[0].z, g[0].planted), (7.895, 0.181));
        // (no verdict line: a failing grid's table; no header; a 6-column row; a short row; no rows)
        assert!(parse_push_grid(&format!("{head}{rows}")).is_err());
        assert!(parse_push_grid(&format!("{rows}{pass}")).is_err());
        assert!(parse_push_grid(&format!("{head}7.8950 0.100 0.0000 40 0.000000 0.000000\n{pass}")).is_err());
        assert!(parse_push_grid(&format!("{head}7.8950 0.100\n{pass}")).is_err());
        assert!(parse_push_grid(&format!("{head}{pass}")).is_err());
    }

    /// The lip margin's known answer at the worst qualifying phase (ironspec 22:22 (1)): tick-exact's
    /// official row (artifact 1032428046539821: arrival 0.15, the napper rolling onto R, its K tick
    /// 12 gt before the press, napper first), official numbering (P = t10): the switch at P - 2 is
    /// cut at t31 (-1, stuck), at P - 1 its move reaches the rider's plane at t34 (2, clean).
    #[test]
    fn lip_margin_on_official_rolling_row() {
        let leader = [
            (25, 22.422, 6.360),
            (26, 22.377, 6.314),
            (27, 22.324, 6.261),
            (28, 22.263, 6.201),
            (29, 22.194, 6.132),
            (30, 22.118, 6.055),
            (31, 22.032, 5.970),
            (32, 21.938, 5.876),
            (33, 21.835, 5.773),
            (34, 21.722, 5.660),
            (35, 21.600, 5.537),
        ];
        let rider: Vec<(i64, f64, f64, f64, f64, f64)> = leader.iter().map(|&(t, y, z)| (t, 3.5, z, y, 0.3, 1.95)).collect();
        let p2 = [(25, 2.710, 7.229), (26, 2.890, 7.390), (27, 3.061, 7.561), (28, 3.170, 7.330), (29, 3.339, 7.161), (30, 3.508, 6.992), (31, 3.508, 6.845), (32, 3.508, 6.845)];
        let p1 = [(28, 2.879, 7.379), (29, 3.045, 7.545), (30, 3.166, 7.334), (31, 3.332, 7.168), (32, 3.497, 7.003), (33, 3.661, 6.839), (34, 3.661, 6.530), (35, 3.661, 6.490)];
        assert_eq!(lip_margin(&p2, &rider, 24.0), Lip::Margin(-1));
        assert_eq!(lip_margin(&p1, &rider, 24.0), Lip::Margin(2));
    }
}
