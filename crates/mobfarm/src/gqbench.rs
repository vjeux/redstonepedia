//! The gravity-queue bench (ironspec Nov 11 16:38, vjeux's lead: paper v0 artifact 2098083320824660):
//! a straight line built in code along +x, top to bottom:
//! - the tail chain: `tail` pockets of the shared module (the stand's pocket, farmopt 16:38: P a flat
//!   detector rail, R the plain rest slope, G a powered slope under the oak fence gate (powered as
//!   the gate opens, the stand's repeater and wire), U a rail rising back one level; pitch 4, one
//!   level down per pocket); the last pocket's gate stands over the pressed run's top slope;
//! - the pressed run: `run` plain slopes, straight;
//! - the escapement (R1's): G0 over P (a powered slope always on, a redstone block under it), G_P
//!   over R (a plain slope); windows G_P at A+6 for 11 gt, G0 at A+13 for 12 gt, three advances a
//!   cycle (A = 32, 56, 80), every gate shifted by `corner`;
//! - the hold: 3 pockets of the same module, filled front-first (a gate is open while the pocket
//!   ahead is latched empty), all released together at T+3 (their gates open for `pulse`, the latch
//!   reset);
//! - the descent and a sink: a cart past it is gone and comes back `ret` gt after its release at the
//!   tail chain's top, `ret_gap` gt behind the cart before it (the climb).
//!
//! The latches read only the P detector rails' real powered state. The tail chain feeds the run only
//! while the run holds fewer than `cap` carts by count (tail releases into the run minus passes into
//! the hold's first pocket, the cart staged on P counted with the run): pass-sensor evidence.
//! Printed: carts per opening at G0 and G_P, every gate edge with a cart's box in the gate's cell,
//! each arrival's stored speed at contact with the run's back cart (the bar 0.2), the run's fill at
//! its emptiest, the hold's carts at each release, the tail chain's occupancy.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct GqOpts {
    pub carts: usize,
    pub riders: Vec<bool>,
    pub tail: usize,
    pub run: i32,
    pub cap: usize,
    pub cycles: i64,
    pub corner: i64,
    pub pulse: i64,
    pub wire: i64,
    pub spacer: i64,
    pub ret: i64,
    pub ret_gap: i64,
    pub ret_speed: f64,
    /// Every cart frozen from tick T for D gt (the latches and gates run); the escapement's and the
    /// hold's openings wait while it lasts (the liveness gate).
    pub freeze: Vec<(i64, i64)>,
    /// The hold's release: every hold gate open together this long (the rear cart passes them all).
    pub hold_pulse: i64,
    /// The hold's release as a front-first ripple: the front gate at T+3, each gate behind D gt
    /// later, all shut together hold_pulse after the last opened (0: together).
    pub hold_ripple: i64,
    /// Live gt from the last tail pocket's pass edge before its gate may release (the cart at rest).
    pub settle: i64,
    /// The overfill junction (ironspec 17:19 (i)) instead of the top-cell-free feed.
    pub overfill: bool,
    /// Overfill: live gt after a G0 admission edge before the last tail gate shuts.
    pub adm_settle: i64,
    /// The intake keyed on the escapement's own admission (ironspec 17:43 (B)): one intake per G0
    /// opening edge, released once the last pocket's cart is latched and settled.
    pub intake_admit: bool,
    /// The ideal tail: no tail chain; each pooled cart placed at rest at the last tail gate's face
    /// whenever the last pocket is empty (the intake junction measured alone).
    pub ideal_tail: bool,
    /// No flat between the intake's G5 and the run (the last tail gate over the run's top slope).
    pub no_intake_flat: bool,
    /// The run filled pressed from G0's rest up to this far past the last tail gate's cell x (0: one
    /// cart a cell).
    pub back_at: f64,
    /// (cc767b78) day-1's live-time cycle clock and liveness gate (`crate::liveness`): the cycle
    /// restarts at each golem event (the column live at a summon boundary: a frozen column makes
    /// none), every pulse (the hold's release at T+3, G_P and G0's openings, the tail chain's
    /// release) ANDed with ALIVE at its rise, a risen pulse completing, the cycle's unfired
    /// remainder dropped when ALIVE falls.
    pub alive: bool,
    /// (cc767b78) T,D: a real freeze: the whole bench at level 32 inside a forced ring of chunks
    /// (redstone running, entities frozen) from tick T for D gt.
    pub freeze32: Vec<(i64, i64)>,
    /// (7b90c3f1) The batch receiver: Tail(0..2) in timed mode, (T_MID, T_REAR) live gt from the
    /// hold's release (committed: paused while ALIVE is low, never dropped).
    pub receiver: Option<(i64, i64)>,
    /// (7b90c3f1) The batch's return offsets from the hold's release + ret (per cart order).
    pub ret_offs: Vec<i64>,
    pub recv_settle: i64,
    pub recv_feed: i64,
    pub recv_timeout: i64,
    /// (7b90c3f1) The receiver alone: a cart fed into Tail(3) leaves at once and comes back at the
    /// next hold release + ret + OFFS[j] in feed order (an ideal downstream; with --carts 3, one
    /// batch a cycle through the receiver only).
    pub recv_ideal: bool,
    /// (7b90c3f1) The receiver's flying feed: the front stays open through the advances.
    pub recv_fly: bool,
    /// (7b90c3f1) The receiver's shuts keyed on the front pocket's P (Tail(2)'s) rising edge with
    /// the receiver idle: the middle at the edge + .0, the rear at + .1 (live gt).
    pub receiver_p: Option<(i64, i64)>,
    /// (7b90c3f1) Flats between the receiver's front gate (Tail(2)'s G) and the U into Tail(3).
    pub recv_run: i32,
    /// (7b90c3f1) The flying ripple keyed on the receiver's exit detector: the next advance this many
    /// live gt after the leaving cart's exit edge (needs recv_run >= 2).
    pub recv_exit_d: Option<i64>,
    /// (7b90c3f1) Day-1's fast ring emulated: an edge every Q live gt (0: off); with it the
    /// receiver's feed spacer counts `recv_spacer_edges` edges and --recv-exit-d counts edges.
    pub ring_q: f64,
    pub recv_spacer_edges: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Tail(usize),
    G0,
    Gp,
    Hold(usize),
}

struct Geo {
    gates: Vec<(Role, i32, i32, Option<(i32, i32)>)>,
    tail_p: Vec<(i32, i32)>,
    hold_p: Vec<(i32, i32)>,
    run_x: (f64, f64),
    g0_plane: f64,
    gp_plane: f64,
    launch: (f64, f64),
    sink_x: f64,
    run_cells: Vec<(i32, i32)>,
    run_sensor: (i32, i32),
    /// (7b90c3f1) The receiver's exit detector: the second flat past its front gate (--recv-run 2+).
    recv_exit: Option<(i32, i32)>,
}

fn build(o: &GqOpts) -> Result<(FarmSpec, Geo), String> {
    let glass = Block::parse("minecraft:glass")?;
    let down = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let up = Block::parse("minecraft:rail[shape=ascending_east,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let det = Block::parse("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    let det_down = Block::parse("minecraft:detector_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    let pdown_off = Block::parse("minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    let pdown_on = Block::parse("minecraft:powered_rail[powered=true,shape=ascending_west,waterlogged=false]")?;
    let gate = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut extra: Vec<(BlockPos, Block)> = Vec::new();
    // (the track: x the next cell, lv the rail's height at its west edge)
    let top = 2 * (o.tail as i32 + 6) + o.run + 8;
    let (mut x, mut lv) = (0i32, top);
    let mut g = Geo { gates: Vec::new(), tail_p: Vec::new(), hold_p: Vec::new(), run_x: (0.0, 0.0), g0_plane: 0.0, gp_plane: 0.0, launch: (0.5, top as f64 + 0.0625), sink_x: 0.0, run_cells: Vec::new(), run_sensor: (0, 0), recv_exit: None };
    let mut put = |blocks: &mut Vec<(BlockPos, Block)>, x: i32, y: i32, b: &Block| {
        blocks.push((BlockPos::new(x, y, 0), b.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
    };
    // (the launch flat, then a rigid block west of every descending slope's high side as needed)
    put(&mut blocks, x, lv, &flat);
    x += 1;
    let intake_flat = !o.no_intake_flat;
    let pocket = |blocks: &mut Vec<(BlockPos, Block)>, extra: &mut Vec<(BlockPos, Block)>, put: &mut dyn FnMut(&mut Vec<(BlockPos, Block)>, i32, i32, &Block), x: &mut i32, lv: &mut i32, last_over_run: bool, run_flats: i32| -> ((i32, i32), (i32, i32), (i32, i32)) {
        // P flat detector at lv; R down; G down (powered, the gate over it at R's level); U up
        let p = (*x, *lv);
        put(blocks, *x, *lv, &det);
        put(blocks, *x + 1, *lv - 1, &down);
        let gate_cell = (*x + 2, *lv - 1);
        let g_cell = (*x + 2, *lv - 2);
        if last_over_run {
            // (the intake, farmopt 17:14 on onechunk's v3: G5 a plain slope (no boost) under its
            // gate, then a flat (the curve at (-4,40,9)), then the pressed run's top slope)
            put(blocks, *x + 2, *lv - 2, &down);
            if intake_flat {
                put(blocks, *x + 3, *lv - 2, &flat);
                *x += 4;
            } else {
                *x += 3;
            }
            *lv -= 2;
            return (p, gate_cell, g_cell);
        }
        put(blocks, *x + 2, *lv - 2, &pdown_off);
        // (`--recv-run N`, the receiver's front pocket only: N flats between its G and the U into
        // the next pocket (tail-v5's G3, flat, curve, flats, U into P4))
        for f in 0..run_flats {
            put(blocks, *x + 3 + f, *lv - 2, &flat);
        }
        put(blocks, *x + 3 + run_flats, *lv - 2, &up);
        // (U rises east: its high side's rigid block is under the next P)
        extra.push((BlockPos::new(*x + 4 + run_flats, *lv - 2, 0), Block::parse("minecraft:glass").unwrap()));
        *x += 4 + run_flats;
        *lv -= 1;
        (p, gate_cell, g_cell)
    };
    let ntail = if o.ideal_tail { 1 } else { o.tail };
    for j in 0..ntail {
        let last = j + 1 == ntail;
        let rf = if j == 2 && o.receiver_p.or(o.receiver).is_some() { o.recv_run } else { 0 };
        let (p, gc, gg) = pocket(&mut blocks, &mut extra, &mut put, &mut x, &mut lv, last, rf);
        if rf >= 2 {
            // (the receiver's exit detector: the second flat past the front gate's slope, sharing no
            // face with a powered rail; crossed only by carts leaving the receiver)
            g.recv_exit = Some((gg.0 + 2, gg.1));
        }
        g.tail_p.push(p);
        g.gates.push((Role::Tail(j), gc.0, gc.1, if last { None } else { Some(gg) }));
    }
    // (the pressed run: its first slope is under the last tail gate)
    let run_x0 = x as f64;
    for k in 0..o.run {
        // (the run's top cell a detector rail (a sloped detector that sees a resting cart: the
        // intake's "top cell free" evidence); no gate stands over it (the flat comes between) and
        // it touches no powered rail)
        if k == 0 {
            put(&mut blocks, x, lv - 1, &det_down);
            g.run_sensor = (x, lv - 1);
        } else {
            put(&mut blocks, x, lv - 1, &down);
        }
        g.run_cells.push((x, lv - 1));
        x += 1;
        lv -= 1;
    }
    g.run_x = (run_x0, x as f64);
    // (the escapement: G0 over P (powered always, a redstone block under it); G_P over R)
    blocks.push((BlockPos::new(x, lv - 1, 0), pdown_on.clone()));
    blocks.push((BlockPos::new(x, lv - 2, 0), rblock.clone()));
    g.gates.push((Role::G0, x, lv, None));
    g.g0_plane = x as f64 + 0.5;
    x += 1;
    lv -= 1;
    put(&mut blocks, x, lv - 1, &down);
    g.gates.push((Role::Gp, x, lv, None));
    g.gp_plane = x as f64 + 0.5;
    x += 1;
    lv -= 1;
    // (the hold: 3 pockets; the front one's G a powered slope into the descent)
    for j in 0..3 {
        let (p, gc, gg) = pocket(&mut blocks, &mut extra, &mut put, &mut x, &mut lv, false, 0);
        g.hold_p.push(p);
        g.gates.push((Role::Hold(j), gc.0, gc.1, Some(gg)));
    }
    // (the descent and the sink: a flat first, at the front pocket's U top, then slopes down)
    put(&mut blocks, x, lv, &flat);
    x += 1;
    for _ in 0..3 {
        put(&mut blocks, x, lv - 1, &down);
        x += 1;
        lv -= 1;
    }
    for k in 0..8 {
        put(&mut blocks, x + k, lv, &flat);
    }
    g.sink_x = (x + 4) as f64;
    blocks.extend(extra);
    if let Some((ex, ey)) = g.recv_exit {
        blocks.push((BlockPos::new(ex, ey, 0), det.clone()));
    }
    for (_, gx, gy, _) in &g.gates {
        blocks.push((BlockPos::new(*gx, *gy, 0), gate.clone()));
    }
    let spec = FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(x + 12, lv, 3),
        lo: BlockPos::new(-4, lv - 8, -4),
        hi: BlockPos::new(x + 14, top + 4, 4),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok((spec, g))
}

fn cart_state(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64)> {
    let i = redsim::entity::index_of(w, id)?;
    let e = &w.entities.list[i];
    Some((e.pos.x, e.pos.y, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()))
}

/// One contact: tick, the mover and the struck cart, their x, both stored speeds the tick before,
/// the region, and for an intake (gt from the release, the run's fill then).
#[derive(Clone, Debug)]
struct Contact {
    t: i64,
    k: usize,
    j: usize,
    x: f64,
    xj: f64,
    pv: f64,
    pj: f64,
    region: &'static str,
    intake: Option<(i64, usize)>,
}

/// Runs the bench; returns the text.
pub fn gq_bench(o: &GqOpts) -> Result<String, String> {
    if o.tail < 2 && !o.ideal_tail {
        return Err("the tail chain needs 2 pockets or more (the spacer keys on its second sensor)".into());
    }
    let (spec, g) = build(o)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    // (an open gate is set unpowered: a powered=true gate with no signal shuts at its next
    // neighbour update)
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let rsrc = Block::parse("minecraft:redstone_block")?;
    let glass_b = Block::parse("minecraft:glass")?;
    let ridden_k = |k: usize| o.riders.get(k % o.riders.len().max(1)).copied().unwrap_or(true);
    let spawn = |run: &mut Run, k: usize, pos: [f64; 3], v: f64| -> Result<u32, String> {
        let rider = if ridden_k(k) { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        let nbt = format!("{{Motion:[{v:.4}d,0.0d,0.0d]{rider}}}");
        redsim::entity::spawn(&mut run.world, "minecraft:minecart", pos, Some(&nbt))
    };
    // (the start: the run filled at rest from G0 up, one cart a cell; the rest launched into the
    // tail chain 30 gt apart)
    let mut ids: Vec<Option<u32>> = vec![None; o.carts];
    let mut pending: Vec<(i64, usize)> = Vec::new();
    let in_run = o.cap.min(o.carts).min(g.run_cells.len());
    if o.back_at > 0.0 {
        // (the run filled pressed (0.790 apart in x, measured) from G0's rest up to the back's target,
        // `back_at` past the last tail gate's cell: the count the intake keeps constant)
        let gx = g.gates.iter().find(|x| x.0 == Role::Tail(g.tail_p.len() - 1)).map_or(0.0, |x| x.1 as f64);
        let x_front = g.g0_plane - 0.125 - 0.49;
        let mut k = 0usize;
        let mut xk = x_front;
        while xk >= gx + o.back_at - 0.01 && k < o.carts {
            let cell = g.run_cells.iter().find(|c| xk >= c.0 as f64 && xk < (c.0 + 1) as f64).copied();
            let y = cell.map_or(0.0, |c| c.1 as f64 + (1.0 - (xk - c.0 as f64)) + 0.0625);
            ids[k] = Some(spawn(&mut run, k, [xk, y, 0.5], 0.0)?);
            k += 1;
            xk -= 0.79;
        }
        for (i, kk) in (k..o.carts).enumerate() {
            pending.push((i as i64 * 30, kk));
        }
    } else {
    for k in 0..in_run {
        let (cx, cy) = g.run_cells[g.run_cells.len() - 1 - k];
        ids[k] = Some(spawn(&mut run, k, [cx as f64 + 0.5, cy as f64 + 0.5 + 0.0625, 0.5], 0.0)?);
    }
    for (i, k) in (in_run..o.carts).enumerate() {
        pending.push((i as i64 * 30, k));
    }
    }
    let n_gates = g.gates.len();
    let mut target_hist: Vec<std::collections::VecDeque<bool>> = vec![std::collections::VecDeque::new(); n_gates];
    let mut gate_open: Vec<bool> = vec![false; n_gates];
    let sensors: Vec<(i32, i32)> = g.tail_p.iter().chain(g.hold_p.iter()).copied().collect();
    let nt = g.tail_p.len();
    let mut was_on = vec![false; sensors.len()];
    let mut occ_t = vec![false; nt];
    let mut occ_h = [false; 3];
    let mut last_rise_t1: i64 = i64::MIN / 2;
    let mut settle_live: i64 = 0;
    let mut g0_was = false;
    let mut g0_prev_w = false;
    let mut owed: i64 = 0;
    let mut owed_at: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
    let mut pool: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut next_spawn_ok: i64 = 0;
    let mut adm_live: Option<i64> = None;
    let mut last_open = false;
    let mut count_q: i64 = in_run as i64;
    let mut tail_release_until: i64 = -1;
    let mut hold_release_until: i64 = -1;
    let mut prev: Vec<Option<(f64, f64, f64)>> = vec![None; o.carts];
    let mut frozen: Option<Vec<(u32, [f64; 3], [f64; 3])>> = None;
    // (measures)
    let mut edges_on_cart: Vec<String> = Vec::new();
    let mut g0_open_count: Option<usize> = None;
    let mut gp_open_count: Option<usize> = None;
    let mut g0_hist: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut gp_hist: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut arrivals: Vec<(f64, i64, usize)> = Vec::new();
    // (intake arrivals: (stored speed at the first contact, tick, the run's fill at the release, cart,
    // gt from the release); intakes that came to rest alone; the hold's release and margins)
    let mut intake_pending: Vec<Option<(i64, usize)>> = vec![None; o.carts];
    let mut intakes: Vec<(f64, i64, usize, usize, i64)> = Vec::new();
    let mut intake_alone: Vec<(i64, usize, usize)> = Vec::new();
    let last_tail_face = g.gates.iter().find(|x| x.0 == Role::Tail(g.tail_p.len() - 1)).map_or(0.0, |x| x.1 as f64 + 0.375);
    let h2_face = g.gates.iter().find(|x| x.0 == Role::Hold(2)).map_or(0.0, |x| x.1 as f64 + 0.375);
    let mut hold_release_at: i64 = -1;
    let mut released: Vec<usize> = Vec::new();
    let mut rear_clear: Option<i64> = None;
    let mut next_at_front: Option<i64> = None;
    let mut margin_pending = false;
    let mut margins: Vec<(i64, Option<i64>, Option<i64>)> = Vec::new();
    let mut hold_shut_at: Option<i64> = None;
    let mut contacts_arr: Vec<Contact> = Vec::new();
    let mut contacts_mov: Vec<Contact> = Vec::new();
    let mut exit_cross: Vec<(i64, usize)> = Vec::new();
    let mut hold_rest: Vec<(i64, usize)> = Vec::new();
    let mut spacing_samples: Vec<f64> = Vec::new();
    let mut run_min: (usize, i64) = (usize::MAX, 0);
    let mut hold_at_release: Vec<(i64, usize)> = Vec::new();
    let mut tail_max = 0usize;
    let mut sunk = 0usize;
    let mut launched_into_run: std::collections::BTreeSet<usize> = Default::default();
    let warm = 200i64;
    let ticks = warm + o.cycles * 100;
    // (cc767b78, `alive`: day-1's gate on a virtual sealed ring in the hold's chunk (or
    // `MCTAS_ALIVE_RING`); the cycle's pulses as offsets from its golem event: the hold at +3,
    // G_P at A+6 and G0 at A+13 for A = 32, 56, 80; each opening completing once risen)
    let col = g.hold_p.first().map_or((0.5, 0.5), |p| (p.0 as f64 + 0.5, 0.5));
    let mut gate = crate::liveness::Liveness::from_config(crate::liveness::Ring::Virtual { x: col.0, z: col.1, period: 30, high: 20, phase: 0 });
    const GP_AT: [i64; 3] = [38, 62, 86];
    const G0_AT: [i64; 3] = [45, 69, 93];
    let (mut gp_until, mut g0_until) = (i64::MIN / 2, i64::MIN / 2);
    let mut golems: Vec<i64> = Vec::new();
    let mut frozen32: Option<[i32; 4]> = None;
    let mut tail_refusing = false;
    let mut tail_refusals = 0usize;
    // (7b90c3f1: the batch receiver, Tail(0..2) in timed mode: the batch's live count since the
    // hold's release (None: idle, the middle and rear open for the next batch), the timers' shuts,
    // the front's feed window, the advances (live gt waiting for the refill edge), the pockets held
    // by construction (rear, middle, front), the measures)
    let mut batch_sunk = 0usize;
    let mut recv_live: Option<i64> = None;
    let mut recv_queue: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
    let mut recv_overlaps = 0usize;
    let mut recv_empty_late: Vec<i64> = Vec::new();
    let mut recycle: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let (mut recv_mid_shut, mut recv_rear_shut) = (false, false);
    let mut recv_feed_until: i64 = -1;
    let mut recv_feeding = false;
    let (mut recv_adv_mid, mut recv_adv_rear): (Option<i64>, Option<i64>) = (None, None);
    let mut recv_hold = [false; 3];
    let mut recv_settled = false;
    let mut last_rise_t3: i64 = i64::MIN / 2;
    let mut recv_entry: [Option<i64>; 3] = [None; 3];
    let mut recv_edges: Vec<String> = Vec::new();
    let mut recv_gap_min: [Option<i64>; 2] = [None; 2];
    // (each timed shut's tick (middle, rear) until the first cart entering its cell after it: the
    // follower margin)
    let mut recv_shut_at: [Option<i64>; 2] = [None; 2];
    let mut recv_foll_min: [Option<i64>; 2] = [None; 2];
    let mut recv_feeds: Vec<i64> = Vec::new();
    let mut recv_timeouts = 0usize;
    let mut recv_max = (0usize, 0i64);
    let recv_on = (o.receiver.is_some() || o.receiver_p.is_some()) && !o.ideal_tail && o.tail >= 4;
    let mut recv_p_edges = 0usize;
    let mut recv_exit_was = false;
    let mut recv_exit_edges: Vec<i64> = Vec::new();
    let mut recv_fly_wait: Option<i64> = None;
    let mut recv_last_feed_start: i64 = i64::MIN / 2;
    // (7b90c3f1, `--ring-q Q`: day-1's fast ring emulated as edge counts: an edge every Q live gt
    // (pausing with the carts: no edge while the column is frozen); the receiver's spacer and its
    // ripple delay count edges)
    let mut ring_acc = 0.0f64;
    let mut ring_edges: i64 = 0;
    let mut ring_edges_prev: i64 = 0;
    let mut recv_last_feed_start_edges: i64 = i64::MIN / 2;
    let mut col_frozen_total: i64 = 0;
    for t in 0..ticks {
        // (7b90c3f1, --recv-ideal: the receiver alone: every cart starts in the recycle pool (the
        // run and the tail empty), three come back after each hold release)
        if t == 0 && o.recv_ideal {
            pending.clear();
            for k in 0..o.carts {
                if let Some(id) = ids[k].take() {
                    let riders: Vec<u32> = redsim::entity::index_of(&run.world, id).map(|i| run.world.entities.list[i].passengers.clone()).unwrap_or_default();
                    for r in riders {
                        let _ = redsim::entity::kill(&mut run.world, r);
                    }
                    let _ = redsim::entity::kill(&mut run.world, id);
                }
                recycle.push_back(k);
            }
            count_q = 0;
        }
        let frz = o.freeze.iter().any(|(a, d)| t >= *a && t < a + d);
        // (cc767b78: the real freeze: the whole bench at level 32 from T for D gt)
        for (a, d) in &o.freeze32 {
            if t == *a && frozen32.is_none() {
                frozen32 = Some(crate::liveness::freeze32(&mut run.world));
                gate.freeze_started(t);
            }
            if t == a + d
                && let Some(pb) = frozen32.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        if o.freeze.iter().any(|(a, _)| t == *a) {
            gate.freeze_started(t);
        }
        let real_frozen = !run.world.entity_ticks_at(col.0, col.1);
        // (ring mode: the whole column's clock pauses with the carts, the hold's release included)
        if o.ring_q > 0.0 && (frz || real_frozen) {
            col_frozen_total += 1;
        }
        // (cc767b78: the climb back is part of the frozen column: a freeze defers every pending
        // launch by its length (no cart spawned into, or rolling freely through, a frozen bench))
        if frz || real_frozen {
            for p in pending.iter_mut() {
                if p.0 >= t {
                    p.0 += 1;
                }
            }
        }
        // (launches)
        let due: Vec<usize> = pending.iter().filter(|p| p.0 == t).map(|p| p.1).collect();
        pending.retain(|p| p.0 != t);
        for k in due {
            if o.ideal_tail {
                pool.push_back(k);
            } else {
                ids[k] = Some(spawn(&mut run, k, [g.launch.0, g.launch.1, 0.5], o.ret_speed)?);
            }
        }
        // (the ideal tail: the next pooled cart placed at rest at the last tail gate's face whenever
        // the last pocket is empty: the intake junction measured alone, with no tail chain upstream)
        if o.ideal_tail && !pool.is_empty() && t >= next_spawn_ok && !gate_open[g.gates.iter().position(|x| x.0 == Role::Tail(0)).unwrap_or(0)] && t >= tail_release_until + 2 {
            let (px5, ph5) = g.tail_p[0];
            let face = g.gates.iter().find(|x| x.0 == Role::Tail(0)).map_or(0.0, |x| x.1 as f64 + 0.375);
            let empty = ids.iter().all(|id| id.and_then(|id| cart_state(&run.world, id)).is_none_or(|(x, _, _)| x < px5 as f64 || x > face + 0.6));
            if empty {
                let k = pool.pop_front().unwrap();
                let xr = face - 0.49;
                let base = (ph5 - 1) as f64;
                let yr = base + (1.0 - (xr - (px5 + 1) as f64)) + 0.0625;
                ids[k] = Some(spawn(&mut run, k, [xr, yr, 0.5], 0.0)?);
                occ_t[0] = true;
                settle_live = o.settle;
                next_spawn_ok = t + 4;
            }
        }
        gate.column_frozen = Some(frz || real_frozen);
        let alive = gate.alive(&run.world, t);
        // (7b90c3f1, cc767b78 19:49: the receiver's feed window and the tail chain's release pulse
        // count the live clock, not world ticks: while the column is frozen (or ALIVE is low with
        // --alive-gate) their ends move out a tick at a time, so neither can shut onto a frozen cart
        // or run out during a freeze)
        {
            let live_now = if o.alive { alive } else { !(frz || real_frozen) };
            if !live_now {
                if recv_feeding && t < recv_feed_until {
                    recv_feed_until += 1;
                }
                if t < tail_release_until {
                    tail_release_until += 1;
                }
            }
        }
        // (gates as targeted `wire` gt ago; a pocket's G slope powered with its gate)
        for gi in 0..n_gates {
            let want = if target_hist[gi].len() > o.wire as usize { target_hist[gi].pop_front().unwrap() } else { gate_open[gi] };
            if want != gate_open[gi] {
                gate_open[gi] = want;
                let (role, gx, gy, gcell) = g.gates[gi];
                run.world.set_block(BlockPos::new(gx, gy, 0), if want { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
                if let Some((cx, cy)) = gcell {
                    // (the gate's signal powers the G slope beneath it, the stand's repeater and
                    // wire: a redstone block under the slope while the gate is open, glass while shut;
                    // a declared source)
                    run.world.set_block(BlockPos::new(cx, cy - 1, 0), if want { rsrc.clone() } else { glass_b.clone() }, redsim::world::flags::ALL);
                }
                // (a shut onto a cart: the gate's box (the cell's middle slice, 1.5 tall) appearing
                // inside a cart's box; a cart pressed on the gate's face is in the cell, not the box)
                for (k, id) in ids.iter().enumerate() {
                    if let Some((x, y, _)) = id.and_then(|id| cart_state(&run.world, id))
                        && !want
                        && x + 0.49 > gx as f64 + 0.375 + 0.02
                        && x - 0.49 < gx as f64 + 0.625
                        && y + 0.7 > gy as f64
                        && y < gy as f64 + 1.5
                    {
                        edges_on_cart.push(format!("t {t}: {role:?} shut onto cart {k} ({x:.2},{y:.2})"));
                    }
                }
                // (the hold's front gate shutting after a release: its margins close at the next
                // release or the run's end)
                if !want && role == Role::Hold(2) && margin_pending && hold_shut_at.is_none() {
                    hold_shut_at = Some(t);
                }
                // (carts per opening: close the count at each opening's start)
                if want && role == Role::G0 {
                    // (the pressed spacing: successive carts at rest on the run's slopes)
                    let mut xs: Vec<f64> = ids.iter().filter_map(|id| id.and_then(|id| cart_state(&run.world, id))).filter(|(x, _, v)| *x >= g.run_x.0 && *x < g.run_x.1 && *v < 0.005).map(|(x, _, _)| x).collect();
                    xs.sort_by(|a, b| a.total_cmp(b));
                    for w in xs.windows(2) {
                        if w[1] - w[0] < 1.6 {
                            spacing_samples.push(w[1] - w[0]);
                        }
                    }
                    if let Some(c) = g0_open_count.take() {
                        *g0_hist.entry(c).or_insert(0) += 1;
                    }
                    g0_open_count = Some(0);
                }
                if want && role == Role::Gp {
                    if let Some(c) = gp_open_count.take() {
                        *gp_hist.entry(c).or_insert(0) += 1;
                    }
                    gp_open_count = Some(0);
                }
            }
        }
        if frz && frozen.is_none() {
            frozen = Some(run.world.entities.list.iter().map(|e| (e.id, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z])).collect());
        }
        if !frz {
            frozen = None;
        }
        run.tick();
        if let Some(snap) = &frozen {
            for (id, p, v) in snap {
                if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                    let e = &mut run.world.entities.list[i];
                    e.pos = redsim::entity::Vec3::new(p[0], p[1], p[2]);
                    e.delta_movement = redsim::entity::Vec3::new(v[0], v[1], v[2]);
                }
            }
        }
        // (the sink: gone, back at the tail chain's top after the climb)
        for k in 0..o.carts {
            if let Some(id) = ids[k]
                && let Some((x, _, _)) = cart_state(&run.world, id)
                && x > g.sink_x
            {
                let riders: Vec<u32> = redsim::entity::index_of(&run.world, id).map(|i| run.world.entities.list[i].passengers.clone()).unwrap_or_default();
                for r in riders {
                    let _ = redsim::entity::kill(&mut run.world, r);
                }
                let _ = redsim::entity::kill(&mut run.world, id);
                ids[k] = None;
                sunk += 1;
                let last = pending.iter().map(|p| p.0).max().unwrap_or(i64::MIN / 2);
                if !o.ret_offs.is_empty() && hold_release_at >= 0 {
                    // (7b90c3f1: the j-th cart sunk after the hold's release comes back at the release
                    // + ret + OFFS[j]: the climb table's spacing per pair)
                    let off = o.ret_offs[batch_sunk.min(o.ret_offs.len() - 1)];
                    pending.push(((hold_release_at + o.ret + off).max(t + 1), k));
                    batch_sunk += 1;
                } else {
                    pending.push(((t + o.ret).max(last + o.ret_gap), k));
                }
                launched_into_run.remove(&k);
            }
        }
        let now: Vec<Option<(f64, f64, f64)>> = ids.iter().map(|id| id.and_then(|id| cart_state(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_GQ_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 3 && t >= p[1] && t < p[2] {
                if let Some((x, y, v)) = now.get(p[0] as usize).copied().flatten() {
                    let gs: String = gate_open.iter().map(|o| if *o { 'o' } else { '.' }).collect();
                    eprintln!("t {t}: cart {} at {x:.3},{y:.3} v {v:.3}; gates {gs}; occ_t {:?} occ_h {:?} count {count_q}", p[0], occ_t, occ_h);
                }
            }
        }
        // (the sensors and the latches)
        for (si, (sx, sy)) in sensors.iter().enumerate() {
            let on = matches!(run.world.get(BlockPos::new(*sx, *sy, 0)), Block::DetectorRail { powered: true, .. });
            if on && !was_on[si] {
                if si < nt {
                    occ_t[si] = true;
                    if si > 0 {
                        occ_t[si - 1] = false;
                    }
                    if si == 1 {
                        last_rise_t1 = t;
                    }
                    if si == nt - 1 {
                        settle_live = 0;
                    }
                    // (7b90c3f1: the receiver's evidence: Tail(3)'s edge ends a feed; Tail(2)'s ends
                    // the middle's advance; Tail(1)'s ends the rear's)
                    if recv_on {
                        if si == 3 {
                            last_rise_t3 = t;
                            // (in exit mode a cart reaches Tail(3) after the front has shut behind it:
                            // every Tail(3) edge is a feed there)
                            if recv_feeding || (o.recv_exit_d.is_some() && g.recv_exit.is_some()) {
                                // (`--recv-exit-d`: the window, the front pocket's flag and the
                                // advances run on the exit detector's edges instead; here only the
                                // feed is counted (and recycled with --recv-ideal))
                                let exit_mode = o.recv_exit_d.is_some() && g.recv_exit.is_some();
                                // (`--recv-fly`: the front stays open through the advances, each cart
                                // behind flying through from the middle's face; its window renewed at
                                // every feed edge, ended by its own timeout once the receiver is empty)
                                if exit_mode {
                                } else if o.recv_fly && (recv_hold[1] || recv_hold[0] || recv_adv_mid.is_some() || recv_adv_rear.is_some()) {
                                    recv_feed_until = t + o.recv_feed;
                                } else {
                                    recv_feeding = false;
                                    recv_feed_until = t;
                                }
                                if !exit_mode {
                                    recv_hold[2] = false;
                                }
                                recv_feeds.push(t);
                                // (the ideal downstream: the fed cart leaves at once and is recycled
                                // at the next hold release)
                                if o.recv_ideal {
                                    let px = sensors[3].0 as f64;
                                    if let Some(k) = (0..o.carts).filter(|&k| now[k].is_some_and(|(x, _, _)| x > px - 1.0 && x < px + 1.5)).max_by(|a, b| now[*a].unwrap().0.total_cmp(&now[*b].unwrap().0))
                                        && let Some(id) = ids[k]
                                    {
                                        let riders: Vec<u32> = redsim::entity::index_of(&run.world, id).map(|i| run.world.entities.list[i].passengers.clone()).unwrap_or_default();
                                        for r in riders {
                                            let _ = redsim::entity::kill(&mut run.world, r);
                                        }
                                        let _ = redsim::entity::kill(&mut run.world, id);
                                        ids[k] = None;
                                        recycle.push_back(k);
                                        occ_t[3] = false;
                                    }
                                }
                                // (ironspec 18:32, (2): the middle and the rear advance together, each
                                // gate shut on its own cart's distinct P edge: the middle's cart into
                                // the front pocket (Tail(2)'s P), the rear's into the middle (Tail(1)'s))
                                if recv_hold[1] && !exit_mode {
                                    recv_adv_mid = Some(0);
                                    recv_hold[1] = false;
                                }
                                if recv_hold[0] && !exit_mode {
                                    recv_adv_rear = Some(0);
                                    recv_hold[0] = false;
                                }
                            }
                        }
                        // (an advance's cart has left its pocket at the advance's start (its flag
                        // cleared then); its arrival edge fills the pocket ahead)
                        // (`--receiver-p`: the batch's shuts keyed on the front pocket's P (Tail(2)'s),
                        // crossed only by the batch's first cart while the receiver is idle (or, with
                        // the early reopen, once every cart of the batch before is past it): its
                        // rising edge starts the middle's and rear's short delays)
                        let past_p = recv_settled && !recv_hold[0] && !recv_hold[1] && recv_adv_mid.is_none() && recv_adv_rear.is_none() && recv_fly_wait.is_none();
                        if si == 2 && std::env::var("MCTAS_GQ_RECV_DEBUG").is_ok() {
                            eprintln!("t {t}: Tail(2) P rise; live {recv_live:?} settled {recv_settled} hold {recv_hold:?} feeding {recv_feeding} adv {recv_adv_mid:?}/{recv_adv_rear:?} wait {recv_fly_wait:?}");
                        }
                        if si == 2 && o.receiver_p.is_some() && (recv_live.is_none() && !recv_feeding || past_p && !recv_hold[2]) && recv_adv_mid.is_none() {
                            recv_live = Some(0);
                            recv_mid_shut = false;
                            recv_rear_shut = false;
                            recv_settled = false;
                            recv_p_edges += 1;
                        }
                        if si == 2 && recv_adv_mid.is_some() {
                            recv_adv_mid = None;
                            recv_hold[2] = true;
                        }
                        if si == 1 && recv_adv_rear.is_some() {
                            recv_adv_rear = None;
                            recv_hold[1] = true;
                            // (the flying feed: a cart reaching the middle while the front is still
                            // open flies on through, the middle opening ahead of it)
                            if o.recv_fly && o.recv_exit_d.is_none() && recv_feeding && recv_adv_mid.is_none() && !recv_hold[2] {
                                recv_hold[1] = false;
                                recv_adv_mid = Some(0);
                            }
                            // (exit mode: a cart reaching the middle after the ripple's wait already
                            // ran out (the front empty and still open) starts its own wait)
                            if o.recv_exit_d.is_some() && recv_feeding && recv_adv_mid.is_none() && !recv_hold[2] && recv_fly_wait.is_none() {
                                recv_fly_wait = Some(0);
                            }
                        }
                    }
                } else {
                    let h = si - nt;
                    occ_h[h] = true;
                    if h > 0 {
                        occ_h[h - 1] = false;
                    }
                    if h == 0 {
                        count_q -= 1;
                    }
                }
            }
            was_on[si] = on;
        }
        // (7b90c3f1, `--recv-exit-d D`: the receiver's exit detector (the second flat past its front
        // gate, crossed only by leaving carts): each rising edge empties the front pocket, keeps the
        // front open while carts remain and starts the next advance D live gt later (the flying
        // ripple keyed on the leaving cart's own evidence); the front shuts on the last one's edge)
        if recv_on
            && let (Some(_), Some((ex, ey))) = (o.recv_exit_d, g.recv_exit)
        {
            let on = matches!(run.world.get(BlockPos::new(ex, ey, 0)), Block::DetectorRail { powered: true, .. });
            if on && !recv_exit_was && recv_feeding {
                recv_hold[2] = false;
                recv_exit_edges.push(t);
                if !o.recv_fly {
                    // (the stop feed keyed on the exit: the front shuts behind the leaving cart (it is
                    // two cells past the gate) and the carts behind advance D live gt later; each
                    // next cart then starts from rest at the front face like the first, so the
                    // spacing holds along the run)
                    recv_feeding = false;
                    recv_feed_until = t;
                    if recv_hold[1] || recv_hold[0] {
                        recv_fly_wait = Some(0);
                    }
                } else if recv_hold[1] || recv_hold[0] || recv_adv_mid.is_some() || recv_adv_rear.is_some() {
                    recv_feed_until = t + o.recv_feed;
                    recv_fly_wait = Some(0);
                } else {
                    recv_feeding = false;
                    recv_feed_until = t;
                }
            }
            recv_exit_was = on;
        }
        // (the cycle: T = warm + 100 c; the hold at T+3; the escapement at A+6 / A+13)
        let ph = (t - warm - o.corner - col_frozen_total).rem_euclid(100);
        let in_cycles = t >= warm;
        // (`alive`: the golem event at the summon boundary, only with the column live: frozen
        // villagers summon nothing; it restarts the cycle)
        if o.alive && in_cycles && ph == 0 && !(frz || real_frozen) {
            let mut offs = vec![3i64];
            offs.extend(GP_AT);
            offs.extend(G0_AT);
            gate.start_with("cycle", t, &offs);
            golems.push(t);
        }
        let hold_rise = if o.alive { gate.rise(&run.world, "cycle", 3, t) } else { in_cycles && ph == 3 && !frz && !(o.ring_q > 0.0 && real_frozen) };
        if hold_rise {
            // (the last release's margins: the shut after the rear cleared, the next cart after
            // the shut)
            if let Some(s) = hold_shut_at.take() {
                margins.push((s, rear_clear.map(|c| s - c), next_at_front.map(|n| n - s)));
            }
            next_at_front = None;
            let held = occ_h.iter().filter(|b| **b).count();
            hold_at_release.push((t, held));
            hold_release_until = t + o.hold_pulse;
            hold_release_at = t;
            occ_h = [false; 3];
            // (the carts released: those in the hold now; the rear-clear tracking restarts)
            let hx0 = g.hold_p[0].0 as f64;
            released = now.iter().enumerate().filter(|(_, c)| c.is_some_and(|(x, _, _)| x >= hx0 && x < h2_face + 0.2)).map(|(k, _)| k).collect();
            rear_clear = None;
            margin_pending = true;
            batch_sunk = 0;
            let mut batch_n = released.len();
            if o.recv_ideal {
                batch_n = 0;
                for j in 0..3 {
                    if let Some(k) = recycle.pop_front() {
                        let off = o.ret_offs.get(j).or(o.ret_offs.last()).copied().unwrap_or(j as i64 * o.ret_gap);
                        pending.push((t + o.ret + off, k));
                        batch_n += 1;
                    }
                }
            }
            // (7b90c3f1: the receiver's committed timers start with the release whose carts it
            // will catch (only a release that sends a batch): a delay line, so a release while the
            // previous batch is still held queues its own count (its timers run on; it becomes
            // current once the receiver is empty))
            if recv_on && batch_n > 0 && o.receiver_p.is_none() {
                if recv_live.is_none() {
                    recv_live = Some(0);
                    recv_mid_shut = false;
                    recv_rear_shut = false;
                    recv_settled = false;
                } else {
                    recv_queue.push_back(0);
                    recv_overlaps += 1;
                }
            }
        }
        // (7b90c3f1: the receiver: its live count (paused while ALIVE is low or the column frozen),
        // the middle's and rear's timed shuts (the entry-to-edge gap: the last cart entering the
        // gate's cell), the batch settled, the feed one cart at a time into Tail(3), the advances'
        // timeouts, idle once empty)
        if recv_on {
            let (t_mid, t_rear) = o.receiver_p.or(o.receiver).unwrap_or((0, 0));
            let live_ok = if o.alive { alive } else { !(frz || real_frozen) };
            // (the emulated fast ring: its edges advance only while the column's carts move)
            ring_edges_prev = ring_edges;
            if o.ring_q > 0.0 && !(frz || real_frozen) {
                ring_acc += 1.0;
                ring_edges = (ring_acc / o.ring_q).floor() as i64;
            }
            if let Some(l) = recv_live.as_mut()
                && live_ok
            {
                *l += 1;
            }
            if live_ok {
                recv_queue.iter_mut().for_each(|q| *q += 1);
            }
            let l = recv_live.unwrap_or(-1);
            for (which, tt, flag) in [(1usize, t_mid, &mut recv_mid_shut), (0usize, t_rear, &mut recv_rear_shut)] {
                if l >= tt && !*flag && recv_live.is_some() {
                    *flag = true;
                    let gap = recv_entry[which].map(|e| t - e);
                    let slot = if which == 1 { 0 } else { 1 };
                    if let Some(gp) = gap
                        && recv_gap_min[slot].is_none_or(|m| gp < m)
                    {
                        recv_gap_min[slot] = Some(gp);
                    }
                    recv_shut_at[slot] = Some(t);
                    if recv_edges.len() < 24 {
                        recv_edges.push(format!("t {t} ({} live gt): {} shut, {}", l, if which == 1 { "middle" } else { "rear" }, gap.map_or("no cart entered its cell yet".into(), |g| format!("{g} gt after the last cart entered its cell"))));
                    }
                }
            }
            if recv_live.is_some() && recv_rear_shut && !recv_settled && l >= t_rear.max(t_mid) + o.recv_settle {
                recv_settled = true;
                recv_hold = [true, true, true];
            }
            // (the feed: the front's cart into Tail(3) when it is latched free and the spacer since
            // its last P edge has run, with no advance in progress)
            let exit_mode_now = o.recv_exit_d.is_some() && g.recv_exit.is_some();
            let spacer_ok = if exit_mode_now && o.ring_q > 0.0 {
                ring_edges - recv_last_feed_start_edges >= o.recv_spacer_edges
            } else if exit_mode_now {
                t - recv_last_feed_start >= o.spacer
            } else {
                t - last_rise_t3 >= o.spacer
            };
            if recv_hold[2] && !recv_feeding && recv_adv_mid.is_none() && recv_adv_rear.is_none() && (exit_mode_now || !occ_t[3]) && spacer_ok && t >= recv_feed_until && live_ok {
                recv_feeding = true;
                recv_feed_until = t + o.recv_feed;
                recv_last_feed_start = t;
                recv_last_feed_start_edges = ring_edges;
            }
            if recv_feeding && t >= recv_feed_until {
                // (no edge at Tail(3) within the feed window: the front was empty)
                recv_feeding = false;
                recv_hold[2] = false;
                recv_timeouts += 1;
                if recv_hold[1] {
                    recv_adv_mid = Some(0);
                    recv_hold[1] = false;
                }
                if recv_hold[0] {
                    recv_adv_rear = Some(0);
                    recv_hold[0] = false;
                }
            }
            // (`--recv-exit-d D`: the next advance D live gt after the leaving cart's exit edge)
            if let Some(d) = o.recv_exit_d
                && let Some(w) = recv_fly_wait.as_mut()
            {
                // (ring mode: the wait counts ring edges (each edge a step); otherwise live gt)
                if o.ring_q > 0.0 {
                    *w += ring_edges - ring_edges_prev;
                } else if live_ok {
                    *w += 1;
                }
                if *w >= d {
                    recv_fly_wait = None;
                    if recv_hold[1] {
                        recv_adv_mid = Some(0);
                        recv_hold[1] = false;
                    }
                    if recv_hold[0] {
                        recv_adv_rear = Some(0);
                        recv_hold[0] = false;
                    }
                }
            }
            for adv in [&mut recv_adv_mid, &mut recv_adv_rear] {
                if let Some(a) = adv.as_mut() {
                    if live_ok {
                        *a += 1;
                    }
                    if *a >= o.recv_timeout {
                        // (no arrival edge: that pocket was empty (its flag already cleared at
                        // the advance's start))
                        *adv = None;
                        recv_timeouts += 1;
                    }
                }
            }
            if recv_settled && !recv_hold.iter().any(|b| *b) && !recv_feeding && recv_adv_mid.is_none() && recv_adv_rear.is_none() {
                // (empty: the next queued batch becomes current with its count so far (a count
                // already past a timer fires it at once: a late receiver, the overlap's cost))
                recv_live = recv_queue.pop_front();
                recv_settled = false;
                recv_mid_shut = false;
                recv_rear_shut = false;
                if recv_live.is_some() {
                    recv_empty_late.push(t);
                }
            }
            // (the receiver idle (no batch's timers running): a cart at rest at the front gate's face
            // is fed as it comes (the warm-up's spaced launches; bench scaffolding, ideal sensing))
            if recv_live.is_none() && !recv_hold[2] && !recv_feeding {
                if let Some(face) = g.gates.iter().find(|x| x.0 == Role::Tail(2)).map(|x| x.1 as f64 + 0.375) {
                    if now.iter().any(|c| c.is_some_and(|(x, _, v)| (x + 0.49 - face).abs() < 0.1 && v < 0.005)) {
                        recv_hold[2] = true;
                    }
                }
            }
            // (the receiver's carts and each gate cell's last entry)
            let x0 = g.tail_p[0].0 as f64;
            let x1 = g.gates.iter().find(|x| x.0 == Role::Tail(2)).map_or(0.0, |x| x.1 as f64 + 1.0);
            let k = now.iter().filter(|c| c.is_some_and(|(x, _, _)| x >= x0 && x < x1)).count();
            if k > recv_max.0 {
                recv_max = (k, t);
            }
            for which in 0..3 {
                if let Some(gx) = g.gates.iter().find(|x| x.0 == Role::Tail(which)).map(|x| x.1 as f64) {
                    for kk in 0..o.carts {
                        if let (Some((x, _, _)), Some((px, _, _))) = (now[kk], prev[kk])
                            && px + 0.49 <= gx
                            && x + 0.49 > gx
                        {
                            recv_entry[which] = Some(t);
                            if which < 2 {
                                let slot = if which == 1 { 0 } else { 1 };
                                if let Some(s) = recv_shut_at[slot].take() {
                                    let m = t - s;
                                    if recv_foll_min[slot].is_none_or(|x| m < x) {
                                        recv_foll_min[slot] = Some(m);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // (the tail chain's release into the run: its front pocket latched, the run's top free by
        // the run's sensor (its top cell's detector rail off), the last release done)
        let run_busy = matches!(run.world.get(BlockPos::new(g.run_sensor.0, g.run_sensor.1, 0)), Block::DetectorRail { powered: true, .. });
        // (the last pocket's cart released only at rest: its pass edge plus a live-time settle count,
        // ironspec 17:19 (i))
        if !frz {
            settle_live += 1;
        }
        // (intake keyed on the escapement's own admission, ironspec 17:43 (B): each G0 opening edge
        // owes one intake; the last tail gate releases its waiting cart (latched, settled) once per
        // owed admission, so the run's count stays constant)
        let g0_w = in_cycles && !frz && G0_AT.iter().any(|a| (ph >= *a && ph < a + 12) || (t - warm >= 100 && ph + 100 >= *a && ph + 100 < a + 12));
        if g0_w && !g0_prev_w {
            owed += 1;
            owed_at.push_back(t);
        }
        g0_prev_w = g0_w;
        let tail_ev = if o.intake_admit {
            // (released once the admission's train shift has settled: `adm_settle` gt after its edge)
            occ_t[nt - 1] && owed > 0 && owed_at.front().is_some_and(|e| t - e >= o.adm_settle) && settle_live >= o.settle && t >= tail_release_until + 4
        } else {
            !o.overfill && occ_t[nt - 1] && !run_busy && settle_live >= o.settle && t >= tail_release_until + 4
        };
        if tail_ev && o.intake_admit && !frz {
            owed -= 1;
            owed_at.pop_front();
        }
        // (`alive`: the tail chain's release ANDed with ALIVE at its rise; a refusal counted once
        // per wait)
        let tail_go = if !tail_ev {
            tail_refusing = false;
            false
        } else if o.alive {
            if alive {
                gate.allow_rise(&run.world, "tail release", t)
            } else {
                if !tail_refusing {
                    tail_refusals += 1;
                }
                tail_refusing = true;
                false
            }
        } else {
            !frz
        };
        if tail_go {
            tail_refusing = false;
            tail_release_until = t + o.pulse;
            occ_t[nt - 1] = false;
            count_q += 1;
            // (the released cart: the one resting at the last tail gate's face; its first contact is
            // the intake arrival, with the run's fill at the release)
            let face = last_tail_face;
            let fill = now.iter().filter(|c| c.is_some_and(|(x, _, _)| x >= g.run_x.0 && x < g.run_x.1 + 0.5)).count();
            if let Some(k) = (0..o.carts).find(|&k| now[k].is_some_and(|(x, _, _)| (x + 0.49 - face).abs() < 0.3)) {
                intake_pending[k] = Some((t, fill));
            }
        }
        // (overfill, ironspec 17:19 (i): the last tail gate stays open over the run's back cart until
        // a G0 admission has moved the train and a live settle count has passed, then shuts; the
        // pocket above feeds the last pocket only while it is shut; the last gate opens again once
        // its new cart has passed P and settled, and that cart rolls about one cell into the back)
        if o.overfill {
            let g0_now = in_cycles && !frz && [32, 56, 80].iter().any(|a| (ph >= a + 13 && ph < a + 13 + 12) || (t - warm >= 100 && ph + 100 >= a + 13 && ph + 100 < a + 13 + 12));
            if g0_now && !g0_was {
                adm_live = Some(0);
            }
            g0_was = g0_now;
            if let Some(a) = adm_live.as_mut()
                && !frz
            {
                *a += 1;
            }
            if last_open {
                if adm_live.is_some_and(|a| a >= o.adm_settle) {
                    last_open = false;
                    adm_live = None;
                }
            } else if occ_t[nt - 1] && settle_live >= o.settle && !frz {
                last_open = true;
                occ_t[nt - 1] = false;
                let face = last_tail_face;
                let fill = now.iter().filter(|c| c.is_some_and(|(x, _, _)| x >= g.run_x.0 && x < g.run_x.1 + 0.5)).count();
                if let Some(k) = (0..o.carts).find(|&k| now[k].is_some_and(|(x, _, _)| (x + 0.49 - face).abs() < 0.3)) {
                    intake_pending[k] = Some((t, fill));
                }
            }
        }
        // (`alive`: G_P's and G0's openings as the cycle's pulses, each completing once risen)
        if o.alive {
            for a in GP_AT {
                if gate.rise(&run.world, "cycle", a, t) {
                    gp_until = t + 11;
                }
            }
            for a in G0_AT {
                if gate.rise(&run.world, "cycle", a, t) {
                    g0_until = t + 12;
                }
            }
        }
        for (gi, (role, _, _, _)) in g.gates.iter().enumerate() {
            let open = match role {
                Role::Tail(j) if o.overfill && *j + 1 == nt => last_open,
                Role::Tail(j) if o.overfill && *j + 2 == nt => !occ_t[j + 1] && !last_open && !(*j == 0 && o.spacer > 0 && t - last_rise_t1 < o.spacer),
                Role::Tail(j) if recv_on && *j == 2 => recv_feeding && t < recv_feed_until,
                Role::Tail(j) if recv_on && *j == 1 => recv_live.is_none() || !recv_mid_shut || recv_adv_mid.is_some() || (recv_settled && !recv_hold[0] && !recv_hold[1] && recv_adv_mid.is_none() && recv_adv_rear.is_none() && recv_fly_wait.is_none()),
                Role::Tail(j) if recv_on && *j == 0 => recv_live.is_none() || !recv_rear_shut || recv_adv_rear.is_some() || (recv_settled && !recv_hold[0] && recv_adv_rear.is_none()),
                Role::Tail(j) => {
                    if *j + 1 == nt {
                        t < tail_release_until
                    } else {
                        let mut sh = occ_t[j + 1];
                        if *j == 0 && o.spacer > 0 && t - last_rise_t1 < o.spacer {
                            sh = true;
                        }
                        !sh
                    }
                }
                Role::Gp => if o.alive { t < gp_until } else { in_cycles && !frz && [32, 56, 80].iter().any(|a| (ph >= a + 6 && ph < a + 6 + 11) || (t - warm >= 100 && ph + 100 >= a + 6 && ph + 100 < a + 6 + 11)) },
                Role::G0 => if o.alive { t < g0_until } else { in_cycles && !frz && [32, 56, 80].iter().any(|a| (ph >= a + 13 && ph < a + 13 + 12) || (t - warm >= 100 && ph + 100 >= a + 13 && ph + 100 < a + 13 + 12)) },
                Role::Hold(j) => {
                    // (together: every hold gate open for hold_pulse from T+3; a ripple of D: the
                    // front gate at T+3, each gate behind D gt later, each for hold_pulse)
                    let from = hold_release_at + if o.hold_ripple > 0 { (2 - *j as i64) * o.hold_ripple } else { 0 };
                    let until = hold_release_at + 2 * o.hold_ripple.max(0) + o.hold_pulse;
                    if hold_release_at >= 0 && t >= from && t < until {
                        true
                    } else if *j == 2 {
                        false
                    } else {
                        !occ_h[j + 1] && !(hold_release_at >= 0 && t >= hold_release_at && t < until)
                    }
                }
            };
            target_hist[gi].push_back(open);
        }
        // (plane crossings at G0 and G_P; arrivals at the run's back; the run's fill)
        let mut in_run_now = 0usize;
        for k in 0..o.carts {
            let (Some((x, _, v)), Some((px, _, pv))) = (now[k], prev[k]) else { continue };
            if px < g.g0_plane && x >= g.g0_plane {
                if let Some(c) = g0_open_count.as_mut() {
                    *c += 1;
                }
            }
            if px < g.gp_plane && x >= g.gp_plane {
                if let Some(c) = gp_open_count.as_mut() {
                    *c += 1;
                }
            }
            if x >= g.run_x.0 && x < g.run_x.1 + 0.5 {
                in_run_now += 1;
                if !launched_into_run.contains(&k) && t >= warm {
                    launched_into_run.insert(k);
                }
            }
            // (every contact, anywhere: the mover's stored speed dropping by 0.04 or more with a cart
            // within 1.3 ahead; an arrival (7b90c3f1's definition, ironspec 17:22) when the struck
            // cart's stored speed was under 0.15 the tick before and the mover faster by 0.05 or
            // more, else two carts closing while moving together)
            if pv - v >= 0.04
                && let Some(j) = (0..o.carts).filter(|&j| j != k && now[j].is_some_and(|(xj, _, _)| xj > x && xj - x < 1.3)).min_by(|a, b| now[*a].unwrap().0.total_cmp(&now[*b].unwrap().0))
            {
                let pj = prev[j].map_or(0.0, |p| p.2);
                let xj = now[j].map_or(f64::NAN, |c| c.0);
                let is_arrival = pj < 0.15 && pv - pj >= 0.05;
                let region = if x < g.run_x.0 - 1.0 {
                    "tail"
                } else if x < g.run_x.1 {
                    "run"
                } else if x < g.gp_plane {
                    "escapement"
                } else if x < h2_face + 0.5 {
                    "hold"
                } else {
                    "descent"
                };
                let intake = intake_pending[k].take();
                let rec = Contact { t, k, j, x, xj, pv, pj, region, intake: intake.map(|(tr, fill)| (t - tr, fill)) };
                if is_arrival {
                    contacts_arr.push(rec);
                } else {
                    contacts_mov.push(rec);
                }
                if let Some((tr, fill)) = intake {
                    intakes.push((pv, t, fill, k, t - tr));
                } else if region == "run" {
                    arrivals.push((pv, t, k));
                }
            }
            // (an intake that came to rest with no contact: the run was empty below it)
            if let Some((tr, fill)) = intake_pending[k]
                && v < 0.003
                && t - tr > 30
            {
                intake_alone.push((t, fill, k));
                intake_pending[k] = None;
            }
            // (the hold's exit: each cart crossing the front gate's plane; each cart coming to rest in
            // the hold)
            if px < h2_face + 0.125 && x >= h2_face + 0.125 {
                exit_cross.push((t, k));
            }
            if x >= g.hold_p[0].0 as f64 && x < h2_face && v < 0.005 && pv >= 0.005 {
                hold_rest.push((t, k));
            }
            // (the hold's rear-clear: every released cart past the front gate's box)
            if released.contains(&k) && x - 0.49 > h2_face + 0.25 {
                released.retain(|r| *r != k);
                if released.is_empty() {
                    rear_clear = Some(t);
                }
            }
            // (the next escapement cart reaching the front gate's face after the release)
            if margin_pending && hold_release_at >= 0 && !released.contains(&k) && x + 0.49 >= h2_face - 0.05 && x < h2_face && px + 0.49 < h2_face - 0.05 && t > hold_release_at {
                next_at_front.get_or_insert(t);
            }
        }
        if t >= warm + 100 && in_run_now < run_min.0 {
            run_min = (in_run_now, t);
        }
        let in_tail = now.iter().filter(|c| c.is_some_and(|(x, _, _)| x < g.run_x.0)).count();
        tail_max = tail_max.max(in_tail);
        prev = now;
    }
    let above: Vec<&(f64, i64, usize)> = arrivals.iter().filter(|a| a.0 > 0.2 && a.1 >= warm).collect();
    let worst = arrivals.iter().filter(|a| a.1 >= warm).map(|a| a.0).fold(0.0f64, f64::max);
    let h = |m: &std::collections::BTreeMap<usize, usize>| m.iter().map(|(n, c)| format!("{n} cart{}: {c}", if *n == 1 { "" } else { "s" })).collect::<Vec<_>>().join(", ");
    let mut l = vec![format!(
        "gravity-queue bench: {} carts ({}), a {}-pocket tail chain (the stand's pocket), a {}-slope pressed run fed while it holds fewer than {} carts by count, R1's escapement (G_P A+6/11, G0 A+13/12, A = 32/56/80), a 3-pocket hold released together at T+3, {} cycles after {warm} gt, corner {:+}, the climb back {} gt (+{} gt per cart){}",
        o.carts,
        (0..o.carts).map(|k| if ridden_k(k) { "R" } else { "E" }).collect::<String>(),
        o.tail,
        o.run,
        o.cap,
        o.cycles,
        o.corner,
        o.ret,
        o.ret_gap,
        if o.freeze.is_empty() { String::new() } else { format!(", freezes {:?}", o.freeze) }
    )];
    l.push(format!("carts per opening (exactly 1): G_P {}; G0 {}", h(&gp_hist), h(&g0_hist)));
    l.push(format!("gate shuts onto a cart (the gate's box inside a cart's; the bar: 0): {}{}", edges_on_cart.len(), if edges_on_cart.is_empty() { String::new() } else { format!(" ({})", edges_on_cart.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
    let _ = (&above, worst);
    l.push(format!("the pressed run's fill (carts on its slopes) at its emptiest after the first cycle: {} (t {}); the tail chain's most carts: {tail_max}", if run_min.0 == usize::MAX { 0 } else { run_min.0 }, run_min.1));
    l.push(format!(
        "the pressed spacing (x between successive carts at rest in the run, sampled at each G0 opening edge): {}",
        if spacing_samples.is_empty() { "-".to_string() } else {
            let mn = spacing_samples.iter().copied().fold(f64::INFINITY, f64::min);
            let mx = spacing_samples.iter().copied().fold(0.0f64, f64::max);
            let mean = spacing_samples.iter().sum::<f64>() / spacing_samples.len() as f64;
            format!("{} samples, {mn:.3}..{mx:.3}, mean {mean:.3}", spacing_samples.len())
        }
    ));
    l.push(format!("the hold at each release (latched carts): {}; carts through the sink: {sunk}", hold_at_release.iter().map(|(t, n)| format!("t {t}: {n}")).collect::<Vec<_>>().join(", ")));
    l.push(format!(
        "the hold's front-gate shut margins (gt from the rear cart clearing it to the shut, bar 2 or more; gt from the shut to the next cart at its face, bar 2 or more): {}",
        if margins.is_empty() { "-".to_string() } else { margins.iter().map(|(s, r, n)| format!("t {s}: {} / {}", r.map_or("rear not clear".into(), |v| v.to_string()), n.map_or("-".into(), |v| v.to_string()))).collect::<Vec<_>>().join(", ") }
    ));
    l.push(format!(
        "the hold's hand-off at its exit (gt between successive carts crossing the front gate's plane after each release; dfb89499's target 6..10, aim 8) and the last fill at rest before each release (gt before T+3): {}",
        hold_at_release
            .iter()
            .filter(|(tr, _)| *tr >= warm + 100)
            .map(|(tr, _)| {
                let mut c: Vec<i64> = exit_cross.iter().filter(|(t, _)| *t >= *tr && *t < tr + 80).map(|(t, _)| *t).collect();
                c.sort();
                let sp: Vec<String> = c.windows(2).map(|w| (w[1] - w[0]).to_string()).collect();
                let last = hold_rest.iter().filter(|(t, _)| *t < *tr && *t > tr - 100).map(|(t, _)| *t).max();
                format!("t {tr}: [{}] last fill {}", sp.join(","), last.map_or("-".into(), |l| format!("{} gt before", tr - l)))
            })
            .collect::<Vec<_>>()
            .join("; ")
    ));
    let iw: Vec<&(f64, i64, usize, usize, i64)> = intakes.iter().filter(|a| a.1 >= warm).collect();
    let iabove = iw.iter().filter(|a| a.0 > 0.2).count();
    let iworst = iw.iter().map(|a| a.0).fold(0.0f64, f64::max);
    l.push(format!(
        "intake arrivals (a cart released by the last tail gate, its first contact with the run's back; the bar 0.2): {} after the warm-up, the hardest {iworst:.3}, {iabove} above 0.2; by the run's fill at the release: {}; came to rest alone (the run empty below): {}",
        iw.len(),
        {
            let mut by: std::collections::BTreeMap<usize, (usize, f64)> = Default::default();
            for a in &iw {
                let e = by.entry(a.2).or_insert((0, 0.0));
                e.0 += 1;
                e.1 = e.1.max(a.0);
            }
            by.iter().map(|(f, (n, m))| format!("fill {f}: {n} (hardest {m:.3})")).collect::<Vec<_>>().join(", ")
        },
        intake_alone.iter().filter(|a| a.0 >= warm).count()
    ));
    l.push(format!(
        "intake travel (x from the rest at the last gate to the contact) and stored speed at contact: {}",
        {
            let rest_x = last_tail_face - 0.49;
            let v: Vec<String> = contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.intake.is_some() && c.t >= warm).take(10).map(|c| format!("{:.2}->{:.3}", c.x - rest_x, c.pv)).collect();
            if v.is_empty() { "-".to_string() } else { v.join(", ") }
        }
    ));
    l.push(format!(
        "the train's own close-ups inside the run (contacts that are not an intake arrival: after each admission at G0 the train moves down and closes up; tick-exact's push bridge): {} after the warm-up, the hardest {:.3}",
        arrivals.iter().filter(|a| a.1 >= warm).count(),
        arrivals.iter().filter(|a| a.1 >= warm).map(|a| a.0).fold(0.0f64, f64::max)
    ));
    // (arrivals by 7b90c3f1's definition, per region: the bar 0.2 on every one; the hardest with its
    // cells and both speeds)
    for region in ["tail", "run", "escapement", "hold", "descent"] {
        let a: Vec<&Contact> = contacts_arr.iter().filter(|c| c.region == region && c.t >= warm).collect();
        let m: Vec<&Contact> = contacts_mov.iter().filter(|c| c.region == region && c.t >= warm).collect();
        if a.is_empty() && m.is_empty() {
            continue;
        }
        let above: Vec<&&Contact> = a.iter().filter(|c| c.pv > 0.2).collect();
        let worst = a.iter().max_by(|x, y| x.pv.total_cmp(&y.pv));
        let mw = m.iter().map(|c| c.pv - c.pj).fold(0.0f64, f64::max);
        l.push(format!(
            "  {region}: arrivals (the struck cart under 0.15 the tick before, the mover 0.05 or more faster; bar 0.2) {}, {} above 0.2{}; closings while moving {} (the largest relative speed {mw:.3})",
            a.len(),
            above.len(),
            worst.map_or(String::new(), |c| format!(", the hardest {:.3}: t {} cart {} at x {:.2} into cart {} at x {:.2} (resting at {:.3}, relative {:.3}){}", c.pv, c.t, c.k, c.x, c.j, c.xj, c.pj, c.pv - c.pj, c.intake.map_or(String::new(), |(dt, f)| format!(", an intake {dt} gt after its release, fill {f}")))),
            m.len()
        ));
    }
    if !o.freeze32.is_empty() {
        l.push(format!("real freezes (the whole bench at level 32 inside a forced ring of chunks: redstone running, entities frozen): {:?}", o.freeze32));
    }
    if o.alive {
        // (one release per cycle: each golem event's release at its +3, or none; the releases'
        // phase against gameTime; cycles with no golem event (a frozen column))
        let per: Vec<String> = golems
            .iter()
            .map(|g0| {
                let n = hold_at_release.iter().filter(|(t, _)| *t >= *g0 && *t < g0 + 100).count();
                let at: Vec<String> = hold_at_release.iter().filter(|(t, _)| *t >= *g0 && *t < g0 + 100).map(|(t, h)| format!("+{} ({h} carts)", t - g0)).collect();
                format!("{g0}: {n}{}", if at.is_empty() { String::new() } else { format!(" {}", at.join(",")) })
            })
            .collect();
        let boundaries = (0..o.cycles).map(|c| warm + o.corner + 100 * c).filter(|b| *b >= warm).count();
        let off_phase = hold_at_release.iter().filter(|(t, _)| (t - warm - o.corner).rem_euclid(100) != 3).count();
        let two = golems.iter().filter(|g0| hold_at_release.iter().filter(|(t, _)| *t >= **g0 && *t < **g0 + 100).count() > 1).count();
        l.push(format!(
            "live-time cycle clock (day-1's: the cycle restarts at each golem event, the column live at a summon boundary): {} golem events of {boundaries} boundaries; releases per cycle {}; releases off T+3 (out of phase) {off_phase}; cycles with two releases {two}; the tail chain's release refused (ALIVE low) {tail_refusals} times",
            golems.len(),
            if per.is_empty() { "-".to_string() } else { per.join(", ") }
        ));
        let (gl, _gp) = gate.lines();
        l.extend(gl);
    }
    if recv_on {
        let (t_mid, t_rear) = o.receiver_p.or(o.receiver).unwrap_or((0, 0));
        let feed_gaps: Vec<i64> = recv_feeds.windows(2).map(|w| w[1] - w[0]).collect();
        if o.receiver_p.is_some() {
            l.push(format!(
                "batch receiver keyed on the front pocket's P (Tail(2)'s, 7b90c3f1): the middle shut {t_mid} and the rear {t_rear} live gt after its rising edge with the receiver idle; batches keyed {recv_p_edges}; leader gap (the last cart into the gate's cell to the shut) middle {}, rear {}; follower margin (the shut to the next cart into its cell) middle {}, rear {}",
                recv_gap_min[0].map_or("-".into(), |g| g.to_string()),
                recv_gap_min[1].map_or("-".into(), |g| g.to_string()),
                recv_foll_min[0].map_or("-".into(), |g| g.to_string()),
                recv_foll_min[1].map_or("-".into(), |g| g.to_string())
            ));
        }
        l.push(format!(
            "batch receiver (7b90c3f1: Tail(0..2) timed, the middle shut {t_mid} and the rear {t_rear} live gt after each hold release, paused while ALIVE is low; then fed one at a time into Tail(3)): least entry-to-edge gap middle {}, rear {} gt (the latency must stay under it); feeds {} (spacing at Tail(3) {}; the bar >= 24); empty-pocket timeouts {}; the receiver's most carts at once {} (t {}); releases while the previous batch was still held {} (became current late at {})",
            recv_gap_min[0].map_or("-".into(), |g| g.to_string()),
            recv_gap_min[1].map_or("-".into(), |g| g.to_string()),
            recv_feeds.len(),
            if feed_gaps.is_empty() { "-".into() } else { format!("{}..{}", feed_gaps.iter().min().unwrap(), feed_gaps.iter().max().unwrap()) },
            recv_timeouts,
            recv_max.0,
            recv_max.1,
            recv_overlaps,
            if recv_empty_late.is_empty() { "-".into() } else { recv_empty_late.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ") }
        ));
        for e in &recv_edges {
            l.push(format!("  receiver edge {e}"));
        }
        l.push(format!("  receiver feeds (Tail(3)'s P edges): {}{}", recv_feeds.iter().take(12).map(|x| x.to_string()).collect::<Vec<_>>().join(", "), if recv_exit_edges.is_empty() { String::new() } else { format!("; exit detector edges: {}", recv_exit_edges.iter().take(12).map(|x| x.to_string()).collect::<Vec<_>>().join(", ")) }));
    }
    Ok(l.join("\n"))
}
