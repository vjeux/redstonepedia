//! The pocket-module bench (farmopt Nov 11 16:26: brake-bench's pocket mode is the shared module
//! bench; ironspec's latch ruling 16:19, conditions 1-6). A straight line built in code: a descent,
//! then N pockets of 9fc3155f's 3-cell module, going down the track along +x:
//! - P (sensor): a flat detector rail at y h (sees every cart entering, empties included; diagonal
//!   to the previous pocket's gate, no shared face with any gate);
//! - R (rest): a plain slope, base h - 1, descending toward the gate;
//! - G (gate column): a plain slope at base h - 2 with the oak fence gate at y h - 1 above it (the
//!   rest slope's level); the next pocket's P is a flat at h - 2 joining G's low end;
//!
//! and after the front pocket a slot-0 stand-in: Dh (a pass detector), a rest slope against the gate
//! S0, then Ds (a pass detector) and a sink. The latch reads only the P detector rails' real powered
//! state each tick: P_j's rising edge sets pocket j and clears pocket j-1, Dh's clears the front
//! pocket; G_j is shut while pocket j+1 is latched, the entry gate also for `spacer` gt after each
//! rising edge of the second sensor (the admission spacer); the front gate opens for `pulse` gt
//! `lead` gt before each slot-0 beat when the front pocket is latched and slot 0 latched empty; S0
//! opens at each beat. Every gate follows its target `wire` gt late (the wiring's delay).
//! Printed: each cart's gate stops and contacts (stored speed), each sensor's passes (rising-edge
//! gaps, carts entering vs edges: merged passes), every gate edge with a cart's box in the gate's
//! cell, every tick the latch says empty while a cart rests in a pocket, the ripple after each
//! release, and slot 0's acceptance (a cart at rest at least 2 gt before the beat that takes it).

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// One injected fault (ironspec Nov 11 16:19, condition 4).
#[derive(Clone, Debug)]
pub enum PocketFault {
    /// The latch ignores sensor S's N-th rising edge (0-based).
    Missed { sensor: usize, nth: usize },
    /// An extra rising edge on sensor S 3 gt after its N-th.
    Double { sensor: usize, nth: usize },
    /// Every cart frozen (position and motion kept) from tick T for D gt; the latch and gates run.
    Freeze { at: i64, dur: i64 },
    /// A rising edge on sensor S at tick T with no cart.
    Ghost { sensor: usize, at: i64 },
}

/// `missed:S:N`, `double:S:N`, `freeze:T:D` or `ghost:S:T` (sensors 0..N-1 the pockets' P from the
/// entry, N = Dh, N+1 = Ds).
pub fn parse_fault(s: &str) -> Result<PocketFault, String> {
    let p: Vec<&str> = s.split(':').collect();
    let n = |i: usize| -> Result<i64, String> { p.get(i).ok_or(format!("fault {s}: a field is missing"))?.parse::<i64>().map_err(|e| format!("fault {s}: {e}")) };
    match p[0] {
        "missed" => Ok(PocketFault::Missed { sensor: n(1)? as usize, nth: n(2)? as usize }),
        "double" => Ok(PocketFault::Double { sensor: n(1)? as usize, nth: n(2)? as usize }),
        "freeze" => Ok(PocketFault::Freeze { at: n(1)?, dur: n(2)? }),
        "ghost" => Ok(PocketFault::Ghost { sensor: n(1)? as usize, at: n(2)? }),
        _ => Err(format!("fault {s}: one of missed:S:N, double:S:N, freeze:T:D, ghost:S:T")),
    }
}

/// The bench's options.
#[derive(Clone, Debug)]
pub struct PocketOpts {
    pub pockets: usize,
    pub speeds: Vec<f64>,
    pub riders: Vec<bool>,
    /// The launch spacing: cart k is launched gaps[k-1] gt after cart k-1 (the last repeats).
    pub gaps: Vec<i64>,
    pub descent: i32,
    pub ticks: i64,
    pub beat: i64,
    pub beat_at: i64,
    pub corner: i64,
    pub lead: i64,
    pub pulse: i64,
    pub wire: i64,
    pub spacer: i64,
    /// Cells per pocket: 3 (P, R, G) or 5 (P, R, G, B a flat powered rail always on over a
    /// redstone block, S a flat plain rail; farmopt 16:33, 9fc3155f's ~12-gt hand-off).
    pub cells: usize,
    /// The release: every pocket gate opens together for the pulse and the latch resets (farmopt
    /// 16:33), else front-first (the front gate, then each gate as the pocket ahead empties).
    pub together: bool,
    /// The normally-shut entry: G_0 opens only to admit (pocket 0 latched, pocket 1 free) and
    /// shuts at P_1's rising edge or after this many gt; 0 = off.
    pub entry_admit: i64,
    /// Feed-gated launches: a cart launches only while pocket 0 is latched free (the hold's feed
    /// driven by its entry pocket), and the together release leaves pocket 0's latch to P_1's edge.
    pub feed_gated: bool,
    pub faults: Vec<PocketFault>,
}

/// The cells.
struct Geo {
    /// Per pocket: P (x, h), the gate cell (x + 2, h - 1).
    p: Vec<(i32, i32)>,
    gate: Vec<(i32, i32)>,
    dh: (i32, i32),
    s0: (i32, i32),
    ds: (i32, i32),
    sink_x: f64,
}

fn build(o: &PocketOpts) -> Result<(FarmSpec, Geo), String> {
    let glass = Block::parse("minecraft:glass")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let det = Block::parse("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    let gate = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let booster = Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let pitch = if o.cells >= 5 { 5 } else { 3 };
    let d = o.descent.max(1);
    let mut blocks: Vec<(BlockPos, Block)> = vec![(BlockPos::new(-1, d - 1, 0), glass.clone())];
    let mut put = |x: i32, y: i32, b: &Block| {
        blocks.push((BlockPos::new(x, y, 0), b.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
    };
    for x in 0..d {
        put(x, d - 1 - x, &slope);
    }
    let mut g = Geo { p: Vec::new(), gate: Vec::new(), dh: (0, 0), s0: (0, 0), ds: (0, 0), sink_x: 0.0 };
    let mut gates: Vec<(i32, i32)> = Vec::new();
    let mut under: Vec<(BlockPos, Block)> = Vec::new();
    for j in 0..o.pockets as i32 {
        let (x, h) = (d + pitch * j, -2 * j);
        put(x, h, &det);
        put(x + 1, h - 1, &slope);
        put(x + 2, h - 2, &slope);
        if pitch == 5 {
            // (B: a flat powered rail always on over a redstone block, diagonal to the gate block;
            // S: a flat plain rail, so B shares no face with the next P)
            put(x + 3, h - 2, &booster);
            under.push((BlockPos::new(x + 3, h - 3, 0), rblock.clone()));
            put(x + 4, h - 2, &flat);
        }
        g.p.push((x, h));
        g.gate.push((x + 2, h - 1));
        gates.push((x + 2, h - 1));
    }
    let (xs, hs) = (d + pitch * o.pockets as i32, -2 * o.pockets as i32);
    put(xs, hs, &det);
    put(xs + 1, hs - 1, &slope);
    put(xs + 2, hs - 2, &slope);
    put(xs + 3, hs - 2, &det);
    for k in 4..12 {
        put(xs + k, hs - 2, &flat);
    }
    blocks.extend(under);
    g.dh = (xs, hs);
    g.s0 = (xs + 2, hs - 1);
    g.ds = (xs + 3, hs - 2);
    g.sink_x = (xs + 6) as f64;
    gates.push(g.s0);
    for (x, y) in gates {
        blocks.push((BlockPos::new(x, y, 0), gate.clone()));
    }
    let spec = FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(xs + 14, 1, 3),
        lo: BlockPos::new(-4, hs - 8, -4),
        hi: BlockPos::new(xs + 16, d + 4, 4),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok((spec, g))
}

/// One cart's record.
#[derive(Clone, Debug, Default)]
pub struct PocketCart {
    pub launched: i64,
    pub ridden: bool,
    pub v0: f64,
    /// (gate index (pockets, then S0), the stored speed the tick before it stopped there, tick).
    pub stops: Vec<(usize, f64, i64)>,
    /// (stored speed, tick, x, the cart ahead).
    pub contacts: Vec<(f64, i64, f64, usize)>,
    /// Slot 0: the tick it came to rest there, the beat that took it.
    pub s0_rest: Option<i64>,
    pub s0_beat: Option<i64>,
    pub sunk: Option<i64>,
    /// The tick its centre crossed the front gate's plane.
    pub front_cross: Option<i64>,
}

/// The results.
#[derive(Debug, Default)]
pub struct PocketReport {
    pub carts: Vec<PocketCart>,
    /// Per sensor: rising-edge ticks (as read), carts entering its detection box.
    pub rises: Vec<Vec<i64>>,
    pub entries: Vec<usize>,
    pub gate_edges_on_cart: Vec<String>,
    /// the normally-shut entry's admissions, and the chain's most carts at once (count, tick)
    pub admits: usize,
    pub max_chain: (usize, i64),
    pub disagreements: Vec<String>,
    pub ripples: Vec<(i64, Option<i64>)>,
    pub releases: usize,
    pub text: String,
}

fn cart_state(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64)> {
    let i = redsim::entity::index_of(w, id)?;
    let e = &w.entities.list[i];
    Some((e.pos.x, e.pos.y, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()))
}

/// Runs the bench.
pub fn pocket_bench(o: &PocketOpts) -> Result<PocketReport, String> {
    if o.pockets < 2 {
        return Err("the pocket bench needs 2 pockets or more (the spacer keys on the second sensor)".into());
    }
    let (spec, g) = build(o)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let n = o.pockets;
    let sensors: Vec<(i32, i32)> = g.p.iter().copied().chain([g.dh, g.ds]).collect();
    let gate_cells: Vec<(i32, i32)> = g.gate.iter().copied().chain([g.s0]).collect();
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    // (launch ticks)
    let mut launch: Vec<i64> = Vec::new();
    let mut t0 = 0i64;
    for k in 0..o.speeds.len() {
        if k > 0 {
            t0 += *o.gaps.get(k - 1).or(o.gaps.last()).unwrap_or(&24);
        }
        launch.push(t0);
    }
    let ridden_k = |k: usize| o.riders.get(k).or(o.riders.last()).copied().unwrap_or(true);
    let mut rep = PocketReport { rises: vec![Vec::new(); sensors.len()], entries: vec![0; sensors.len()], ..Default::default() };
    rep.carts = (0..o.speeds.len()).map(|k| PocketCart { launched: launch[k], ridden: ridden_k(k), v0: o.speeds[k], ..Default::default() }).collect();
    let mut ids: Vec<Option<u32>> = vec![None; o.speeds.len()];
    let mut prev: Vec<Option<(f64, f64, f64)>> = vec![None; o.speeds.len()];
    let mut was_on: Vec<bool> = vec![false; sensors.len()];
    let mut inside: Vec<std::collections::BTreeSet<usize>> = vec![Default::default(); sensors.len()];
    let mut seen_rises: Vec<usize> = vec![0; sensors.len()];
    let mut occ: Vec<bool> = vec![false; n];
    let mut s0_occ = false;
    let mut last_rise_p1: i64 = i64::MIN / 2;
    let mut front_open_until: i64 = -1;
    let mut all_open_until: i64 = -1;
    // (the normally-shut entry: G_0 open until this tick; admissions; the chain's most carts)
    let mut admit_until: i64 = -1;
    let mut admits = 0usize;
    let mut max_chain = (0usize, 0i64);
    let mut feed_in_flight = false;
    // (each cart's tick crossing the front gate's plane: the hand-off spacing per pair)
    let front_plane = g.gate[n - 1].0 as f64 + 0.5;
    let mut s0_open_until: i64 = -1;
    // (each gate's target history, applied `wire` gt late; the state as set)
    let mut target_hist: Vec<std::collections::VecDeque<bool>> = vec![std::collections::VecDeque::new(); gate_cells.len()];
    let mut gate_open: Vec<bool> = vec![true; gate_cells.len()];
    let mut frozen: Option<(i64, Vec<(u32, [f64; 3], [f64; 3])>)> = None;
    let mut ripple_from: Option<i64> = None;
    let mut ghost_edges: Vec<(usize, i64)> = o.faults.iter().filter_map(|f| if let PocketFault::Ghost { sensor, at } = f { Some((*sensor, *at)) } else { None }).collect();
    let mut extra_edges: Vec<(usize, i64)> = Vec::new();
    for t in 0..o.ticks {
        // (launches; `feed_gated`: a cart launches only while pocket 0 is latched free and no
        // launched cart is still short of P_0 (the hold's feed, the pressed run's release, driven
        // by pocket 0 free), the next one no sooner than its gap after this one)
        for k in 0..o.speeds.len() {
            if o.feed_gated && launch[k] == t && ids[k].is_none() && (occ[0] || feed_in_flight) {
                launch[k] = t + 1;
                for j in k + 1..o.speeds.len() {
                    launch[j] = launch[j].max(launch[j - 1] + *o.gaps.get(j - 1).or(o.gaps.last()).unwrap_or(&24));
                }
                continue;
            }
            if launch[k] == t {
                if o.feed_gated {
                    feed_in_flight = true;
                    rep.carts[k].launched = t;
                    for j in k + 1..o.speeds.len() {
                        launch[j] = launch[j].max(t + *o.gaps.get(j - 1).or(o.gaps.last()).unwrap_or(&24));
                    }
                }
                let rider = if ridden_k(k) { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
                let nbt = format!("{{Motion:[{:.4}d,0.0d,0.0d]{rider}}}", o.speeds[k]);
                let y = (o.descent.max(1) - 1) as f64 + 0.9 + 0.0625;
                ids[k] = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [0.1, y, 0.5], Some(&nbt))?);
            }
        }
        // (the gates as their targets were `wire` gt ago)
        for (gi, (gx, gy)) in gate_cells.iter().enumerate() {
            let want = if target_hist[gi].len() > o.wire as usize { target_hist[gi].pop_front().unwrap() } else { gate_open[gi] };
            if want != gate_open[gi] {
                gate_open[gi] = want;
                run.world.set_block(BlockPos::new(*gx, *gy, 0), if want { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
                for (k, id) in ids.iter().enumerate() {
                    if let Some((x, y, _)) = id.and_then(|id| cart_state(&run.world, id)) {
                        if x + 0.49 > *gx as f64 && x - 0.49 < (*gx + 1) as f64 && y + 0.7 > *gy as f64 && y < (*gy + 1) as f64 {
                            rep.gate_edges_on_cart.push(format!("t {t}: {} {} with cart {k}'s box in its cell (cart at {x:.2},{y:.2})", if gi < n { format!("G{gi}") } else { "S0".into() }, if want { "opened" } else { "shut" }));
                        }
                    }
                }
            }
        }
        // (the tick; a freeze keeps every cart where it was, its motion too)
        let freeze_now = o.faults.iter().any(|f| matches!(f, PocketFault::Freeze { at, dur } if t >= *at && t < at + dur));
        if freeze_now && frozen.is_none() {
            let snap: Vec<(u32, [f64; 3], [f64; 3])> = run.world.entities.list.iter().map(|e| (e.id, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z])).collect();
            frozen = Some((t, snap));
        }
        if !freeze_now {
            frozen = None;
        }
        run.tick();
        if let Some((_, snap)) = &frozen {
            for (id, p, v) in snap {
                if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                    let e = &mut run.world.entities.list[i];
                    e.pos = redsim::entity::Vec3::new(p[0], p[1], p[2]);
                    e.delta_movement = redsim::entity::Vec3::new(v[0], v[1], v[2]);
                }
            }
        }
        // (the sink: a cart past it is gone, its rider too)
        for k in 0..ids.len() {
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
                rep.carts[k].sunk = Some(t);
            }
        }
        let now: Vec<Option<(f64, f64, f64)>> = ids.iter().map(|id| id.and_then(|id| cart_state(&run.world, id))).collect();
        // (the chain's carts: between the first pocket's sensor and the front gate)
        {
            let (x0, x1) = (g.p[0].0 as f64, g.gate[n - 1].0 as f64 + 1.0);
            let k = now.iter().filter(|c| c.is_some_and(|(x, _, _)| x >= x0 && x < x1)).count();
            if k > max_chain.0 {
                max_chain = (k, t);
            }
        }
        // (the sensors: the real detector rails' powered state; entries into each detection box)
        let mut edges: Vec<(usize, i64)> = Vec::new();
        for (si, (sx, sy)) in sensors.iter().enumerate() {
            let on = matches!(run.world.get(BlockPos::new(*sx, *sy, 0)), Block::DetectorRail { powered: true, .. });
            if on && !was_on[si] {
                rep.rises[si].push(t);
                let nth = seen_rises[si];
                seen_rises[si] += 1;
                let missed = o.faults.iter().any(|f| matches!(f, PocketFault::Missed { sensor, nth: m } if *sensor == si && *m == nth));
                if !missed {
                    edges.push((si, t));
                }
                if o.faults.iter().any(|f| matches!(f, PocketFault::Double { sensor, nth: m } if *sensor == si && *m == nth)) {
                    extra_edges.push((si, t + 3));
                }
            }
            was_on[si] = on;
            for (k, c) in now.iter().enumerate() {
                let inbox = c.is_some_and(|(x, y, _)| x + 0.49 > *sx as f64 + 0.2 && x - 0.49 < *sx as f64 + 0.8 && y < *sy as f64 + 0.8 && y + 0.7 > *sy as f64);
                if inbox && inside[si].insert(k) {
                    rep.entries[si] += 1;
                } else if !inbox {
                    inside[si].remove(&k);
                }
            }
        }
        edges.extend(extra_edges.iter().filter(|e| e.1 == t).copied());
        edges.extend(ghost_edges.iter().filter(|e| e.1 == t).copied());
        ghost_edges.retain(|e| e.1 != t);
        // (the latch: P_j's edge sets j and clears j-1; Dh's clears the front pocket and sets slot 0;
        // Ds's clears slot 0)
        for (si, _) in &edges {
            let si = *si;
            if si < n {
                occ[si] = true;
                if si == 0 {
                    feed_in_flight = false;
                }
                if si > 0 {
                    occ[si - 1] = false;
                }
                if si == 1 {
                    last_rise_p1 = t;
                    // (the admitted cart is in pocket 1: the entry shuts)
                    if o.entry_admit > 0 && admit_until > t {
                        admit_until = t;
                    }
                }
            } else if si == n {
                occ[n - 1] = false;
                s0_occ = true;
            } else {
                s0_occ = false;
            }
        }
        // (the beats: slot 0's at beat_at + corner + k beat; the front release `lead` gt before; in
        // the together release every pocket gate opens for the pulse and the latch resets)
        let ph = (t - o.beat_at - o.corner).rem_euclid(o.beat);
        if ph == (o.beat - o.lead).rem_euclid(o.beat) && occ[n - 1] && !s0_occ {
            front_open_until = t + o.pulse;
            if o.together {
                all_open_until = t + o.pulse;
                // (`feed_gated`: pocket 0's latch is left to P_1's edge, so a cart still resting
                // in the entry pocket stays known)
                let from = if o.feed_gated { 1 } else { 0 };
                occ.iter_mut().skip(from).for_each(|b| *b = false);
            }
            rep.releases += 1;
            ripple_from = Some(t);
        }
        if ph == 0 && t >= o.beat_at + o.corner {
            // (the cart resting in slot 0 at the beat, if any)
            for (k, c) in now.iter().enumerate() {
                if c.is_some_and(|(x, _, _)| x > g.dh.0 as f64 && x < g.s0.0 as f64 + 0.5) && rep.carts[k].s0_beat.is_none() {
                    rep.carts[k].s0_beat = Some(t);
                }
            }
            if s0_occ {
                s0_open_until = t + o.pulse;
            }
        }
        // (the normally-shut entry: admit when pocket 0 holds a cart and pocket 1 is free, outside
        // the spacer after P_1's last edge)
        if o.entry_admit > 0 && occ[0] && !occ[1] && t >= admit_until && t - last_rise_p1 >= o.spacer && t >= all_open_until {
            admit_until = t + o.entry_admit;
            admits += 1;
        }
        // (the targets)
        for gi in 0..gate_cells.len() {
            let open = if gi == n {
                t < s0_open_until
            } else if gi == n - 1 {
                t < front_open_until
            } else if t < all_open_until {
                true
            } else if gi == 0 && o.entry_admit > 0 {
                t < admit_until
            } else {
                let mut sh = occ[gi + 1];
                if gi == 0 && o.spacer > 0 && t - last_rise_p1 < o.spacer {
                    sh = true;
                }
                !sh
            };
            target_hist[gi].push_back(open);
        }
        // (stops, contacts, slot 0's rests, disagreements, the front plane)
        for k in 0..ids.len() {
            let (Some((x, y, v)), Some((px0, _, pv))) = (now[k], prev[k]) else { continue };
            if px0 < front_plane && x >= front_plane && rep.carts[k].front_cross.is_none() {
                rep.carts[k].front_cross = Some(t);
            }
            if pv > 0.02 && v < 0.005 {
                if let Some(gi) = gate_cells.iter().position(|(gx, _)| (x + 0.49 - (*gx as f64 + 0.375)).abs() < 0.08) {
                    rep.carts[k].stops.push((gi, pv, t));
                }
            }
            if pv - v >= 0.04 {
                if let Some(j) = (0..ids.len()).find(|&j| j != k && now[j].is_some_and(|(xj, _, _)| xj > x && xj - x < 1.3)) {
                    rep.carts[k].contacts.push((pv, t, x, j));
                }
            }
            if x > g.dh.0 as f64 + 1.0 && x < g.s0.0 as f64 && v < 0.005 && rep.carts[k].s0_rest.is_none() {
                rep.carts[k].s0_rest = Some(t);
            }
            for j in 0..n {
                let (px, _) = g.p[j];
                if x > (px + 1) as f64 && x < px as f64 + 2.4 && v < 0.005 && !occ[j] && !freeze_now {
                    rep.disagreements.push(format!("t {t}: pocket {j} latched empty while cart {k} rests in it (x {x:.2}, y {y:.2})"));
                }
            }
        }
        // (the ripple: from a release until every pocket's latch matches its true occupancy and every
        // cart in the chain is at rest)
        if let Some(t_r) = ripple_from {
            let settled = (0..n).all(|j| {
                let (px, _) = g.p[j];
                let truly = now.iter().any(|c| c.is_some_and(|(x, _, _)| x >= px as f64 && x < (px + 3) as f64));
                truly == occ[j]
            }) && now.iter().all(|c| c.is_none_or(|(x, _, v)| x > g.dh.0 as f64 || v < 0.005));
            if settled && t > t_r + 2 {
                rep.ripples.push((t_r, Some(t - t_r)));
                ripple_from = None;
            } else if t - t_r > o.beat * 3 {
                rep.ripples.push((t_r, None));
                ripple_from = None;
            }
        }
        prev = now;
    }
    rep.admits = admits;
    rep.max_chain = max_chain;
    rep.text = report_text(o, &g, &rep);
    Ok(rep)
}

fn report_text(o: &PocketOpts, g: &Geo, rep: &PocketReport) -> String {
    let n = o.pockets;
    let mut l = vec![format!(
        "pocket bench: {n} pockets ({}), a {}-slope descent, {} carts ({}), launched {}, slot 0's beat every {} gt from {} (corner {:+}), the front release {} gt before it for {} gt ({}), wiring {} gt, spacer {} gt{}",
        if o.cells >= 5 { "9fc3155f's 5-cell module: P detector flat, R rest slope, G gate column, B booster flat on a redstone block, S flat" } else { "9fc3155f's 3-cell module: P detector flat, R rest slope, G gate column" },
        o.descent.max(1),
        o.speeds.len(),
        rep.carts.iter().map(|c| if c.ridden { "R" } else { "E" }).collect::<String>(),
        rep.carts.iter().map(|c| format!("{}@{:.2}", c.launched, c.v0)).collect::<Vec<_>>().join(" "),
        o.beat,
        o.beat_at,
        o.corner,
        o.lead,
        o.pulse,
        if o.together { "every pocket gate together, the latch reset" } else { "front-first" },
        o.wire,
        o.spacer,
        if o.faults.is_empty() { String::new() } else { format!(", faults {:?}", o.faults) }
    )];
    let mut contacts_above = 0;
    let mut accept_short = 0;
    for (k, c) in rep.carts.iter().enumerate() {
        let gname = |gi: usize| if gi < n { format!("G{gi}") } else { "S0".into() };
        let worst = c.contacts.iter().map(|x| x.0).fold(0.0f64, f64::max);
        if worst > 0.2 {
            contacts_above += 1;
        }
        let acc = match (c.s0_rest, c.s0_beat) {
            (Some(r), Some(b)) => {
                if b - r < 2 {
                    accept_short += 1;
                }
                format!("at rest in slot 0 at t {r}, {} gt before its beat (t {b}){}", b - r, if b - r < 2 { " -- SHORT of 2" } else { "" })
            }
            (None, Some(b)) => {
                accept_short += 1;
                format!("not at rest in slot 0 at its beat (t {b}) -- FAILS")
            }
            (Some(r), None) => format!("at rest in slot 0 at t {r}, no beat took it in the run"),
            (None, None) => "never reached slot 0".into(),
        };
        l.push(format!(
            "  cart {k} ({}, launched t {} at {:.2}): stops {}; contacts {}; {acc}{}",
            if c.ridden { "ridden" } else { "empty" },
            c.launched,
            c.v0,
            if c.stops.is_empty() { "none".into() } else { c.stops.iter().map(|(gi, v, t)| format!("{} at {v:.3} (t {t})", gname(*gi))).collect::<Vec<_>>().join(", ") },
            if c.contacts.is_empty() { "none".into() } else { c.contacts.iter().map(|(v, t, x, j)| format!("{v:.3} into cart {j} (t {t}, x {x:.2})")).collect::<Vec<_>>().join(", ") },
            c.sunk.map_or(String::new(), |t| format!("; sunk t {t}"))
        ));
    }
    let names: Vec<String> = (0..n).map(|j| format!("P{j}")).chain(["Dh".to_string(), "Ds".to_string()]).collect();
    let mut merged_total = 0usize;
    let mut sens = Vec::new();
    for (si, r) in rep.rises.iter().enumerate() {
        let gaps: Vec<i64> = r.windows(2).map(|w| w[1] - w[0]).collect();
        let merged = rep.entries[si].saturating_sub(r.len());
        merged_total += merged;
        sens.push(format!("{} {} rises / {} carts entering{} (rise gaps {})", names[si], r.len(), rep.entries[si], if merged > 0 { format!(", {merged} MERGED") } else { String::new() }, if gaps.is_empty() { "-".into() } else { gaps.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",") }));
    }
    l.push(format!("sensors (each pass must start at least the pulse + 2 gt after the one before; a cart entering while the rail is still on merges): {}", sens.join("; ")));
    if o.entry_admit > 0 {
        l.push(format!("normally-shut entry (G_0 opens only to admit: pocket 0 latched, pocket 1 free, outside the {}-gt spacer; shut at P_1's edge or after {} gt): {} admissions; the chain's most carts at once {} (t {})", o.spacer, o.entry_admit, rep.admits, rep.max_chain.0, rep.max_chain.1));
    } else {
        l.push(format!("the chain's most carts at once {} (t {})", rep.max_chain.0, rep.max_chain.1));
    }
    l.push(format!("gate edges with a cart's box in the gate's cell (the bar: 0): {}{}", rep.gate_edges_on_cart.len(), if rep.gate_edges_on_cart.is_empty() { String::new() } else { format!(" ({})", rep.gate_edges_on_cart.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
    l.push(format!("the latch empty while a cart rests in its pocket (the bar: 0): {}{}", rep.disagreements.len(), if rep.disagreements.is_empty() { String::new() } else { format!(" (first: {})", rep.disagreements[0]) }));
    let rmax = rep.ripples.iter().filter_map(|r| r.1).max();
    let unsettled = rep.ripples.iter().filter(|r| r.1.is_none()).count();
    l.push(format!("releases {}; the ripple after each (gt until every latch matches its pocket and the chain is at rest; inside the beat of {}): the longest {}{}", rep.releases, o.beat, rmax.map_or("-".into(), |v| v.to_string()), if unsettled > 0 { format!(", {unsettled} never settled") } else { String::new() }));
    let stopped_dead = rep.carts.iter().flat_map(|c| c.stops.iter()).map(|s| s.1).fold(0.0f64, f64::max);
    let mut fc: Vec<(usize, i64)> = rep.carts.iter().enumerate().filter_map(|(k, c)| c.front_cross.map(|t| (k, t))).collect();
    fc.sort_by_key(|x| x.1);
    l.push(format!(
        "the hand-off at the front gate (gt between successive carts crossing its plane): {}",
        if fc.len() < 2 { "-".to_string() } else { fc.windows(2).map(|w| format!("carts {}->{} {}", w[0].0, w[1].0, w[1].1 - w[0].1)).collect::<Vec<_>>().join(", ") }
    ));
    l.push(format!("gate stops: the fastest stored speed stopped dead at a gate {stopped_dead:.3}; cart-to-cart contacts above 0.2: {contacts_above}; slot-0 acceptance short of 2 gt: {accept_short}; merged passes: {merged_total}"));
    let _ = g;
    l.join("\n")
}
