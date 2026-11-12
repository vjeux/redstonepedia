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
    /// The edge-triggered escapement (ironspec's ruling (a)): G0 on the staged cart's pass into the
    /// hold, G_P on staged AND the hold's entry pocket free AND the release window past.
    pub edge_esc: bool,
    /// Edge escapement: live gt from G0's opening to the staged latch.
    pub stage_delay: i64,
    /// Edge escapement: gt after the hold's release before G_P may fire (the first fill reaches Ga
    /// after it shuts).
    pub fill_after: i64,
    /// Edge escapement: the least gt between G_P firings (the 3 admissions spread through the fill
    /// window, ironspec 18:11 (ii)).
    pub esc_gap: i64,
    /// The escapement's own pass sensor at R's foot: G0 fires on its edge (sooner than the hold's
    /// entry sensor).
    pub esc_sensor: bool,
    /// Edge escapement: the hold's room by count (fills since the last release under 3).
    pub fill_count: bool,
    /// The jam's reset at this tick: every empty cart broken, the latches re-read, G0 re-kicked.
    pub reset_at: i64,
    /// The hold's release gates shut once Ds has counted the carts latched at the release.
    pub hold_close_ds: bool,
    /// Ga closes this many gt after the rear cart's pass over P_b in a release (0: off).
    pub hold_close_pb: i64,
    /// The hold's middle and rear gates open on cart 0's rising edge at Ds (ironspec's (A)).
    pub hold_stagger_ds: bool,
    /// Ds a sloped detector right after the hold's line (v4's).
    pub ds_slope: bool,
    /// Two empties adjacent in the pool come in through the intake as a merged pair.
    pub merged_intake: bool,
    /// Edge escapement: G_P only after no hold sensor edge for this many gt (the hold settled).
    pub fill_quiet: i64,
    /// The start state: the run empty, carts admitted seated one at a time until N are on it.
    pub start_fill: usize,
    /// The start fill by placement: each cart placed at the back, seated before the next.
    pub gentle: bool,
    /// The gentle fill's control: never seat.
    pub gentle_no_seat: bool,
    /// (cc767b78) day-1's live-time cycle clock and liveness gate (`crate::liveness`): the cycle
    /// restarts at each golem event (the column live at a summon boundary: a frozen column makes
    /// none), every pulse (the hold's release at T+3, G_P and G0's openings, the tail chain's
    /// release) ANDed with ALIVE at its rise, a risen pulse completing, the cycle's unfired
    /// remainder dropped when ALIVE falls.
    pub alive: bool,
    /// (cc767b78) T,D: a real freeze: the whole bench at level 32 inside a forced ring of chunks
    /// (redstone running, entities frozen) from tick T for D gt.
    pub freeze32: Vec<(i64, i64)>,
    /// The hold as 9fc3155f's (B) on onechunk's v3 (hold-tail-v3.txt): net-0 V-pockets, pitch 5.
    pub hold_v: bool,
    /// The tail chain as 9fc3155f's v3 draft (hold-tail-v3.txt, peaks fixed): 3 stand pockets, a
    /// V-pocket, the intake pocket (5 pockets).
    pub tail_v: bool,
    /// The brake-boost hold (`HOLD_BB`): the stand's pocket with its rest slope powered with its gate.
    pub hold_bb: bool,
    /// The hold as a line of tokens (`line`'s: r. rv r^ P. P^ Pv R* G g), 3 sensors and 3 gates.
    pub hold_line: Option<String>,
    /// The escapement's three advances a cycle (A; G_P at A+6 for 11 gt, G0 at A+13 for 12 gt).
    pub esc_at: Vec<i64>,
    /// The hold alone (`hold_release`): 3 carts at rest in its pockets, one release.
    pub hold_only: bool,
}

/// A track line from tokens (9fc3155f's v3/v4 hold and tail chain, the corners laid flat): `r.` a
/// flat rail, `rv` a slope down, `r^` a slope up, `P.` a flat detector rail, `P^` / `Pv` a detector
/// rail rising / falling, `G` a powered slope down under its gate (powered with the gate), `g` a plain
/// slope down under its gate. Each P is a sensor; each G or g a gate (its cell over the slope, at the
/// level the slope leaves) with the powered slope's cell for a G; `R*` a powered slope down powered
/// with the next gate (the brake-boost rest cell: braking while its gate is shut, launching as it
/// opens). hold-v4.txt's (5/5) hold (the stand's pockets, a flat in each valley) is `HOLD_V4`.
pub const HOLD_V: &str = "r. rv P. rv G r^ r^ P. rv G r^ r. r^ P. rv G r. r.";
pub const HOLD_V4: &str = "r. rv P. rv G r. r^ P. rv G r. r^ P. rv G r. r.";
pub const TAIL_V: &str = "r. P. rv G r^ P. rv G r^ P. rv G rv r. r. r. r. r. r^ P. rv G r^ r^ P. rv g r.";
/// The brake-boost hold (ironspec 17:33's lever): the stand's pocket (P flat on top, R, G under its
/// gate, U) with R a powered slope powered with its gate, so all three carts launch at the release
/// from rest on a booster; P's block shares no face with R's or G's (R's block is one lower).
pub const HOLD_BB: &str = "r. rv P. R* G r^ P. R* G r^ P. R* G r. r.";

struct LineBlocks {
    glass: Block,
    flat: Block,
    down: Block,
    up: Block,
    det: Block,
    det_up: Block,
    det_down: Block,
    pdown_off: Block,
}

/// A gate of a line: its cell, the powered slope under it (a G), and the powered rest slope before
/// it (an R*), both powered with the gate.
type LineGate = (i32, i32, Option<(i32, i32)>, Option<(i32, i32)>);

fn line(b: &LineBlocks, blocks: &mut Vec<(BlockPos, Block)>, x: &mut i32, lv: &mut i32, spec: &str) -> Result<(Vec<(i32, i32)>, Vec<LineGate>), String> {
    let mut sensors = Vec::new();
    let mut gates = Vec::new();
    let mut rest: Option<(i32, i32)> = None;
    for tok in spec.split_whitespace() {
        let (y, rail, dl) = match tok {
            "r." => (*lv, &b.flat, 0),
            "rv" => (*lv - 1, &b.down, -1),
            "r^" => (*lv, &b.up, 1),
            "P." => (*lv, &b.det, 0),
            "P^" => (*lv, &b.det_up, 1),
            "Pv" => (*lv - 1, &b.det_down, -1),
            "R*" => (*lv - 1, &b.pdown_off, -1),
            "G" | "g" => (*lv - 1, if tok == "G" { &b.pdown_off } else { &b.down }, -1),
            _ => return Err(format!("{tok:?}: a line token of r. rv r^ P. P^ Pv R* G g")),
        };
        blocks.push((BlockPos::new(*x, y, 0), rail.clone()));
        blocks.push((BlockPos::new(*x, y - 1, 0), b.glass.clone()));
        if tok.starts_with('P') {
            sensors.push((*x, y));
        }
        if tok == "R*" {
            rest = Some((*x, y));
        }
        if tok == "G" || tok == "g" {
            gates.push((*x, *lv, if tok == "G" { Some((*x, y)) } else { None }, rest.take()));
        }
        *lv += dl;
        *x += 1;
    }
    Ok((sensors, gates))
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
    /// The descent's first cell, a pass detector: the hold's front pocket left.
    ds: (i32, i32),
    /// The escapement's own pass sensor at R's foot (`--esc-sensor`).
    esc_s: Option<(i32, i32)>,
    run_x: (f64, f64),
    g0_plane: f64,
    gp_plane: f64,
    launch: (f64, f64),
    sink_x: f64,
    run_cells: Vec<(i32, i32)>,
    run_sensor: (i32, i32),
    /// Gates (by index) with a brake-boost rest slope powered with them (an R*).
    gate_rest: std::collections::HashMap<usize, (i32, i32)>,
    /// The hold's front gate's cell's east edge (a cart's centre past it has left the hold).
    hold_exit: f64,
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
    let mut g = Geo { gates: Vec::new(), tail_p: Vec::new(), hold_p: Vec::new(), run_x: (0.0, 0.0), g0_plane: 0.0, gp_plane: 0.0, launch: (0.5, top as f64 + 0.0625), sink_x: 0.0, run_cells: Vec::new(), run_sensor: (0, 0), ds: (0, 0), gate_rest: Default::default(), hold_exit: f64::MAX, esc_s: None };
    let lb = LineBlocks {
        glass: glass.clone(),
        flat: flat.clone(),
        down: down.clone(),
        up: up.clone(),
        det: det.clone(),
        det_up: Block::parse("minecraft:detector_rail[powered=false,shape=ascending_east,waterlogged=false]")?,
        det_down: det_down.clone(),
        pdown_off: pdown_off.clone(),
    };
    let mut put = |blocks: &mut Vec<(BlockPos, Block)>, x: i32, y: i32, b: &Block| {
        blocks.push((BlockPos::new(x, y, 0), b.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
    };
    // (the launch flat, then a rigid block west of every descending slope's high side as needed)
    put(&mut blocks, x, lv, &flat);
    x += 1;
    let intake_flat = !o.no_intake_flat;
    let pocket = |blocks: &mut Vec<(BlockPos, Block)>, extra: &mut Vec<(BlockPos, Block)>, put: &mut dyn FnMut(&mut Vec<(BlockPos, Block)>, i32, i32, &Block), x: &mut i32, lv: &mut i32, last_over_run: bool| -> ((i32, i32), (i32, i32), (i32, i32)) {
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
        put(blocks, *x + 3, *lv - 2, &up);
        // (U rises east: its high side's rigid block is under the next P)
        extra.push((BlockPos::new(*x + 4, *lv - 2, 0), Block::parse("minecraft:glass").unwrap()));
        *x += 4;
        *lv -= 1;
        (p, gate_cell, g_cell)
    };
    let ntail = if o.ideal_tail { 1 } else { o.tail };
    if o.tail_v && !o.ideal_tail {
        // (9fc3155f's tail chain: the launch flat stands for its buffer cell)
        let (ps, gs) = line(&lb, &mut blocks, &mut x, &mut lv, &TAIL_V[3..])?;
        if ps.len() != o.tail || gs.len() != o.tail {
            return Err(format!("--tail-v lays {} pockets: run it with --tail {}", gs.len(), gs.len()));
        }
        g.tail_p = ps;
        for (j, (gx, gy, gg, gr)) in gs.into_iter().enumerate() {
            if let Some(c) = gr {
                g.gate_rest.insert(g.gates.len(), c);
            }
            g.gates.push((Role::Tail(j), gx, gy, gg));
        }
    }
    for j in 0..if o.tail_v && !o.ideal_tail { 0 } else { ntail } {
        let last = j + 1 == ntail;
        let (p, gc, gg) = pocket(&mut blocks, &mut extra, &mut put, &mut x, &mut lv, last);
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
    // (the escapement's own pass sensor (--esc-sensor): a flat detector at R's foot, diagonal to G_P's
    // gate block, two cells from P's powered slope: the admitted cart's pass, G0's edge)
    if o.esc_sensor {
        put(&mut blocks, x, lv, &det);
        g.esc_s = Some((x, lv));
        x += 1;
    }
    // (the hold: 3 pockets; the front one's G a powered slope into the descent; or a line of tokens:
    // its 3 P detectors are the hold's sensors, hold_p[0] the entry pocket's)
    if o.hold_v || o.hold_bb || o.hold_line.is_some() {
        let spec = o.hold_line.as_deref().unwrap_or(if o.hold_bb { HOLD_BB } else { HOLD_V });
        let (ps, gs) = line(&lb, &mut blocks, &mut x, &mut lv, spec)?;
        if gs.len() != 3 || ps.len() != 3 {
            return Err(format!("the hold's line {spec:?} lays {} sensors and {} gates (3 each)", ps.len(), gs.len()));
        }
        g.hold_p = ps;
        for (j, (gx, gy, gg, gr)) in gs.into_iter().enumerate() {
            if let Some(c) = gr {
                g.gate_rest.insert(g.gates.len(), c);
            }
            g.gates.push((Role::Hold(j), gx, gy, gg));
        }
    }
    for j in 0..if o.hold_v || o.hold_bb || o.hold_line.is_some() { 0 } else { 3 } {
        let (p, gc, gg) = pocket(&mut blocks, &mut extra, &mut put, &mut x, &mut lv, false);
        g.hold_p.push(p);
        g.gates.push((Role::Hold(j), gc.0, gc.1, Some(gg)));
    }
    g.hold_exit = g.gates.iter().filter(|gt| gt.0 == Role::Hold(2)).map(|gt| gt.1 as f64 + 1.0).next().unwrap_or(f64::MAX);
    // (the descent and the sink: a flat first, at the front pocket's U top (a detector rail: the
    // front pocket's leaving edge, 7b90c3f1's per-pocket latch clear), then slopes down; or, with
    // --ds-slope, Ds a sloped detector right after the line (v4's Ds at (7,14,5): the first cell
    // after G_a, a continuing descent))
    if o.ds_slope {
        put(&mut blocks, x, lv - 1, &det_down);
        g.ds = (x, lv - 1);
        x += 1;
        lv -= 1;
    } else {
        put(&mut blocks, x, lv, &det);
        g.ds = (x, lv);
        x += 1;
    }
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

/// The hold alone (9fc3155f, ironspec 17:33 and 18:04's hand-off targets): the bench's line built as
/// for a run, one cart set at rest in each hold pocket (on its rest slope, 0.6 short of the gate cell;
/// the carts ridden or empty by `riders`, the front pocket's first), then every hold gate opened with
/// its powered cells at t 40 for `hold_pulse` gt: each cart's exit past the front gate's cell, the
/// gaps, the closest centres while they leave, the front gate's shut onto a cart, the rest heights.
pub fn hold_release(o: &GqOpts) -> Result<String, String> {
    let (spec, g) = build(o)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let rsrc = Block::parse("minecraft:redstone_block")?;
    let glass_b = Block::parse("minecraft:glass")?;
    let holds: Vec<usize> = (0..g.gates.len()).filter(|gi| matches!(g.gates[*gi].0, Role::Hold(_))).collect();
    // (the carts: pocket 2 the front, its cart first)
    let mut ids = Vec::new();
    for (n, gi) in holds.iter().rev().enumerate() {
        let (_, gx, gy, _) = g.gates[*gi];
        let ridden = o.riders.get(n % o.riders.len().max(1)).copied().unwrap_or(true);
        let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        let nbt = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
        ids.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [gx as f64 - 0.6, gy as f64 + 0.45, 0.5], Some(&nbt))?);
    }
    let set = |run: &mut Run, open: bool| {
        for gi in &holds {
            let (_, gx, gy, gcell) = g.gates[*gi];
            run.world.set_block(BlockPos::new(gx, gy, 0), if open { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
            for c in [gcell, g.gate_rest.get(gi).copied()].into_iter().flatten() {
                run.world.set_block(BlockPos::new(c.0, c.1 - 1, 0), if open { rsrc.clone() } else { glass_b.clone() }, redsim::world::flags::ALL);
            }
        }
    };
    let t0 = 40i64;
    let mut exits: Vec<(usize, i64)> = Vec::new();
    let mut rest = Vec::new();
    let mut closest = f64::MAX;
    let mut shut_onto = Vec::new();
    let mut prev: Vec<Option<(f64, f64, f64)>> = vec![None; ids.len()];
    for t in 0..t0 + 160 {
        if t == t0 {
            set(&mut run, true);
            for id in &ids {
                if let Some((_, y, _)) = cart_state(&run.world, *id) {
                    rest.push(y);
                }
            }
        }
        if t == t0 + o.hold_pulse {
            set(&mut run, false);
            for (k, id) in ids.iter().enumerate() {
                if let Some((x, y, _)) = cart_state(&run.world, *id) {
                    for gi in &holds {
                        let (role, gx, gy, _) = g.gates[*gi];
                        if x + 0.49 > gx as f64 + 0.375 && x - 0.49 < gx as f64 + 0.625 && y + 0.7 > gy as f64 && y < gy as f64 + 1.5 {
                            shut_onto.push(format!("{role:?} onto cart {k}"));
                        }
                    }
                }
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64)>> = ids.iter().map(|id| cart_state(&run.world, *id)).collect();
        if t >= t0 {
            for k in 0..ids.len() {
                if let (Some((x, _, _)), Some((px, _, _))) = (now[k], prev[k])
                    && px < g.hold_exit
                    && x >= g.hold_exit
                {
                    exits.push((k, t - t0));
                }
                for j in k + 1..ids.len() {
                    // (while both are still on the hold's line or the 6 cells after its front
                    // gate: the descent the hand-off feeds, not the bench's run-out)
                    if let (Some((xa, _, _)), Some((xb, _, _))) = (now[k], now[j])
                        && xa < g.hold_exit + 6.0
                        && xb < g.hold_exit + 6.0
                    {
                        closest = closest.min((xa - xb).abs());
                    }
                }
            }
        }
        prev = now;
    }
    let order: String = exits.iter().map(|(k, _)| ["a", "b", "c"].get(*k).copied().unwrap_or("?")).collect::<Vec<_>>().join("");
    let gaps: Vec<i64> = exits.windows(2).map(|w| w[1].1 - w[0].1).collect();
    Ok(format!(
        "hold alone ({}; riders {}; the gates and their powered cells open at t 0 for {} gt): exits past the front gate's cell at {} (order {order}), gaps {}; closest centres while leaving {:.2}; rest heights {}; shut onto a cart at the window's end: {}",
        o.hold_line.as_deref().unwrap_or(if o.hold_bb { HOLD_BB } else if o.hold_v { HOLD_V } else { "the stand's pocket" }),
        (0..ids.len()).map(|n| if o.riders.get(n % o.riders.len().max(1)).copied().unwrap_or(true) { 'R' } else { 'E' }).collect::<String>(),
        o.hold_pulse,
        exits.iter().map(|(_, t)| format!("+{t}")).collect::<Vec<_>>().join(" "),
        gaps.iter().map(|g| g.to_string()).collect::<Vec<_>>().join("/"),
        closest,
        rest.iter().map(|y| format!("{y:.3}")).collect::<Vec<_>>().join(" "),
        if shut_onto.is_empty() { "none".to_string() } else { shut_onto.join(", ") }
    ))
}

/// Runs the bench; returns the text.
pub fn gq_bench(o: &GqOpts) -> Result<String, String> {
    if o.hold_only {
        return hold_release(o);
    }
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
    let spawn = |run: &mut Run, pos: [f64; 3], v: f64, rid: bool| -> Result<u32, String> {
        let rider = if rid { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        let nbt = format!("{{Motion:[{v:.4}d,0.0d,0.0d]{rider}}}");
        redsim::entity::spawn(&mut run.world, "minecraft:minecart", pos, Some(&nbt))
    };
    // (the start: the run filled at rest from G0 up, one cart a cell; the rest launched into the
    // tail chain 30 gt apart)
    let mut ids: Vec<Option<u32>> = vec![None; o.carts];
    // (carts broken at the jam's reset and replaced by the player with seated villagers)
    let mut replaced: Vec<bool> = vec![false; o.carts];
    let mut pending: Vec<(i64, usize)> = Vec::new();
    let in_run = o.cap.min(o.carts).min(g.run_cells.len());
    if o.start_fill > 0 && o.gentle {
        // (the gentle start: carts 0..N placed one at a time at the back by the procedure below;
        // the rest seated and pooled at the staging cell)
        for (i, k) in (o.start_fill.min(o.carts)..o.carts).enumerate() {
            pending.push((i as i64 * 30, k));
        }
    } else if o.start_fill > 0 {
        // (the start state: the run empty, every cart seated and pooled at the staging cell)
        for k in 0..o.carts {
            pending.push((k as i64 * 30, k));
        }
    } else if o.back_at > 0.0 {
        // (the run filled pressed (0.790 apart in x, measured) from G0's rest up to the back's target,
        // `back_at` past the last tail gate's cell: the count the intake keeps constant)
        let gx = g.gates.iter().find(|x| x.0 == Role::Tail(g.tail_p.len() - 1)).map_or(0.0, |x| x.1 as f64);
        let x_front = g.g0_plane - 0.125 - 0.49;
        let mut k = 0usize;
        let mut xk = x_front;
        while xk >= gx + o.back_at - 0.01 && k < o.carts {
            let cell = g.run_cells.iter().find(|c| xk >= c.0 as f64 && xk < (c.0 + 1) as f64).copied();
            let y = cell.map_or(0.0, |c| c.1 as f64 + (1.0 - (xk - c.0 as f64)) + 0.0625);
            ids[k] = Some(spawn(&mut run, [xk, y, 0.5], 0.0, ridden_k(k))?);
            k += 1;
            xk -= 0.79;
        }
        for (i, kk) in (k..o.carts).enumerate() {
            pending.push((i as i64 * 30, kk));
        }
    } else {
    for k in 0..in_run {
        let (cx, cy) = g.run_cells[g.run_cells.len() - 1 - k];
        ids[k] = Some(spawn(&mut run, [cx as f64 + 0.5, cy as f64 + 0.5 + 0.0625, 0.5], 0.0, ridden_k(k))?);
    }
    for (i, k) in (in_run..o.carts).enumerate() {
        pending.push((i as i64 * 30, k));
    }
    }
    let n_gates = g.gates.len();
    let mut target_hist: Vec<std::collections::VecDeque<bool>> = vec![std::collections::VecDeque::new(); n_gates];
    let mut gate_open: Vec<bool> = vec![false; n_gates];
    let sensors: Vec<(i32, i32)> = g.tail_p.iter().chain(g.hold_p.iter()).copied().chain([g.ds]).collect();
    let nt = g.tail_p.len();
    let mut was_on = vec![false; sensors.len()];
    let mut occ_t = vec![false; nt];
    let mut occ_h = [false; 3];
    let mut last_rise_t1: i64 = i64::MIN / 2;
    let mut settle_live: i64 = 0;
    let mut g0_was = false;
    let mut g0_prev_w = false;
    let mut owed: i64 = 0;
    let mut egp_until: i64 = -1;
    let mut eg0_until: i64 = -1;
    let mut staged_at: Option<i64> = None;
    let mut staged = false;
    let mut gp_cond_was = false;
    let mut fills_since: usize = 0;
    let mut last_hold_edge: i64 = i64::MIN / 2;
    let mut release_n: usize = 0;
    let mut ds_since: usize = 0;
    let mut pb_close_at: Option<i64> = None;
    let mut ds_first_at: Option<i64> = None;
    let mut gone: Vec<bool> = vec![false; o.carts];
    let mut reset_log = String::new();
    let mut esc_was_on = false;
    let mut g0_wait = false;
    let mut last_gp: i64 = i64::MIN / 2;
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
    let mut merged_pairs: std::collections::BTreeSet<(usize, usize)> = Default::default();
    let mut merge_log: Vec<String> = Vec::new();
    let mut merge_open: std::collections::BTreeMap<(usize, usize), (i64, f64, f64)> = Default::default();
    let mut front_v_max: f64 = 0.0;
    let mut vmax_any: (f64, i64, usize) = (0.0, 0, 0);
    let mut off_track: Vec<(i64, usize, f64, f64)> = Vec::new();
    let rail_y_min = spec.lo.y as f64 + 7.0;
    let rail_x_span = (spec.lo.x as f64, spec.hi.x as f64);
    let mut back_moves: usize = 0;
    let mut back_max: f64 = 0.0;
    let mut run_min: (usize, i64) = (usize::MAX, 0);
    let mut hold_at_release: Vec<(i64, usize)> = Vec::new();
    let mut tail_max = 0usize;
    let mut sunk = 0usize;
    let mut launched_into_run: std::collections::BTreeSet<usize> = Default::default();
    let warm = if o.start_fill > 0 { 200 + 60 * o.start_fill as i64 } else { 200i64 };
    let mut run_still_since: i64 = 0;
    let mut fill_done = false;
    let mut fill_done_at: i64 = -1;
    let mut gentle_next: usize = 0;
    let mut gentle_seat: Option<(usize, i64)> = None;
    let mut gentle_log: Vec<String> = Vec::new();
    let mut gentle_last_place: i64 = i64::MIN / 2;
    // (--merged-intake: each merged pair's spawn tick and carts; per cart: released, the lowest point
    // reached (largest x), the highest point after it (its bounce back up), the tick of that)
    let mut merged_in: Vec<(i64, usize, usize)> = Vec::new();
    let mut merged_track: std::collections::BTreeMap<usize, (bool, f64, f64, i64)> = Default::default();
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
    for t in 0..ticks {
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
                ids[k] = Some(spawn(&mut run, [g.launch.0, g.launch.1, 0.5], o.ret_speed, ridden_k(k) || replaced[k])?);
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
                ids[k] = Some(spawn(&mut run, [xr, yr, 0.5], 0.0, ridden_k(k) || replaced[k])?);
                // (--merged-intake: an empty cart followed in the pool by another empty comes in as a
                // merged pair: both placed on the same spot at the staging cell (the jam case after two
                // deaths), released together)
                if o.merged_intake
                    && !(ridden_k(k) || replaced[k])
                    && let Some(&k2) = pool.front()
                    && !(ridden_k(k2) || replaced[k2])
                {
                    pool.pop_front();
                    ids[k2] = Some(spawn(&mut run, [xr, yr, 0.5], 0.0, false)?);
                    merged_in.push((t, k, k2));
                }
                occ_t[0] = true;
                settle_live = o.settle;
                next_spawn_ok = t + 4;
            }
        }
        gate.column_frozen = Some(frz || real_frozen);
        let alive = gate.alive(&run.world, t);
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
                if let Some((cx, cy)) = g.gate_rest.get(&gi).copied() {
                    // (an R*, the brake-boost rest slope: powered with its gate the same way)
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
                if !gone[k] {
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
                } else {
                    let h = si - nt;
                    last_hold_edge = t;
                    if h == 3 {
                        // (Ds: the hold's front pocket left)
                        occ_h[2] = false;
                        ds_since += 1;
                        // (--hold-stagger-ds: cart 0's rising edge at Ds opens the middle and rear gates)
                        if hold_release_at >= 0 && t > hold_release_at && ds_first_at.is_none() {
                            ds_first_at = Some(t);
                        }
                        if std::env::var("MCTAS_GQ_EVENTS").is_ok() {
                            eprintln!("t {t}: Ds edge (count {ds_since} of {release_n} since the release at {hold_release_at})");
                        }
                    } else {
                    occ_h[h] = true;
                    // (--hold-close-pb D: the rear cart's pass over the middle pocket's detector (P_b),
                    // the only cart to cross it during a release, keys Ga's close D gt later)
                    if h == 1 && o.hold_close_pb > 0 && hold_release_at >= 0 && t > hold_release_at && t < hold_release_at + o.hold_pulse && pb_close_at.is_none() {
                        pb_close_at = Some(t + o.hold_close_pb);
                        if std::env::var("MCTAS_GQ_EVENTS").is_ok() {
                            eprintln!("t {t}: P_b edge in the release at {hold_release_at}: Ga closes at {}", t + o.hold_close_pb);
                        }
                    }
                    if h > 0 {
                        occ_h[h - 1] = false;
                    }
                    if h == 0 {
                        count_q -= 1;
                        // (edge-triggered escapement, ironspec's ruling (a): G0 opens on the staged
                        // cart's pass edge into the hold)
                        if o.edge_esc && !o.esc_sensor && !frz {
                            eg0_until = t + 12;
                            staged_at = Some(t);
                            if std::env::var("MCTAS_GQ_EVENTS").is_ok() {
                                eprintln!("t {t}: P_h0 edge, G0 fires");
                            }
                        }
                    }
                    }
                }
            }
            was_on[si] = on;
        }
        // (the escapement's own pass sensor: G0 fires on its edge)
        if let Some((ex, ey)) = g.esc_s {
            let on = matches!(run.world.get(BlockPos::new(ex, ey, 0)), Block::DetectorRail { powered: true, .. });
            if on && !esc_was_on && o.edge_esc && !frz {
                // (with --alive-gate the edge's pulse rises only while ALIVE: a refused edge waits,
                // latched, for ALIVE)
                if o.alive && !(alive && gate.allow_rise(&run.world, "G0 (escapement edge)", t)) {
                    g0_wait = true;
                } else {
                    eg0_until = t + 12;
                    staged_at = Some(t);
                }
                if std::env::var("MCTAS_GQ_EVENTS").is_ok() {
                    eprintln!("t {t}: escapement sensor edge, G0 fires{}", if g0_wait { " (waiting for ALIVE)" } else { "" });
                }
            }
            esc_was_on = on;
        }
        if g0_wait && alive && !frz && gate.allow_rise(&run.world, "G0 (escapement edge, waited)", t) {
            g0_wait = false;
            eg0_until = t + 12;
            staged_at = Some(t);
        }
        // (the jam's reset (ironspec 18:48), at --reset-at T: the player breaks every empty cart (the
        // merged pair first: the carts are simply removed), then presses the escapement's reset: the
        // latches re-read from where the carts are (each hold pocket latched if a cart is in it, the
        // fill count from them), and G0 fires once if P holds no cart (else P's cart counts as staged))
        if o.reset_at > 0 && t == o.reset_at {
            let mut broken = Vec::new();
            for k in 0..o.carts {
                if !ridden_k(k) {
                    if let Some(id) = ids[k] {
                        let _ = redsim::entity::kill(&mut run.world, id);
                        ids[k] = None;
                    }
                    // (a riderless cart off the queue's track (on the climb or in the stand) is broken
                    // there too: the census names it)
                    gone[k] = true;
                    broken.push(k);
                }
            }
            pending.retain(|p| !gone[p.1]);
            pool.retain(|k| !gone[*k]);
            // (the procedure ends with the player placing a new cart with a seated villager for each
            // broken one, at the climb's top, until the census reads 21 again (ironspec 19:04))
            for (i, k) in broken.iter().enumerate() {
                gone[*k] = false;
                replaced[*k] = true;
                pending.push((t + 100 + 30 * i as i64, *k));
            }
            let now2: Vec<Option<(f64, f64, f64)>> = ids.iter().map(|id| id.and_then(|id| cart_state(&run.world, id))).collect();
            for j in 0..3 {
                let px = g.hold_p[j].0 as f64;
                let next = if j < 2 { g.hold_p[j + 1].0 as f64 } else { h2_face + 0.5 };
                occ_h[j] = now2.iter().any(|c| c.is_some_and(|(x, _, _)| x >= px && x < next));
            }
            fills_since = occ_h.iter().filter(|b| **b).count();
            // (the intake's owed admissions cleared too: the run's count restarts from what is on it)
            owed = 0;
            owed_at.clear();
            let p_has = now2.iter().any(|c| c.is_some_and(|(x, _, _)| x >= g.g0_plane - 0.5 && x < g.gp_plane));
            if p_has {
                staged = true;
            } else {
                eg0_until = t + 12;
                staged_at = Some(t);
            }
            reset_log = format!("t {t}: broke the empty carts {broken:?}; latches re-read: hold {occ_h:?}, fills {fills_since}; P {}", if p_has { "held a cart (staged)" } else { "empty: G0 fired once" });
        }
        // (the cycle: T = warm + 100 c; the hold at T+3; the escapement at A+6 / A+13)
        let ph = (t - warm - o.corner).rem_euclid(100);
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
        let hold_rise = if o.alive { gate.rise(&run.world, "cycle", 3, t) } else { in_cycles && ph == 3 && !frz };
        if hold_rise {
            // (the last release's margins: the shut after the rear cleared, the next cart after
            // the shut)
            if let Some(s) = hold_shut_at.take() {
                margins.push((s, rear_clear.map(|c| s - c), next_at_front.map(|n| n - s)));
            }
            next_at_front = None;
            let held = occ_h.iter().filter(|b| **b).count();
            release_n = held;
            ds_since = 0;
            pb_close_at = None;
            ds_first_at = None;
            hold_at_release.push((t, held));
            hold_release_until = t + o.hold_pulse;
            hold_release_at = t;
            fills_since = 0;
            occ_h = if o.edge_esc { occ_h } else { [false; 3] };
            // (the carts released: those in the hold now; the rear-clear tracking restarts)
            let hx0 = g.hold_p[0].0 as f64;
            released = now.iter().enumerate().filter(|(_, c)| c.is_some_and(|(x, _, _)| x >= hx0 && x < h2_face + 0.2)).map(|(k, _)| k).collect();
            rear_clear = None;
            margin_pending = true;
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
        let g0_w = if o.edge_esc { t < eg0_until && !frz } else { in_cycles && !frz && G0_AT.iter().any(|a| (ph >= *a && ph < a + 12) || (t - warm >= 100 && ph + 100 >= *a && ph + 100 < a + 12)) };
        if g0_w && !g0_prev_w {
            owed += 1;
            owed_at.push_back(t);
        }
        g0_prev_w = g0_w;
        // (--start-fill N, the guide's fill procedure: carts join the run only ridden, one at a time:
        // each seated at the last tail gate (the staging cell above the intake), then admitted once
        // the run below is still (no cart on it moving for 5 gt), until N carts are on the run)
        let run_count = ids.iter().filter(|id| id.and_then(|id| cart_state(&run.world, id)).is_some_and(|(x, _, _)| x >= g.run_x.0 as f64 && x < g.run_x.1 as f64)).count();
        let run_moving = ids.iter().any(|id| id.and_then(|id| cart_state(&run.world, id)).is_some_and(|(x, _, v)| x >= g.run_x.0 as f64 && x < g.run_x.1 as f64 && v > 0.01));
        if run_moving {
            run_still_since = t;
        }
        // (--gentle, ironspec 19:29: each cart placed empty on the run's rail cell just above the
        // current back (the highest cell centre at least the pressed spacing above the back cart, so
        // it rolls under one cell onto it), its villager seated once the run is still (the empty
        // replaced in place by a ridden cart at rest), and only then the next placed: at most one
        // empty, at rest at the back; --gentle-no-seat never seats (the empty-fill control))
        if o.start_fill > 0 && o.gentle && !fill_done && t >= 10 {
            // (the control never waits for stillness, as an empty run never stills: a placement
            // every 25 gt, as in tick-exact's empty fill)
            let still = t - run_still_since >= 5 || (o.gentle_no_seat && t - gentle_last_place >= 25);
            match gentle_seat {
                None if still && gentle_next < o.start_fill.min(o.carts) => {
                    let back_x = ids.iter().filter_map(|id| id.and_then(|id| cart_state(&run.world, id))).filter(|(x, _, _)| *x >= g.run_x.0 as f64 && *x < g.run_x.1 as f64).map(|c| c.0).fold(f64::INFINITY, f64::min);
                    let cell = if back_x.is_infinite() {
                        g.run_cells.last().copied()
                    } else {
                        g.run_cells.iter().filter(|c| c.0 as f64 + 0.5 <= back_x - 0.79).max_by_key(|c| c.0).copied()
                    };
                    if let Some((cx, cy)) = cell {
                        let k = gentle_next;
                        ids[k] = Some(spawn(&mut run, [cx as f64 + 0.5, cy as f64 + 0.5 + 0.0625, 0.5], 0.0, false)?);
                        gentle_seat = Some((k, t));
                        gentle_last_place = t;
                        gentle_log.push(format!("t {t}: cart {k} placed at x {:.1}{}", cx as f64 + 0.5, if back_x.is_infinite() { " (the run empty)".to_string() } else { format!(" (the back at {back_x:.3}, gap {:.3})", back_x - 0.79 - (cx as f64 + 0.5)) }));
                    } else {
                        gentle_next = o.start_fill;
                    }
                }
                Some((k, t0)) if (still || o.gentle_no_seat) && t > t0 + 5 => {
                    if !o.gentle_no_seat
                        && let Some(id) = ids[k]
                        && let Some((x, y, _)) = cart_state(&run.world, id)
                    {
                        let _ = redsim::entity::kill(&mut run.world, id);
                        ids[k] = Some(spawn(&mut run, [x, y, 0.5], 0.0, true)?);
                        replaced[k] = true;
                    }
                    gentle_seat = None;
                    gentle_next += 1;
                }
                _ => {}
            }
        }
        let gentle_filling = o.start_fill > 0 && o.gentle && !fill_done;
        if o.start_fill > 0 && !fill_done && run_count >= o.start_fill && t - run_still_since >= 5 && (!o.gentle || gentle_next >= o.start_fill) {
            fill_done = true;
            fill_done_at = t;
        }
        let filling = o.start_fill > 0 && !fill_done;
        let tail_ev = if gentle_filling {
            false
        } else if filling {
            occ_t[nt - 1] && t - run_still_since >= 5 && settle_live >= o.settle && t >= tail_release_until + 4
        } else if o.intake_admit {
            // (released once the admission's train shift has settled: `adm_settle` gt after its edge)
            occ_t[nt - 1] && owed > 0 && owed_at.front().is_some_and(|e| t - e >= o.adm_settle) && settle_live >= o.settle && t >= tail_release_until + 4
        } else {
            !o.overfill && occ_t[nt - 1] && !run_busy && settle_live >= o.settle && t >= tail_release_until + 4
        };
        if tail_ev && o.intake_admit && !filling && !frz {
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
            let g0_now = in_cycles && !frz && o.esc_at.iter().any(|a| (ph >= a + 13 && ph < a + 13 + 12) || (t - warm >= 100 && ph + 100 >= a + 13 && ph + 100 < a + 13 + 12));
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
        // (the edge-triggered escapement, ironspec's ruling (a): the staged latch set `stage_delay`
        // live gt after G0's opening (the admitted cart at rest on P against G_P); G_P fires on the
        // rising edge of staged AND the hold's entry pocket latched free AND the hold's release
        // window past `fill_after` gt (the first fill reaches Ga after it shuts); G0 fires on the
        // staged cart's pass into the hold (P_h0's edge, above); one kick at the first cycle)
        if o.edge_esc {
            if t == warm {
                eg0_until = t + 12;
                staged_at = Some(t);
            }
            if let Some(s) = staged_at
                && t - s >= o.stage_delay
            {
                staged = true;
                staged_at = None;
            }
            let quiet = hold_release_at < 0 || t >= hold_release_at + o.fill_after;
            let spaced = t >= last_gp + o.esc_gap;
            // (--fill-count: the hold's room by count (fills since the last release under 3) instead
            // of its entry pocket's latch, so fills may follow each other into the hold)
            let room = if o.fill_count { fills_since < 3 } else { !occ_h[0] };
            // (--fill-quiet: the hold settled: no hold sensor edge in the last Q gt, so a fill never
            // follows a cart shifting between pockets through a gate that is about to shut)
            let room = room && t - last_hold_edge >= o.fill_quiet;
            let cond = staged && room && quiet && spaced && !frz && t >= warm && (!o.alive || alive);
            if cond && !gp_cond_was {
                if o.alive {
                    let _ = gate.allow_rise(&run.world, "G_P (edge)", t);
                }
                egp_until = t + 11;
                staged = false;
                last_gp = t;
                fills_since += 1;
                if std::env::var("MCTAS_GQ_EVENTS").is_ok() {
                    eprintln!("t {t} (ph {ph}): G_P fires; occ_h {occ_h:?}");
                }
            }
            if std::env::var("MCTAS_GQ_EVENTS").is_ok() && staged && !cond && (t % 5 == 0) {
                eprintln!("t {t} (ph {ph}): staged, waiting: occ_h {occ_h:?} quiet {quiet} spaced {spaced}");
            }
            gp_cond_was = cond;
        }
        for (gi, (role, _, _, _)) in g.gates.iter().enumerate() {
            let open = match role {
                Role::Tail(j) if o.overfill && *j + 1 == nt => last_open,
                Role::Tail(j) if o.overfill && *j + 2 == nt => !occ_t[j + 1] && !last_open && !(*j == 0 && o.spacer > 0 && t - last_rise_t1 < o.spacer),
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
                Role::Gp if o.edge_esc => t < egp_until,
                Role::G0 if o.edge_esc => t < eg0_until,
                Role::Gp => if o.alive { t < gp_until } else { in_cycles && !frz && o.esc_at.iter().any(|a| (ph >= a + 6 && ph < a + 6 + 11) || (t - warm >= 100 && ph + 100 >= a + 6 && ph + 100 < a + 6 + 11)) },
                Role::G0 => if o.alive { t < g0_until } else { in_cycles && !frz && o.esc_at.iter().any(|a| (ph >= a + 13 && ph < a + 13 + 12) || (t - warm >= 100 && ph + 100 >= a + 13 && ph + 100 < a + 13 + 12)) },
                Role::Hold(j) => {
                    // (together: every hold gate open for hold_pulse from T+3; a ripple of D: the
                    // front gate at T+3, each gate behind D gt later, each for hold_pulse)
                    let from = if o.hold_stagger_ds && *j < 2 {
                        // (the middle and rear gates open on cart 0's rising edge at Ds; not before)
                        ds_first_at.unwrap_or(i64::MAX / 2)
                    } else {
                        hold_release_at + if o.hold_ripple > 0 { (2 - *j as i64) * o.hold_ripple } else { 0 }
                    };
                    let mut until = hold_release_at + 2 * o.hold_ripple.max(0) + o.hold_pulse;
                    // (--hold-close-ds: the release's gates shut once Ds has counted as many leaving
                    // edges as carts were latched at the release (evidence, ruling (a)); hold_pulse
                    // the cap)
                    if o.hold_close_ds && hold_release_at >= 0 {
                        if ds_since >= release_n && t > hold_release_at {
                            until = until.min(t);
                        }
                    }
                    if let Some(c) = pb_close_at {
                        until = until.min(c);
                    }
                    if hold_release_at >= 0 && t >= from && t < until {
                        true
                    } else if *j == 2 {
                        false
                    } else if o.hold_stagger_ds && hold_release_at >= 0 && t >= hold_release_at && t < until && ds_first_at.is_none() {
                        // (before cart 0's edge at Ds the middle and rear gates keep the fill rule: shut
                        // while the pocket ahead is latched, so nothing shuts at the release itself)
                        !occ_h[j + 1]
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
        // (merges: two carts' centres within 0.5 of each other (one cart pushed onto another's spot);
        // each merge's start, place and the ticks until the centres are 0.9 apart again)
        for a in 0..o.carts {
            for b in (a + 1)..o.carts {
                let d = match (now[a], now[b]) {
                    (Some((xa, ya, _)), Some((xb, yb, _))) => Some((((xa - xb).powi(2) + (ya - yb).powi(2)).sqrt(), xa, ya)),
                    _ => None,
                };
                match (d, merge_open.get(&(a, b)).copied()) {
                    (Some((dd, xa, ya)), None) if dd < 0.5 => {
                        merge_open.insert((a, b), (t, xa, ya));
                        merged_pairs.insert((a, b));
                    }
                    (Some((dd, xs, _)), Some((t0, xa, ya))) if dd > 0.9 => {
                        merge_open.remove(&(a, b));
                        merge_log.push(format!("t {t0}: carts {a} ({}) and {b} ({}) at {xa:.2},{ya:.2}, separated after {} gt at x {xs:.2}{}", if ridden_k(a) { "R" } else { "E" }, if ridden_k(b) { "R" } else { "E" }, t - t0, if xs < g.gp_plane { " (before the hold)" } else { " (IN OR PAST THE ESCAPEMENT)" }));
                    }
                    (None, Some((t0, xa, ya))) => {
                        merge_open.remove(&(a, b));
                        merge_log.push(format!("t {t0}: carts {a} and {b} at {xa:.2},{ya:.2}, one left the bench merged after {} gt", t - t0));
                    }
                    _ => {}
                }
            }
        }
        if t >= warm + 100 && in_run_now < run_min.0 {
            run_min = (in_run_now, t);
        }
        // (blow-ups and carts off the track: the highest stored speed of any cart at any tick; a cart
        // below the bench's lowest rail by 2 or outside its x span by 1)
        {
            let face0 = g.gates.iter().find(|x| x.0 == Role::Tail(0)).map_or(0.0, |x| x.1 as f64 + 0.375);
            for (t0, k1, k2) in merged_in.iter().copied() {
                for k in [k1, k2] {
                    if t - t0 > 400 {
                        continue;
                    }
                    if let Some((x, _, _)) = now[k] {
                        let e = merged_track.entry(k).or_insert((false, x, f64::INFINITY, 0));
                        if !e.0 && x > face0 + 0.6 {
                            e.0 = true;
                        }
                        if e.0 {
                            if x > e.1 && e.2.is_infinite() {
                                e.1 = x;
                            } else if x < e.1 - 0.05 && x < e.2 {
                                e.2 = x;
                                e.3 = t;
                            }
                        }
                    }
                }
            }
        }
        // (blow-ups and carts off the track: the highest stored speed of any cart at any tick; a cart
        // below the bench's lowest rail by 2 or outside its x span by 1)
        for k in 0..o.carts {
            if let Some((x, y, v)) = now[k] {
                if v > vmax_any.0 {
                    vmax_any = (v, t, k);
                }
                if (y < rail_y_min - 2.0 || x < rail_x_span.0 - 1.0 || x > rail_x_span.1 + 1.0) && off_track.len() < 20 && !off_track.iter().any(|(_, kk, _, _)| *kk == k) {
                    off_track.push((t, k, x, y));
                }
            }
        }
        // (the front cart at G0's face: its stored speed at rest; any cart in the run moving back up
        // it: ironspec 18:27, the empties' tolerance)
        let g0_face = g.g0_plane - 0.125;
        for k in 0..o.carts {
            if let (Some((x, _, v)), Some((px, _, _))) = (now[k], prev[k]) {
                if (x + 0.49 - g0_face).abs() < 0.05 && (x - px).abs() < 0.001 {
                    front_v_max = front_v_max.max(v);
                }
                if x >= g.run_x.0 && x < g.run_x.1 && x - px < -0.05 && t >= warm {
                    back_moves += 1;
                    back_max = back_max.max(px - x);
                }
            }
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
    for ((a, b), (t0, xa, ya)) in &merge_open {
        merge_log.push(format!("t {t0}: carts {a} and {b} at {xa:.2},{ya:.2}, STILL MERGED at the end"));
    }
    let _ = &merged_pairs;
    l.push(format!("the front cart at G0's face: the highest stored speed at rest {front_v_max:.3}; backward moves in the run (a cart moving back up it by 0.05 or more in a tick) {back_moves}, the largest {back_max:.3} b/t"));
    if !reset_log.is_empty() {
        l.push(format!("the jam's reset: {reset_log}"));
    }
    l.push(format!("merges (two carts' centres within 0.5; separated when 0.9 apart): {}{}", merge_log.len(), if merge_log.is_empty() { String::new() } else { format!(" ({})", merge_log.iter().take(if o.cycles > 200 { 400 } else { 12 }).cloned().collect::<Vec<_>>().join("; ")) }));
    l.push(format!(
        "stored speeds: the highest of any cart at any tick {:.3} (t {}, cart {}){}; carts off the track (below the lowest rail by 2 or outside the bench by 1): {}",
        vmax_any.0, vmax_any.1, vmax_any.2,
        if vmax_any.0 > 2.0 { " BLOW-UP" } else { "" },
        if off_track.is_empty() { "0".to_string() } else { off_track.iter().map(|(t, k, x, y)| format!("t {t} cart {k} at {x:.1},{y:.1}")).collect::<Vec<_>>().join("; ") }
    ));
    // (--merged-intake: each merged pair's release: its lowest point, its bounce back up (the highest
    // point after it), the gap left to the staging cell's cart face, and every contact within 300 gt
    // of it; contacts into a ridden cart above 0.46)
    if !merged_in.is_empty() {
        let face0 = g.gates.iter().find(|x| x.0 == Role::Tail(0)).map_or(0.0, |x| x.1 as f64 + 0.375);
        for (t0, k1, k2) in &merged_in {
            let tr = |k: &usize| merged_track.get(k).map_or("-".to_string(), |(rel, lo, hi, th)| if !rel { "never released".to_string() } else if hi.is_infinite() { format!("lowest x {lo:.2}, no bounce") } else { format!("lowest x {lo:.2}, bounced back up {:.2} in x ({:.2} along the slope) to x {hi:.2} at t {th}, {:.2} below the gate's face", lo - hi, (lo - hi) * std::f64::consts::SQRT_2, hi - face0) });
            let cs: Vec<&Contact> = contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.t >= *t0 && c.t <= t0 + 300).collect();
            let into_r: Vec<&&Contact> = cs.iter().filter(|c| (ridden_k(c.j) || replaced[c.j]) && c.pv > 0.46).collect();
            l.push(format!(
                "merged empty pair (carts {k1}, {k2}) placed at the staging cell at t {t0}: cart {k1}: {}; cart {k2}: {}; contacts within 300 gt: {} (hardest {:.3}); into a ridden cart above 0.46: {}{}",
                tr(k1), tr(k2), cs.len(), cs.iter().map(|c| c.pv).fold(0.0f64, f64::max),
                into_r.len(),
                if into_r.is_empty() { String::new() } else { format!(" ({})", into_r.iter().take(6).map(|c| format!("t {} cart {} into {} at {:.3}", c.t, c.k, c.j, c.pv)).collect::<Vec<_>>().join(", ")) }
            ));
        }
    }
    // (every ridden intake striking an empty back (R onto E): count and speeds in 0.05 bins)
    {
        let re: Vec<f64> = contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.intake.is_some() && c.t >= warm && ridden_k(c.k) && !ridden_k(c.j)).map(|c| c.pv).collect();
        let mut bins: std::collections::BTreeMap<i64, usize> = Default::default();
        for v in &re {
            *bins.entry((v / 0.05).floor() as i64).or_default() += 1;
        }
        l.push(format!(
            "R onto E intake contacts (a ridden intake striking the empty at the back): {}{}",
            re.len(),
            if re.is_empty() { String::new() } else { format!(", {:.3}..{:.3}; by 0.05: {}", re.iter().copied().fold(f64::INFINITY, f64::min), re.iter().copied().fold(0.0f64, f64::max), bins.iter().map(|(b, n)| format!("{:.2}-{:.2}: {n}", *b as f64 * 0.05, (*b + 1) as f64 * 0.05)).collect::<Vec<_>>().join(", ")) }
        ));
    }
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
    if o.start_fill > 0 {
        // (the start state's fill: every contact before the fill completed, by its struck cart's
        // seat and its stored speed; and the fill's end)
        let fill_end = if fill_done_at >= 0 { fill_done_at } else { i64::MAX };
        let fc: Vec<&Contact> = contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.t < fill_end).collect();
        let mx = fc.iter().map(|c| c.pv).fold(0.0f64, f64::max);
        l.push(format!(
            "the start fill ({} carts {}): done at {}; {} contacts, {} above 0.2, {} above 0.46, the hardest {mx:.3}; each (mover, struck as seated at the end): {}",
            o.start_fill,
            if o.gentle { if o.gentle_no_seat { "placed one at a time at the back, never seated (the control)" } else { "placed one at a time at the back, each seated before the next" } } else { "admitted one at a time from the staging cell, each once the run is still" },
            if fill_done_at >= 0 { format!("t {fill_done_at}") } else { "never".to_string() },
            fc.len(),
            fc.iter().filter(|c| c.pv > 0.2).count(),
            fc.iter().filter(|c| c.pv > 0.46).count(),
            fc.iter().take(18).map(|c| format!("t {} {} {:.3}", c.t, c.k, c.pv)).collect::<Vec<_>>().join(", ")
        ));
        if !gentle_log.is_empty() {
            l.push(format!("the placements: {}", gentle_log.iter().take(16).cloned().collect::<Vec<_>>().join("; ")));
        }
    }
    l.push(format!(
        "intake travel (x from the rest at the last gate to the contact) and stored speed at contact: {}",
        {
            let rest_x = last_tail_face - 0.49;
            let v: Vec<String> = contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.intake.is_some() && c.t >= warm).take(10).map(|c| format!("{:.2}->{:.3}", c.x - rest_x, c.pv)).collect();
            if v.is_empty() { "-".to_string() } else { v.join(", ") }
        }
    ));
    // (the intake contacts by pair type, ironspec 18:11: mover ridden or empty, struck ridden or
    // empty: count and range)
    l.push(format!(
        "intake contacts by pair (mover-struck, R ridden / E empty): {}",
        {
            let mut by: std::collections::BTreeMap<String, (usize, f64, f64)> = Default::default();
            for c in contacts_arr.iter().chain(contacts_mov.iter()).filter(|c| c.intake.is_some() && c.t >= warm) {
                let key = format!("{}{}", if ridden_k(c.k) { "R" } else { "E" }, if ridden_k(c.j) { "R" } else { "E" });
                let e = by.entry(key).or_insert((0, f64::INFINITY, 0.0));
                e.0 += 1;
                e.1 = e.1.min(c.pv);
                e.2 = e.2.max(c.pv);
            }
            if by.is_empty() { "-".to_string() } else { by.iter().map(|(k, (n, a, b))| format!("{k} {n}: {a:.3}..{b:.3}")).collect::<Vec<_>>().join(", ") }
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
        // (the consequence row, ironspec: a golem event's release refused at T+3 with ALIVE low,
        // during a freeze's latency-free tail or the post-thaw window before the ring's next edge)
        let lost: Vec<String> = gate.refused.iter().filter(|(_, w)| w == "cycle +3").map(|(t, _)| format!("t {t}")).collect();
        l.push(format!("releases lost (a golem event's T+3 refused with ALIVE low; a consequence row): {}{}", lost.len(), if lost.is_empty() { String::new() } else { format!(" ({})", lost.join(", ")) }));
        l.extend(gl);
    }
    Ok(l.join("\n"))
}
