//! The single-lane carousel's queue + release-3 (brief farm-iron-carousel.md, "The design (vjeux
//! Nov 10 05:58)", part 1): all 21 villagers in carts back to back on one track, a release at the
//! head letting exactly the front 3 go, the rest held still, the 3 rejoining the tail.
//!
//! First the queue physics, as experiments on redsim: a scene (`mctas mobfarm queue --scene ...`)
//! and a run (`queue-run`) that switches the rails' controls on a schedule and logs every cart.
//! A powered rail is a brake unpowered (a cart under 0.03 a tick stops, faster halves each tick)
//! and a booster powered (but a stopped cart on a flat one starts only off a solid block at its
//! rail's end); carts within 0.2 of each other push each other (`pushOtherMinecart`: the two
//! motions averaged, 0.2 of each kept, a separation push), which a moving train turns into speed.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// A scene's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Scene {
    /// tick-exact's: the queue on flat brakes (powered rails, unpowered) along +x, the front
    /// `release` carts on braked slopes down east (each its own cell), a flat exit; the tail end
    /// a solid block (a stopped cart there kicked east when its rail is powered).
    Flat,
    /// Every cart on its own braked slope, a staircase down east, a flat exit.
    Stair,
    /// A sawtooth: each cart at rest on its own braked slope down east (the period's controlled
    /// rail), then a plain slope up and a plain flat peak (the slope down's sturdy high side): a
    /// pulse of the slopes' power rolls every cart down, over the peak and onto the next braked
    /// slope, one period; the front one off the end, out.
    Saw,
}

/// The scene's options.
#[derive(Clone, Copy, Debug)]
pub struct QueueOpts {
    pub scene: Scene,
    pub carts: i32,
    /// The queue's spacing at rest (flat scene), along the track.
    pub spacing: f64,
    /// Carts the release lets go.
    pub release: i32,
    /// Villagers in the carts.
    pub riders: bool,
    /// The saw scene: each cart at its own offset down its slope (0.1 at the head .. 0.95 at the
    /// tail), to see from how deep a slot a pulse still lifts a cart out.
    pub sweep: bool,
}

impl Default for QueueOpts {
    fn default() -> QueueOpts {
        QueueOpts { scene: Scene::Flat, carts: 21, spacing: 1.0, release: 3, riders: true, sweep: false }
    }
}

/// Where the scene's parts are.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Every queue rail's control (its support: a redstone block powers the rail), head first.
    pub queue_ctrl: Vec<BlockPos>,
    /// The release slopes' controls, head first.
    pub release_ctrl: Vec<BlockPos>,
    /// A cart past this x has left (released).
    pub exit_x: f64,
}

/// The scene's layout.
pub fn layout(o: &QueueOpts) -> Layout {
    let p = BlockPos::new;
    match o.scene {
        Scene::Flat => {
            // the release slopes at x q..q+r (down east from y 0), the queue's flat cells west
            let q = queue_len(o);
            let r = o.release;
            Layout {
                queue_ctrl: (0..q).rev().map(|x| p(x, -1, 0)).collect(),
                release_ctrl: (0..r).rev().map(|i| p(q + i, -2 - i, 0)).collect(),
                exit_x: (q + r) as f64,
            }
        }
        Scene::Stair => {
            let n = o.carts;
            Layout { queue_ctrl: (o.release..n).map(|i| p(n - 1 - i, i - n - 1, 0)).collect(), release_ctrl: (0..o.release).map(|i| p(n - 1 - i, i - n - 1, 0)).collect(), exit_x: n as f64 }
        }
        Scene::Saw => {
            // period i (head first, i = 0 the front) at x 3 (n - 1 - i): the slope down, the slope
            // up, the peak; every slope down's support its control
            let n = o.carts;
            let ctrl = |i: i32| p(3 * (n - 1 - i), -1, 0);
            Layout { queue_ctrl: (o.release..n).map(ctrl).collect(), release_ctrl: (0..o.release).map(ctrl).collect(), exit_x: (3 * n) as f64 }
        }
    }
}

/// The flat scene's queue cells (the carts behind the release at `spacing`).
fn queue_len(o: &QueueOpts) -> i32 {
    (((o.carts - o.release) as f64) * o.spacing).ceil() as i32 + 1
}

/// The scene's design.
pub fn design(o: &QueueOpts) -> Result<String> {
    if o.release < 1 || o.carts <= o.release {
        bail!("more carts than the release takes, and a release of 1 or more");
    }
    let mut b = Builder::new("carousel-queue-scene");
    let pr = |s: &str| format!("powered_rail[powered=false,shape={s},waterlogged=false]");
    let rider = if o.riders { Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}") } else { Some("{Rotation:[0f,0f]}") };
    let y_end: i32;
    let x_end: i32;
    match o.scene {
        Scene::Flat => {
            let q = queue_len(o);
            // the tail's stop, the queue's flat brakes, the release slopes down east
            b.set(-1, 0, 0, "stone");
            for x in 0..q {
                b.set(x, 0, 0, &pr("east_west"));
                b.set(x, -1, 0, "glass");
            }
            for i in 0..o.release {
                b.set(q + i, -1 - i, 0, &pr("ascending_west"));
                b.set(q + i, -2 - i, 0, "glass");
            }
            // the release carts, one a slope cell; the queue behind at `spacing` from the head
            for i in 0..o.release {
                let x = q + o.release - 1 - i;
                b.entity("minecart", [x as f64 + 0.5, -(o.release - i) as f64 + 0.5, 0.5], rider);
            }
            for i in 0..(o.carts - o.release) {
                b.entity("minecart", [q as f64 - 0.5 - i as f64 * o.spacing, 0.0625, 0.5], rider);
            }
            y_end = -o.release;
            x_end = q + o.release;
        }
        Scene::Stair => {
            let n = o.carts;
            // (the top slope's high side: a sturdy block, or the rail pops)
            b.set(-1, -1, 0, "stone");
            for i in 0..n {
                let x = n - 1 - i;
                b.set(x, i - n, 0, &pr("ascending_west"));
                b.set(x, i - n - 1, 0, "glass");
                b.entity("minecart", [x as f64 + 0.5, (i - n) as f64 + 0.5, 0.5], rider);
            }
            y_end = -n;
            x_end = n;
        }
        Scene::Saw => {
            let n = o.carts;
            // (a peak before the first period too: the first slope's high side)
            b.set(-1, 1, 0, "rail[shape=east_west,waterlogged=false]");
            b.set(-1, 0, 0, "glass");
            for i in 0..n {
                let x = 3 * (n - 1 - i);
                b.set(x, 0, 0, &pr("ascending_west"));
                b.set(x, -1, 0, "glass");
                b.set(x + 1, 0, 0, "rail[shape=ascending_east,waterlogged=false]");
                b.set(x + 1, -1, 0, "glass");
                b.set(x + 2, 1, 0, "rail[shape=east_west,waterlogged=false]");
                b.set(x + 2, 0, 0, "glass");
                let off = if o.sweep { 0.1 + 0.85 * i as f64 / (n - 1).max(1) as f64 } else { 0.5 };
                b.entity("minecart", [x as f64 + off, 1.0 - off + 0.05, 0.5], rider);
            }
            y_end = 1;
            x_end = 3 * n;
        }
    }
    // the exit: plain rails east, 30 cells
    for x in x_end..x_end + 30 {
        b.set(x, y_end, 0, "rail[shape=east_west,waterlogged=false]");
        b.set(x, y_end - 1, 0, "glass");
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("queue".into(), format!("scene={:?} carts={} spacing={} release={} riders={} sweep={}", o.scene, o.carts, o.spacing, o.release, o.riders as i32, o.sweep as i32));
    b.d.ports.insert("output".into(), [x_end + 31, y_end, 2]);
    Ok(b.text(&["The single-lane carousel's queue + release-3: a physics scene (carts back to back, a release slope)."]))
}

/// The scene options from a design's `queue:` line.
pub fn opts_of(meta: &std::collections::BTreeMap<String, String>) -> Result<QueueOpts> {
    let line = meta.get("queue").ok_or_else(|| anyhow::anyhow!("not a queue scene (no `queue:` line)"))?;
    let mut o = QueueOpts::default();
    for kv in line.split_whitespace() {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("bad `queue:` entry {kv:?}"))?;
        match k {
            "scene" => o.scene = match v { "Stair" => Scene::Stair, "Saw" => Scene::Saw, _ => Scene::Flat },
            "carts" => o.carts = v.parse()?,
            "spacing" => o.spacing = v.parse()?,
            "release" => o.release = v.parse()?,
            "riders" => o.riders = v != "0",
            "sweep" => o.sweep = v != "0",
            _ => bail!("unknown `queue:` entry {k:?}"),
        }
    }
    Ok(o)
}

/// A run's schedule: each step at a tick sets the release or the queue rails powered or not.
#[derive(Clone, Debug)]
pub struct RunPlan {
    pub ticks: i64,
    /// (tick, release powered?)
    pub release: Vec<(i64, bool)>,
    /// (tick, queue powered?)
    pub queue: Vec<(i64, bool)>,
    /// Lockstep cycles (the staircase): at each tick every rail of the scene powered; braked
    /// again when that cycle's first held cart (the cart `release` places behind the cycle's
    /// released ones, cart index `release * (c + 1)`) passes this x.
    pub cycles: Vec<i64>,
    pub trigger_x: f64,
    /// With a pulse (> 0), each cycle brakes again this many ticks after it starts instead of at
    /// the trigger.
    pub pulse: i64,
    /// Print every cart's x and speed every this many ticks (0: only the summary).
    pub every: i64,
}

/// Runs a scene on its plan; prints the log and returns the summary line.
pub fn run(spec: &FarmSpec, o: &QueueOpts, plan: &RunPlan, seed: i64) -> Result<String, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, plan.ticks as u32 + 10, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let lay = layout(o);
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| e.id).collect();
    let start: Vec<(f64, f64, f64)> = carts.iter().map(|id| { let e = run.world.entities.list.iter().find(|e| e.id == *id).unwrap(); (e.pos.x, e.pos.y, e.pos.z) }).collect();
    let mut gone: Vec<Option<i64>> = vec![None; carts.len()];
    let mut moved_max = vec![0.0f64; carts.len()];
    let mut speed_max = vec![0.0f64; carts.len()];
    let mut bounces = vec![0u32; carts.len()];
    let mut last_vx = vec![0.0f64; carts.len()];
    let mut armed: Option<usize> = None;
    let mut cycle_rows: Vec<String> = Vec::new();
    let all_ctrl: Vec<BlockPos> = lay.release_ctrl.iter().chain(lay.queue_ctrl.iter()).copied().collect();
    for t in 0..plan.ticks {
        let fl = redsim::world::flags::ALL;
        if let Some(c) = plan.cycles.iter().position(|s| *s == t) {
            // (the positions the last cycle left: each remaining cart's offset in its cell)
            let rest: Vec<String> = carts.iter().enumerate().filter(|(k, _)| gone[*k].is_none()).filter_map(|(_, id)| run.world.entities.list.iter().find(|e| e.id == *id)).map(|e| format!("{:.3}", e.pos.x)).collect();
            cycle_rows.push(format!("cycle {c} start t {t}: {} carts at x [{}]", rest.len(), rest.join(" ")));
            for p in &all_ctrl {
                run.world.set_block(*p, on.clone(), fl);
            }
            armed = Some(o.release as usize * (c + 1));
        }
        for (at, p) in &plan.release {
            if *at == t {
                for c in &lay.release_ctrl {
                    run.world.set_block(*c, if *p { on.clone() } else { off.clone() }, fl);
                }
            }
        }
        for (at, p) in &plan.queue {
            if *at == t {
                for c in &lay.queue_ctrl {
                    run.world.set_block(*c, if *p { on.clone() } else { off.clone() }, fl);
                }
            }
        }
        run.tick();
        if let Some(k) = armed {
            let x = carts.get(k).and_then(|id| run.world.entities.list.iter().find(|e| e.id == *id)).map(|e| e.pos.x);
            let started = plan.cycles.iter().rev().find(|s| **s <= t).copied().unwrap_or(0);
            let due = if plan.pulse > 0 { t + 1 - started >= plan.pulse } else { x.map_or(true, |x| x >= plan.trigger_x) };
            if due {
                for p in &all_ctrl {
                    run.world.set_block(*p, off.clone(), redsim::world::flags::ALL);
                }
                cycle_rows.push(format!("  braked t {t}: cart {k} at x {:.3}", x.unwrap_or(f64::NAN)));
                armed = None;
            }
        }
        let mut line = format!("t {t:4}:");
        for (k, id) in carts.iter().enumerate() {
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) else { continue };
            let (x, vx) = (e.pos.x, e.delta_movement.x);
            if gone[k].is_none() && x > lay.exit_x {
                gone[k] = Some(t);
            }
            let d = ((x - start[k].0).powi(2) + (e.pos.y - start[k].1).powi(2)).sqrt();
            moved_max[k] = moved_max[k].max(d);
            speed_max[k] = speed_max[k].max(vx.abs());
            if vx * last_vx[k] < 0.0 && vx.abs() > 1e-4 {
                bounces[k] += 1;
            }
            last_vx[k] = vx;
            if plan.every > 0 && t % plan.every == 0 {
                line.push_str(&format!(" {:.2}/{:+.3}", x, vx));
            }
        }
        if plan.every > 0 && t % plan.every == 0 {
            println!("{line}");
        }
    }
    let left: Vec<String> = gone.iter().enumerate().filter_map(|(k, g)| g.map(|t| format!("cart {k} at t {t}"))).collect();
    let rows: Vec<String> = (0..carts.len()).map(|k| format!("{k}: moved {:.3} vmax {:.3} bounces {}", moved_max[k], speed_max[k], bounces[k])).collect();
    Ok(format!("left: {} [{}]\n{}\n{}", left.len(), left.join(", "), cycle_rows.join("\n"), rows.join("\n")))
}

// ------------------------------------------------------------------------------ the whole loop

/// What a loop path cell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// A sawtooth slot's slope down (a powered rail on its control: glass a brake, a redstone
    /// block a booster), the slot's resting cell.
    Slot,
    /// A powered rail on a plain support, powered through the slot slope it is linked to (a
    /// powered rail passes power to the powered rails it connects to).
    Linked,
    /// A detector rail (on stone): its signal times a brake.
    Detector,
    /// A plain rail (the slope up, the peak, a run, a curve).
    Plain,
    /// A powered rail on a redstone block (always boosting).
    Boost,
}

/// The single-lane loop's shape (the prototype: two queue rows folded into a U, the release run
/// to the stand beside the return run).
#[derive(Clone, Copy, Debug)]
pub struct LoopOpts {
    /// Queue slots (the release group the front 3 of them).
    pub slots: i32,
    /// The release run's flat cells (the queue head to the stand).
    pub run: i32,
    /// The departure run's flat cells (the stand to the U-turn back).
    pub depart: i32,
    /// A booster every this many flat run cells (0: none).
    pub boost_every: i32,
    /// Boosters on the first this many cells of the release run and of the departure run (every
    /// cart of a train to the speed cap at once, so the train keeps its spacing).
    pub boost_first: i32,
    /// The queue in one straight row (else folded in two rows with a U-turn).
    pub straight: bool,
    /// Each train falls off a 1-block ledge before the stand and before the queue's tail (its
    /// speed clamped to 0.4 off the rail, so the slots catch it halfway down their slopes).
    pub drop: bool,
    /// The stand's slots as V slots (slope down, valley, its own braked slope up), else catch
    /// slots (the sawtooth slot with a braked flat cell before each slope).
    pub vstand: bool,
    /// The queue's 3 tail slots as V slots (the returning train arrives fast; a cart that stops
    /// on a slope up rolls back first, then leaves forward).
    pub tail_v: bool,
    /// The bridge layout: the stand across the ends of the release run and the return run, the
    /// queue beyond them (a return as short as the release run).
    pub bridge: bool,
    /// The bridge layout's queue rows this far apart: 1 (a U-turn of two curves), 2 (a curve, a
    /// booster, a curve).
    pub row_gap: i32,
    /// A booster in the middle of the queue's U-turn (rows 2 apart).
    pub uturn_boost: bool,
    /// The line layout: the queue one straight row, the return the long way round it (12 slots;
    /// two batches on the return at any time).
    pub line: bool,
    /// The queue's 3 tail slots as deep slots (a braked slope down linked to the slot's own: a
    /// fast returning cart stops on one of the two, never on a flat cell; two plain slopes up).
    pub tail_deep: bool,
    /// Plain loop: every slot (queue, stand) a plain sawtooth slot; the trains kept slow (one
    /// booster after the queue and after the stand), each slot braked as its own cart comes,
    /// and the stand's batch leaves like a queue advance (3 pulses).
    pub plain: bool,
    /// A descending stand: each slot a braked slope linked to its own (C, A) and a plain slope up,
    /// each slot a block lower than the last (the catch is a slope wherever the brake comes).
    pub stand_desc: bool,
    /// A detector rail on the release run, 4 cells before the stand's curve, to time the stand's
    /// brake (`queue-loop-run --stand-detect D`: the brake D gt after its signal rises).
    pub detector: bool,
}

impl Default for LoopOpts {
    fn default() -> LoopOpts {
        LoopOpts { slots: 18, run: 20, depart: 3, boost_every: 0, boost_first: 3, straight: false, drop: false, vstand: false, tail_v: true, bridge: true, row_gap: 2, uturn_boost: false, line: false, tail_deep: false, plain: false, stand_desc: false, detector: false }
    }
}

/// The loop's parts: its path and every group of controls.
#[derive(Clone, Debug)]
pub struct LoopLayout {
    /// The path in travel order, from the queue's entry (the return run's end) round to it.
    pub path: Vec<([i32; 3], Cell)>,
    /// Each queue slot's control (its slope down's support), tail (slot 0) first.
    pub slots: Vec<BlockPos>,
    /// The stand's three slots' controls, front (S1) first.
    pub stand: Vec<BlockPos>,
    /// Each queue slot's resting cell (its slope down), tail first; the stand's, front first.
    pub slot_cells: Vec<[i32; 3]>,
    pub stand_cells: Vec<[i32; 3]>,
    /// Each stand slot's slope-up control (its own line), front (S1) first; empty where the
    /// stand's slots are plain sawtooth.
    pub stand_b: Vec<BlockPos>,
    /// The queue's tail slots' (0, 1, 2) slope-up controls (their own line) when they are V
    /// slots; empty otherwise.
    pub tail_b: Vec<BlockPos>,
    /// Each queue slot's travel direction (x, z) and its peak cell.
    pub slot_dir: Vec<[i32; 2]>,
    pub slot_peak: Vec<[i32; 3]>,
}

/// The loop's layout: the entry at (0, 1, 0); queue row A going -x (slots 0 .. slots/2), a U-turn,
/// row B going +x (the rest, the head ending at x -1, z 4); the release run +x along z 4, the
/// stand (S3, S2, S1 in travel order), the departure run, a U-turn to z 0 and the return run -x
/// back to the entry.
pub fn loop_layout(o: &LoopOpts) -> LoopLayout {
    if o.straight {
        return loop_layout_straight(o);
    }
    if o.line {
        return loop_layout_line(o);
    }
    if o.bridge {
        return loop_layout_bridge(o);
    }
    // rows 1 apart (z 0 going -x, z 1 going +x): a 2-cell U-turn in the queue (curves at the
    // peaks' height), the release run and stand along z 1, the return along z 0
    let mut path: Vec<([i32; 3], Cell)> = vec![([0, 1, 0], Cell::Plain)];
    let mut slot_cells = Vec::new();
    let mut stand_cells = Vec::new();
    let half = o.slots / 2;
    let saw = |path: &mut Vec<([i32; 3], Cell)>, x: i32, z: i32, d: i32| {
        path.push(([x, 0, z], Cell::Slot));
        path.push(([x + d, 0, z], Cell::Plain));
        path.push(([x + 2 * d, 1, z], Cell::Plain));
    };
    // row A: with `tail_v` its first 3 slots (the queue's tail, where the returning train comes
    // in fast) are V slots (slope down, plain valley, slope up on its own control, peak), the
    // rest sawtooth slots
    let mut tail_b = Vec::new();
    let mut x = -1;
    for k in 0..half {
        slot_cells.push([x, 0, 0]);
        if o.tail_v && k < 3 {
            path.push(([x, 0, 0], Cell::Slot));
            path.push(([x - 1, 0, 0], Cell::Plain));
            path.push(([x - 2, 0, 0], Cell::Slot));
            path.push(([x - 3, 1, 0], Cell::Plain));
            tail_b.push(BlockPos::new(x - 2, -1, 0));
            x -= 4;
        } else {
            saw(&mut path, x, 0, -1);
            x -= 3;
        }
    }
    // the U-turn at the peaks' height, rows 2 apart: a curve, a booster, a curve
    let zb = 2;
    let xu = x;
    path.push(([xu, 1, 0], Cell::Plain));
    path.push(([xu, 1, 1], Cell::Boost));
    path.push(([xu, 1, zb], Cell::Plain));
    for j in 0..(o.slots - half) {
        let x = xu + 1 + 3 * j;
        slot_cells.push([x, 0, zb]);
        saw(&mut path, x, zb, 1);
    }
    let x0 = xu + 1 + 3 * (o.slots - half);
    let flat = |path: &mut Vec<([i32; 3], Cell)>, x: i32, z: i32, i: i32, n: i32| {
        let boost = (o.boost_every > 0 && i > 0 && i < n - 1 && i % o.boost_every == 0) || (n > 2 && i < o.boost_first.min(n - 1));
        path.push(([x, 1, z], if boost { Cell::Boost } else { Cell::Plain }));
    };
    // with `drop`, each train falls off a 1-block ledge before the stand and before the queue's
    // tail: off its rail a cart's speed is clamped to 0.4 (`comeOffTrack`), so every cart lands
    // at the same gentle speed whatever boosts it took, the train keeps its spacing, and the
    // slots catch it halfway down their slopes
    let (up, land) = if o.drop { (1, 4) } else { (0, 0) };
    if o.drop {
        // a plain slope up onto the high run, the run, a gap, the landing cells
        path.push(([x0, 1, zb], Cell::Plain));
        for i in 1..o.run - land - 1 {
            let boost = i <= o.boost_first;
            path.push(([x0 + i, 2, zb], if boost { Cell::Boost } else { Cell::Plain }));
        }
        for i in o.run - land..o.run {
            path.push(([x0 + i, 1, zb], Cell::Plain));
        }
    } else {
        for i in 0..o.run {
            flat(&mut path, x0 + i, zb, i, o.run);
        }
    }
    let _ = up;
    // the stand's slots, two kinds:
    // - catch slots (default): the sawtooth slot, and the flat cell before each slope (S2's
    //   peak, S3's peak, the run's last cell) a powered rail too, linked to that slope (a powered
    //   rail takes its neighbour's power): braked together, a fast cart crosses the braked flat
    //   cell and stops early on its slope, so it has the run-up to climb out at the departure;
    // - V slots (`vstand`): a braked slope down (A), a plain flat valley, a braked slope up (B, on
    //   its own control), a plain peak; for the departure B goes on first (a cart on B rolls back
    //   onto A and stops), then A and B
    let mut stand_b = Vec::new();
    let xs = x0 + o.run;
    let per = if o.vstand { 4 } else { 3 };
    if !o.vstand {
        if let Some(last) = path.last_mut() {
            last.1 = Cell::Linked;
        }
    }
    for j in 0..3 {
        let x = xs + per * j;
        stand_cells.insert(0, [x, 0, zb]);
        if o.vstand {
            path.push(([x, 0, zb], Cell::Slot));
            path.push(([x + 1, 0, zb], Cell::Plain));
            path.push(([x + 2, 0, zb], Cell::Slot));
            path.push(([x + 3, 1, zb], Cell::Plain));
            stand_b.insert(0, BlockPos::new(x + 2, -1, zb));
        } else {
            path.push(([x, 0, zb], Cell::Slot));
            path.push(([x + 1, 0, zb], Cell::Plain));
            path.push(([x + 2, 1, zb], if j < 2 { Cell::Linked } else { Cell::Plain }));
        }
    }
    let xd = xs + 3 * per;
    for i in 0..o.depart {
        flat(&mut path, xd + i, zb, i, o.depart);
    }
    let xe = xd + o.depart;
    if o.drop {
        // up onto the high return: the U-turn and the return run at y 2, a gap, the landing
        // cells at y 1 into the entry
        path.push(([xe, 1, zb], Cell::Plain));
        path.push(([xe + 1, 2, zb], Cell::Plain));
        path.push(([xe + 2, 2, zb], Cell::Plain));
        path.push(([xe + 2, 2, 1], Cell::Boost));
        path.push(([xe + 2, 2, 0], Cell::Plain));
        let n = xe + 2 - 1 - land;
        for i in 0..n - 1 {
            path.push(([xe + 1 - i, 2, 0], Cell::Plain));
        }
        for x in (1..=land).rev() {
            path.push(([x, 1, 0], Cell::Plain));
        }
    } else {
        path.push(([xe, 1, zb], Cell::Plain));
        path.push(([xe, 1, 1], Cell::Boost));
        path.push(([xe, 1, 0], Cell::Plain));
        let n = xe - 1;
        for i in 0..n {
            flat(&mut path, xe - 1 - i, 0, i, n);
        }
    }
    finish_layout(path, slot_cells, stand_cells, stand_b, tail_b)
}

/// A loop's layout from its path: a booster's support (a redstone block) powers every powered
/// rail beside it, so no booster whose support touches a slot's slope down.
fn finish_layout(mut path: Vec<([i32; 3], Cell)>, slot_cells: Vec<[i32; 3]>, stand_cells: Vec<[i32; 3]>, stand_b: Vec<BlockPos>, tail_b: Vec<BlockPos>) -> LoopLayout {
    let ctrl = |c: &[i32; 3]| BlockPos::new(c[0], c[1] - 1, c[2]);
    // (each slot's travel direction and peak, from the path: the cell after its slope down, and
    // the first cell back at the peaks' height)
    let n = path.len();
    let geo = |c: &[i32; 3]| -> ([i32; 2], [i32; 3]) {
        let i = path.iter().position(|(p, _)| p == c).unwrap_or(0);
        let next = path[(i + 1) % n].0;
        let peak = (1..n).map(|d| path[(i + d) % n].0).find(|p| p[1] == c[1] + 1).unwrap_or(next);
        ([next[0] - c[0], next[2] - c[2]], peak)
    };
    let (slot_dir, slot_peak): (Vec<[i32; 2]>, Vec<[i32; 3]>) = slot_cells.iter().map(geo).unzip();
    let slot_rails: std::collections::BTreeSet<[i32; 3]> = path.iter().filter(|(_, k)| *k == Cell::Slot || *k == Cell::Linked).map(|(c, _)| *c).collect();
    for (c, k) in path.iter_mut() {
        if *k == Cell::Boost {
            let s = [c[0], c[1] - 1, c[2]];
            let touches = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]].iter().any(|d| slot_rails.contains(&[s[0] + d[0], s[1] + d[1], s[2] + d[2]]));
            if touches {
                *k = Cell::Plain;
            }
        }
    }
    LoopLayout { slots: slot_cells.iter().map(ctrl).collect(), stand: stand_cells.iter().map(ctrl).collect(), stand_b, tail_b, slot_dir, slot_peak, path, slot_cells, stand_cells }
}

/// The bridge loop: the stand along +x at z 0 (S3's slope at x 0, catch slots) bridging the two
/// runs: the release run comes south along x -2 into its west end, the departure leaves its east
/// end north along x 10, jogs west along z 20 and enters the queue's tail; the queue folded in
/// two rows north of z 20 (row A, the tail, north along x 0; a U-turn at its far end; row B, the
/// head, south along x -2, straight into the release run). The return is about as short as the
/// release run, so a returning train is in the queue's tail before the next release.
fn loop_layout_bridge(o: &LoopOpts) -> LoopLayout {
    let zr = 20;
    let mut path: Vec<([i32; 3], Cell)> = Vec::new();
    let mut slot_cells = Vec::new();
    let mut stand_cells = Vec::new();
    let mut tail_b = Vec::new();
    // (with a turn slot, row A gets one slot fewer than half: its V slots are longer, and the
    // queue's head lands 19 cells from the stand; with deep tail slots, two fewer)
    let half = if o.row_gap == 3 { if o.tail_deep { o.slots / 2 - 2 } else { o.slots / 2 - 1 } } else { o.slots / 2 };
    // row A from its entry curve at (0, 1, zr): north (+z)
    path.push(([0, 1, zr], Cell::Plain));
    let mut z = zr + 1;
    for k in 0..half {
        if o.tail_deep && k < 3 {
            // a deep slot (see `loop_layout_line`): braked slopes C (linked) and A, plain slopes
            // up B and B', the peak
            slot_cells.push([0, -1, z + 1]);
            path.push(([0, 0, z], Cell::Linked));
            path.push(([0, -1, z + 1], Cell::Slot));
            path.push(([0, -1, z + 2], Cell::Plain));
            path.push(([0, 0, z + 3], Cell::Plain));
            path.push(([0, 1, z + 4], Cell::Plain));
            z += 5;
            continue;
        }
        slot_cells.push([0, 0, z]);
        if o.tail_v && k < 3 {
            path.push(([0, 0, z], Cell::Slot));
            path.push(([0, 0, z + 1], Cell::Plain));
            path.push(([0, 0, z + 2], Cell::Slot));
            path.push(([0, 1, z + 3], Cell::Plain));
            tail_b.push(BlockPos::new(0, -1, z + 2));
            z += 4;
        } else {
            path.push(([0, 0, z], Cell::Slot));
            path.push(([0, 0, z + 1], Cell::Plain));
            path.push(([0, 1, z + 2], Cell::Plain));
            z += 3;
        }
    }
    // the U-turn west at the peaks' height: rows 1 apart, two curves; rows 2 apart, a curve, a
    // booster, a curve; rows 3 apart (`row_gap` 3), a turn slot: row A's last peak is a curve, the
    // next slot runs west (slope down, slope up, a curve for its peak) and row B starts below it,
    // so no queue move is longer than another
    let xb = -o.row_gap;
    let mut z = z;
    let mut row_b = o.slots - half;
    if o.row_gap == 3 {
        slot_cells.push([-1, 0, z - 1]);
        path.push(([-1, 0, z - 1], Cell::Slot));
        path.push(([-2, 0, z - 1], Cell::Plain));
        path.push(([-3, 1, z - 1], Cell::Plain));
        row_b -= 1;
        z -= 1;
    } else {
        path.push(([0, 1, z], Cell::Plain));
        if o.row_gap == 2 {
            path.push(([-1, 1, z], if o.uturn_boost { Cell::Boost } else { Cell::Plain }));
        }
        path.push(([xb, 1, z], Cell::Plain));
    }
    // row B south (-z) along x xb
    let mut z = z - 1;
    for _ in 0..row_b {
        slot_cells.push([xb, 0, z]);
        path.push(([xb, 0, z], Cell::Slot));
        path.push(([xb, 0, z - 1], Cell::Plain));
        path.push(([xb, 1, z - 2], Cell::Plain));
        z -= 3;
    }
    // the release run south along x xb down to z 1, its first cells boosters; with `detector`, a
    // detector rail 4 cells before the stand's curve (no powered rail beside it), whose signal
    // times the stand's brake
    let mut i = 0;
    while z >= 1 {
        let k = if i < o.boost_first { Cell::Boost } else if o.detector && z == 4 { Cell::Detector } else { Cell::Plain };
        path.push(([xb, 1, z], k));
        z -= 1;
        i += 1;
    }
    // the curve into the stand, the catch cell before S3's slope
    let xs = xb + 2;
    path.push(([xb, 1, 0], Cell::Plain));
    if o.stand_desc {
        // a descending stand: each slot a braked slope C linked to its own slope A, then a plain
        // slope up B, every slot one block lower than the last (S3's slope at y -1, S1's at -3):
        // a fast cart entering a braked slot stops on C or A, always on a slope, wherever the
        // brake caught it
        for j in 0..3 {
            let x = xb + 1 + 3 * j;
            let y = -j;
            stand_cells.insert(0, [x + 1, y - 1, 0]);
            path.push(([x, y, 0], Cell::Linked));
            path.push(([x + 1, y - 1, 0], Cell::Slot));
            path.push(([x + 2, y - 1, 0], Cell::Plain));
        }
        // the departure at y -2: east, north along x xd, up 3 blocks, on north at y 1, west along
        // zr back to row A
        let xd = xb + 11;
        path.push(([xd - 1, -2, 0], Cell::Boost));
        path.push(([xd, -2, 0], Cell::Plain));
        for z in 1..=3 {
            path.push(([xd, -2, z], Cell::Boost));
        }
        path.push(([xd, -2, 4], Cell::Plain));
        path.push(([xd, -1, 5], Cell::Plain));
        path.push(([xd, 0, 6], Cell::Plain));
        for z in 7..zr {
            path.push(([xd, 1, z], Cell::Plain));
        }
        path.push(([xd, 1, zr], Cell::Plain));
        for x in (1..xd).rev() {
            path.push(([x, 1, zr], Cell::Plain));
        }
        return finish_layout(path, slot_cells, stand_cells, Vec::new(), tail_b);
    }
    path.push(([xb + 1, 1, 0], if o.plain { Cell::Plain } else { Cell::Linked }));
    for j in 0..3 {
        let x = xs + 3 * j;
        stand_cells.insert(0, [x, 0, 0]);
        path.push(([x, 0, 0], Cell::Slot));
        path.push(([x + 1, 0, 0], Cell::Plain));
        path.push(([x + 2, 1, 0], if j < 2 && !o.plain { Cell::Linked } else { Cell::Plain }));
    }
    // the departure: east, north along x xd (boosters first), west along zr back to row A; with
    // `drop` the last stretch is a ledge: up to y 2 before the turn west, off the edge at x 5,
    // landing on y 1 at x 4 (the landing clamps every cart's speed to 0.4 and the fall slows it
    // to ~0.3, so the tail's plain slots catch it like a queue move)
    let xd = xs + 10;
    path.push(([xd - 1, 1, 0], Cell::Boost));
    path.push(([xd, 1, 0], Cell::Plain));
    let top = if o.drop { zr - 2 } else { zr };
    for z in 1..top {
        path.push(([xd, 1, z], if z <= o.boost_first.max(1) { Cell::Boost } else { Cell::Plain }));
    }
    if o.drop {
        path.push(([xd, 1, zr - 2], Cell::Plain));
        path.push(([xd, 2, zr - 1], Cell::Plain));
        path.push(([xd, 2, zr], Cell::Plain));
        for x in (6..xd).rev() {
            path.push(([x, 2, zr], Cell::Plain));
        }
        for x in (1..5).rev() {
            path.push(([x, 1, zr], Cell::Plain));
        }
    } else {
        path.push(([xd, 1, zr], Cell::Plain));
        for x in (1..xd).rev() {
            path.push(([x, 1, zr], Cell::Plain));
        }
    }
    finish_layout(path, slot_cells, stand_cells, Vec::new(), tail_b)
}

/// The line loop: the queue one straight row (no U-turn inside it) along -z at x 0, its tail at
/// the north end, its head 19 cells north of the stand; the release run south along x 0 into the
/// stand's west end (the stand along +x at z 0, catch slots); the departure north along x 12, past
/// the queue, west along the queue's north end into its tail. With 12 slots the loop holds 12
/// carts in the queue, 3 at the stand and 2 batches (6 carts) on the long return: the return takes
/// about two slots, and each batch rejoins the queue's tail in the window after the advance.
fn loop_layout_line(o: &LoopOpts) -> LoopLayout {
    let mut path: Vec<([i32; 3], Cell)> = Vec::new();
    let mut slot_cells = Vec::new();
    let mut stand_cells = Vec::new();
    let mut tail_b = Vec::new();
    let nv = if o.tail_v { 3 } else { 0 };
    let nd = if o.tail_deep { 3 } else { 0 };
    let len = 4 * nv + 5 * nd + 3 * (o.slots - nv - nd);
    // the head's slot at z >= 19 (its cart 19.3+ from the pad's edge at z -1)
    let ze = 19 + len - 2;
    path.push(([0, 1, ze], Cell::Plain));
    let mut z = ze - 1;
    for k in 0..o.slots {
        if k < nd {
            // a deep slot: a braked slope down linked to the slot's own (two blocks of braked
            // descent: a fast cart stops on one of them, never on a flat cell), two plain slopes
            // up, the peak
            slot_cells.push([0, -1, z - 1]);
            path.push(([0, 0, z], Cell::Linked));
            path.push(([0, -1, z - 1], Cell::Slot));
            path.push(([0, -1, z - 2], Cell::Plain));
            path.push(([0, 0, z - 3], Cell::Plain));
            path.push(([0, 1, z - 4], Cell::Plain));
            z -= 5;
            continue;
        }
        slot_cells.push([0, 0, z]);
        if k < nv {
            path.push(([0, 0, z], Cell::Slot));
            path.push(([0, 0, z - 1], Cell::Plain));
            path.push(([0, 0, z - 2], Cell::Slot));
            path.push(([0, 1, z - 3], Cell::Plain));
            tail_b.push(BlockPos::new(0, -1, z - 2));
            z -= 4;
        } else {
            path.push(([0, 0, z], Cell::Slot));
            path.push(([0, 0, z - 1], Cell::Plain));
            path.push(([0, 1, z - 2], Cell::Plain));
            z -= 3;
        }
    }
    // the release run south down to z 1, its first cells boosters
    let mut i = 0;
    while z >= 1 {
        path.push(([0, 1, z], if i < o.boost_first { Cell::Boost } else { Cell::Plain }));
        z -= 1;
        i += 1;
    }
    // the curve into the stand, the catch cell before S3's slope
    path.push(([0, 1, 0], Cell::Plain));
    path.push(([1, 1, 0], Cell::Linked));
    let xs = 2;
    for j in 0..3 {
        let x = xs + 3 * j;
        stand_cells.insert(0, [x, 0, 0]);
        path.push(([x, 0, 0], Cell::Slot));
        path.push(([x + 1, 0, 0], Cell::Plain));
        path.push(([x + 2, 1, 0], if j < 2 { Cell::Linked } else { Cell::Plain }));
    }
    // the departure: east, north along x xd (boosters first) to the queue's north end, west into
    // the tail's entry curve
    let xd = xs + 10;
    path.push(([xd - 1, 1, 0], Cell::Boost));
    path.push(([xd, 1, 0], Cell::Plain));
    for z in 1..ze {
        path.push(([xd, 1, z], if z <= o.boost_first.max(1) { Cell::Boost } else { Cell::Plain }));
    }
    path.push(([xd, 1, ze], Cell::Plain));
    for x in (1..xd).rev() {
        path.push(([x, 1, ze], Cell::Plain));
    }
    finish_layout(path, slot_cells, stand_cells, Vec::new(), tail_b)
}

/// The straight loop: the queue one row along +x at z 0 (slot 0 at x 0, the entry at x -1), the
/// release run, the stand and the departure run on along z 0, a U-turn (curve, booster, curve)
/// to z 2, the return run back along z 2 beside them, a U-turn to z 0 at x -3 into the entry.
fn loop_layout_straight(o: &LoopOpts) -> LoopLayout {
    let mut path: Vec<([i32; 3], Cell)> = vec![([-1, 1, 0], Cell::Plain)];
    let mut slot_cells = Vec::new();
    let mut stand_cells = Vec::new();
    let saw = |path: &mut Vec<([i32; 3], Cell)>, x: i32, z: i32, d: i32| {
        path.push(([x, 0, z], Cell::Slot));
        path.push(([x + d, 0, z], Cell::Plain));
        path.push(([x + 2 * d, 1, z], Cell::Plain));
    };
    let flat = |path: &mut Vec<([i32; 3], Cell)>, x: i32, z: i32, i: i32, n: i32, first: i32| {
        let boost = (o.boost_every > 0 && i > first && i < n - 1 && i % o.boost_every == 0) || (n > 2 && i < first.min(n - 1));
        path.push(([x, 1, z], if boost { Cell::Boost } else { Cell::Plain }));
    };
    for k in 0..o.slots {
        slot_cells.push([3 * k, 0, 0]);
        saw(&mut path, 3 * k, 0, 1);
    }
    let x0 = 3 * o.slots;
    for i in 0..o.run {
        flat(&mut path, x0 + i, 0, i, o.run, o.boost_first);
    }
    let xs = x0 + o.run;
    for j in 0..3 {
        let x = xs + 3 * j;
        stand_cells.insert(0, [x, 0, 0]);
        saw(&mut path, x, 0, 1);
    }
    let xd = xs + 9;
    for i in 0..o.depart {
        flat(&mut path, xd + i, 0, i, o.depart, o.boost_first);
    }
    let xe = xd + o.depart;
    path.push(([xe, 1, 0], Cell::Plain));
    path.push(([xe, 1, 1], Cell::Boost));
    path.push(([xe, 1, 2], Cell::Plain));
    let n = xe + 2;
    for i in 0..n {
        flat(&mut path, xe - 1 - i, 2, i, n, 0);
    }
    path.push(([-3, 1, 2], Cell::Plain));
    path.push(([-3, 1, 1], Cell::Boost));
    path.push(([-3, 1, 0], Cell::Plain));
    path.push(([-2, 1, 0], Cell::Plain));
    finish_layout(path, slot_cells, stand_cells, Vec::new(), Vec::new())
}

/// The loop's design: its path (each rail on its support), 21 carts with villagers: the queue's
/// slots full, the stand's three slots full.
pub fn loop_design(o: &LoopOpts) -> Result<String> {
    // (the line loop starts with one batch on its return: the bench adds it)
    let held = o.slots + 3 + if o.line { 3 } else { 0 };
    if held != 21 {
        bail!("the queue's {} slots and the stand's 3{} must hold the 21 carts", o.slots, if o.line { " and the returning batch's 3" } else { "" });
    }
    let lay = loop_layout(o);
    let mut b = Builder::new("carousel-loop-scene");
    let n = lay.path.len();
    for i in 0..n {
        let (c, kind) = lay.path[i];
        let prev = lay.path[(i + n - 1) % n].0;
        let next = lay.path[(i + 1) % n].0;
        // (a ledge's two ends: a neighbour that is no rail step away is the straight run on)
        let near = |a: [i32; 3], b: [i32; 3]| (a[0] - b[0]).abs() + (a[2] - b[2]).abs() == 1 && (a[1] - b[1]).abs() <= 1;
        let (prev, next) = match (near(prev, c), near(next, c)) {
            (true, true) => (prev, next),
            (true, false) => (prev, [2 * c[0] - prev[0], c[1], 2 * c[2] - prev[2]]),
            (false, true) => ([2 * c[0] - next[0], c[1], 2 * c[2] - next[2]], next),
            (false, false) => bail!("the loop's path breaks on both sides of {c:?}"),
        };
        let sh = crate::yard::shape(Some(prev), c, Some(next))?;
        let (rail, support) = match kind {
            Cell::Slot => (format!("powered_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
            Cell::Linked => (format!("powered_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
            Cell::Boost => (format!("powered_rail[powered=true,shape={sh},waterlogged=false]"), "redstone_block"),
            Cell::Plain => (format!("rail[shape={sh},waterlogged=false]"), "glass"),
            Cell::Detector => (format!("detector_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
        };
        if b.d.blocks.contains_key(&c) {
            bail!("the loop's path meets itself at {c:?}");
        }
        b.set(c[0], c[1], c[2], &rail);
        b.set(c[0], c[1] - 1, c[2], support);
    }
    let rider = Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}");
    // (each cart where the loop's steady state rests it: a queue cart 0.32 down its slope, a
    // stand cart 0.45, measured from the slope's top edge along its travel direction)
    for (k, c) in lay.slot_cells.iter().chain(lay.stand_cells.iter()).enumerate() {
        let (off, d) = if k < lay.slot_cells.len() { (0.32, lay.slot_dir[k]) } else { (0.45, [1, 0]) };
        let x = c[0] as f64 + 0.5 + d[0] as f64 * (off - 0.5);
        let z = c[2] as f64 + 0.5 + d[1] as f64 * (off - 0.5);
        b.entity("minecart", [x, c[1] as f64 + 1.0 - off + 0.05, z], rider);
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("queue".into(), format!("scene=Loop carts=21 spacing=3 release=3 riders=1 slots={} run={} depart={} boost={} boost_first={} straight={} drop={} vstand={} tail_v={} bridge={} row_gap={} uturn_boost={} line={} tail_deep={} plain={} stand_desc={} detector={}", o.slots, o.run, o.depart, o.boost_every, o.boost_first, o.straight as i32, o.drop as i32, o.vstand as i32, o.tail_v as i32, o.bridge as i32, o.row_gap, o.uturn_boost as i32, o.line as i32, o.tail_deep as i32, o.plain as i32, o.stand_desc as i32, o.detector as i32));
    b.d.ports.insert("output".into(), [-5, 1, -3]);
    Ok(b.text(&["The single-lane carousel's loop (prototype): a sawtooth queue register, the release run, the 3-slot stand, the return run."]))
}

/// The loop options from a design's `queue:` line.
pub fn loop_opts_of(meta: &std::collections::BTreeMap<String, String>) -> Result<LoopOpts> {
    let line = meta.get("queue").ok_or_else(|| anyhow::anyhow!("not a queue scene (no `queue:` line)"))?;
    let mut o = LoopOpts::default();
    for kv in line.split_whitespace() {
        let Some((k, v)) = kv.split_once('=') else { continue };
        match k {
            "slots" => o.slots = v.parse()?,
            "run" => o.run = v.parse()?,
            "depart" => o.depart = v.parse()?,
            "boost" => o.boost_every = v.parse()?,
            "boost_first" => o.boost_first = v.parse()?,
            "straight" => o.straight = v != "0",
            "drop" => o.drop = v != "0",
            "vstand" => o.vstand = v != "0",
            "tail_v" => o.tail_v = v != "0",
            "bridge" => o.bridge = v != "0",
            "row_gap" => o.row_gap = v.parse()?,
            "uturn_boost" => o.uturn_boost = v != "0",
            "line" => o.line = v != "0",
            "tail_deep" => o.tail_deep = v != "0",
            "plain" => o.plain = v != "0",
            "stand_desc" => o.stand_desc = v != "0",
            "detector" => o.detector = v != "0",
            _ => {}
        }
    }
    Ok(o)
}

/// The loop bench's timing (ideal controls, every gt from the slot's start t0 = the golem pulse).
#[derive(Clone, Copy, Debug)]
pub struct LoopTiming {
    /// The release group's (the queue's front 3 slots') power: t0 .. t0 + this.
    pub release_for: i64,
    /// The advance's three pulses on every queue slot: at t0 + `advance_at` + k * `advance_every`,
    /// each `pulse` long.
    pub advance_at: i64,
    pub advance_every: i64,
    pub pulse: i64,
    /// The stand's slots powered for the departure front first, this many gt apart.
    pub depart_every: i64,
    /// Time-based brakes (what a timer circuit does) instead of the bench's position triggers:
    /// the stand braked this many gt after its slot's start; the queue's tail slots 1-2 and
    /// slot 0 braked these many gt after the returning train's departure.
    pub stand_brake: Option<i64>,
    /// The stand braked this many gt after the release train's front cart first stands on S2's
    /// slope up (a detector rail there), instead of the bench's position trigger.
    pub stand_detect: Option<i64>,
    /// Each stand slot braked this many gt after its own cart first stands on the cell two
    /// before its slope (a detector rail per slot).
    pub stand_detect_each: Option<i64>,
    pub intake_brake: Option<(i64, i64)>,
    /// The line loop's startup: the gt (in slot 0) at which the bench places the batch that is on
    /// the return at the start into the queue's tail.
    pub inject_at: Option<i64>,
    /// Plain loops: the stand's 3 departure pulses (gt after the slot's start, apart), and how long
    /// after the last one ends the stand opens (all slopes powered) for the next batch.
    pub depart_at: i64,
    pub depart_period: i64,
    pub open_after: i64,
    /// The design's own redstone drives the controls: the bench only powers this input cell
    /// (a redstone block) for `golem_for` gt at each slot's start and checks the carts.
    pub wired: Option<BlockPos>,
    pub golem_for: i64,
    /// With `wired`: the stand latch's reset input, which the bench pulses (a redstone block for
    /// 2 gt) at its own stand-brake trigger, until the stand's brake is wired too.
    pub stand_reset: Option<BlockPos>,
    /// The stand's batch leaves like a queue advance (3 pulses on all 3 stand slopes,
    /// `depart_at` + k·`depart_period`, then the stand opens) instead of front first.
    pub shift_depart: bool,
}

impl Default for LoopTiming {
    fn default() -> LoopTiming {
        LoopTiming { release_for: 30, advance_at: 40, advance_every: 20, pulse: 9, depart_every: 12, stand_brake: None, stand_detect: None, stand_detect_each: None, intake_brake: None, inject_at: None, depart_at: 4, depart_period: 16, open_after: 8, wired: None, golem_for: 10, stand_reset: None, shift_depart: false }
    }
}

/// Runs the loop for `cycles` slots of 100 gt with ideal controls: at each slot the stand's batch
/// leaves as a train (its slots powered) and the queue's front 3 leave as a train for the stand
/// (their slots powered `release_for`), the stand braked as the train's front cart reaches S1's
/// slope; the queue advanced 3 slots; the queue's 3 empty tail slots powered for the returning
/// train, braked as its front cart reaches the third. Checks the loop order (every cart 3
/// places on per slot) and prints each slot's times.
pub fn run_loop(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, cycles: i64, seed: i64, verbose: bool, watch: Option<&[usize]>, drive_out: Option<&std::path::Path>) -> Result<String, String> {
    let ticks = 200 + cycles * 100 + 200;
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, ticks as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let lay = loop_layout(o);
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    // (the line loop's startup: one batch is on its long return at the start; the bench parks its
    // 3 carts off the loop and places them in the queue's tail at `inject_at`, bench-driven)
    let mut parked: Vec<u32> = Vec::new();
    if tm.inject_at.is_some() {
        let stone = Block::parse("minecraft:stone").map_err(|e| e.to_string())?;
        for k in 0..3 {
            run.world.set_block(BlockPos::new(60, -1, 60 + 3 * k), stone.clone(), redsim::world::flags::ALL);
            parked.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [60.5, 0.0, 60.5 + 3.0 * k as f64], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?);
        }
        run.own = (first..run.world.entities.next_id).collect();
    }
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 21 {
        return Err(format!("{} carts, not 21", carts.len()));
    }
    // the loop's places in travel order: slot 0 .. slot q-1, S3, S2, S1
    let q = o.slots as usize;
    let mut places: Vec<[i32; 3]> = lay.slot_cells.clone();
    places.extend(lay.stand_cells.iter().rev());
    // (a place's span along x: a queue slot its slope ±1, a V stand slot its slope, valley and
    // slope up)
    let wide = !lay.stand_b.is_empty();
    let place_of = |w: &redsim::world::World, id: u32| -> Option<usize> {
        let e = w.entities.list.iter().find(|e| e.id == id)?;
        places.iter().enumerate().position(|(k, c)| {
            // (along and across the place's travel direction, from its slope cell's centre)
            let d = if k < q { lay.slot_dir[k] } else { [1, 0] };
            let (ox, oz) = (e.pos.x - (c[0] as f64 + 0.5), e.pos.z - (c[2] as f64 + 0.5));
            let along = ox * d[0] as f64 + oz * d[1] as f64;
            let across = (ox * d[1] as f64).abs() + (oz * d[0] as f64).abs();
            let tail = !lay.tail_b.is_empty() && k < 3;
            let deep = o.tail_deep && k < 3;
            let span = if (wide && k >= q) || tail { (-1.0, 2.5) } else if deep || (o.stand_desc && k >= q) { (-2.0, 1.0) } else { (-1.0, 1.0) };
            let e_y_ok = if deep { e.pos.y < 1.5 } else { true };
            along > span.0 && along < span.1 && across < 0.6 && e.pos.y < 1.5 && e.delta_movement.horizontal_distance_sqr() < 1e-6
        })
    };
    let pos_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id).map(|e| e.pos);
    // the drive log (mobviewer's replay format): every control change as `act <t> set x y z
    // <state>`, t = gt after the entities' spawn, applied before the tick that brings the time
    // to spawn + t
    let t_spawn = run.world.time;
    let drive: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    // (`wired`: the controls are the design's own redstone; the bench only pulses the golem input
    // cell each slot and checks where the carts go)
    let wired = tm.wired;
    let set = |w: &mut redsim::world::World, ps: &[BlockPos], p: bool| {
        if wired.is_some() {
            return;
        }
        for c in ps {
            w.set_block(*c, if p { on.clone() } else { off.clone() }, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", w.time - t_spawn, c.x, c.y, c.z, if p { "redstone_block" } else { "glass" }));
        }
    };
    let input_was = wired.map(|p| run.world.get(p));
    let reset_was = tm.stand_reset.map(|p| run.world.get(p));
    let mut reset_off_at: Option<i64> = None;
    // (who is where at the start: the design's order)
    for _ in 0..40 {
        run.tick();
    }
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let release_group: Vec<BlockPos> = lay.slots[q - 3..].to_vec();
    // (slots from a departure to the slot whose advance must find that batch in the queue's
    // tail: the line loop's long return takes two)
    let lag: i64 = if o.line { 2 } else { 1 };
    // (the stand's slope-up controls lead its slopes down by this much at the departure)
    let lead: i64 = if lay.stand_b.is_empty() { 0 } else { 5 };
    let intake: Vec<BlockPos> = lay.slots[..3].to_vec();
    // (a peak cell: the cell 2 on from a slot's slope down, at y 1)
    // (a peak cell: the cell 2 on from a slot's slope down at y 1, 3 on from a V slot's)
    let tpk = if lay.tail_b.is_empty() { 2 } else { 3 };
    let peak = |c: [i32; 3], d: i32| [c[0] + tpk * d, 1, c[2]];
    let in_cell = |w: &redsim::world::World, id: u32, c: [i32; 3]| pos_of(w, id).is_some_and(|p| p.x.floor() as i32 == c[0] && p.z.floor() as i32 == c[2] && (p.y.floor() as i32 == c[1] || (p.y - c[1] as f64).abs() < 0.2));
    // (the loop's cell before a given one)
    let cell_before = |c: [i32; 3]| -> [i32; 3] {
        let n = lay.path.len();
        let i = lay.path.iter().position(|(p, _)| *p == c).unwrap_or(0);
        lay.path[(i + n - 1) % n].0
    };
    // settled snapshots: every cart at rest in a place
    let mut snaps: Vec<(i64, Vec<usize>)> = Vec::new();
    let t_start = run.world.time;
    // the triggers: (slot, the train's carts front first)
    let mut stand_train: Option<(i64, Vec<u32>, u8)> = None;
    let mut detect_at: Option<i64> = None;
    let det_cell: Option<BlockPos> = lay.path.iter().find(|(_, k)| *k == Cell::Detector).map(|(c, _)| BlockPos::new(c[0], c[1], c[2]));
    let mut each_at: [Option<i64>; 3] = [None; 3];
    let mut each_done = [false; 3];
    let mut intake_trains: Vec<(i64, Vec<u32>, u8)> = Vec::new();
    let mut arrivals: Vec<String> = Vec::new();
    let mut stand_braked_at: Vec<i64> = Vec::new();
    let mut intake_braked_at: Vec<(i64, i64)> = Vec::new();
    for t in 0..cycles * 100 + 150 {
        let (c, dt) = (t / 100, t % 100);
        if c < cycles && dt == 0 {
            // (who sits in a slot's slope cell, moving or not)
            let in_slot = |id: u32, cell: [i32; 3]| pos_of(&run.world, id).is_some_and(|p| p.x.floor() as i32 == cell[0] && p.z.floor() as i32 == cell[2] && p.y < 1.5);
            let by_cell = |cell: [i32; 3]| carts.iter().copied().find(|id| in_slot(*id, cell));
            let rel: Vec<u32> = (0..3).filter_map(|k| by_cell(lay.slot_cells[q - 1 - k])).collect();
            let dep: Vec<u32> = (0..3).filter_map(|k| { let c = lay.stand_cells[k]; let span = if lay.stand_b.is_empty() { 1.0 } else { 3.0 }; let back = if o.stand_desc { 1.0 } else { 0.0 }; carts.iter().copied().find(|id| pos_of(&run.world, *id).is_some_and(|p| p.x >= c[0] as f64 - back && p.x < c[0] as f64 + span && (p.z - (c[2] as f64 + 0.5)).abs() < 0.6 && p.y < 1.5)) }).collect();
            if rel.len() != 3 || dep.len() != 3 {
                problems.push(format!("slot {c}: the release group holds {} carts and the stand {} at the slot's start", rel.len(), dep.len()));
            }
            set(&mut run.world, &release_group, true);
            stand_train = Some((c, rel, if wired.is_some() && tm.stand_reset.is_none() { 2 } else { 0 }));
            detect_at = None;
            each_at = [None; 3];
            each_done = [false; 3];
            intake_trains.push((c, dep, 0));
            if let Some(p) = wired {
                // the golem pulse into the design's input cell
                run.world.set_block(p, on.clone(), redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_spawn, p.x, p.y, p.z));
            }
        }
        if let (Some(p), Some(was), true) = (wired, &input_was, c < cycles && dt == tm.golem_for) {
            run.world.set_block(p, was.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_spawn, p.x, p.y, p.z, was));
        }
        if let (Some(r), Some(was), true) = (tm.stand_reset, &reset_was, reset_off_at == Some(t)) {
            run.world.set_block(r, was.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_spawn, r.x, r.y, r.z, was));
            reset_off_at = None;
        }
        if c == 0 && tm.inject_at == Some(dt) {
            // the startup batch into the queue's tail, at rest where a caught train rests
            set(&mut run.world, &intake, false);
            set(&mut run.world, &lay.tail_b, false);
            for (k, id) in parked.iter().enumerate() {
                let cell = lay.slot_cells[2 - k];
                let d = lay.slot_dir[2 - k];
                let off = 0.32;
                let p = redsim::entity::Vec3::new(cell[0] as f64 + 0.5 + d[0] as f64 * (off - 0.5), cell[1] as f64 + 1.0 - off + 0.05, cell[2] as f64 + 0.5 + d[1] as f64 * (off - 0.5));
                if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                    run.world.entities.set_pos(i, p);
                    run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                }
                drive.borrow_mut().push(format!("act {} tp e{} {:.3} {:.3} {:.3}", run.world.time - t_spawn, carts.iter().position(|c| c == id).unwrap_or(0), p.x, p.y, p.z));
            }
        }
        if c < cycles && dt == tm.advance_at {
            // the queue's tail must hold every earlier returning train before the advance
            for (c0, _, st) in &intake_trains {
                if *c0 + lag <= c && (*st < 2 || (o.plain && *st != 9)) {
                    problems.push(format!("slot {c}: slot {c0}'s returning train not in the queue's tail at the advance"));
                }
            }
            intake_trains.retain(|(c0, _, st)| *c0 + lag > c || (*st < 2 || (o.plain && *st != 9)));
        }
        if c < cycles {
            if dt == tm.release_for {
                set(&mut run.world, &release_group, false);
            }
            // the stand's batch leaves one slot at a time, front first (S1 at the slot's start,
            // S2 and S3 `depart_every` gt apart; a cart behind crosses more powered cells and
            // would catch the one ahead), each slot's slope up `lead` gt before its slope down
            let shift = o.plain || tm.shift_depart;
            if shift {
                // plain stand: the batch leaves like a queue advance, 3 pulses on all 3 stand
                // slopes (each pulse moves every stand cart one slot on, S1's out), so the 3 leave
                // one by one at the same speed; then the stand opens for the next batch
                for k in 0..3i64 {
                    let a = tm.depart_at + k * tm.depart_period;
                    if dt == a {
                        set(&mut run.world, &lay.stand, true);
                    }
                    if dt == a + tm.pulse {
                        set(&mut run.world, &lay.stand, false);
                    }
                }
                if dt == tm.depart_at + 2 * tm.depart_period + tm.pulse + tm.open_after {
                    set(&mut run.world, &lay.stand, true);
                }
            }
            for k in 0..3usize {
                if shift {
                    break;
                }
                let at = k as i64 * tm.depart_every;
                if let (true, Some(b)) = (dt == at, lay.stand_b.get(k)) {
                    set(&mut run.world, &[*b], true);
                }
                if dt == at + lead {
                    set(&mut run.world, &[lay.stand[k]], true);
                }
            }
            for k in 0..3 {
                let a = tm.advance_at + k * tm.advance_every;
                // (the tail's slopes up go on 12 gt before the first pulse and stay on through
                // the advance and the intake's pass-through: a cart that stopped on one rolls
                // back onto its braked slope down first, the leaving carts climb out boosted,
                // and an arriving queue cart, slow, stops on the slope down before it)
                if k == 0 && !lay.tail_b.is_empty() && dt == a - 12 {
                    set(&mut run.world, &lay.tail_b, true);
                }
                if dt == a {
                    set(&mut run.world, &lay.slots, true);
                }
                if dt == a + tm.pulse {
                    set(&mut run.world, &lay.slots, false);
                    if k == 2 {
                        // the tail open for the returning train: slopes down powered too
                        set(&mut run.world, &intake, true);
                    }
                }
            }
        }
        run.tick();
        if let Some(w) = watch {
            let ps: Vec<String> = w.iter().filter_map(|k| carts.get(*k)).map(|id| run.world.entities.list.iter().find(|e| e.id == *id).map(|e| format!("{:.2},{:.2},{:.2} v{:.3}", e.pos.x, e.pos.y, e.pos.z, e.delta_movement.horizontal_distance())).unwrap_or_default()).collect();
            println!("c{c} +{dt}: {}", ps.join(" | "));
        }
        // the stand fills front first: every slot braked at once (slopes down and up) as the
        // train's front cart reaches S2's peak; its second cart is then in S2 (the train runs 3
        // cells apart), its third in S3, and each stops in its own slot
        if o.plain {
            // plain stand slots: each braked as its own cart is one cell before its slope (S1 for
            // the front cart at S2's peak, S2 for the second at S3's peak, S3 for the third on the
            // run's last cell)
            if let Some((c0, tr, st)) = &mut stand_train {
                if tr.len() == 3 && *st < 7 {
                    // (st's bits: which of S1, S2, S3 are braked; each checked on its own)
                    for k in 0..3usize {
                        if *st & (1 << k) == 0 && in_cell(&run.world, tr[k], cell_before(lay.stand_cells[k])) {
                            set(&mut run.world, &[lay.stand[k]], false);
                            if k == 0 {
                                stand_braked_at.push(t - *c0 * 100);
                            }
                            *st |= 1 << k;
                        }
                    }
                }
                if *st == 7 {
                    let (s1, s2, s3) = (lay.stand_cells[0], lay.stand_cells[1], lay.stand_cells[2]);
                    let rest = [s1, s2, s3].iter().zip(tr.iter()).all(|(cell, id)| place_of(&run.world, *id).is_some_and(|p| places[p] == *cell));
                    if rest {
                        let offs: Vec<String> = [s1, s2, s3].iter().zip(tr.iter()).filter_map(|(cell, id)| pos_of(&run.world, *id).map(|p| format!("{:+.2}", p.x - cell[0] as f64))).collect();
                        arrivals.push(format!("slot {c0}: batch at the stand +{} (offsets {})", t - *c0 * 100, offs.join(" ")));
                        *st = 8;
                    }
                }
            }
        } else if let Some((c0, tr, st)) = &mut stand_train {
            let (s1, s2, s3) = (lay.stand_cells[0], lay.stand_cells[1], lay.stand_cells[2]);
            let trig = if lay.stand_b.is_empty() { [s2[0] + 1, 0, s2[2]] } else { [s2[0] + 3, 1, s2[2]] };
            // (catch slots: braked once the front cart is 0.45 into its catch cell, S2's peak; it
            // then stops ~0.4 down S1's slope, the train's other two likewise in theirs)
            let caught = |w: &redsim::world::World| if o.stand_desc { in_cell(w, tr[0], [s2[0] + 1, s2[1], s2[2]]) } else if lay.stand_b.is_empty() { pos_of(w, tr[0]).is_some_and(|p| p.x >= s2[0] as f64 + 2.45 && (p.z - (s2[2] as f64 + 0.5)).abs() < 0.6) } else { in_cell(w, tr[0], trig) };
            // (a detector rail on S2's slope up, the cell before S1's catch cell: the brake
            // `stand_detect` gt after the front cart first stands on it)
            if let Some(d) = tm.stand_detect {
                // (the layout's detector rail if it has one, else the front cart standing on
                // S2's slope up)
                let fired = match det_cell {
                    Some(dp) => matches!(run.world.get(dp), Block::DetectorRail { powered: true, .. }),
                    None => in_cell(&run.world, tr[0], [s2[0] + 1, s2[1], s2[2]]),
                };
                if detect_at.is_none() && tr.len() == 3 && fired {
                    detect_at = Some(t + d);
                }
            }
            // (one detector rail per stand slot, two cells before its slope: each slot braked
            // `stand_detect_each` gt after its own cart first stands on its detector)
            if let (Some(d), true, 0) = (tm.stand_detect_each, tr.len() == 3, *st) {
                for k in 0..3usize {
                    let det = cell_before(cell_before(lay.stand_cells[k]));
                    if each_at[k].is_none() && in_cell(&run.world, tr[k], det) {
                        each_at[k] = Some(t + d);
                    }
                    if each_at[k] == Some(t) {
                        set(&mut run.world, &[lay.stand[k]], false);
                        each_done[k] = true;
                        if k == 0 {
                            stand_braked_at.push(t - *c0 * 100);
                        }
                    }
                }
                if each_done.iter().all(|b| *b) {
                    *st = 2;
                }
            }
            let detected = detect_at == Some(t);
            let due = tr.len() == 3 && tm.stand_detect_each.is_none() && match (tm.stand_brake, tm.stand_detect) { (Some(b), _) => t - *c0 * 100 == b, (None, Some(_)) => detected, (None, None) => caught(&run.world) };
            if *st == 0 && tr.len() == 3 && due {
                set(&mut run.world, &lay.stand, false);
                set(&mut run.world, &lay.stand_b, false);
                if let (Some(r), Some(_)) = (tm.stand_reset, wired) {
                    // the wired stand's brake: the bench pulses the latch's reset input
                    run.world.set_block(r, on.clone(), redsim::world::flags::ALL);
                    drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_spawn, r.x, r.y, r.z));
                    reset_off_at = Some(t + 2);
                }
                stand_braked_at.push(t - *c0 * 100);
                *st = 2;
            }
            if *st == 2 {
                let rest = [s1, s2, s3].iter().zip(tr.iter()).all(|(cell, id)| place_of(&run.world, *id).is_some_and(|p| places[p] == *cell));
                if rest {
                    let offs: Vec<String> = [s1, s2, s3].iter().zip(tr.iter()).filter_map(|(cell, id)| pos_of(&run.world, *id).map(|p| format!("{:+.2}", p.x - cell[0] as f64))).collect();
                    arrivals.push(format!("slot {c0}: batch at the stand +{} (offsets {})", t - *c0 * 100, offs.join(" ")));
                    *st = 3;
                }
            }
        }
        // the queue's tail fills front first: slots 1 and 2 braked as the returning front cart
        // reaches slot 1's peak, slot 0 as its second reaches slot 0's peak
        for (c0, tr, st) in intake_trains.iter_mut() {
            if wired.is_some() {
                // the design's own redstone brakes the tail: done once the train's 3 carts rest
                // in slots 2, 1, 0
                if *st < 2 && tr.len() == 3 && (0..3).all(|k| place_of(&run.world, tr[k]) == Some(2 - k)) {
                    arrivals.push(format!("slot {c0}: returning train at rest in the tail +{}", t - *c0 * 100));
                    *st = 2;
                }
                continue;
            }
            if o.plain {
                // plain tail slots: slot 2 braked as the front cart is one cell before its slope,
                // slot 1 likewise for the second, slot 0 for the third
                if (t - *c0 * 100) > tm.advance_at + 2 * tm.advance_every + tm.pulse && tr.len() == 3 && *st < 7 {
                    // (st's bits: which of slots 2, 1, 0 are braked; each checked on its own)
                    for k in 0..3usize {
                        let slot = 2 - k;
                        if *st & (1 << k) == 0 && in_cell(&run.world, tr[k], cell_before(lay.slot_cells[slot])) {
                            set(&mut run.world, &[intake[slot]], false);
                            intake_braked_at.push((t - *c0 * 100, slot as i64));
                            *st |= 1 << k;
                        }
                    }
                    if *st == 7 {
                        arrivals.push(format!("slot {c0}: returning train braked into the tail +{}", t - *c0 * 100));
                        *st = 9;
                    }
                }
                continue;
            }
            if (t - *c0 * 100) > tm.advance_at + 2 * tm.advance_every + tm.pulse && tr.len() == 3 {
                let rel_t = t - *c0 * 100;
                let due1 = match tm.intake_brake { Some((b, _)) => rel_t == b, None => in_cell(&run.world, tr[0], lay.slot_peak[1]) };
                if *st == 0 && due1 {
                    set(&mut run.world, &[intake[1], intake[2]], false);
                    set(&mut run.world, &lay.tail_b.iter().skip(1).copied().collect::<Vec<_>>(), false);
                    intake_braked_at.push((t - *c0 * 100, 1));
                    *st = 1;
                }
                let due0 = match tm.intake_brake { Some((_, b)) => rel_t == b, None => in_cell(&run.world, tr[1], lay.slot_peak[0]) };
                if *st == 1 && due0 {
                    set(&mut run.world, &[intake[0]], false);
                    set(&mut run.world, &lay.tail_b.iter().take(1).copied().collect::<Vec<_>>(), false);
                    intake_braked_at.push((t - *c0 * 100, 0));
                    *st = 2;
                    arrivals.push(format!("slot {c0}: returning train braked into the tail +{}", t - *c0 * 100));
                }
            }
        }
        // the drive log's expectations: every cart's position at each slot's end
        if dt == 99 {
            for (k, id) in carts.iter().enumerate() {
                if let Some(p) = pos_of(&run.world, *id) {
                    drive.borrow_mut().push(format!("expect {} pos e{k} {:.3} {:.3} {:.3}", run.world.time - t_spawn, p.x, p.y, p.z));
                }
            }
        }
        // a settled snapshot
        let now: Vec<Option<usize>> = carts.iter().map(|id| place_of(&run.world, *id)).collect();
        if now.iter().all(|p| p.is_some()) {
            let v: Vec<usize> = now.iter().map(|p| p.unwrap()).collect();
            if snaps.last().map_or(true, |(_, l)| *l != v) {
                snaps.push((run.world.time - t_start, v));
            }
        }
    }
    // the order: each settled snapshot every cart 3 places on from the last
    let mut moves = 0;
    for w in snaps.windows(2) {
        let (a, b) = (&w[0].1, &w[1].1);
        if a.iter().zip(b).all(|(x, y)| (x + 3) % 21 == *y) {
            moves += 1;
        } else {
            problems.push(format!("t {}: settled snapshot not 3 places on: {:?} -> {:?}", w[1].0, a, b));
        }
    }
    out.push(format!("{} settled snapshots, {} clean 3-place moves", snaps.len(), moves));
    let span = |v: &[i64]| if v.is_empty() { "-".to_string() } else { format!("+{}..+{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
    out.push(format!("stand braked at {} after its slot's start; tail slots 1-2 braked at {}, slot 0 at {} after the train's departure", span(&stand_braked_at), span(&intake_braked_at.iter().filter(|x| x.1 == 1).map(|x| x.0).collect::<Vec<_>>()), span(&intake_braked_at.iter().filter(|x| x.1 == 0).map(|x| x.0).collect::<Vec<_>>())));
    if let Some(path) = drive_out {
        let mut text = format!("seed {seed}\ndays 0\nwalk random\n# ticks {}\n# t0 {t_spawn}\n# mctas mobfarm queue-loop-run <design> --cycles {cycles} --seed {seed} --release-for {} --advance-at {} --advance-every {} --pulse {}\n# start: Run::start(spec, RunOpts {{ ai: true, ..steady(seed, ticks, Random) }}); settle_tick until false; spawn the design's entity lines in order (e0 .. e20, each a minecart with its villager e<N>.p0); t0 = the time right after that spawn; act t applied when the time is t0 + t, before the next tick\n", run.world.time - t_spawn, tm.release_for, tm.advance_at, tm.advance_every, tm.pulse);
        for l in drive.borrow().iter() {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::write(path, text).map_err(|e| e.to_string())?;
        out.push(format!("drive log: {}", path.display()));
    }
    out.extend(arrivals);
    if verbose {
        let ps: Vec<String> = carts.iter().map(|id| pos_of(&run.world, *id).map(|p| format!("{:.1},{:.1},{:.1}", p.x, p.y, p.z)).unwrap_or_default()).collect();
        out.push(format!("  carts: {}", ps.join(" ")));
        for c in lay.stand_cells.iter() {
            let p = BlockPos::new(c[0], c[1], c[2]);
            out.push(format!("  stand rail {:?}: {:?} on {:?}", c, run.world.get(p), run.world.get(BlockPos::new(c[0], c[1] - 1, c[2]))));
        }
    }
    Ok(format!("{}\n{} problems{}", out.join("\n"), problems.len(), if problems.is_empty() { String::new() } else { format!(":\n  {}", problems.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")) }))
}
