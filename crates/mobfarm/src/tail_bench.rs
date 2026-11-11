//! The re-packing tail's arrival injector (ironspec's decisions, Nov 10 19:21 and 19:45): the
//! escapement tail of a `queue-loop --tail-esc` design benched alone. The design's own blocks,
//! zombies and tail carts; every other cart removed. Ridden carts are spawned on the return run
//! (x 8.5 → west along z 20, the tail's level), group by group, at the times, gaps and speeds a
//! pattern gives. The register's first slots advance at the loop's times, and a cart is removed
//! once it climbs out of slot 2. The bench drives the gate, P and B as `queue-loop-run` does.
//!
//! It checks the escapement's contract: one cart into slot 0 per advance while a cart waits, none
//! twice, in arrival order. Nothing falls, leaves its rail, goes through a shut gate or moves
//! backward on a powered rail. Riders are unhurt and none converts. It also measures the
//! minimum box gap and the position drift of carts at rest in the tail, the per-cart cycle
//! (the gate's opening to P's next cart at rest) and the tail's most carts.

use crate::queue::{loop_layout, LoopOpts, LoopTiming};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// One cycle's arrivals: `n` carts, `gap` gt apart (0: spawned in contact, 1 block apart, all at
/// once; `gaps`, when given, each cart's own gap after the one before, the last repeated), the
/// first at the cycle's `dt` (on the return run at x 8.5), moving at `speed` b/t.
#[derive(Clone, Debug)]
pub struct Batch {
    pub n: usize,
    pub gap: i64,
    pub gaps: Vec<i64>,
    pub dt: i64,
    pub speed: f64,
}

impl Batch {
    /// The i-th cart's delay after the first.
    fn offset(&self, i: usize) -> i64 {
        if self.gaps.is_empty() {
            return i as i64 * self.gap;
        }
        (0..i).map(|k| *self.gaps.get(k).or(self.gaps.last()).unwrap_or(&0)).sum()
    }
}

/// A pattern: space-separated per-cycle entries `N[/GAP][@DT][:SPEED][*REPEAT]` (defaults 3 carts,
/// 12 gt apart, dt 76, 1.2 b/t; the normal stand's batch); GAP may be a comma list, each cart's
/// own (`4/10,4,9`: 51b2aacd's REL-missed group). `0` is a cycle with nothing arriving.
/// Example: `3*5 0 6/3 3*10`, five normal cycles, one empty, one burst of six 3 gt apart, ten normal.
pub fn parse_pattern(s: &str) -> Result<Vec<Batch>, String> {
    let mut out = Vec::new();
    for tok in s.split_whitespace() {
        let (body, rep) = match tok.split_once('*') {
            Some((b, r)) => (b, r.parse::<usize>().map_err(|e| format!("{tok}: repeat {e}"))?),
            None => (tok, 1),
        };
        let mut b = Batch { n: 3, gap: 12, gaps: Vec::new(), dt: 76, speed: 1.2 };
        let mut rest = body;
        let num_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        b.n = rest[..num_end].parse().map_err(|e| format!("{tok}: count {e}"))?;
        rest = &rest[num_end..];
        while !rest.is_empty() {
            let tag = rest.as_bytes()[0] as char;
            let val_end = rest[1..].find(['/', '@', ':']).map_or(rest.len(), |i| i + 1);
            let val = &rest[1..val_end];
            match tag {
                '/' if val.contains(',') => b.gaps = val.split(',').map(|g| g.parse::<i64>().map_err(|e| format!("{tok}: gap {e}"))).collect::<Result<_, _>>()?,
                '/' => b.gap = val.parse().map_err(|e| format!("{tok}: gap {e}"))?,
                '@' => b.dt = val.parse().map_err(|e| format!("{tok}: dt {e}"))?,
                ':' => b.speed = val.parse().map_err(|e| format!("{tok}: speed {e}"))?,
                _ => return Err(format!("{tok}: unknown part {rest}")),
            }
            rest = &rest[val_end..];
        }
        for _ in 0..rep {
            out.push(b.clone());
        }
    }
    Ok(out)
}

/// The bench's placement and acts.
#[derive(Clone, Debug, Default)]
pub struct BenchOpts {
    /// The design placed shifted by (dx, dz): its `chunk:` alignment (loop cell (0, 0) at world
    /// (dx, dz) mod 16), so redsim's chunks are the aligned world's.
    pub shift: (i32, i32),
    /// Carts spawned at the departure curve (x 9.5, y −3, z 0.5, moving +z) instead of on the
    /// return run: they ride the climb, the lip and the return run into the tail.
    pub from_climb: bool,
    /// Partial unloads: (cycle, dt, the chunk box [x0, z0, x1, z1] in the shifted world, N): the
    /// box's forced tickets dropped at that tick and back N ticks later (`redsim::reload`).
    pub unloads: Vec<(i64, i64, [i32; 4], i64)>,
}

/// The design moved by (dx, dz): blocks, block entities, entities, its box.
fn shifted(spec: &FarmSpec, dx: i32, dz: i32) -> FarmSpec {
    let m = |p: BlockPos| BlockPos::new(p.x + dx, p.y, p.z + dz);
    let mut s = spec.clone();
    s.blocks = spec.blocks.iter().map(|(p, b)| (m(*p), b.clone())).collect();
    s.block_entities = spec.block_entities.iter().map(|(p, t)| (m(*p), t.clone())).collect();
    s.entities = spec.entities.iter().map(|(t, p, n)| (t.clone(), [p[0] + dx as f64, p[1], p[2] + dz as f64], n.clone())).collect();
    s.output = m(spec.output);
    s.lo = m(spec.lo);
    s.hi = m(spec.hi);
    s
}

/// Runs the pattern (one entry per cycle, cycle 0 first) and `extra` cycles after it.
pub fn run_tail_bench(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, pattern: &[Batch], extra: i64, seed: i64, trace: bool, bo: &BenchOpts) -> Result<String, String> {
    let lay = loop_layout(o);
    let (dx, dz) = bo.shift;
    let (fx, fz) = (dx as f64, dz as f64);
    let m = |p: BlockPos| BlockPos::new(p.x + dx, p.y, p.z + dz);
    let mc = |c: [i32; 3]| [c[0] + dx, c[1], c[2] + dz];
    let spec = &shifted(spec, dx, dz);
    // (B: the slope escapement's braked B control, or the flat escapement's B cell and its gate)
    let b_ctl: Option<BlockPos> = lay.esc.map(m);
    let gb: Option<BlockPos> = lay.esc_gb.map(m);
    if b_ctl.is_none() && gb.is_none() {
        return Err("not a --tail-esc design (no B)".into());
    }
    let p_ctl = m(lay.esc_p.ok_or("not a --tail-esc design (no P control)")?);
    let (gate, p_rest) = *lay.fifo.last().ok_or("not a --tail-esc design (no gate)")?;
    let (gate, p_rest) = (m(gate), mc(p_rest));
    let cycles = pattern.len() as i64 + extra;
    let ticks = 100 * cycles + 300;
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, ticks as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    // (a partial unload needs the forced chunk box: the design's, as an exact run mirrors it)
    if !bo.unloads.is_empty() {
        run.world.entity_chunks = Some(crate::exact::forced_chunks(spec));
    }
    // (the design's entity lines, as queue-loop-run spawns them: its carts with their villagers,
    // its zombies)
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    // the tail: x 0 from the entry curve (z 20) to slot 0; slot 0's rest cell; B's and P's rail
    // cells (the powered rails a cart could be boosted backward on)
    let s0 = mc(lay.slot_cells[0]);
    let z_entry = 20.0 + fz;
    let z_slot0 = s0[2] as f64;
    let z_kill = mc(lay.slot_cells[2])[2] as f64 + 1.5;
    let in_tail = |p: &redsim::entity::Vec3| p.x > fx - 0.5 && p.x < fx + 1.5 && p.z > z_entry + 0.5 && p.z < z_slot0;
    let in_slot0 = |p: &redsim::entity::Vec3| p.x > fx - 0.5 && p.x < fx + 1.5 && p.z >= z_slot0 && p.z < z_slot0 + 1.0 && p.y > s0[1] as f64 - 0.2 && p.y < s0[1] as f64 + 1.3;
    let b_cell: [i32; 3] = match (b_ctl, lay.esc_flat_b) {
        (Some(b), _) => [b.x, b.y + 1, b.z],
        (None, Some(fb)) => mc(fb),
        _ => [i32::MIN; 3],
    };
    // (the flat escapement's X, the always-on booster after G_B)
    let x_cell: Option<[i32; 3]> = lay.esc_flat_b.map(|fb| mc([fb[0], fb[1] - 1, fb[2] + 1]));
    let p_cell = [p_ctl.x, p_ctl.y + 1, p_ctl.z];
    let on_cell = |p: &redsim::entity::Vec3, c: [i32; 3]| p.x.floor() as i32 == c[0] && p.z.floor() as i32 == c[2] && p.y >= c[1] as f64 - 0.1 && p.y < c[1] as f64 + 1.2;
    let ye = crate::queue::esc_ye(o);
    // every cart: keep the tail's (its initial queue), remove the rest with their riders
    let all: Vec<(u32, redsim::entity::Vec3, Vec<u32>)> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, e.pos, e.passengers.clone())).collect();
    let mut carts: Vec<u32> = Vec::new();
    for (id, p, riders) in all {
        if in_tail(&p) {
            carts.push(id);
        } else {
            redsim::entity::kill(&mut run.world, id)?;
            for r in riders {
                redsim::entity::kill(&mut run.world, r)?;
            }
        }
    }
    // (the initial tail carts in queue order, front first: they arrived before any injected one)
    let pos_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| e.pos);
    carts.sort_by(|a, b| pos_of(&run.world, *b).map(|p| p.z).partial_cmp(&pos_of(&run.world, *a).map(|p| p.z)).unwrap_or(std::cmp::Ordering::Equal));
    let initial = carts.len();
    let rider_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id).and_then(|e| e.passengers.first().copied());
    let mut riders: Vec<Option<u32>> = carts.iter().map(|c| rider_of(&run.world, *c)).collect();
    let set = |w: &mut redsim::world::World, c: BlockPos, p: bool| {
        let b = if p { on.clone() } else { off.clone() };
        if w.get(c) != b {
            w.set_block(c, b, redsim::world::flags::ALL);
        }
    };
    let set_gate = |w: &mut redsim::world::World, open: bool| {
        if let Ok(b) = Block::parse(&format!("minecraft:oak_fence_gate[facing=south,in_wall=false,open={open},powered=false]"))
            && w.get(gate) != b
        {
            w.set_block(gate, b, redsim::world::flags::ALL);
        }
    };
    // (the B door, `esc_door`: both halves set, open with B's pulse)
    let door = lay.esc_door.map(m);
    // (the flat escapement's G_B, open with B's pulse)
    let set_gb = |w: &mut redsim::world::World, open: bool| {
        if let Some(g) = gb
            && let Ok(b) = Block::parse(&format!("minecraft:oak_fence_gate[facing=south,in_wall=false,open={open},powered=false]"))
            && w.get(g) != b
        {
            w.set_block(g, b, redsim::world::flags::ALL);
        }
    };
    let set_door = |w: &mut redsim::world::World, open: bool| {
        if let Some(dp) = door {
            for (half, y) in [("lower", 0), ("upper", 1)] {
                if let Ok(b) = Block::parse(&format!("minecraft:iron_door[facing=south,half={half},hinge=left,open={open},powered=false]")) {
                    let p = BlockPos::new(dp.x, dp.y + y, dp.z);
                    if w.get(p) != b {
                        w.set_block(p, b, redsim::world::flags::ALL);
                    }
                }
            }
        }
    };
    let (d, w_gate) = tm.fifo_intake;
    let (kb, wb) = tm.esc_b;
    let adv = |k: i64| tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
    // the spawn queue: (absolute tick, position, speed); on the return run moving −x, or with
    // `from_climb` at the departure curve moving +z
    let mut spawns: Vec<(i64, [f64; 3], f64)> = Vec::new();
    for (c, b) in pattern.iter().enumerate() {
        for i in 0..b.n {
            let t = c as i64 * 100 + b.dt + if b.gap > 0 || !b.gaps.is_empty() { b.offset(i) } else { 0 };
            // (in contact: the front furthest along, each next one block behind it)
            let back = if b.gap > 0 || !b.gaps.is_empty() { 0.0 } else { (b.n - 1 - i) as f64 };
            let p = if bo.from_climb { [9.5 + fx, -3.0 + 0.0625, 0.5 + fz - back] } else { [8.5 + fx + back, ye as f64 + 0.0625, 20.5 + fz] };
            spawns.push((t, p, b.speed));
        }
    }
    spawns.sort_by_key(|s| s.0);
    let mut next_spawn = 0;
    // (the unloads' acts: (tick, box, load) in time order)
    let mut acts: Vec<(i64, [i32; 4], bool)> = bo.unloads.iter().flat_map(|(c, dt, b, n)| [(c * 100 + dt, *b, false), (c * 100 + dt + n, *b, true)]).collect();
    acts.sort_by_key(|a| (a.0, a.2));
    let mut next_act = 0;
    // the metrics
    let mut log: Vec<String> = Vec::new();
    let mut entered: Vec<usize> = Vec::new(); // carts in the order they first rested in slot 0
    let mut seen0 = vec![false; 0];
    let mut holes_waiting: Vec<String> = Vec::new();
    let mut starved: Vec<String> = Vec::new();
    let mut last_intake: Option<(bool, usize)> = None;
    let mut holes_empty = 0usize;
    let mut doubles: Vec<String> = Vec::new();
    let mut fell: Vec<String> = Vec::new();
    let mut backward: Vec<String> = Vec::new();
    let mut max_back = 0.0f64;
    let mut max_back_braked = 0.0f64;
    let mut back_braked = 0usize;
    let mut rolled: Vec<String> = Vec::new();
    let mut gate_through: Vec<String> = Vec::new();
    let mut min_gap = f64::INFINITY;
    let mut min_gap_at = String::new();
    let mut max_step = 0.0f64;
    let mut max_drift = 0.0f64;
    // (per place: on P, on B, in the queue: the largest one-tick move and quiet-stretch drift)
    let mut creep = [(0.0f64, 0.0f64); 3];
    let mut rest_from: Vec<Option<redsim::entity::Vec3>> = Vec::new();
    let mut last_pos: Vec<Option<redsim::entity::Vec3>> = Vec::new();
    let mut slow: Vec<u32> = Vec::new();
    let mut brake_slow: Vec<u32> = Vec::new();
    let mut stuck_brake: Vec<String> = Vec::new();
    let mut removed_past = 0usize;
    // (`tail_brake`'s entry brake: the never-powered braked slope ahead of the queue)
    let entry_brake: Option<[i32; 3]> = if o.tail_brake { lay.queue_cells.first().copied().map(mc) } else { None };
    let mut max_tail = 0usize;
    let mut reload: Vec<i64> = Vec::new();
    let mut reload_open: Option<i64> = None;
    let mut hurt: Vec<String> = Vec::new();
    let mut tail_rider_ticks = 0usize;
    let mut panic_rider_ticks = 0usize;
    let mut zv_seen = 0usize;
    let mut per_cycle: Vec<String> = Vec::new();
    let t0 = run.world.time;
    for t in 0..ticks {
        let c = t.div_euclid(100);
        let dt = t.rem_euclid(100);
        let live = c < cycles;
        // (spawns due now)
        while next_spawn < spawns.len() && spawns[next_spawn].0 == t {
            let (_, p, v) = spawns[next_spawn];
            let nbt = if bo.from_climb {
                format!("{{Rotation:[0f,0f],Motion:[0d,0d,{v:.4}d],Passengers:[{{id:\"minecraft:villager\"}}]}}")
            } else {
                format!("{{Rotation:[90f,0f],Motion:[{:.4}d,0d,0d],Passengers:[{{id:\"minecraft:villager\"}}]}}", -v)
            };
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", p, Some(&nbt))?;
            carts.push(id);
            riders.push(rider_of(&run.world, id));
            let top = run.world.entities.next_id;
            run.own.extend(id..top);
            next_spawn += 1;
        }
        // (the partial unloads' acts due now, between two ticks)
        while next_act < acts.len() && acts[next_act].0 == t {
            let (_, b, load) = acts[next_act];
            if load {
                redsim::reload::load_some(&mut run.world, b);
            } else {
                redsim::reload::unload_some(&mut run.world, b);
            }
            log.push(format!("act slot {c} +{dt}: {} {b:?}", if load { "load" } else { "unload" }));
            next_act += 1;
        }
        while seen0.len() < carts.len() {
            seen0.push(false);
            rest_from.push(None);
            last_pos.push(None);
            slow.push(0);
            brake_slow.push(0);
        }
        // the register's slots 0..=3: the loop's advance pulses
        for k in 0..3 {
            let a = adv(k);
            let wk = tm.pulse + tm.adv_width[k as usize];
            for s in lay.slots.iter().take(4).map(|s| m(*s)) {
                if live && dt == a {
                    set(&mut run.world, s, true);
                }
                if dt == a + wk {
                    set(&mut run.world, s, false);
                }
            }
            // the escapement: the gate and P from `fifo_intake` D for W gt, B `esc_b` K after the
            // gate opens for its W gt
            if live && dt == a + d {
                set_gate(&mut run.world, true);
                set(&mut run.world, p_ctl, true);
                reload_open = Some(t);
            }
            if dt == a + d + w_gate {
                set_gate(&mut run.world, false);
                set(&mut run.world, p_ctl, false);
            }
            if live && dt == a + d + kb {
                if let Some(b) = b_ctl {
                    set(&mut run.world, b, true);
                }
                set_door(&mut run.world, true);
                set_gb(&mut run.world, true);
            }
            if dt == a + d + kb + wb {
                if let Some(b) = b_ctl {
                    set(&mut run.world, b, false);
                }
                set_door(&mut run.world, false);
                set_gb(&mut run.world, false);
            }
        }
        // at each advance's start: slot 0's carts against what the intake before it had on P
        if live && (0..3).any(|k| dt == adv(k)) {
            let k = (0..3).find(|k| dt == adv(*k)).unwrap_or(0);
            let in0: Vec<usize> = carts.iter().enumerate().filter(|(_, id)| pos_of(&run.world, **id).is_some_and(|p| in_slot0(&p))).map(|(i, _)| i).collect();
            let waiting = carts.iter().filter(|id| pos_of(&run.world, **id).is_some_and(|p| in_tail(&p))).count();
            if in0.len() > 1 {
                doubles.push(format!("slot {c} advance {k}: slot 0 holds carts {in0:?}"));
            }
            if in0.is_empty() {
                // (the intake before: P loaded and its cart lost on the way, or carts waiting in
                // the tail with P empty, or nothing to feed)
                match last_intake {
                    Some((true, _)) => holes_waiting.push(format!("slot {c} advance {k}: P was loaded at the intake before, slot 0 empty")),
                    Some((false, n)) if n > 0 => starved.push(format!("slot {c} advance {k} ({n} waiting, P empty)")),
                    _ => holes_empty += 1,
                }
            }
            for i in &in0 {
                if !seen0[*i] {
                    seen0[*i] = true;
                    entered.push(*i);
                }
            }
            if k == 0 {
                per_cycle.push(format!("{c}:{waiting}"));
            }
            log.push(format!("slot {c} adv {k}: slot0 {:?} tail {waiting}", in0));
        }
        // (at each gate opening: P loaded or not, and how many wait in the tail)
        if live && (0..3).any(|k| dt == adv(k) + d) {
            let p_loaded = carts.iter().any(|id| pos_of(&run.world, *id).is_some_and(|p| on_cell(&p, [p_rest[0], p_rest[1], p_rest[2]])));
            let waiting = carts.iter().filter(|id| pos_of(&run.world, **id).is_some_and(|p| in_tail(&p))).count();
            last_intake = Some((p_loaded, waiting));
        }
        run.tick();
        // per tick: falls, backward motion on B/P/slot 0, the gate, rest gaps and drift, the
        // per-cart cycle, riders
        let gate_open = matches!(run.world.get(gate), Block::FenceGate { open: true, .. });
        let mut resting: Vec<(usize, redsim::entity::Vec3)> = Vec::new();
        let mut tail_n = 0;
        for (i, id) in carts.iter().enumerate() {
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id && !e.removed) else {
                continue;
            };
            let p = e.pos;
            let v = e.delta_movement;
            if p.y < -6.0 {
                if !fell.iter().any(|f| f.starts_with(&format!("cart {i} "))) {
                    fell.push(format!("cart {i} at slot {c} +{dt}: {:.2},{:.2},{:.2}", p.x, p.y, p.z));
                }
                continue;
            }
            // (pushed back down the return: past the return run's east end onto the climb)
            // (rolled back down the climb: on the climb's column moving back toward the stand)
            if p.x > fx + 8.9 && p.x < fx + 10.1 && p.z < fz + 19.5 && v.z < -0.01 && !rolled.iter().any(|f: &String| f.starts_with(&format!("cart {i} "))) {
                rolled.push(format!("cart {i} at slot {c} +{dt}: {:.2},{:.2},{:.2}", p.x, p.y, p.z));
            }
            if in_tail(&p) {
                tail_n += 1;
            }
            // (backward on a powered rail: B, P or slot 0, split by the rail's state: boosted
            // backward while powered, or braked while unpowered; the flat escapement's X is always
            // powered)
            let ctl = if b_ctl.is_some() && on_cell(&p, b_cell) { b_ctl } else if on_cell(&p, p_cell) { Some(p_ctl) } else if on_cell(&p, s0) { lay.slots.first().copied().map(m) } else { None };
            if x_cell.is_some_and(|xc| on_cell(&p, xc)) && v.z < -0.003 {
                max_back = max_back.max(-v.z);
                if backward.len() < 12 {
                    backward.push(format!("cart {i} at slot {c} +{dt}: on X, z {:.2} vz {:.3}", p.z, v.z));
                }
            }
            if let Some(cb) = ctl
                && v.z < -0.003
                && p.x > fx - 0.5
                && p.x < fx + 1.5
            {
                let powered = matches!(run.world.get(cb), Block::RedstoneBlock);
                if powered {
                    max_back = max_back.max(-v.z);
                    if backward.len() < 12 {
                        backward.push(format!("cart {i} at slot {c} +{dt}: z {:.2} vz {:.3}", p.z, v.z));
                    }
                } else {
                    max_back_braked = max_back_braked.max(-v.z);
                    back_braked += 1;
                }
            }
            // (through the gate while shut: the centre crossing the gate cell's middle)
            if let Some(lp) = last_pos[i] {
                let gz = gate.z as f64 + 0.5;
                if !gate_open && lp.z < gz && p.z >= gz && p.x > -0.5 && p.x < 1.5 {
                    gate_through.push(format!("cart {i} at slot {c} +{dt}"));
                }
                // (at rest, the parent's creep check: while the gate is shut and P and B are off
                // (quiet), each tail cart moving under 0.02 b/t: its move this tick and its
                // displacement over the quiet stretch, on P, on B and in the queue apart)
                let quiet = !gate_open && !matches!(run.world.get(p_ctl), Block::RedstoneBlock) && !b_ctl.is_some_and(|b| matches!(run.world.get(b), Block::RedstoneBlock)) && !gb.is_some_and(|g| matches!(run.world.get(g), Block::FenceGate { open: true, .. }));
                if v.horizontal_distance() < 0.02 {
                    slow[i] += 1;
                } else {
                    slow[i] = 0;
                }
                if in_tail(&p) && quiet && slow[i] > 5 {
                    let step = ((p.x - lp.x).powi(2) + (p.y - lp.y).powi(2) + (p.z - lp.z).powi(2)).sqrt();
                    let r0 = *rest_from[i].get_or_insert(lp);
                    let dr = ((p.x - r0.x).powi(2) + (p.y - r0.y).powi(2) + (p.z - r0.z).powi(2)).sqrt();
                    let w = if on_cell(&p, p_cell) { 0 } else if on_cell(&p, b_cell) { 1 } else { 2 };
                    creep[w].0 = creep[w].0.max(step);
                    creep[w].1 = creep[w].1.max(dr);
                    max_step = max_step.max(step);
                    max_drift = max_drift.max(dr);
                    resting.push((i, p));
                } else {
                    rest_from[i] = None;
                    if in_tail(&p) && v.horizontal_distance() < 0.02 {
                        resting.push((i, p));
                    }
                }
            }
            last_pos[i] = Some(p);
            // (past slot 2: out of the bench)
            if p.z > z_kill && p.x > fx - 0.5 && p.x < fx + 1.5 {
                let r = e.passengers.clone();
                redsim::entity::kill(&mut run.world, *id)?;
                for x in r {
                    redsim::entity::kill(&mut run.world, x)?;
                }
                riders[i] = None;
                removed_past += 1;
                continue;
            }
            // (the entry brake's 0.03 trap: a cart under 0.03 b/t on it for 20 gt)
            if let Some(bc) = entry_brake {
                if on_cell(&p, bc) && v.horizontal_distance() < 0.03 {
                    brake_slow[i] += 1;
                    if brake_slow[i] == 20 {
                        stuck_brake.push(format!("cart {i} at slot {c} +{dt}"));
                    }
                } else {
                    brake_slow[i] = 0;
                }
            }
        }
        max_tail = max_tail.max(tail_n);
        for a in 0..resting.len() {
            for b in a + 1..resting.len() {
                let (pa, pb) = (resting[a].1, resting[b].1);
                let gap = ((pa.x - pb.x).abs() - 0.98).max((pa.y - pb.y).abs() - 0.7).max((pa.z - pb.z).abs() - 0.98);
                if gap < min_gap {
                    min_gap = gap;
                    min_gap_at = format!("carts {} and {} at slot {c} +{dt} (z {:.2}, {:.2})", resting[a].0, resting[b].0, pa.z, pb.z);
                }
            }
        }
        // (the per-cart cycle: the gate's opening to P's next cart at rest)
        if let Some(t_open) = reload_open
            && t > t_open + 3
            && resting.iter().any(|(_, p)| on_cell(p, [p_rest[0], p_rest[1], p_rest[2]]))
        {
            reload.push(t - t_open);
            reload_open = None;
        }
        for (i, r) in riders.iter().enumerate() {
            // (panicking riders in the tail: the rider-push case the contact bar is about)
            if let Some(r) = r
                && let Some(ix) = redsim::entity::index_of(&run.world, *r)
                && pos_of(&run.world, carts[i]).is_some_and(|p| in_tail(&p))
            {
                tail_rider_ticks += 1;
                if redsim::entity::mob::ai::brain::has_exact(&run.world, ix) && redsim::entity::mob::ai::brain::ex(&run.world, ix).active == redsim::entity::mob::ai::brain::Act::Panic {
                    panic_rider_ticks += 1;
                }
            }
            if let Some(r) = r
                && let Some(e) = run.world.entities.list.iter().find(|e| e.id == *r)
                && let EntityKind::Mob(m) = &e.kind
                && (m.health < 20.0 || e.removed)
                && !hurt.iter().any(|h| h.starts_with(&format!("rider of cart {i} ")))
            {
                hurt.push(format!("rider of cart {i} at slot {c} +{dt}: health {}{}", m.health, if e.removed { ", removed" } else { "" }));
            }
        }
        zv_seen = zv_seen.max(run.world.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty.def().id == "zombie_villager")).count());
        if trace && live {
            let ps: Vec<String> = carts.iter().enumerate().filter_map(|(i, id)| pos_of(&run.world, *id).filter(|p| p.x > fx - 0.5 && p.x < fx + 1.5 && p.z > z_entry && p.z < z_kill).map(|p| format!("{i}@{:.2},{:.2}", p.y, p.z))).collect();
            log.push(format!("t slot {c} +{dt}: G{} | {}", if gate_open { 'o' } else { '-' }, ps.join(" ")));
        }
    }
    let _ = t0;
    // FIFO: the carts in the order they rested in slot 0 against the arrival order (the initial
    // tail carts first, front first, then the spawns in order)
    let out_of_order: Vec<String> = entered.windows(2).filter(|w| w[1] < w[0]).map(|w| format!("{} before {}", w[0], w[1])).collect();
    let fed = entered.len();
    let spawned = carts.len() - initial;
    // (the hard count: every cart and rider in is out past slot 2, still in the bench, or lost)
    let alive = |id: u32| run.world.entities.list.iter().any(|e| e.id == id && !e.removed);
    let still: usize = carts.iter().filter(|id| alive(**id) && pos_of(&run.world, **id).is_some_and(|p| p.y > -6.0)).count();
    let riders_still: usize = riders.iter().flatten().filter(|r| alive(**r)).count();
    let out_past = removed_past;
    let mut s = String::new();
    s += &format!("tail bench: {} cycles ({} patterned), seed {seed}; gate and P at advance {:+} for {w_gate} gt, B at gate {:+} for {wb} gt\n", cycles, pattern.len(), d, kb);
    s += &format!("carts: {initial} in the tail at the start, {spawned} spawned; {fed} rested in slot 0\n");
    s += &format!("count: {} carts in, {out_past} out past slot 2 with their riders, {still} still in the bench ({riders_still} riders), {} lost\n", initial + spawned, initial + spawned - out_past - still);
    s += &format!("stuck on the entry brake (under 0.03 b/t for 20 gt on it): {}\n", if stuck_brake.is_empty() { "0".to_string() } else { format!("{} ({})", stuck_brake.len(), stuck_brake.join("; ")) });
    s += &format!("lost intakes (P loaded, slot 0 empty at the next advance): {} {}\n", holes_waiting.len(), if holes_waiting.is_empty() { String::new() } else { format!("({})", holes_waiting.iter().take(10).cloned().collect::<Vec<_>>().join("; ")) });
    s += &format!("starved intakes (carts waiting in the tail, P empty): {} {}\n", starved.len(), if starved.is_empty() { String::new() } else { format!("({})", starved.iter().take(10).cloned().collect::<Vec<_>>().join("; ")) });
    s += &format!("holes with nothing waiting (no supply): {holes_empty}\n");
    s += &format!("doubles (two carts in slot 0 at an advance): {} {}\n", doubles.len(), doubles.iter().take(6).cloned().collect::<Vec<_>>().join("; "));
    s += &format!("out of arrival order: {} {}\n", out_of_order.len(), out_of_order.iter().take(10).cloned().collect::<Vec<_>>().join(", "));
    s += &format!("fell or left the rails: {} {}\n", fell.len(), fell.join("; "));
    s += &format!("pushed back down the return (onto the climb): {} {}\n", rolled.len(), rolled.iter().take(6).cloned().collect::<Vec<_>>().join("; "));
    s += &format!("through the shut gate: {} {}\n", gate_through.len(), gate_through.iter().take(6).cloned().collect::<Vec<_>>().join("; "));
    s += &format!("backward on a powered rail (B, P, slot 0) while it is powered: {} ticks, max {:.3} b/t {}; while braked: {back_braked} ticks, max {:.3} b/t\n", backward.len(), max_back, backward.iter().take(4).cloned().collect::<Vec<_>>().join("; "), max_back_braked);
    s += &format!("at rest in the tail: min cart-to-cart box gap {} ({}), max per-tick drift {:.4}, max drift over a rest {:.4}\n", if min_gap.is_finite() { format!("{min_gap:.3}") } else { "none".into() }, min_gap_at, max_step, max_drift);
    s += &format!("creep while quiet (gate shut, P and B off, the cart under 0.02 b/t for 5 gt; one tick / over the stretch): on P {:.4} / {:.4}, on B {:.4} / {:.4}, in the queue {:.4} / {:.4}\n", creep[0].0, creep[0].1, creep[1].0, creep[1].1, creep[2].0, creep[2].1);
    let (rmin, rmax) = (reload.iter().min().copied().unwrap_or(-1), reload.iter().max().copied().unwrap_or(-1));
    s += &format!("per-cart cycle (gate opening to P's next cart at rest): {rmin}..{rmax} gt over {} intakes\n", reload.len());
    s += &format!("the tail's most carts at once: {max_tail}\n");
    s += &format!("villagers: {} hurt or gone, {} zombie villagers {}\n", hurt.len(), zv_seen, hurt.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
    let zombies = run.world.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty.def().id == "zombie")).count();
    s += &format!("riders panicking in the tail: {panic_rider_ticks} of {tail_rider_ticks} rider-ticks ({:.0}%; {zombies} zombies in the scene)\n", 100.0 * panic_rider_ticks as f64 / tail_rider_ticks.max(1) as f64);
    s += &format!("{}\n", per_cycle.join(" | "));
    if trace {
        s += &log.join("\n");
        s += "\n";
    }
    Ok(s)
}
