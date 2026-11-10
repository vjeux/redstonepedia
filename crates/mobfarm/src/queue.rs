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
}

impl Default for LoopOpts {
    fn default() -> LoopOpts {
        LoopOpts { slots: 18, run: 20, depart: 3, boost_every: 0, boost_first: 3, straight: false }
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
}

/// The loop's layout: the entry at (0, 1, 0); queue row A going -x (slots 0 .. slots/2), a U-turn,
/// row B going +x (the rest, the head ending at x -1, z 4); the release run +x along z 4, the
/// stand (S3, S2, S1 in travel order), the departure run, a U-turn to z 0 and the return run -x
/// back to the entry.
pub fn loop_layout(o: &LoopOpts) -> LoopLayout {
    if o.straight {
        return loop_layout_straight(o);
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
    for k in 0..half {
        let x = -1 - 3 * k;
        slot_cells.push([x, 0, 0]);
        saw(&mut path, x, 0, -1);
    }
    // the U-turn at the peaks' height, rows 2 apart: a curve, a booster, a curve
    let zb = 2;
    let xu = -1 - 3 * half;
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
    for i in 0..o.run {
        flat(&mut path, x0 + i, zb, i, o.run);
    }
    let xs = x0 + o.run;
    for j in 0..3 {
        let x = xs + 3 * j;
        stand_cells.insert(0, [x, 0, zb]);
        saw(&mut path, x, zb, 1);
    }
    let xd = xs + 9;
    for i in 0..o.depart {
        flat(&mut path, xd + i, zb, i, o.depart);
    }
    let xe = xd + o.depart;
    path.push(([xe, 1, zb], Cell::Plain));
    path.push(([xe, 1, 1], Cell::Boost));
    path.push(([xe, 1, 0], Cell::Plain));
    let n = xe - 1;
    for i in 0..n {
        flat(&mut path, xe - 1 - i, 0, i, n);
    }
    finish_layout(path, slot_cells, stand_cells)
}

/// A loop's layout from its path: a booster's support (a redstone block) powers every powered
/// rail beside it, so no booster whose support touches a slot's slope down.
fn finish_layout(mut path: Vec<([i32; 3], Cell)>, slot_cells: Vec<[i32; 3]>, stand_cells: Vec<[i32; 3]>) -> LoopLayout {
    let ctrl = |c: &[i32; 3]| BlockPos::new(c[0], c[1] - 1, c[2]);
    let slot_rails: std::collections::BTreeSet<[i32; 3]> = path.iter().filter(|(_, k)| *k == Cell::Slot).map(|(c, _)| *c).collect();
    for (c, k) in path.iter_mut() {
        if *k == Cell::Boost {
            let s = [c[0], c[1] - 1, c[2]];
            let touches = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]].iter().any(|d| slot_rails.contains(&[s[0] + d[0], s[1] + d[1], s[2] + d[2]]));
            if touches {
                *k = Cell::Plain;
            }
        }
    }
    LoopLayout { slots: slot_cells.iter().map(ctrl).collect(), stand: stand_cells.iter().map(ctrl).collect(), path, slot_cells, stand_cells }
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
    finish_layout(path, slot_cells, stand_cells)
}

/// The loop's design: its path (each rail on its support), 21 carts with villagers: the queue's
/// slots full, the stand's three slots full.
pub fn loop_design(o: &LoopOpts) -> Result<String> {
    if o.slots + 3 != 21 {
        bail!("the queue's slots and the stand's 3 must hold the 21 carts (slots 18)");
    }
    let lay = loop_layout(o);
    let mut b = Builder::new("carousel-loop-scene");
    let n = lay.path.len();
    for i in 0..n {
        let (c, kind) = lay.path[i];
        let prev = lay.path[(i + n - 1) % n].0;
        let next = lay.path[(i + 1) % n].0;
        let sh = crate::yard::shape(Some(prev), c, Some(next))?;
        let (rail, support) = match kind {
            Cell::Slot => (format!("powered_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
            Cell::Boost => (format!("powered_rail[powered=true,shape={sh},waterlogged=false]"), "redstone_block"),
            Cell::Plain => (format!("rail[shape={sh},waterlogged=false]"), "glass"),
        };
        if b.d.blocks.contains_key(&c) {
            bail!("the loop's path meets itself at {c:?}");
        }
        b.set(c[0], c[1], c[2], &rail);
        b.set(c[0], c[1] - 1, c[2], support);
    }
    let rider = Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}");
    for c in lay.slot_cells.iter().chain(lay.stand_cells.iter()) {
        b.entity("minecart", [c[0] as f64 + 0.5, 0.5, c[2] as f64 + 0.5], rider);
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("queue".into(), format!("scene=Loop carts=21 spacing=3 release=3 riders=1 slots={} run={} depart={} boost={} boost_first={} straight={}", o.slots, o.run, o.depart, o.boost_every, o.boost_first, o.straight as i32));
    b.d.ports.insert("output".into(), [0, 1, 8]);
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
}

impl Default for LoopTiming {
    fn default() -> LoopTiming {
        LoopTiming { release_for: 30, advance_at: 40, advance_every: 20, pulse: 9 }
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
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 21 {
        return Err(format!("{} carts, not 21", carts.len()));
    }
    // the loop's places in travel order: slot 0 .. slot q-1, S3, S2, S1
    let q = o.slots as usize;
    let mut places: Vec<[i32; 3]> = lay.slot_cells.clone();
    places.extend(lay.stand_cells.iter().rev());
    let place_of = |w: &redsim::world::World, id: u32| -> Option<usize> {
        let e = w.entities.list.iter().find(|e| e.id == id)?;
        places.iter().position(|c| (e.pos.x - (c[0] as f64 + 0.5)).abs() < 1.0 && (e.pos.z - (c[2] as f64 + 0.5)).abs() < 0.6 && e.pos.y < 1.5 && e.delta_movement.horizontal_distance_sqr() < 1e-6)
    };
    let pos_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id).map(|e| e.pos);
    // the drive log (mobviewer's replay format): every control change as `act <t> set x y z
    // <state>`, t = gt after the entities' spawn, applied before the tick that brings the time
    // to spawn + t
    let t_spawn = run.world.time;
    let drive: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    let set = |w: &mut redsim::world::World, ps: &[BlockPos], p: bool| {
        for c in ps {
            w.set_block(*c, if p { on.clone() } else { off.clone() }, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", w.time - t_spawn, c.x, c.y, c.z, if p { "redstone_block" } else { "glass" }));
        }
    };
    // (who is where at the start: the design's order)
    for _ in 0..40 {
        run.tick();
    }
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let release_group: Vec<BlockPos> = lay.slots[q - 3..].to_vec();
    let intake: Vec<BlockPos> = lay.slots[..3].to_vec();
    // (a peak cell: the cell 2 on from a slot's slope down, at y 1)
    let peak = |c: [i32; 3], d: i32| [c[0] + 2 * d, 1, c[2]];
    let in_cell = |w: &redsim::world::World, id: u32, c: [i32; 3]| pos_of(w, id).is_some_and(|p| p.x.floor() as i32 == c[0] && p.z.floor() as i32 == c[2] && (p.y.floor() as i32 == c[1] || (p.y - c[1] as f64).abs() < 0.2));
    // settled snapshots: every cart at rest in a place
    let mut snaps: Vec<(i64, Vec<usize>)> = Vec::new();
    let t_start = run.world.time;
    // the triggers: (slot, the train's carts front first)
    let mut stand_train: Option<(i64, Vec<u32>, u8)> = None;
    let mut intake_train: Option<(i64, Vec<u32>, u8)> = None;
    let mut arrivals: Vec<String> = Vec::new();
    for t in 0..cycles * 100 + 150 {
        let (c, dt) = (t / 100, t % 100);
        if c < cycles && dt == 0 {
            let now: Vec<Option<usize>> = carts.iter().map(|id| place_of(&run.world, *id)).collect();
            let by_place = |p: usize| carts.iter().zip(&now).find(|(_, x)| **x == Some(p)).map(|(id, _)| *id);
            let rel: Vec<u32> = (0..3).filter_map(|k| by_place(q - 1 - k)).collect();
            let dep: Vec<u32> = (0..3).filter_map(|k| by_place(q + 2 - k)).collect();
            if rel.len() != 3 || dep.len() != 3 {
                problems.push(format!("slot {c}: the release group holds {} carts and the stand {} at the slot's start", rel.len(), dep.len()));
            }
            set(&mut run.world, &release_group, true);
            set(&mut run.world, &lay.stand, true);
            stand_train = Some((c, rel, 0));
            if let Some((c0, _, st)) = &intake_train {
                if *st < 2 {
                    problems.push(format!("slot {c}: slot {c0}'s returning train not in the queue yet"));
                }
            }
            intake_train = Some((c, dep, 0));
        }
        if c < cycles {
            if dt == tm.release_for {
                set(&mut run.world, &release_group, false);
            }
            for k in 0..3 {
                let a = tm.advance_at + k * tm.advance_every;
                if dt == a {
                    set(&mut run.world, &lay.slots, true);
                }
                if dt == a + tm.pulse {
                    set(&mut run.world, &lay.slots, false);
                    if k == 2 {
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
        // the stand fills front first: S1 and S2 braked as the train's front cart reaches S2's
        // peak, S3 as its second reaches S3's peak
        if let Some((c0, tr, st)) = &mut stand_train {
            let (s1, s2, s3) = (lay.stand_cells[0], lay.stand_cells[1], lay.stand_cells[2]);
            if *st == 0 && tr.len() == 3 && in_cell(&run.world, tr[0], peak(s2, 1)) {
                set(&mut run.world, &[lay.stand[0], lay.stand[1]], false);
                *st = 1;
            }
            if *st == 1 && in_cell(&run.world, tr[1], peak(s3, 1)) {
                set(&mut run.world, &[lay.stand[2]], false);
                *st = 2;
            }
            if *st == 2 {
                let rest = [s1, s2, s3].iter().zip(tr.iter()).all(|(cell, id)| place_of(&run.world, *id).is_some_and(|p| places[p] == *cell));
                if rest {
                    arrivals.push(format!("slot {c0}: batch at the stand +{}", t - *c0 * 100));
                    *st = 3;
                }
            }
        }
        // the queue's tail fills front first: slots 1 and 2 braked as the returning front cart
        // reaches slot 1's peak, slot 0 as its second reaches slot 0's peak
        if let Some((c0, tr, st)) = &mut intake_train {
            if c >= *c0 && (t - *c0 * 100) > tm.advance_at + 2 * tm.advance_every + tm.pulse && tr.len() == 3 {
                if *st == 0 && in_cell(&run.world, tr[0], peak(lay.slot_cells[1], -1)) {
                    set(&mut run.world, &[intake[1], intake[2]], false);
                    *st = 1;
                }
                if *st == 1 && in_cell(&run.world, tr[1], peak(lay.slot_cells[0], -1)) {
                    set(&mut run.world, &[intake[0]], false);
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
