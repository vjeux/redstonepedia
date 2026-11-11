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
    /// Zombie posts (a zombie in a boat on glass, leaves over it) every this many cells along
    /// x `zombie_x` from z 5 north; 0: none.
    pub zombie_every: i32,
    pub zombie_x: i32,
    /// A deep stand: each slot a braked slope linked to its own, two plain slopes up and a peak
    /// (5 cells): a cart that climbs out of a braked slot rolls back into it.
    pub stand_deep: bool,
    /// The queue's 3 tail slots as descending deep slots (braked slopes C and A, one plain slope
    /// up, each a block lower), the return coming in at y 4: fast queue moves out of the tail.
    pub tail_desc: bool,
    /// With `tail_desc`: the 3 spare cells as second peak cells on slots 3-5 instead of flat
    /// cells before the tail.
    pub peak_pad: bool,
    /// The departure turns north right after S1's peak (one cell shorter each way).
    pub short_depart: bool,
    /// farmopt's pocket stand: each cart of the batch in its own gate pocket (counting gates, no
    /// brake timing, no contact), all three released at once.
    pub stand_stop: bool,
    /// With `stand_stop`: the pockets' gates wired (real redstone in the design, no bench
    /// triggers): a detector rail D1 on pocket 1's flat sets a latch Q (a two-torch loop) that
    /// shuts G1 and G2; one input cell REL (a redstone block on, glass off) opens all three gates
    /// and resets Q (the reset dominating while REL is on). G0 open = REL; G1, G2 open = REL or
    /// not Q.
    pub stand_wired: bool,
    /// With `stand_wired`: the stand's fault guard (farmopt's A+B, 2026-11-10): the two booster-ups
    /// plain rails, and each gate slope powered from its own gate's driver dust (a stone under the
    /// dust) instead of a redstone block, so it boosts only while its gate is open: a cart that
    /// drifts back from a crowded pocket is braked and held on a shut gate's slope, then launched
    /// forward when the gate opens; none is boosted back through G1/G2. With it, C: the D1
    /// fallback (`stand_circuit`), the latch set at REL's fall + 37 gt when D1 has not set it.
    pub stand_guard: bool,
    /// The whole design and the bench's frame moved by (x, z) (`queue-loop --shift`): the loop's
    /// cell (0, y, 0) at (x, y, z), so that the game's chunk lines (every 16 from 0) fall where a
    /// build aligned at that offset puts them; the loop's own frame is unchanged otherwise.
    pub shift: [i32; 2],
    /// The re-packing tail (ironspec's call, 2026-11-10): the returning carts roll into a FIFO of
    /// this many gate pockets in place of the 3 tail slots (each pocket's rest slope a detector
    /// rail; a pocket's gate opens while the next pocket is empty; the last pocket's gate, the
    /// intake, opens once after each advance: one cart into slot 0 per advance), and the register
    /// is 3 slots shorter (15 with 21 carts). 0 = the timed tail.
    pub tail_fifo: u32,
    /// The re-packing tail as a two-stage escapement (ironspec's calls, 19:21 and 19:45): the
    /// returning carts roll down a gravity queue of this many plain slopes onto B (a braked
    /// powered slope that holds the next cart), X (plain), P (a braked powered slope against a
    /// gate, pulsed with the gate), R (plain), then the register's slot 0, a sawtooth slot. Each
    /// advance lets P's cart into slot 0, then moves B's onto P. 0 = off.
    pub tail_esc: u32,
    /// The departure climb's boosted cells (from its foot), the rest plain slopes: the returning
    /// carts' speed into the tail. None = all boosted.
    pub climb_boost: Option<u32>,
    /// farmopt's approach lab (2026-11-10): the release run climbs to a flat top and comes down
    /// the last N cells before pocket 3's rest slope with no flat and no curve (slopes down and a
    /// snap turn onto an east-west slope; see `loop_layout_bridge`). 0 = the flat run and curve.
    pub approach: u32,
    /// The departure's crest (2026-11-10): the climb ends this many cells before the chunk line
    /// z 9|10 in a flat crest, then slopes down to the line (a cart halted by frozen carts beyond
    /// it held forward by gravity), boosters past the line and a climb back to the tail's level.
    /// 0 = the climb straight to the run north.
    pub depart_crest: u32,
    /// C' wired (`--cprime`, with `approach` 5 and the guard): the approach detector's
    /// train-synchronous D1 fallback in place of C's timer (`approach::cprime_cells`), its repeater
    /// delays [R_in, R1, R2, R3, R4, R5] in redstone ticks; None = C.
    pub cprime: Option<[i32; 6]>,
    /// The escapement tail's entry: one never-powered braked slope instead of the ledge (a cart
    /// at ≤ 0.4 b/t of movement spends ~3.5 gt on it, its speed halved each tick).
    pub tail_brake: bool,
    /// With `tail_brake`: that slope plain (no speed reducer at the entry; for slow returns).
    pub tail_plain: bool,
    /// The departure climb: a booster on every Nth cell from its foot, plain slopes between
    /// (2: alternate, the speed held about level up the climb); 0 = every cell boosted.
    pub climb_every: u32,
    /// The ledge's landing as a booster slope then a plain slope (not two flats).
    pub ledge_slope: bool,
    /// The escapement's B door: an iron door at B's exit, opened with B's pulse.
    pub esc_door: bool,
    /// The flat escapement: B a flat plain rail held by a fence gate, X an always-on booster.
    pub esc_flat: bool,
    /// The ledge's landing: both landing flats boosters (the landed cart clears the landing point
    /// faster).
    pub landing_boost2: bool,
    /// The escapement's G0: a fence gate at B's entry, shut through B's pulse.
    pub esc_g0: bool,
    /// Admission control: the ledge on the return run, Q queue slopes, G0, A (booster), A'
    /// (latch reset), B, X, P/G_P, R, slot 0.
    pub esc_adm: bool,
    /// Admission control's entry: the 1-block ledge on the return run (else a plain entry).
    pub adm_ledge: bool,
    /// Admission control: a mid-queue gate G1 over the K-th queue slope (0: none).
    pub adm_g1: u32,
    /// Admission control's short entry (with `adm_ledge`): the entry curve as the pre-ledge, a
    /// one-cell booster landing.
    pub adm_short: bool,
    /// Admission control: the last queue slope an always-on booster.
    pub adm_qboost: bool,
    /// Admission control: B, a flat always-on booster at the queue's foot, the front pressed
    /// against G0 on it (its power runs into P: P always on).
    pub adm_flatb: bool,
}

impl Default for LoopOpts {
    fn default() -> LoopOpts {
        LoopOpts { slots: 18, run: 20, depart: 3, boost_every: 0, boost_first: 3, straight: false, drop: false, vstand: false, tail_v: true, bridge: true, row_gap: 2, uturn_boost: false, line: false, tail_deep: false, plain: false, stand_desc: false, detector: false, zombie_every: 0, zombie_x: 3, stand_deep: false, tail_desc: false, peak_pad: false, short_depart: false, stand_stop: false, stand_wired: false, stand_guard: false, shift: [0, 0], tail_fifo: 0, tail_esc: 0, climb_boost: None, approach: 0, depart_crest: 0, cprime: None, tail_brake: false, tail_plain: false, climb_every: 0, ledge_slope: false, esc_door: false, esc_flat: false, landing_boost2: false, esc_g0: false, esc_adm: false, adm_ledge: false, adm_g1: 0, adm_short: false, adm_qboost: false, adm_flatb: false }
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
    /// The stop-based stand (`stand_stop`): its gate (the stop), and the re-spacer's pockets from
    /// the last (gate 0) back: each pocket's gate and the rail cell under it (the gate's slope).
    pub stop_gate: Option<BlockPos>,
    pub pockets: Vec<(BlockPos, [i32; 3])>,
    /// The wired pocket stand's (`stand_wired`) input cell REL: a redstone block opens the three
    /// gates and resets the latch, glass lets the latch count.
    pub stand_rel: Option<BlockPos>,
    /// The re-packing tail's pockets (`tail_fifo`), in travel order (the first the returning carts
    /// reach first, the last the intake into slot 0): each pocket's gate and its rest cell (a
    /// detector rail slope).
    pub fifo: Vec<(BlockPos, [i32; 3])>,
    /// The escapement tail (`tail_esc`): E's control (the block under it) and the queue's cells
    /// (its plain slopes and E, in travel order).
    pub esc: Option<BlockPos>,
    /// The escapement's P control (the block under P, pulsed with the gate).
    pub esc_p: Option<BlockPos>,
    /// The escapement's B door (`esc_door`): an iron door's lower half over X, its plate at X's
    /// south face, so B's cart rests at most 0.51 into B and its follower's centre is never on B.
    pub esc_door: Option<BlockPos>,
    /// The flat escapement's (`esc_flat`) G_B: the fence gate at the flat B's end, opened by the
    /// bench in B's window.
    pub esc_gb: Option<BlockPos>,
    /// The escapement's G0 (`esc_g0`): a fence gate at B's entry, shut through B's pulse.
    pub esc_g0: Option<BlockPos>,
    /// Admission control's A' cell (`esc_adm`): a cart crossing it resets the latch (G0 shuts).
    pub esc_adm_a1: Option<[i32; 3]>,
    /// Admission control's mid-queue gate G1 (`adm_g1`), pulsed with G0.
    pub esc_g1: Option<BlockPos>,
    /// The flat escapement's B rail cell.
    pub esc_flat_b: Option<[i32; 3]>,
    pub queue_cells: Vec<[i32; 3]>,
    /// Rail shapes set explicitly (`approach`'s snap turn: an east-west slope under a north-south
    /// run's end), and extra blocks (the slope's sturdy high side, the departure crest's).
    pub shape_over: Vec<([i32; 3], &'static str)>,
    pub extra: Vec<([i32; 3], &'static str)>,
    /// Boosters the layout asked for but placed plain (their redstone block would touch a
    /// controlled rail): the generator refuses a design with any.
    pub downgraded: Vec<[i32; 3]>,
    /// The design's intended flights (`crate::flight`): take-off cell, landing cells, maximum
    /// airtime; the runners exempt them from the off-the-rails check.
    pub flights: Vec<crate::flight::Flight>,
}

/// The loop's layout in the bench's frame: [`loop_layout_frame`] moved by `o.shift`.
pub fn loop_layout(o: &LoopOpts) -> LoopLayout {
    let mut lay = loop_layout_frame(o);
    let [sx, sz] = o.shift;
    if sx != 0 || sz != 0 {
        let c = |p: &mut [i32; 3]| {
            p[0] += sx;
            p[2] += sz;
        };
        let b = |p: &mut BlockPos| *p = BlockPos::new(p.x + sx, p.y, p.z + sz);
        lay.path.iter_mut().for_each(|(p, _)| c(p));
        lay.slots.iter_mut().for_each(b);
        lay.stand.iter_mut().for_each(b);
        lay.slot_cells.iter_mut().for_each(c);
        lay.stand_cells.iter_mut().for_each(c);
        lay.stand_b.iter_mut().for_each(b);
        lay.tail_b.iter_mut().for_each(b);
        lay.slot_peak.iter_mut().for_each(c);
        lay.stop_gate.iter_mut().for_each(b);
        lay.pockets.iter_mut().for_each(|(g, p)| {
            b(g);
            c(p);
        });
        lay.stand_rel.iter_mut().for_each(b);
        lay.shape_over.iter_mut().for_each(|(p, _)| c(p));
        lay.extra.iter_mut().for_each(|(p, _)| c(p));
        lay.fifo.iter_mut().for_each(|(g, p)| {
            b(g);
            c(p);
        });
        for f in [&mut lay.esc, &mut lay.esc_p, &mut lay.esc_door, &mut lay.esc_gb, &mut lay.esc_g0, &mut lay.esc_g1] {
            f.iter_mut().for_each(b);
        }
        lay.esc_adm_a1.iter_mut().for_each(c);
        lay.esc_flat_b.iter_mut().for_each(c);
        lay.queue_cells.iter_mut().for_each(c);
        lay.flights = lay.flights.iter().map(|f| f.shifted(sx, sz)).collect();
    }
    lay
}

/// The loop's layout: the entry at (0, 1, 0); queue row A going -x (slots 0 .. slots/2), a U-turn,
/// row B going +x (the rest, the head ending at x -1, z 4); the release run +x along z 4, the
/// stand (S3, S2, S1 in travel order), the departure run, a U-turn to z 0 and the return run -x
/// back to the entry.
/// A design file's own declared states and flights (its `declared:` and `flights:` header lines,
/// a pasted part's carried in by `mobfarm paste`), which the runners check on top of the ones
/// derived from the loop's options.
#[derive(Clone, Debug, Default)]
pub struct Headers {
    pub declared: Vec<crate::declared::Decl>,
    pub flights: Vec<crate::flight::Flight>,
}

/// The design's `declared:` and `flights:` headers.
pub fn headers_of(meta: &std::collections::BTreeMap<String, String>) -> Result<Headers, String> {
    Ok(Headers {
        declared: match meta.get("declared") { Some(s) => crate::declared::parse(s)?, None => Vec::new() },
        flights: match meta.get("flights") { Some(s) => crate::flight::parse(s)?, None => Vec::new() },
    })
}

/// The derived declared states with a header's added: an identical entry once, a cell declared two
/// different ways a mismatch of its own (returned with the merged list).
pub fn merge_declared(mut ds: Vec<crate::declared::Decl>, hdr: &[crate::declared::Decl]) -> (Vec<crate::declared::Decl>, Vec<String>) {
    let mut bad = Vec::new();
    for h in hdr {
        match ds.iter().find(|d| d.cell == h.cell) {
            Some(d) if d == h => {}
            Some(d) => bad.push(format!("{},{},{}: declared two ways: {} (options) and {} (header)", h.cell[0], h.cell[1], h.cell[2], crate::declared::format(std::slice::from_ref(d)), crate::declared::format(std::slice::from_ref(h)))),
            None => ds.push(h.clone()),
        }
    }
    (ds, bad)
}

/// The derived flights with a header's added (an identical one once).
pub fn merge_flights(mut fs: Vec<crate::flight::Flight>, hdr: &[crate::flight::Flight]) -> Vec<crate::flight::Flight> {
    for h in hdr {
        if !fs.contains(h) {
            fs.push(h.clone());
        }
    }
    fs
}

/// A loop design's declared states (`crate::declared`), derived from its options: every rail cell
/// of the layout (a booster powered, always on; a braked slot or linked rail unpowered; a plain or
/// detector rail present), the tail's gates in their starting states, `adm_qboost`'s booster. The
/// generator writes them as the design's `declared:` line; the runners check the world against
/// them at tick 0 and the boosters again after the warm-up, so a requested cell that the design
/// does not have (the qboost hazard) fails the run.
pub fn declared_states(o: &LoopOpts) -> Vec<crate::declared::Decl> {
    use crate::declared::Decl;
    let lay = loop_layout(o);
    let mut ds: Vec<Decl> = Vec::new();
    for (c, k) in &lay.path {
        ds.push(match k {
            Cell::Boost => Decl::new(*c, "powered_rail", &[("powered", "true")], true),
            Cell::Slot | Cell::Linked => Decl::new(*c, "powered_rail", &[("powered", "false")], false),
            Cell::Plain => Decl::new(*c, "rail", &[], false),
            Cell::Detector => Decl::new(*c, "detector_rail", &[], false),
        });
    }
    // (an explicit rail shape that must hold: the approach's snap turn, an east-west slope under
    // the north-south run's end)
    for (c, s) in &lay.shape_over {
        if let Some(d) = ds.iter_mut().find(|d| d.cell == *c) {
            d.props.push(("shape".into(), s.to_string()));
        }
    }
    let gate = |p: BlockPos, open: bool| Decl::new([p.x, p.y, p.z], "oak_fence_gate", &[("open", if open { "true" } else { "false" })], false);
    // (the pocket stand's gates G0, G1, G2: shut at tick 0, the batch held in its pockets)
    if o.stand_stop {
        for (g, _) in &lay.pockets {
            ds.push(gate(*g, false));
        }
    }
    for (g, _) in &lay.fifo {
        ds.push(gate(*g, false));
    }
    if let Some(g) = lay.esc_g0 {
        ds.push(gate(g, !o.esc_adm));
    }
    for g in lay.esc_g1.iter().chain(lay.esc_gb.iter()) {
        ds.push(gate(*g, false));
    }
    if let (true, Some(g0)) = (o.esc_adm && o.adm_qboost, lay.esc_g0) {
        let c = [g0.x, g0.y, g0.z - 1];
        ds.retain(|d| d.cell != c);
        ds.push(Decl::new(c, "powered_rail", &[("powered", "true")], true));
    }
    // (`adm_flatb`: B always on, and P with it (B's power runs along the rail))
    if let (true, Some(g0)) = (o.esc_adm && o.adm_flatb, lay.esc_g0) {
        let b = [g0.x, g0.y, g0.z - 1];
        let p = [g0.x, g0.y - 1, g0.z];
        ds.retain(|d| d.cell != b && d.cell != p);
        ds.push(Decl::new(b, "powered_rail", &[("powered", "true")], true));
        ds.push(Decl::new(p, "powered_rail", &[("powered", "true")], true));
    }
    ds
}

/// The escapement tail's level (`tail_esc`): the return run's and the ledge's y (the queue of Q
/// slopes, B, X, P and R stepping down to slot 0 at y 0; 2 more with `ledge_slope`'s landing).
pub fn esc_ye(o: &LoopOpts) -> i32 {
    if o.esc_adm {
        // (slot 0 at y 0, R 1, P 2, G0's level 3, the Q queue slopes up to the curve at 3 + Q;
        // with the ledge the return run (the pre-ledge) one higher)
        return o.tail_esc as i32 + 3 + if o.adm_ledge { 1 } else { 0 };
    }
    if o.esc_flat {
        // (slot 0 at y 0, R 1, P 2, Y 3, X 4, the flat B 5, then the Q queue slopes)
        return o.tail_esc as i32 + 5;
    }
    o.tail_esc as i32 + 6 + if o.ledge_slope && !o.tail_brake { if o.tail_esc >= 5 { 1 } else { 2 } } else { 0 }
}

/// The loop's layout: the entry at (0, 1, 0); queue row A going -x (slots 0 .. slots/2), a U-turn,
/// row B going +x (the rest, the head ending at x -1, z 4); the release run +x along z 4, the
/// stand (S3, S2, S1 in travel order), the departure run, a U-turn to z 0 and the return run -x
/// back to the entry.
pub fn loop_layout_frame(o: &LoopOpts) -> LoopLayout {
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
    // (a booster whose redstone block would touch a controlled rail is placed plain; recorded, so
    // the generator can refuse a requested booster that silently vanished (ironspec 01:04))
    let mut downgraded: Vec<[i32; 3]> = Vec::new();
    for (c, k) in path.iter_mut() {
        if *k == Cell::Boost {
            let s = [c[0], c[1] - 1, c[2]];
            let touches = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]].iter().any(|d| slot_rails.contains(&[s[0] + d[0], s[1] + d[1], s[2] + d[2]]));
            if touches {
                *k = Cell::Plain;
                downgraded.push(*c);
            }
        }
    }
    LoopLayout { slots: slot_cells.iter().map(ctrl).collect(), stand: stand_cells.iter().map(ctrl).collect(), stand_b, tail_b, slot_dir, slot_peak, path, slot_cells, stand_cells, stop_gate: None, pockets: Vec::new(), stand_rel: None, fifo: Vec::new(), esc: None, esc_p: None, esc_door: None, esc_gb: None, esc_g0: None, esc_adm_a1: None, esc_g1: None, esc_flat_b: None, queue_cells: Vec::new(), shape_over: Vec::new(), extra: Vec::new(), downgraded, flights: Vec::new() }
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
    let half = if o.row_gap == 3 { if o.tail_fifo > 0 || o.tail_esc > 0 { o.slots - 10 } else if o.tail_deep || o.tail_desc { o.slots / 2 - 2 } else { o.slots / 2 - 1 } } else { o.slots / 2 };
    // (`tail_desc`: the return comes in 3 blocks higher, at y 4, and the tail's 3 slots step down
    // to the queue's level; `tail_fifo` N: at y 1 + N, the FIFO's pockets each a block lower;
    // `tail_esc` Q: at y Q + 6, the ledge, the gravity queue and the escapement stepping down)
    let ye = if o.tail_esc > 0 { esc_ye(o) } else if o.tail_fifo > 0 { 2 + o.tail_fifo as i32 } else if o.tail_desc { 4 } else { 1 };
    // row A from its entry curve at (0, ye, zr): north (+z); with admission control (`esc_adm`)
    // the ledge sits on the return run and the curve is the landing's level, one lower
    path.push(([0, ye, zr], Cell::Plain));
    let mut z = zr + 1;
    let mut fifo: Vec<(BlockPos, [i32; 3])> = Vec::new();
    let mut esc: Option<BlockPos> = None;
    let mut esc_p: Option<BlockPos> = None;
    let mut esc_door: Option<BlockPos> = None;
    let mut esc_gb: Option<BlockPos> = None;
    let mut esc_g0: Option<BlockPos> = None;
    let mut esc_flat_b: Option<[i32; 3]> = None;
    let mut esc_adm_a1: Option<[i32; 3]> = None;
    let mut esc_g1: Option<BlockPos> = None;
    let mut flights: Vec<crate::flight::Flight> = Vec::new();
    let mut queue_cells: Vec<[i32; 3]> = Vec::new();
    if o.tail_esc > 0 && o.esc_adm {
        // admission control (ironspec's approval 23:10), as measured at 23:34: Q plain slopes
        // north from the curve (the gravity queue: storage only), G0 (a fence gate over P's upper
        // end: the queue's front rests against it), P (a braked powered slope, pulsed with its gate
        // G_P), R (a plain slope under G_P), slot 0. Each advance: G_P and P (P's cart into slot
        // 0); then G0 opens (the latch's set) and shuts once a cart is on P (the latch's reset, a
        // tripwire over P) or after its window. The admitted cart rolls onto P from rest and stops
        // there (the brake); its follower, in contact behind it, stays off P; once G0 is shut the
        // queue's impacts stop at G0 and can push P's cart at most 0.675 into P, the follower's
        // centre held 0.115 short of P. A gate that shuts on a cart already inside its box lets that
        // cart move on out, so G0 needs no speed difference to split a pressed pair. The tripwire
        // keeps G0 shut while P holds a cart (an extra set does nothing)
        let q = o.tail_esc as i32;
        // (`adm_ledge`: escL3's entry inside the tail, the pre-ledge flat at the return's level, a
        // gap, the landing (a booster flat, then a plain flat) a level lower; else the queue's
        // slopes start at the curve)
        let top = if o.adm_ledge && o.adm_short {
            // (`adm_short`: the entry curve itself is the pre-ledge; the gap, then a one-cell
            // booster landing: two cells for two more queue slopes)
            path.push(([0, ye - 1, z + 1], Cell::Boost));
            z += 2;
            ye - 1
        } else if o.adm_ledge {
            path.push(([0, ye, z], Cell::Plain));
            path.push(([0, ye - 1, z + 2], Cell::Boost));
            path.push(([0, ye - 1, z + 3], Cell::Plain));
            // (the ledge's flight, declared: off the pre-ledge, onto either landing flat, within 6 gt)
            flights.push(crate::flight::Flight { take_off: [0, ye, z], landing: vec![[0, ye - 1, z + 2], [0, ye - 1, z + 3]], max_air: 6 });
            z += 4;
            ye - 1
        } else {
            ye
        };
        for k in 0..q {
            // (`adm_g1` K: a second gate G1 over the K-th slope's upper end splits the queue in
            // two stages; G1 pulses with G0, so a cart enters the lower stage from rest)
            if o.adm_g1 > 0 && k == o.adm_g1 as i32 {
                esc_g1 = Some(BlockPos::new(0, top - k, z));
            }
            path.push(([0, top - 1 - k, z], Cell::Plain));
            queue_cells.push([0, top - 1 - k, z]);
            z += 1;
        }
        let lg = top - q; // G0's level: the last queue slope's lower end
        // (`adm_flatb`: B, a flat always-on booster at the queue's foot (placed by the generator,
        // powered by a redstone block beside it, east: one under it would touch P), the front
        // pressed against G0 at its end, its follower 0.98 back on the plain slope above; B's power
        // runs on along the rail into P: P always on)
        if o.adm_flatb {
            path.push(([0, lg, z], Cell::Plain));
            queue_cells.push([0, lg, z]);
            z += 1;
        }
        esc_g0 = Some(BlockPos::new(0, lg, z));
        let pz = z;
        path.push(([0, lg - 1, pz], Cell::Linked));
        queue_cells.push([0, lg - 1, pz]);
        esc_adm_a1 = Some([0, lg - 1, pz]);
        fifo.push((BlockPos::new(0, lg - 1, pz + 1), [0, lg - 1, pz]));
        esc_p = Some(BlockPos::new(0, lg - 2, pz));
        path.push(([0, lg - 2, pz + 1], Cell::Plain));
        path.push(([0, lg - 3, pz + 2], Cell::Slot));
        path.push(([0, lg - 3, pz + 3], Cell::Plain));
        path.push(([0, lg - 2, pz + 4], Cell::Plain));
        slot_cells.push([0, lg - 3, pz + 2]);
        z = pz + 5;
    } else if o.tail_esc > 0 && o.esc_flat {
        // the flat escapement (`esc_flat`): 7 − Q flats, Q plain slopes down (the gravity queue),
        // B a FLAT plain rail with a fence gate G_B at its end, X an always-on booster slope under
        // G_B, Y a plain slope (X's power kept off P), P a braked powered slope against its gate
        // G_P (pulsed with it), R a plain slope, slot 0. On the flat B the cart rests against G_B
        // at 0.885 and its follower's box meets it with its centre 0.095 short of B, on the slope:
        // B holds exactly one cart whatever arrives. G_B opens: the follower's weight pushes B's
        // cart onto X, X boosts it away; the follower slides onto B slowly and G_B shuts long before
        // it reaches it. No power on B, so nothing on B is ever boosted, either way.
        let q = o.tail_esc as i32;
        for _ in 0..(7 - q).max(0) {
            path.push(([0, ye, z], Cell::Plain));
            z += 1;
        }
        for k in 0..q {
            path.push(([0, ye - 1 - k, z], Cell::Plain));
            queue_cells.push([0, ye - 1 - k, z]);
            z += 1;
        }
        let lb = ye - q;
        path.push(([0, lb, z], Cell::Plain));
        queue_cells.push([0, lb, z]);
        esc_flat_b = Some([0, lb, z]);
        esc_gb = Some(BlockPos::new(0, lb, z + 1));
        path.push(([0, lb - 1, z + 1], Cell::Boost));
        queue_cells.push([0, lb - 1, z + 1]);
        path.push(([0, lb - 2, z + 2], Cell::Plain));
        queue_cells.push([0, lb - 2, z + 2]);
        let pz = z + 3;
        path.push(([0, lb - 3, pz], Cell::Linked));
        queue_cells.push([0, lb - 3, pz]);
        fifo.push((BlockPos::new(0, lb - 3, pz + 1), [0, lb - 3, pz]));
        esc_p = Some(BlockPos::new(0, lb - 4, pz));
        path.push(([0, lb - 4, pz + 1], Cell::Plain));
        path.push(([0, lb - 5, pz + 2], Cell::Slot));
        path.push(([0, lb - 5, pz + 3], Cell::Plain));
        path.push(([0, lb - 4, pz + 4], Cell::Plain));
        slot_cells.push([0, lb - 5, pz + 2]);
        z = pz + 5;
    } else if o.tail_esc > 0 {
        // the escapement tail (ironspec's decisions 19:21, 19:45): a flat (the queue's head stays
        // where the timed tail left it, z 19, and slots 8/9/10 sit where the timed tail's 12/13/14
        // were: the nap drop lines unmoved), a flat, a one-cell gap (the ledge: off its rail a
        // cart's speed is clamped to 0.4 b/t and halved while it still touches the rail's top, so
        // every returning cart lands at ~0.1-0.2 b/t whatever its train did), a booster flat (the
        // landed cart leaves the landing point at once: a slow one lingering there was hit by the
        // next cart in the air, which fell into the gap) and a flat a block lower, Q plain slopes
        // down (the gravity queue), B (a braked powered slope, its own control: it holds the next
        // cart), X (a plain slope: B's and P's powered rails apart, no box of B's cart reaching
        // P's), P (a braked powered slope, its own control, pulsed with the gate: its cart rests
        // against the gate), the gate over R (a plain slope: P's power stays off slot 0), then
        // slot 0 as a sawtooth slot. Each advance: gate and P open together (P's cart leaves
        // boosted and enters slot 0 as a queue move does), then B (its cart crosses X and is
        // braked on P)
        let q = o.tail_esc as i32;
        let l = ye - 1;
        if o.tail_brake {
            // (`tail_brake`: no ledge. Four flats, then one braked slope that is never powered:
            // a returning cart moves at most 0.4 b/t whatever its speed, so it spends ~3.5 gt on
            // the cell and leaves with its speed halved each tick, ~1/11 of it (1.0-2.4 b/t in,
            // 0.1-0.2 out), carts in contact included; arrivals are always fast off the boosted
            // climb, and the queue below holds the worst burst, so no slow cart ever stops on it)
            // (the flats keep the queue's head at z 19 whatever Q: 7 − Q of them)
            for _ in 0..(7 - q).max(0) {
                path.push(([0, ye, z], Cell::Plain));
                z += 1;
            }
            path.push(([0, l, z], if o.tail_plain { Cell::Plain } else { Cell::Linked }));
            queue_cells.push([0, l, z]);
            z += 1;
        } else {
            // (8 − Q cells before the queue's slopes keep the head at z 19: 4 − Q head flats (none
            // from Q = 4), the pre-ledge flat, the gap and the landing, one landing cell for Q = 5)
            for _ in 0..(4 - q).max(0) {
                path.push(([0, ye, z], Cell::Plain));
                z += 1;
            }
            path.push(([0, ye, z], Cell::Plain));
            let take_off = [0, ye, z];
            z += 2;
            let landing: &[Cell] = if q >= 5 { &[Cell::Boost] } else if o.landing_boost2 { &[Cell::Boost, Cell::Boost] } else { &[Cell::Boost, Cell::Plain] };
            let mut land_cells: Vec<[i32; 3]> = Vec::new();
            if o.ledge_slope {
                // (`ledge_slope`: the landing is a booster slope then a plain slope down, so a
                // landed cart leaves the landing point at once and a cart behind it, a few gt
                // later, lands clear of it)
                for (k, cell) in landing.iter().enumerate() {
                    path.push(([0, l - 1 - k as i32, z], *cell));
                    queue_cells.push([0, l - 1 - k as i32, z]);
                    land_cells.push([0, l - 1 - k as i32, z]);
                    z += 1;
                }
            } else {
                for cell in landing {
                    path.push(([0, l, z], *cell));
                    queue_cells.push([0, l, z]);
                    land_cells.push([0, l, z]);
                    z += 1;
                }
            }
            // (the ledge's flight, declared: off the pre-ledge, onto a landing cell, within 6 gt)
            flights.push(crate::flight::Flight { take_off, landing: land_cells, max_air: 6 });
        }
        // (the queue's slopes start below the landing: lower by its slopes with `ledge_slope`)
        let l = if o.ledge_slope && !o.tail_brake { l - if q >= 5 { 1 } else { 2 } } else { l };
        for k in 0..q {
            path.push(([0, l - 1 - k, z], Cell::Plain));
            queue_cells.push([0, l - 1 - k, z]);
            z += 1;
        }
        let by = l - q - 1;
        path.push(([0, by, z], Cell::Linked));
        queue_cells.push([0, by, z]);
        esc = Some(BlockPos::new(0, by - 1, z));
        if o.esc_g0 {
            // (G0: a fence gate at B's entry, over B's upper end, shut through B's pulse: a cart
            // arriving fast down the queue while B is powered waits against it instead of being
            // launched with B's cart)
            esc_g0 = Some(BlockPos::new(0, by + 1, z));
        }
        if o.esc_door {
            esc_door = Some(BlockPos::new(0, by, z + 1));
        }
        // (X a plain slope between B and P: their powered rails stay apart, and no box of B's cart
        // reaches P's; P a braked powered slope pulsed with the gate: a cart let through leaves
        // boosted, and P is braked whenever a cart could arrive on it, so nothing on it is ever
        // boosted backward)
        path.push(([0, by - 1, z + 1], Cell::Plain));
        queue_cells.push([0, by - 1, z + 1]);
        let pz = z + 2;
        path.push(([0, by - 2, pz], Cell::Linked));
        queue_cells.push([0, by - 2, pz]);
        fifo.push((BlockPos::new(0, by - 2, pz + 1), [0, by - 2, pz]));
        esc_p = Some(BlockPos::new(0, by - 3, pz));
        // (R plain, then slot 0 a plain sawtooth slot: a cart let out of P gathers speed down R
        // and enters slot 0 as a queue move enters a slot, so the register's first moves are its
        // usual ones; R keeps P's power off slot 0)
        path.push(([0, by - 3, pz + 1], Cell::Plain));
        path.push(([0, by - 4, pz + 2], Cell::Slot));
        path.push(([0, by - 4, pz + 3], Cell::Plain));
        path.push(([0, by - 3, pz + 4], Cell::Plain));
        slot_cells.push([0, by - 4, pz + 2]);
        z = pz + 5;
    }
    if o.tail_fifo > 0 {
        // the re-packing tail: flat cells first (the head where the timed tail left it), then N
        // gate pockets as the pocket stand's (a flat; the rest slope down, the cart pressed by
        // gravity against the gate; the gate's slope, a booster under a fence gate over its high
        // end; a booster back up), each a block lower; then the register's slot 0, a descending
        // deep slot as the timed tail's last (a flat, braked slopes C and A, one plain slope up,
        // the peak): the last pocket's gate opens while slot 0 is empty between advances, and a
        // cart from it, fast off the boosters, stops on C or A
        let n = o.tail_fifo as i32;
        // (pocket 0's flat is the entry curve itself)
        z -= 1;
        for k in 0..n {
            let y = ye - k;
            if k > 0 {
                path.push(([0, y, z], Cell::Plain));
            }
            // (the rest slope a booster too, powered rail all through: a cart let through starts at
            // once, not creeping from rest; it is pressed against the shut gate harmlessly)
            path.push(([0, y - 1, z + 1], Cell::Boost));
            path.push(([0, y - 2, z + 2], Cell::Boost));
            path.push(([0, y - 2, z + 3], Cell::Boost));
            fifo.push((BlockPos::new(0, y - 1, z + 2), [0, y - 1, z + 1]));
            z += 4;
        }
        path.push(([0, 2, z], Cell::Plain));
        path.push(([0, 1, z + 1], Cell::Linked));
        path.push(([0, 0, z + 2], Cell::Slot));
        path.push(([0, 0, z + 3], Cell::Plain));
        path.push(([0, 1, z + 4], Cell::Plain));
        slot_cells.push([0, 0, z + 2]);
        z += 5;
    }
    if o.tail_desc && !o.peak_pad && o.tail_fifo == 0 && o.tail_esc == 0 {
        // (3 flat cells first: the descending slots are 3 cells shorter than deep ones, and the
        // queue's head stays 19 cells north of the stand, outside the follow box)
        for _ in 0..3 {
            path.push(([0, ye, z], Cell::Plain));
            z += 1;
        }
    }
    for k in (if o.tail_fifo > 0 || o.tail_esc > 0 { 1 } else { 0 })..half {
        if o.tail_desc && k < 3 && o.tail_fifo == 0 && o.tail_esc == 0 {
            // a descending deep slot: a braked slope C linked to the slot's slope A (two blocks of
            // braked descent: a fast returning cart stops on one of them), one plain slope up B,
            // the peak a block lower than the last slot's: a queue move climbs only one block
            // out, as fast as a sawtooth slot's
            let h = ye - k as i32;
            slot_cells.push([0, h - 2, z + 1]);
            path.push(([0, h - 1, z], Cell::Linked));
            path.push(([0, h - 2, z + 1], Cell::Slot));
            path.push(([0, h - 2, z + 2], Cell::Plain));
            path.push(([0, h - 1, z + 3], Cell::Plain));
            z += 4;
            continue;
        }
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
            if o.tail_desc && (3..6).contains(&k) && o.peak_pad {
                // (a 2-cell peak on slots 3-5: the descending tail slots are 3 cells shorter than
                // deep ones, and the queue's head stays 19 cells north of the stand, outside the
                // follow box, without lengthening the return; a move over a 2-cell peak takes
                // ~17 gt, inside the 20 between pulses)
                path.push(([0, 1, z], Cell::Plain));
                z += 1;
            }
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
    let mut shape_over: Vec<([i32; 3], &'static str)> = Vec::new();
    let mut extra: Vec<([i32; 3], &'static str)> = Vec::new();
    if o.approach > 0 && o.stand_stop {
        // farmopt's approach lab (`approach` N, 2026-11-10): no curve and no flat in the last N
        // cells before pocket 3's rest slope. The head's peak and pocket 3's rest slope are level
        // (both at height 1), and every cell a stopped cart leaves by itself costs a block of
        // descent (a slope down; a kick booster sits after a one-block step), while every climb
        // ends in a flat: so the run climbs first. Boosters at the peaks' height from the head's
        // peak to the climb's foot, N plain slopes up, the top T (a flat at height N + 1: the one
        // cell where a stopped cart stays), N - 2 slopes down along x xb to z 1, then the snap
        // turn: an east-west slope down east one block below the run's last cell, (xb, 2, 0): a
        // cart whose centre crosses into that column takes the rail below at once (no air, no
        // ledge to perch on) and keeps its speed, turned east (the old movement's
        // min(2, |v|) along the rail; gravity picks east at 0), then pocket 3's cell, a slope down
        // too (from height 2). A cart stopped anywhere from T's next cell to pocket 3's rest slope
        // rolls on by gravity. The climb's third cell a detector rail (C', the train-synchronous
        // D1 fallback's edge: one rising edge per train, no powered rail beside it, no rest cell
        // under it)
        let n = o.approach as i32;
        let foot = 2 * n - 1;
        while z > foot {
            path.push(([xb, 1, z], Cell::Boost));
            z -= 1;
        }
        for k in 0..n {
            path.push(([xb, 1 + k, foot - k], if k == 2 && n >= 4 { Cell::Detector } else { Cell::Plain }));
        }
        path.push(([xb, n + 1, n - 1], Cell::Plain));
        for k in (1..=n - 2).rev() {
            path.push(([xb, 2 + k, k], Cell::Plain));
        }
        // (the landing: an ascending_west rail, its high side's sturdy block west of it)
        path.push(([xb, 2, 0], Cell::Plain));
        shape_over.push(([xb, 2, 0], "ascending_west"));
        extra.push(([xb - 1, 2, 0], "glass"));
    } else {
        while z >= 1 {
            let k = if i < o.boost_first { Cell::Boost } else if o.detector && z == 4 { Cell::Detector } else { Cell::Plain };
            path.push(([xb, 1, z], k));
            z -= 1;
            i += 1;
        }
    }
    // the curve into the stand, the catch cell before S3's slope
    let xs = xb + 2;
    if !(o.approach > 0 && o.stand_stop) {
        path.push(([xb, 1, 0], Cell::Plain));
    }
    if o.stand_desc || o.stand_deep {
        // a descending stand: each slot a braked slope C linked to its own slope A, then a plain
        // slope up B, every slot one block lower than the last (S3's slope at y -1, S1's at -3):
        // a fast cart entering a braked slot stops on C or A, always on a slope, wherever the
        // brake caught it; a deep stand (`stand_deep`): the same slot level, with a second plain
        // slope up B' and a peak (5 cells), and a cart that climbs out of a braked slot rolls
        // back into it
        let per = if o.stand_deep { 5 } else { 3 };
        for j in 0..3 {
            let x = xb + 1 + per * j;
            let y = if o.stand_deep { 0 } else { -j };
            stand_cells.insert(0, [x + 1, y - 1, 0]);
            path.push(([x, y, 0], Cell::Linked));
            path.push(([x + 1, y - 1, 0], Cell::Slot));
            path.push(([x + 2, y - 1, 0], Cell::Plain));
            if o.stand_deep {
                path.push(([x + 3, y, 0], Cell::Plain));
                path.push(([x + 4, y + 1, 0], Cell::Plain));
            }
        }
        // the departure (descending: at y -2; deep: at y 1): east, north along x xd, west along
        // zr back to row A
        let yd = if o.stand_deep { 1 } else { -2 };
        let xd = xb + 1 + 3 * per + 1;
        path.push(([xd - 1, yd, 0], Cell::Boost));
        path.push(([xd, yd, 0], Cell::Plain));
        if o.stand_deep {
            for z in 1..zr {
                path.push(([xd, 1, z], if z <= 3 { Cell::Boost } else { Cell::Plain }));
            }
        } else {
            for z in 1..=3 {
                path.push(([xd, -2, z], Cell::Boost));
            }
            path.push(([xd, -2, 4], Cell::Plain));
            path.push(([xd, -1, 5], Cell::Plain));
            path.push(([xd, 0, 6], Cell::Plain));
            // (with `tail_desc`: boosters on the flat, then up 3 more blocks to the jog at y 4)
            let top = if o.tail_desc { zr - 4 } else { zr };
            for z in 7..top {
                path.push(([xd, 1, z], if o.tail_desc && z <= 10 { Cell::Boost } else { Cell::Plain }));
            }
            if o.tail_desc {
                path.push(([xd, 1, zr - 4], Cell::Plain));
                path.push(([xd, 2, zr - 3], Cell::Plain));
                path.push(([xd, 3, zr - 2], Cell::Plain));
                path.push(([xd, 4, zr - 1], Cell::Plain));
            }
        }
        let ye = if o.tail_desc && !o.stand_deep { 4 } else { 1 };
        path.push(([xd, ye, zr], Cell::Plain));
        for x in (1..xd).rev() {
            path.push(([x, ye, zr], Cell::Plain));
        }
        return finish_layout(path, slot_cells, stand_cells, Vec::new(), tail_b);
    }
    if o.stand_stop {
        // farmopt's pocket stand (2026-11-10, the parent's call (a)): each cart of the batch stops
        // in its own gate pocket, the pockets 4 cells apart: a flat (a rail peak needs one: a slope
        // up straight into a slope down is reshaped flat by its neighbour updates), a plain slope
        // down (the cart rests on it, pressed by gravity against the gate), the gate's slope (a
        // fence gate over its high end; a booster), a booster back up. Counting gates (the bench's
        // position triggers): the first pocket's gate lets two carts by, the second's one, the
        // last's none, so each cart stops in its own pocket with no brake timing, and no two
        // carts touch (a contact jam is pushed apart by its panicking riders). The departure: all
        // three gates opened at once; the pockets being alike, the three leave in step, 4 cells
        // apart. Riders at x -1, 3, 7 (y 0, -1, -2): `linear-check` gives them 40, 44 and 40
        // valid columns, 0 strays, 99.03% per slot (3 cells apart would give 44 each, 99.30%)
        let mut pockets = Vec::new();
        for k in 0..3 {
            let (b, l) = (xb + 1 + 4 * k, 1 - k);
            // (wired: pocket 1's flat a detector rail D1, the first cart past G1 sets the latch)
            path.push(([b, l, 0], if o.stand_wired && k == 2 { Cell::Detector } else { Cell::Plain }));
            path.push(([b + 1, l - 1, 0], Cell::Plain));
            // (the guard: the gate slope a powered rail on glass, powered from its gate's driver
            // dust through a stone under the dust (`stand_circuit`); else a booster on a redstone
            // block)
            path.push(([b + 2, l - 2, 0], if o.stand_guard { Cell::Linked } else { Cell::Boost }));
            pockets.insert(0, (BlockPos::new(b + 2, l - 1, 0), [b + 2, l - 2, 0]));
            if k < 2 {
                // (the guard: the booster-up a plain rail: nothing boosts a cart drifting back)
                path.push(([b + 3, l - 2, 0], if o.stand_guard { Cell::Plain } else { Cell::Boost }));
            }
        }
        // (S1, S2, S3: the rest cells, front first; no rail control)
        stand_cells.extend([[xb + 10, -2, 0], [xb + 6, -1, 0], [xb + 2, 0, 0]]);
        // the departure: a curve north right after the last gate, a boosted climb along the run up
        // to the tail's level (y 4 with `tail_desc`), the run north, west along zr into the tail
        // (the shortest return from the stand: the returning batch reaches the tail ~20 gt sooner
        // than by a ramp east first)
        let xd = xb + 12;
        let top = ye;
        // (`tail_esc`: the climb tops out 2 higher, then a flat peak and two slopes back down to the
        // return run's level: from the climb's top to B the track never rises, so a cart bounced
        // backward by a contact anywhere ahead climbs a slope and comes back instead of rolling
        // back down the boosted climb)
        let lip = if o.esc_adm { if o.adm_ledge && !o.adm_short { 2 } else { 1 } } else if o.tail_esc > 0 { 2 } else { 0 };
        path.push(([xd, -3, 0], Cell::Plain));
        let mut z = 1;
        if o.depart_crest > 0 {
            // the departure's crest (`depart_crest` D): the boosted climb ends at a flat crest D
            // cells before the chunk line z 9|10, D plain slopes down to the line (a cart halted
            // there by frozen carts beyond is held forward by gravity, never back over the crest),
            // boosters past the line at the low level, then a boosted climb back to the tail's
            // level by z zr - 1
            let d = o.depart_crest as i32;
            let zc = 9 - d;
            for (k, zz) in (1..zc).enumerate() {
                path.push(([xd, -3 + k as i32, zz], Cell::Boost));
            }
            let c = -3 + (zc - 1);
            path.push(([xd, c, zc], Cell::Plain));
            for k in 1..=d {
                path.push(([xd, c - k, zc + k], Cell::Plain));
            }
            let low = c - d;
            let foot = zr - (top - low);
            let mut zz = 10;
            while zz < foot {
                path.push(([xd, low, zz], Cell::Boost));
                zz += 1;
            }
            for k in 0..(top - low) {
                path.push(([xd, low + k, foot + k], Cell::Boost));
            }
            z = zr;
        }
        for y in -3..(if o.depart_crest > 0 { -3 } else { top + lip }) {
            // (`climb_boost` N: only the climb's first N cells boosted, the rest plain slopes;
            // `climb_every` K: a booster on every Kth cell)
            let boosted = o.climb_boost.is_none_or(|n| (y + 3) < n as i32) && (o.climb_every <= 1 || (y + 3) % o.climb_every as i32 == 0);
            path.push(([xd, y, z], if boosted { Cell::Boost } else { Cell::Plain }));
            z += 1;
        }
        if lip > 0 && o.depart_crest == 0 {
            path.push(([xd, top + lip, z], Cell::Plain));
            z += 1;
            for k in 1..=lip {
                path.push(([xd, top + lip - k, z], Cell::Plain));
                z += 1;
            }
        }
        let run_start = z;
        while z < zr {
            // (the guard: the run's first cell a booster, so the stand's departures, slower out of
            // the guard's braked gate slopes, reach the tail as fast as before)
            path.push(([xd, top, z], if o.stand_guard && z == run_start { Cell::Boost } else { Cell::Plain }));
            z += 1;
        }
        path.push(([xd, top, zr], Cell::Plain));
        for x in (1..xd).rev() {
            path.push(([x, top, zr], Cell::Plain));
        }
        let mut lay = finish_layout(path, slot_cells, stand_cells, Vec::new(), tail_b);
        // (no rail control: the stand's departure is its three gates)
        lay.stand = Vec::new();
        lay.stop_gate = None;
        lay.pockets = pockets;
        // (wired: the input cell REL, right behind G0's driver dust: see `stand_circuit`)
        lay.stand_rel = o.stand_wired.then(|| BlockPos::new(xb + 11, -2, 2));
        lay.fifo = fifo;
        lay.esc = esc;
        lay.esc_p = esc_p;
        lay.esc_door = esc_door;
        lay.esc_gb = esc_gb;
        lay.esc_g0 = esc_g0;
        lay.esc_adm_a1 = esc_adm_a1;
        lay.esc_g1 = esc_g1;
        lay.flights = flights;
        lay.esc_flat_b = esc_flat_b;
        lay.queue_cells = queue_cells;
        lay.shape_over = shape_over;
        lay.extra = extra;
        return lay;
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
    // to ~0.3, so the tail's plain slots catch it like a queue move); with `tail_desc` the turn
    // north is right after S1's peak (2 cells shorter: the return's last cart reaches the tail
    // sooner)
    let xd = if o.short_depart { xs + 9 } else { xs + 10 };
    if !o.short_depart {
        path.push(([xd - 1, 1, 0], Cell::Boost));
    }
    path.push(([xd, 1, 0], Cell::Plain));
    let top = if o.drop { zr - 2 } else if o.tail_desc { zr - 4 } else { zr };
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
    } else if o.tail_desc {
        // up 3 blocks before the turn west: the jog and the tail's entry at y 4
        path.push(([xd, 1, zr - 4], Cell::Plain));
        path.push(([xd, 2, zr - 3], Cell::Plain));
        path.push(([xd, 3, zr - 2], Cell::Plain));
        path.push(([xd, 4, zr - 1], Cell::Plain));
        path.push(([xd, 4, zr], Cell::Plain));
        for x in (1..xd).rev() {
            path.push(([x, 4, zr], Cell::Plain));
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

/// The wired pocket stand's circuit (`stand_wired`), every cell north of the stand (z 1..5; nothing
/// over the track, nothing at z <= -1), `ox` the gate G2's x (G1 at `ox` + 4, G0 at `ox` + 8,
/// D1 at `ox` + 6), as written in the latched state (Q set: the three carts in their pockets).
/// - Gate drivers: a dust pointing north into each gate from z 1 (a repeater would shut G1 and
///   G2 2 gt later: release stagger 2 then fails), so all three open in REL's own tick.
/// - REL (`ox` + 8, -2, 2): G0's dust takes it directly; a dust on top of it climbs a glass step
///   (one way: power goes up a glass step, never down) into the bus O.
/// - The latch: D1's dust d1 points into block Y; torch T2 on Y is "not Q" and feeds O; O (REL or
///   T2) drives G1's and G2's dusts and, through repeater RX, block X; wall torch T1 on X (not O)
///   sits under Y and holds it. D1 rises: Y on, T2 off (2 gt), O off: G1 and G2 shut 2 gt after
///   the first cart reaches D1 (the second cart is then past G2: the train runs ~11 gt apart, and
///   D2's detector could not count it: a detector rail stays on 20 gt, so carts 1 and 2 give it
///   one pulse); then RX off, T1 on: Y held after D1 falls. REL on: O on (G0, G1, G2 open
///   together), RX on, T1 off, Y off once D1 is off, T2 on: reset; while REL is on, O stays on
///   whatever D1 does, and REL must outlast D1's last departing cart (`rel-for`'s early end).
pub fn stand_circuit(ox: i32, guard: bool) -> Vec<([i32; 3], String)> {
    let mut v: Vec<([i32; 3], String)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| v.push(([ox + x, y, z], s.to_string()));
    let wire = |n: &str, e: &str, s: &str, w: &str, p: i32| format!("redstone_wire[east={e},north={n},power={p},south={s},west={w}]");
    let rep = "repeater[delay=1,facing=south,locked=false,powered=false]";
    // the gate drivers: a dust north into each gate from z 1 (each on glass; dust, not a
    // repeater: the gates shut 2 gt sooner, and all three still open in the same tick), and REL
    // (glass: off) right behind G0's; with the guard G0's dust on a stone instead, which the dust
    // powers, and the stone G0's gate slope beside it (a powered rail on glass): G0's slope boosts
    // exactly while G0 is open (G1's and G2's slopes: below)
    put(0, 0, 1, &wire("side", "none", "side", "none", 0));
    put(4, -1, 1, &wire("side", "none", "side", "none", 0));
    put(8, -2, 1, &wire("side", "side", "side", "side", 0));
    for (x, y) in [(0, 0), (4, -1), (8, -2)] {
        put(x, y - 1, 1, if guard && x == 8 { "stone" } else { "glass" });
    }
    if guard {
        // the guard's G1 and G2 slopes: powered 8 gt after their gates open (O through RX, 2 gt,
        // into X; R1 reads X), each through its support (a stone the loop's layout gives it, set
        // in `loop_design`): a cart held against a gate's downstream side rolls onto a braking
        // slope when the gate opens, stops, and is launched forward when the slope powers
        let r = |d: i32, f: &str| format!("repeater[delay={d},facing={f},locked=false,powered=false]");
        put(5, -2, 3, &r(1, "east"));
        put(5, -3, 3, "glass");
        put(4, -2, 3, &wire("none", "side", "none", "side", 0));
        put(4, -3, 3, "stone");
        // (G1's: two repeaters north into its slope's support)
        put(4, -3, 2, &r(1, "south"));
        put(4, -4, 2, "glass");
        put(4, -3, 1, &r(1, "south"));
        put(4, -4, 1, "glass");
        // (G2's: a dust line west under the bus, a dust north, a repeater into its slope's support)
        for x in 1..=3 {
            put(x, -2, 3, &wire("none", "side", "none", "side", 0));
            put(x, -3, 3, "glass");
        }
        put(0, -2, 3, &wire("side", "side", "none", "none", 0));
        put(0, -3, 3, "glass");
        put(0, -2, 2, &wire("side", "none", "side", "none", 0));
        put(0, -3, 2, "glass");
        put(0, -2, 1, &r(2, "south"));
        put(0, -3, 1, "glass");
        // C, the D1 fallback's set input F: G1's step dust a stone, G1's driver fed from a dust on
        // it (down its north side, still instant), a stone over F (cuts F's diagonals to that
        // dust and to the bus; glass over it: no golem spawns there), F pointing into Y from its
        // stone, which torch T3 under it strongly powers
        put(4, -1, 2, "stone");
        put(4, 0, 2, &wire("side", "none", "side", "none", 0));
        put(5, 0, 2, "stone");
        put(5, 1, 2, "glass");
        put(5, -1, 2, &wire("side", "side", "side", "side", 15));
        put(5, -2, 2, "stone");
        put(5, -3, 2, "redstone_torch[lit=true]");
        // C's timer: REL tapped from G0's stone by TP into S; the dust under S, down a dust line to
        // repeater Dr (direct) and, through the stone under its first dust, the chain K1..K5
        // (delayed); the two meet at the dust Xd (its east stone and its support, both strongly
        // powered), which points into a dust Rq reads, Rq through a dust into B3: T3 on B3 is not
        // (REL or REL delayed): lit from REL's fall + 35 gt to the next REL + 9, so the latch is
        // set at REL's fall + 37 if D1 has not set it, and released 11 gt into the next REL
        // (centered in its window, ironspec 2026-11-10: the REL corners end REL at +44..+76, the
        // latest normal D1 sets the latch at +77, the next REL starts at +100..+116: set at +81
        // at the early corner, 4 gt after that D1; +113 at the late one, 3 gt before the next REL)
        put(7, -3, 1, &r(1, "east"));
        put(7, -4, 1, "glass");
        put(6, -3, 1, "stone");
        put(6, -4, 1, &wire("side", "none", "side", "none", 0));
        put(6, -5, 1, "stone");
        put(6, -4, 2, &wire("side", "side", "none", "none", 0));
        put(6, -5, 2, "glass");
        put(7, -4, 2, &wire("none", "none", "side", "side", 0));
        put(7, -5, 2, "glass");
        put(7, -4, 3, &wire("side", "none", "side", "none", 0));
        put(7, -5, 3, "glass");
        put(7, -4, 4, &r(1, "north"));
        put(7, -5, 4, "glass");
        put(7, -4, 5, "stone");
        put(7, -3, 5, "glass");
        let kd = [4, 4, 2, 2, 2];
        put(7, -5, 1, &r(kd[0], "west"));
        put(7, -6, 1, "glass");
        put(8, -5, 1, "stone");
        for (i, z) in [2, 3, 4].into_iter().enumerate() {
            put(8, -5, z, &r(kd[1 + i], "north"));
            put(8, -6, z, "glass");
        }
        // (a dust turn, not a stone: no golem spawns on it)
        put(8, -5, 5, &wire("side", "none", "none", "side", 0));
        put(8, -6, 5, "glass");
        put(7, -5, 5, &r(kd[4], "east"));
        put(7, -6, 5, "glass");
        put(6, -5, 5, "stone");
        put(6, -4, 5, &wire("none", "side", "none", "side", 0));
        put(5, -4, 5, &wire("side", "side", "none", "none", 0));
        put(5, -5, 5, "glass");
        put(5, -4, 4, &r(2, "south"));
        put(5, -5, 4, "glass");
        put(5, -4, 3, &wire("side", "none", "side", "none", 0));
        put(5, -5, 3, "glass");
        put(5, -4, 2, "stone");
    } else {
        put(4, -1, 2, &wire("side", "none", "up", "none", 0));
        put(4, -2, 2, "glass");
    }
    put(8, -2, 2, "glass");
    // REL's dust, up a glass step into O
    put(8, -1, 2, &wire("none", "side", "none", "up", 0));
    put(7, -1, 2, "glass");
    // the bus O at y 0: round T2 (6, 0, 2), west along z 3 to G2's dust, down a stone step to
    // G1's, south to RX
    put(7, 0, 2, &wire("none", "side", "side", "side", 0));
    put(7, 0, 3, &wire("side", "none", "none", "side", 0));
    put(6, 0, 3, &wire("side", "side", "side", "side", 0));
    put(5, 0, 3, &wire("none", "side", "none", "side", 0));
    put(4, 0, 3, &wire("side", "side", "none", "side", 0));
    for x in 1..=3 {
        put(x, 0, 3, &wire("none", "side", "none", "side", 0));
    }
    put(0, 0, 3, &wire("side", "side", "none", "none", 0));
    put(0, 0, 2, &wire("side", "none", "side", "none", 0));
    for x in [7, 6, 5, 3, 2, 1, 0] {
        put(x, -1, 3, "glass");
    }
    // (a stone step: power goes down it, a glass one would only let it up)
    put(4, -1, 3, "stone");
    put(0, -1, 2, "glass");
    put(6, 0, 4, &wire("side", "none", "side", "none", 0));
    put(6, -1, 4, "stone");
    put(6, -1, 5, &wire("up", "none", "side", "none", 0));
    put(6, -2, 5, "stone");
    put(6, -2, 4, rep);
    put(6, -3, 4, "glass");
    // the latch: X with T1 (lit: Q set), Y with T2 (off), D1's dust d1 (powered through Y)
    put(6, -2, 3, "stone");
    put(6, -2, 2, "redstone_wall_torch[facing=north,lit=true]");
    put(6, -1, 2, "stone");
    put(6, 0, 2, "redstone_torch[lit=false]");
    put(6, -1, 1, &wire("side", "none", "side", "none", 15));
    put(6, -2, 1, "glass");
    v
}

/// The loop's design: its path (each rail on its support), 21 carts with villagers: the queue's
/// slots full, the stand's three slots full.
pub fn loop_design(o: &LoopOpts) -> Result<String> {
    // (the line loop starts with one batch on its return: the bench adds it)
    // (the re-packing tail holds the rest at the start: 21 less the register and the stand)
    let fifo_init = if o.tail_fifo > 0 || o.tail_esc > 0 { (21 - o.slots - 3).max(0) } else { 0 };
    let held = o.slots + 3 + if o.line { 3 } else { 0 } + fifo_init;
    if held != 21 || (o.tail_fifo > 0 && fifo_init > o.tail_fifo as i32) || (o.tail_esc > 0 && fifo_init > 2 + o.tail_esc as i32 + if o.ledge_slope { 2 } else { 0 }) {
        bail!("the queue's {} slots and the stand's 3{} must hold the 21 carts", o.slots, if o.line { " and the returning batch's 3" } else if o.tail_fifo > 0 { " and the FIFO's pockets" } else if o.tail_esc > 0 { " and the escapement tail's (P, B and the queue: --slots 14 leaves 4)" } else { "" });
    }
    if o.tail_fifo > 0 && (!o.stand_stop || !o.bridge || o.tail_fifo < 3) {
        bail!("the re-packing tail (--tail-fifo) needs the bridge loop with the pocket stand, and at least 3 pockets");
    }
    if o.tail_esc > 0 && (!o.stand_stop || !o.bridge || o.tail_fifo > 0) {
        bail!("the escapement tail (--tail-esc) needs the bridge loop with the pocket stand, and no --tail-fifo");
    }
    let lay = loop_layout_frame(o);
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
        let sh = match lay.shape_over.iter().find(|(p, _)| *p == c) {
            Some((_, s)) => s.to_string(),
            None => crate::yard::shape(Some(prev), c, Some(next))?,
        };
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
    // (the stop stand's braked slope starts braked, as the catch leaves it)
    if o.stand_stop && false {
        for (c, k) in &lay.path {
            if matches!(k, Cell::Slot | Cell::Linked) && c[2] == 0 && (2..=3).contains(&c[0]) {
                let i = lay.path.iter().position(|(p, _)| p == c).unwrap_or(0);
                let sh = crate::yard::shape(Some(lay.path[(i + n - 1) % n].0), *c, Some(lay.path[(i + 1) % n].0))?;
                b.set(c[0], c[1], c[2], &format!("powered_rail[powered=true,shape={sh},waterlogged=false]"));
            }
        }
        for s in &lay.stand {
            b.set(s.x, s.y, s.z, "redstone_block");
        }
    }
    let rider = Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}");
    // (each cart where the loop's steady state rests it: a queue cart 0.32 down its slope, a
    // stand cart 0.45, measured from the slope's top edge along its travel direction; the stop
    // stand's three where its catch rests them, against the gate and each other)
    let stand_carts: Vec<[f64; 3]> = if o.stand_stop {
        // (on the approach flats, rolling in: they settle against the gate as any batch does; set
        // down at the rest itself, the push of the first tick sends the third back off its slope)
        // (each at rest in its own pocket, pressed against its gate: no two touch)
        vec![[7.885, -1.8225, 0.5], [3.885, -0.8225, 0.5], [-0.115, 0.1775, 0.5]]
    } else {
        lay.stand_cells.iter().map(|c| [c[0] as f64 + 0.5 + (0.45 - 0.5), c[1] as f64 + 1.0 - 0.45 + 0.05, c[2] as f64 + 0.5]).collect()
    };
    for (k, c) in lay.slot_cells.iter().enumerate() {
        let (off, d) = (0.32, lay.slot_dir[k]);
        let x = c[0] as f64 + 0.5 + d[0] as f64 * (off - 0.5);
        let z = c[2] as f64 + 0.5 + d[1] as f64 * (off - 0.5);
        b.entity("minecart", [x, c[1] as f64 + 1.0 - off + 0.05, z], rider);
    }
    // (the stop stand's three rolling in at a train's speed)
    // (the stand's three at rest in their pockets from the start)
    let stand_rider = Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}");
    for p in &stand_carts {
        b.entity("minecart", *p, if o.stand_stop { stand_rider } else { rider });
    }
    // the re-packing tail: the rest at rest in the FIFO's last pockets, each pressed against its
    // gate (as the pocket stand's: 0.885 along the rest slope); a gate open only while its next
    // pocket is empty (the last's next is slot 0, full)
    let nf = lay.fifo.len();
    let fifo_init = fifo_init as usize;
    for (j, (g, rest)) in lay.fifo.iter().enumerate() {
        if j + fifo_init >= nf {
            b.entity("minecart", [rest[0] as f64 + 0.5, rest[1] as f64 + 0.1775, rest[2] as f64 + 0.885], stand_rider);
        }
        let open = j + 1 < nf && j + 1 + fifo_init < nf;
        b.set(g.x, g.y, g.z, &format!("oak_fence_gate[facing=south,in_wall=false,open={open},powered=false]"));
    }
    // (the escapement's B door, shut: an iron door facing south, its plate at the cell's north face)
    if let Some(d) = lay.esc_door {
        b.set(d.x, d.y, d.z, "iron_door[facing=south,half=lower,hinge=left,open=false,powered=false]");
        b.set(d.x, d.y + 1, d.z, "iron_door[facing=south,half=upper,hinge=left,open=false,powered=false]");
    }
    // the escapement tail: the front cart on P against the gate (above), the next braked early on
    // B (its centre 0.2 into the cell), the rest up the queue's slopes 0.79 apart (the packing
    // measured on the bench), where the queue's gravity would leave them
    if let Some(e) = lay.esc.filter(|_| !o.esc_adm) {
        let ez = e.z as f64;
        let ey = (e.y + 1) as f64;
        for k in 0..fifo_init.saturating_sub(1) {
            let zz = ez + 0.2 - 0.79 * k as f64;
            let cell_z = zz.floor();
            let ys = ey + (ez - cell_z);
            b.entity("minecart", [0.5, ys + (1.0 - (zz - cell_z)) + 0.0625, zz], stand_rider);
        }
    }
    // (admission control: the queue's front against G0 (0.885 along the last queue slope) and
    // the rest up the slopes 0.79 apart; P's cart is the fifo's; G0 shut. With `adm_flatb` the
    // front rests on B, the flat before G0, and its follower 0.98 back on the last slope)
    if let (true, Some(g0)) = (o.esc_adm, lay.esc_g0) {
        let n = fifo_init.saturating_sub(1);
        let mut first = 0;
        // (the slopes: the last one ends at G0's cell, or at B's with `adm_flatb`)
        let zq_end = if o.adm_flatb { (g0.z - 2) as f64 } else { (g0.z - 1) as f64 };
        let mut z_front = zq_end + 0.885;
        if o.adm_flatb && n > 0 {
            b.entity("minecart", [0.5, g0.y as f64 + 0.0625, (g0.z - 1) as f64 + 0.885], stand_rider);
            first = 1;
            z_front = (g0.z - 1) as f64 + 0.885 - 0.98 + 0.79;
        }
        let yq = g0.y as f64;
        for k in 0..n.saturating_sub(first) {
            let zz = z_front - 0.79 * (k + if first > 0 { 1 } else { 0 }) as f64;
            let cell_z = zz.floor();
            let s = zz - cell_z;
            let yc = yq + (zq_end - cell_z);
            b.entity("minecart", [0.5, yc + (1.0 - s) + 0.0625, zz], stand_rider);
        }
    }
    // (the flat escapement: B's cart against G_B at 0.885 on the flat, its followers up the
    // queue's slopes, the first with its centre 0.095 short of B, then 0.79 apart; G_B shut)
    if let (Some(fb), Some(gb)) = (lay.esc_flat_b, lay.esc_gb) {
        let (yb, zb) = (fb[1] as f64, fb[2] as f64);
        if fifo_init >= 2 {
            b.entity("minecart", [0.5, yb + 0.0625, zb + 0.885], stand_rider);
        }
        for k in 0..fifo_init.saturating_sub(2) {
            // (the slope above B descends to yb at zb: at z, its surface is at yb + (zb − z))
            let zz = zb - 0.095 - 0.79 * k as f64;
            b.entity("minecart", [0.5, yb + (zb - zz) + 0.0625, zz], stand_rider);
        }
        b.set(gb.x, gb.y, gb.z, "oak_fence_gate[facing=south,in_wall=false,open=false,powered=false]");
    }
    // (G0, open at the start: B's pulse is not on; with admission control shut: it opens to admit)
    if let Some(g0) = lay.esc_g0 {
        b.set(g0.x, g0.y, g0.z, &format!("oak_fence_gate[facing=south,in_wall=false,open={},powered=false]", !o.esc_adm));
    }
    // (admission control's mid-queue gate G1, shut)
    if let Some(g1) = lay.esc_g1 {
        b.set(g1.x, g1.y, g1.z, "oak_fence_gate[facing=south,in_wall=false,open=false,powered=false]");
    }
    // (`adm_flatb`: B, the flat cell before G0, an always-on booster powered by a redstone block
    // beside it (east); its power runs along the rail into P)
    if let (true, Some(g0)) = (o.esc_adm && o.adm_flatb, lay.esc_g0) {
        let c = [g0.x, g0.y, g0.z - 1];
        let cur = b.d.blocks.get(&c).cloned().unwrap_or_default();
        if !cur.contains("rail[shape=north_south") {
            bail!("adm_flatb: the cell before G0 at {c:?} is {cur}, not a flat north-south rail");
        }
        b.set(c[0], c[1], c[2], "powered_rail[powered=true,shape=north_south,waterlogged=false]");
        if b.d.blocks.contains_key(&[c[0] + 1, c[1], c[2]]) {
            bail!("adm_flatb: the cell east of B at {:?} is taken", [c[0] + 1, c[1], c[2]]);
        }
        b.set(c[0] + 1, c[1], c[2], "redstone_block");
    }
    // (`adm_qboost`: the last queue slope, where the front rests against G0, an always-on booster
    // powered by a redstone block beside it (east), not under it: a block under it would touch P's
    // rail (the layout turns such a booster plain). It presses the front against G0 and brings the
    // follower up fast; P, the next rail down, takes its power along the rail: P always on)
    if let (true, Some(g0)) = (o.esc_adm && o.adm_qboost, lay.esc_g0) {
        let c = [g0.x, g0.y, g0.z - 1];
        let cur = b.d.blocks.get(&c).cloned().unwrap_or_default();
        if !cur.starts_with("rail[") && !cur.starts_with("minecraft:rail[") {
            bail!("adm_qboost: the last queue slope at {c:?} is {cur}, not a plain rail");
        }
        let powered = cur.replacen("rail[", "powered_rail[powered=true,", 1);
        b.set(c[0], c[1], c[2], &powered);
        b.set(c[0] + 1, c[1], c[2], "redstone_block");
    }
    // the stop stand's gates: the stand's stop shut; the pockets' last gate shut, the other two
    // open (gate 2 lets two carts by, gate 1 one)
    if let Some(g) = lay.stop_gate {
        b.set(g.x, g.y, g.z, "oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]");
    }
    for (k, (g, _)) in lay.pockets.iter().enumerate() {
        b.set(g.x, g.y, g.z, &format!("oak_fence_gate[facing={},in_wall=false,open={},powered=false]", if o.stand_stop { "east" } else { "south" }, k > 0 && !o.stand_stop));
    }
    // the wired pocket stand's circuit (G2 at x xb + 3), nothing in its cells yet
    if o.stand_stop && o.stand_wired {
        let ox = -o.row_gap + 3;
        for (c, s) in stand_circuit(ox, o.stand_guard) {
            if b.d.blocks.contains_key(&c) {
                bail!("the stand's circuit meets the loop at {c:?}");
            }
            b.set(c[0], c[1], c[2], &s);
        }
        // (the guard: G1's and G2's slope supports stone, powered by the circuit's repeaters)
        if o.stand_guard {
            for c in [[ox + 4, -3, 0], [ox, -2, 0]] {
                if b.d.blocks.get(&c).map(|s| s.as_str()) != Some("minecraft:glass") {
                    bail!("the guard expects a glass support at {c:?}");
                }
                b.set(c[0], c[1], c[2], "stone");
            }
        }
        // (C' wired: C's timer out, the approach detector's edge circuit in)
        if let Some(d) = o.cprime {
            if o.approach != 5 || !o.stand_guard {
                bail!("--cprime needs --approach 5 and --stand-guard");
            }
            let (removed, added) = crate::approach::cprime_cells(ox, d);
            for c in &removed {
                if b.d.blocks.remove(c).is_none() {
                    bail!("C' expects C's cell at {c:?}");
                }
            }
            for (c, s) in added {
                if let Some(old) = b.d.blocks.get(&c) {
                    bail!("C''s {s} at {c:?} meets {old}");
                }
                b.set(c[0], c[1], c[2], &s);
            }
        }
    }
    // the zombie posts (PANIC for every rider): a zombie in a boat on glass (no golem spawns on
    // glass) at y 3, out of the carts' attack reach (2 above the highest rail; a zombie's attack
    // box grows only sideways), leaves over it for shade, every `zombie_every` cells along the
    // strip's middle line north of the stand (the stand's own zombies are the pad's)
    if o.zombie_every > 0 {
        let zx = o.zombie_x;
        let mut posts: Vec<(i32, i32)> = Vec::new();
        let mut z = 5;
        while z <= 58 {
            posts.push((zx, z));
            z += o.zombie_every;
        }
        if o.tail_desc || o.tail_fifo > 0 || o.tail_esc > 0 {
            // (the raised jog's supports at y 3 hide the queue's head from the post at z 21: one
            // more post west of the head)
            posts.push((-6, 21));
        }
        // (no post within a zombie's reach of a rider: a zombie in a boat attacks over the boat's
        // box (1.375 wide) grown 0.83 sideways, so a rider (0.6 wide) is hit when its centre comes
        // within 1.82 of the post on both axes while their heights overlap, rails at y 1..4 with
        // the zombie at y 3; a post in reach moves north a cell at a time, up to 4, else goes. The
        // raised return's jog at y 4 along z 20 was in reach of the post at z 21 and killed a
        // rider every 7 laps on day-1's pocket loop)
        let in_reach = |px: i32, pz: i32| lay.path.iter().any(|(c, _)| {
            let gap = |a: i32, p: i32| ((p as f64 + 0.5) - (a as f64 + 0.5)).abs() - 0.5;
            (1..=4).contains(&c[1]) && gap(c[0], px) < 1.82 && gap(c[2], pz) < 1.82
        });
        let posts: Vec<(i32, i32)> = posts.into_iter().filter_map(|(px, pz)| (0..=4).map(|d| (px, pz + d)).find(|(x, z)| !in_reach(*x, *z))).collect();
        for (px, pz) in posts {
            b.set(px, 2, pz, "glass");
            b.set(px, 6, pz, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
            b.entity("oak_boat", [px as f64 + 0.5, 3.0, pz as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
        }
    }
    // (the layout's extra blocks: the snap turn's sturdy high side)
    for (c, s) in &lay.extra {
        if b.d.blocks.contains_key(c) {
            bail!("the layout's extra {s} at {c:?} meets the loop");
        }
        b.set(c[0], c[1], c[2], s);
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("queue".into(), format!("scene=Loop carts=21 spacing=3 release=3 riders=1 slots={} run={} depart={} boost={} boost_first={} straight={} drop={} vstand={} tail_v={} bridge={} row_gap={} uturn_boost={} line={} tail_deep={} plain={} stand_desc={} detector={} zombie_every={} zombie_x={} stand_deep={} tail_desc={} peak_pad={} short_depart={} stand_stop={}{}{}", o.slots, o.run, o.depart, o.boost_every, o.boost_first, o.straight as i32, o.drop as i32, o.vstand as i32, o.tail_v as i32, o.bridge as i32, o.row_gap, o.uturn_boost as i32, o.line as i32, o.tail_deep as i32, o.plain as i32, o.stand_desc as i32, o.detector as i32, o.zombie_every, o.zombie_x, o.stand_deep as i32, o.tail_desc as i32, o.peak_pad as i32, o.short_depart as i32, o.stand_stop as i32, if o.stand_wired { if o.stand_guard { " stand_wired=1 stand_guard=1" } else { " stand_wired=1" } } else { "" }, (if o.tail_fifo > 0 { format!(" tail_fifo={}", o.tail_fifo) } else { String::new() }) + &(if o.tail_esc > 0 { format!(" tail_esc={}", o.tail_esc) } else { String::new() }) + &(o.climb_boost.map(|n| format!(" climb_boost={n}")).unwrap_or_default() + if o.tail_brake { " tail_brake=1" } else { "" } + if o.tail_plain { " tail_plain=1" } else { "" } + &(if o.climb_every > 0 { format!(" climb_every={}", o.climb_every) } else { String::new() }) + if o.ledge_slope { " ledge_slope=1" } else { "" } + if o.esc_door { " esc_door=1" } else { "" } + if o.esc_flat { " esc_flat=1" } else { "" } + if o.landing_boost2 { " landing_boost2=1" } else { "" } + if o.esc_g0 { " esc_g0=1" } else { "" } + if o.esc_adm { " esc_adm=1" } else { "" } + if o.adm_ledge { " adm_ledge=1" } else { "" } + &if o.adm_g1 > 0 { format!(" adm_g1={}", o.adm_g1) } else { String::new() } + &(if o.approach > 0 { format!(" approach={}", o.approach) } else { String::new() }) + &(if o.depart_crest > 0 { format!(" depart_crest={}", o.depart_crest) } else { String::new() }) + if o.adm_short { " adm_short=1" } else { "" } + if o.adm_qboost { " adm_qboost=1" } else { "" } + if o.adm_flatb { " adm_flatb=1" } else { "" } + &o.cprime.map(|d| format!(" cprime={}", d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(":"))).unwrap_or_default())));
    b.d.ports.insert("output".into(), [-5, 1, -3]);
    // (the design's declared flights (`crate::flight`), in its final, shifted coordinates)
    if !lay.flights.is_empty() {
        let fl: Vec<crate::flight::Flight> = lay.flights.iter().map(|f| f.shifted(o.shift[0], o.shift[1])).collect();
        b.d.meta.insert("flights".into(), crate::flight::format(&fl));
    }
    // (its declared states (`crate::declared`), so a file pasted into another carries them)
    b.d.meta.insert("declared".into(), crate::declared::format(&declared_states(o)));
    // (a booster the layout asked for but placed plain: refused, so an option can't silently
    // vanish (ironspec 01:04, the qboost hazard))
    if !lay.downgraded.is_empty() {
        bail!("boosters requested but placed plain (their redstone block would touch a controlled rail): {:?}", lay.downgraded);
    }
    // (`shift`: everything built in the loop's frame, then moved)
    let [sx, sz] = o.shift;
    if sx != 0 || sz != 0 {
        let d = &mut b.d;
        d.blocks = std::mem::take(&mut d.blocks).into_iter().map(|(p, s)| ([p[0] + sx, p[1], p[2] + sz], s)).collect();
        d.block_entities = std::mem::take(&mut d.block_entities).into_iter().map(|(p, s)| ([p[0] + sx, p[1], p[2] + sz], s)).collect();
        for e in d.entities.iter_mut() {
            e.pos[0] += sx as f64;
            e.pos[2] += sz as f64;
        }
        for p in d.ports.values_mut() {
            p[0] += sx;
            p[2] += sz;
        }
        if let Some(q) = d.meta.get_mut("queue") {
            q.push_str(&format!(" shift={sx},{sz}"));
        }
    }
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
            "stand_deep" => o.stand_deep = v != "0",
            "tail_desc" => o.tail_desc = v != "0",
            "peak_pad" => o.peak_pad = v != "0",
            "short_depart" => o.short_depart = v != "0",
            "stand_stop" => o.stand_stop = v != "0",
            "stand_wired" => o.stand_wired = v != "0",
            "stand_guard" => o.stand_guard = v != "0",
            "tail_fifo" => o.tail_fifo = v.parse()?,
            "tail_esc" => o.tail_esc = v.parse()?,
            "climb_boost" => o.climb_boost = Some(v.parse()?),
            "approach" => o.approach = v.parse()?,
            "depart_crest" => o.depart_crest = v.parse()?,
            "cprime" => {
                let d: Vec<i32> = v.split(':').map(|t| t.parse()).collect::<Result<_, _>>()?;
                anyhow::ensure!(d.len() == 6, "cprime={v}: six delays");
                o.cprime = Some([d[0], d[1], d[2], d[3], d[4], d[5]]);
            }
            "tail_brake" => o.tail_brake = v == "1",
            "tail_plain" => o.tail_plain = v == "1",
            "climb_every" => o.climb_every = v.parse()?,
            "ledge_slope" => o.ledge_slope = v == "1",
            "esc_door" => o.esc_door = v == "1",
            "esc_flat" => o.esc_flat = v == "1",
            "landing_boost2" => o.landing_boost2 = v == "1",
            "esc_g0" => o.esc_g0 = v == "1",
            "esc_adm" => o.esc_adm = v == "1",
            "adm_ledge" => o.adm_ledge = v == "1",
            "adm_g1" => o.adm_g1 = v.parse().map_err(|_| anyhow::anyhow!("bad adm_g1 {v}"))?,
            "adm_short" => o.adm_short = v == "1",
            "adm_qboost" => o.adm_qboost = v == "1",
            "adm_flatb" => o.adm_flatb = v == "1",
            "detector" => o.detector = v != "0",
            "zombie_every" => o.zombie_every = v.parse()?,
            "zombie_x" => o.zombie_x = v.parse()?,
            "shift" => {
                let (x, z) = v.split_once(',').ok_or_else(|| anyhow::anyhow!("shift={v}: give x,z"))?;
                o.shift = [x.parse()?, z.parse()?];
            }
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
    /// S1's departure slope on at this gt (S2 and S3 at `depart_every`, twice that).
    pub s1_at: i64,
    /// The tail's slot 0 opens this many gt after slots 1-2.
    pub tail0_late: i64,
    /// With `wired`: the bench still drives the stand's 3 controls (the departure lines and the
    /// brake), the design's redstone the queue's 18.
    pub bench_stand: bool,
    /// Added to the stand's departure times (with `wired`, the release starts after the golem
    /// pulse: day-1's GO at +4).
    pub stand_offset: i64,
    /// The release group's 3 slopes on front first, this many gt apart (0: together).
    pub release_stagger: i64,
    /// When (gt into a slot) the check that every earlier returning batch rests in the queue's
    /// tail runs (default: the first advance pulse).
    pub tail_check_at: Option<i64>,
    /// S1, S2, S3 braked these many gt after the detector rail's rise (one brake line each).
    pub stand_detect_slots: Option<[i64; 3]>,
    /// The tolerance probes: the release group's start, each advance pulse's time and width,
    /// each stand slot's departure time (shifts in gt), and a speed kick on the release group's
    /// carts at the release (b/t along their slopes).
    pub release_start: i64,
    pub adv_shift: [i64; 3],
    pub adv_width: [i64; 3],
    pub stand_shift: [i64; 3],
    pub entry_speed: f64,
    /// Each queue slot's advance pulses this many gt late (slot 0 first), as a wiring that
    /// reaches the head first delivers them.
    pub skew: [i64; 24],
    /// A diagnostic: at this gt of each slot the batch, if it is near the stand, is put at rest
    /// at the stand's nominal offsets (a stand that always delivers).
    pub ideal_stand_at: Option<i64>,
    /// The at-the-advance check requires the returning train at rest (as the wired run's flag),
    /// not only braked into the tail.
    pub tail_check_rest: bool,
    /// Slot 2's tail brake on its own, at this gt after the train's departure (else with slot 1's).
    pub tail2_at: Option<i64>,
    /// The design placed as written (blocks raw, block entities merged, no shape updates, no
    /// settle): a file that works only built the judge's way fails here.
    pub placed_raw: bool,
    /// Built in order (the brief's check): the world starts empty and the design is placed cell
    /// by cell in its build order with the game's updates, this many ticks between placements
    /// (the priced rate: prices.tsv's `place`, 1 s = 20 gt at every stage), then the carts.
    pub build_gap: Option<i64>,
    /// With `build_gap`: each cell's own priced gap instead (20 gt, 40 gt for a place:edge cell).
    pub build_priced: bool,
    /// With `release_stagger`: each staggered slot's off delayed like its on (a repeater in front
    /// of its control), not all three off together at `release_for`.
    pub stagger_off: bool,
    /// Day-1's merge (b), the side drop (see `SideDrop`).
    pub side_drop: Option<SideDrop>,
    /// The stop stand (`stand_stop`): its gate opened and its braked slope powered at
    /// `stand_offset` + `stop_open`, shut and braked again at `stand_offset` + `stop_close`; the
    /// re-spacer's three pockets all opened at `stand_offset` + `pocket_release` and reset (the
    /// last gate shut, the other two open) `pocket_reset` gt later.
    pub stop_open: i64,
    pub stop_close: i64,
    pub pocket_release: i64,
    pub pocket_reset: i64,
    /// The stop stand's brake: set this many gt after the batch's third cart is on the braked
    /// slopes (the bench's trigger; a tolerance probe).
    pub stop_brake_delay: i64,
    /// The stop stand's braked slope powered at `stand_offset` + `stop_cv` (after the gate has
    /// opened and the front cart has rolled off).
    pub stop_cv: i64,
    /// The pocket stand's counting gates shut this many gt after their count is reached (0: at
    /// once; the tolerance probe of the gates' own circuit).
    pub pocket_close_delay: i64,
    /// The wired pocket stand (`stand_wired`): its input cell REL a redstone block from
    /// `stand_offset` + `pocket_release` for this many gt (then glass), the only control the
    /// bench drives there.
    pub rel_for: i64,
    /// The wired stand's REL start shifted alone, its end left at `pocket_release` + `rel_for`
    /// (the tolerance probe whose windows stack with `rel_for`'s in the corners).
    pub rel_on: i64,
    /// With `--watch`: these cells' block states printed too (up to 8).
    pub watch_cells: [Option<[i32; 3]>; 8],
    /// With `wired`: each slot's golem pulse this many gt late at most (0..=J, from the seed and
    /// the slot): the gaps between pulses vary over 100 - J ..= 100 + J (the golem's jitter).
    pub golem_jitter: i64,
    /// With `wired`: the next golem up to this many gt late (0..=J more gt at each slot's end,
    /// from the seed and the slot): the gaps between pulses vary over 100 ..= 100 + J.
    pub golem_late: i64,
    /// The day time held in the day (no night: a file's night window, its siding visits, never
    /// opens).
    pub day_only: bool,
    /// `--slept`: every rider's LAST_SLEPT set at the start and each night (bench-driven: the
    /// station bench's stand-in for the siding's naps, for a pad's check without a siding).
    pub slept: bool,
    /// The golem pulse to count (the design's `output.pulse` port, a pad's).
    pub pulse_at: Option<BlockPos>,
    /// The slots' phase: slot +K on the villagers' summon tick (gameTime % 100 == 0).
    pub summon_at: Option<i64>,
    /// Fault injection (`--act`): each sets a block at a slot offset, and with a hold restores
    /// the block it replaced that many gt later; while held, the bench's own drive of that cell
    /// (the wired stand's REL) leaves it.
    pub acts: [Option<Act>; 8],
    /// A diagnostic hand reset of the queue only, at this slot's start (see `run_loop`).
    pub reset_queue_at: Option<i64>,
    /// With `reset_queue_at`: the stand's stragglers (a cart in its region not at rest in a
    /// pocket) moved to the queue too.
    pub reset_stragglers: bool,
    /// With `reset_queue_at`: every cart placed by hand, the stand's three pockets first (the
    /// design's rest positions), then the queue; no block touched (the stand's redstone kept).
    pub reset_stand_carts: bool,
    /// With `reset_queue_at`: the queue reset again at every later slot's start (a tail that
    /// always re-packs: whatever the stand sends comes back as the queue's next trains of 3).
    pub reset_queue_every: bool,
    /// `--no-inject` (day-1's pad runs): with `wired`, no golem pulse from the bench at all, the
    /// controls left to the design (its pad gives the pulse).
    pub no_inject: bool,
    /// `--start-random`: the run starts at a day time drawn from the seed (the power-on sweep).
    pub start_random: bool,
    /// The bench-driven pocket stand's gates shut on a trigger position instead of their count
    /// (an ideal-input lab for a wired trigger): G1, then G2, shut this many gt after the first
    /// cart's centre passes this x (a detector rail at cell x0 fires at x0 - 0.29).
    pub pocket_trigger: Option<[(f64, i64); 2]>,
    /// A nap visit to the bed siding (see `Visit`).
    pub visit: Option<Visit>,
    /// A design's `build: last x,y,z ...` cells (up to 64), placed after every other in the
    /// built-in-order mode.
    pub build_last: [Option<BlockPos>; 64],
    /// A diagnostic, never a design check: every rider holds GOLEM_DETECTED_RECENTLY, so none
    /// summons a golem (what a loop does once its off-pad golem cells are gone).
    pub no_summons: bool,
    /// (tickexact 2026-11-11) A diagnostic, never a design check: every zombie killed at the
    /// start, so the riders stay calm (the `cant-reach:` line's positive control: a calm rider in
    /// REST builds its streak and releases HOME past 1200 gt).
    pub calm: bool,
    /// Every villager's block position each tick, printed at the end (`--record-positions`: the
    /// golem-spots check's positions over a full bench night).
    pub record_positions: bool,
    /// Slots whose golem pulse never comes (`--golem-miss`: a missed golem; the design's own
    /// expiry must release that slot); -1 = unused.
    pub golem_miss: [i64; 8],
    /// `build: fills-last`: the containers' stocks after every block (main's build plan).
    pub build_fills_last: bool,
    /// The re-packing tail's intake (`tail_fifo`): its gate opened D gt after each advance's start,
    /// for W gt (the bench's; one cart into slot 0 per advance).
    pub fifo_intake: (i64, i64),
    /// The escapement tail's B (the next cart's hold): powered K gt after the gate opens, for W gt
    /// (P's cart away first, so B's cart never catches it on the flats).
    pub esc_b: (i64, i64),
    /// Print the queue's first slots' carts at each slot's start (a re-packing tail diagnostic).
    pub verbose_tail: bool,
    /// Every cart in the tail's cells each tick over cycles C0..=C1 (`--tail-trace C0,C1`).
    pub tail_trace: Option<(i64, i64)>,
    /// C', the train-synchronous D1 fallback, emulated (`--c-prime X,Y,Z,DELAY,FOR`): each rising
    /// edge of the detector rail at (X, Y, Z) (the loop's frame) puts a redstone block on F's stone
    /// (5, -2, 2) DELAY gt later for FOR gt (the wiring's stand-in; T3 removed by an act); an edge
    /// within LOCK gt of the last one taken is ignored (one pulse per train).
    pub c_prime: Option<([i32; 3], i64, i64, i64)>,
    /// With `c_prime`: REL held on to C''s edge + this many gt when it is on at the edge (a
    /// train-synchronous end for the stand's release window; `--rel-ext E`).
    pub rel_ext: Option<i64>,
    /// The stand's REL held at least this many gt from its rise (a pulse extender on REL,
    /// `--rel-min L`; 0: off).
    pub rel_min: i64,
    /// The late-arrival hold, emulated (`--late-hold X,Y,Z,N,W`): a cart's centre entering that
    /// loop-frame cell (the arrival detector before pocket 3) within the last N gt, while the
    /// controller's REL rose within the last W gt (the bench's own pulse, before `rel_min`'s
    /// extension, or an act holding the REL cell a redstone block), holds G2 shut, so a cart
    /// reaching the stand while the pocket carts start from rest rests against G2 and leaves from
    /// rest instead of chasing the cart ahead.
    pub late_hold: [Option<([i32; 3], i64, i64, usize)>; 3],
    /// The settled-release gate, emulated (`--rel-gate S,T,L`): the controller's REL (the bench's
    /// pulse and any act on the REL cell, now raw inputs) reaches the stand only as REL_eff, a
    /// fixed L-gt pulse triggered once per raw pulse when the latch has been set S gt (the batch
    /// at rest) or, the timeout path, when the raw REL has been on T gt; the stand's REL cell
    /// carries REL_eff (the latch resets on it).
    pub rel_gate: Option<(i64, i64, i64)>,
    /// The escapement's G0: shut from LEAD gt before B's pulse to LAG gt after it.
    pub esc_g0: (i64, i64),
    /// Admission control: G0 opened DELAY gt after the gate (G_P) opens, shut by a cart crossing
    /// A' or after MAX gt.
    pub esc_adm: (i64, i64),
    /// Admission control's latch: G0 also shuts once a cart is on P (a sensor), and stays shut
    /// while P holds one; without it G0 is a plain timed pulse.
    pub adm_latch: bool,
    /// Admission control: P always powered (a booster held shut by G_P alone), from the first
    /// gate opening on.
    pub p_always: bool,
    /// Admission control: G1's window (0: G0's MAX).
    pub adm_g1_w: i64,
    /// Admission control: G0's delay after the gate opens for the third advance of each cycle (the
    /// next G_P is 60 gt away there, so its window can open later); −1: as the others.
    pub adm_k2: i64,
}

/// A nap visit: during slot `slot` the stand's cart `k` (0 = S1, the batch's front, first across
/// the departure corner) is diverted to the bed siding: the divert control on for 10 gt from
/// +`divert_at` + 10k; T1/T2 set for its drop slot (q-4-k: its hole rests there in the long rest
/// of slot `slot`+4) as its siding release comes; the siding's release control on for 10 gt at
/// +`release_at` from slot `slot`'s start. Checked: the rider sleeps, the cart rolls off its drop
/// rail, rests in its own slot, and its batch is in order (slots q-4, q-5, q-6) before slot
/// `slot`+5's first advance. With `every` > 0, a nightly schedule instead: from the first slot
/// whose day time is `from` or later, a visit every `every` slots, 21 a night, the j-th taking the
/// stand's cart j div 7 (with `every` prime to 7, every batch and every place in it once); checked
/// besides: every rider asleep once a night, every rider's HOME unchanged at each dawn and midday.
#[derive(Clone, Copy, Debug)]
pub struct Visit {
    pub slot: i64,
    pub k: usize,
    pub divert_at: i64,
    pub release_at: i64,
    pub divert: BlockPos,
    pub t1: BlockPos,
    pub t2: BlockPos,
    pub release: BlockPos,
    pub every: i64,
    pub from: i64,
    /// The siding's release per stand cart (its drop slot's route), `release_at` where unset.
    pub release_k: [Option<i64>; 3],
    /// The release control per stand cart (a hold on each drop line), `release` where unset.
    pub release_cells: [Option<BlockPos>; 3],
    /// T1/T2 set the tick before the release (one release past them) instead of at the divert.
    pub t_at_release: bool,
    /// T1/T2's set moved by this many gt (the sweep's "t-set").
    pub t_at: i64,
    /// The nightly schedule's j-th visit takes the stand's cart j mod 3 (day-1's scheduler: 0, 1,
    /// 2, 0, ...: consecutive visits on different drop lines) instead of j div 7.
    pub k_mod3: bool,
    /// Observed: the file drives its own controls; a visit is a cart on `branch`, the divert's
    /// branch rail (the bench writes no control cell).
    pub observe: bool,
    pub branch: BlockPos,
}

/// One nap visit as it runs (see `Visit`).
#[derive(Clone, Debug, Default)]
struct VisitRun {
    slot: i64,
    k: usize,
    cart: Option<u32>,
    rider: Option<u32>,
    batch: Vec<u32>,
    sleep: (Option<i64>, Option<i64>),
    ready: Option<i64>,
    /// The rider's last seen tick, place and health (a rider gone from the world: when, where).
    seen: Option<(i64, [f64; 3], f32)>,
    on_rail: bool,
    rolled: Option<(i64, f64)>,
    /// The first tick the cart is on T1's rail (visit-relative).
    t1_pass: Option<i64>,
    back: Option<(i64, usize)>,
}

/// One injected block (`--act SLOT,DT,X,Y,Z,BLOCK[,FOR]`): set at slot `slot` + `dt`; with
/// `hold`, the block it replaced put back `hold` gt later.
#[derive(Clone, Copy, Debug)]
pub struct Act {
    pub slot: i64,
    pub dt: i64,
    pub cell: [i32; 3],
    pub block: Block,
    pub hold: Option<i64>,
    /// (tick-exact's unload/reload act) `Some(None)`: every chunk of the part unloaded
    /// (`reload::unload_all`), loaded again `hold` gt later; `Some(Some([x0, z0, x1, z1]))`: the
    /// forced tickets of that chunk box dropped (`reload::unload_some`, a partial unload: the
    /// chunks next to a loaded one tick their blocks at level 32, entities frozen; level 33
    /// nothing), back `hold` gt later. `cell` and `block` unused then.
    pub unload: Option<Option<[i32; 4]>>,
    /// `SLOT,DT,place,K,X,Y,Z`: cart K (the bench's numbering) and its rider set at (X, Y, Z) in
    /// the loop's frame, at rest (a lab act: the merged-pairs control, a cart stopped anywhere).
    pub place: Option<(usize, [f64; 3])>,
    /// (tickexact 2026-11-11, the reload-order rule) A whole-part unload's load order
    /// (`SLOT,DT,unload,N,ORDER`: `save`, `reverse` or `seed:K`; `reload::load_all_ordered`).
    pub order: redsim::reload::Order,
}

/// Parses `SLOT,DT,X,Y,Z,BLOCK[,FOR]` (the block state may hold commas inside its brackets),
/// `SLOT,DT,place,K,X,Y,Z` (cart K and its rider set at a loop-frame point, at rest),
/// `SLOT,DT,unload,N[,ORDER]` (the whole part away N gt, its load in ORDER: save, reverse or
/// seed:K) or `SLOT,DT,unload,X0,Z0,X1,Z1,N` (a chunk box).
pub fn parse_act(s: &str) -> Result<Act, String> {
    let f: Vec<&str> = s.split(',').map(|t| t.trim()).collect();
    if f.len() >= 3 && f[2] == "unload" {
        let num = |t: &str, what: &str| t.parse::<i64>().map_err(|e| format!("--act {s}: {what}: {e}"));
        let (slot, dt) = (num(f[0], "SLOT")?, num(f[1], "DT")?);
        let (unload, n, order) = match f.len() {
            4 => (None, num(f[3], "N")?, redsim::reload::Order::Save),
            5 => (None, num(f[3], "N")?, redsim::reload::Order::parse(f[4]).ok_or_else(|| format!("--act {s}: ORDER is save, reverse or seed:K (K < 1000)"))?),
            8 => (Some([num(f[3], "X0")? as i32, num(f[4], "Z0")? as i32, num(f[5], "X1")? as i32, num(f[6], "Z1")? as i32]), num(f[7], "N")?, redsim::reload::Order::Save),
            _ => return Err(format!("--act {s}: give SLOT,DT,unload,N[,ORDER] or SLOT,DT,unload,X0,Z0,X1,Z1,N (a chunk box)")),
        };
        let air = Block::parse("minecraft:air").map_err(|e| format!("--act {s}: {e}"))?;
        return Ok(Act { slot, dt, cell: [0; 3], block: air, hold: Some(n.max(1)), unload: Some(unload), place: None, order });
    }
    if f.len() == 7 && f[2] == "place" {
        let num = |t: &str, what: &str| t.parse::<i64>().map_err(|e| format!("--act {s}: {what}: {e}"));
        let fl = |t: &str, what: &str| t.parse::<f64>().map_err(|e| format!("--act {s}: {what}: {e}"));
        let air = Block::parse("minecraft:air").map_err(|e| format!("--act {s}: {e}"))?;
        // (K `head`: the cart nearest the queue's head slot when the act fires, so the next
        // release group is one short and the placed cart is the one it misses, in order)
        let k = if f[3] == "head" { usize::MAX } else { num(f[3], "K")? as usize };
        return Ok(Act { slot: num(f[0], "SLOT")?, dt: num(f[1], "DT")?, cell: [0; 3], block: air, hold: None, unload: None, place: Some((k, [fl(f[4], "X")?, fl(f[5], "Y")?, fl(f[6], "Z")?])), order: redsim::reload::Order::Save });
    }
    let mut parts = s.splitn(6, ',');
    let mut num = |what: &str| -> Result<i64, String> { parts.next().ok_or(format!("--act {s}: no {what}"))?.trim().parse::<i64>().map_err(|e| format!("--act {s}: {what}: {e}")) };
    let (slot, dt, x, y, z) = (num("SLOT")?, num("DT")?, num("X")?, num("Y")?, num("Z")?);
    let rest = parts.next().ok_or(format!("--act {s}: no BLOCK"))?.trim();
    // (a trailing ",FOR" outside the brackets)
    let (block, hold) = match rest.rsplit_once(',') {
        Some((b, f)) if f.trim().chars().all(|ch| ch.is_ascii_digit()) && !f.trim().is_empty() && b.matches('[').count() == b.matches(']').count() => (b.trim(), Some(f.trim().parse::<i64>().map_err(|e| e.to_string())?)),
        _ => (rest, None),
    };
    let name = if block.contains(':') { block.to_string() } else { format!("minecraft:{block}") };
    let block = Block::parse(&name).map_err(|e| format!("--act {s}: {e}"))?;
    Ok(Act { slot, dt, cell: [x as i32, y as i32, z as i32], block, hold, unload: None, place: None, order: redsim::reload::Order::Save })
}

/// Day-1's merge (b): at cycle `cycle`, +4, the cart in queue slot `park` leaves the loop; at +`at`
/// it comes back by a side rail beside slot `slot`'s braked slope, moving toward it at `speed`
/// b/t, or (`staged`) arriving at `speed` 20 gt earlier onto a braked staging slope that is
/// powered for 10 gt at +`at`.
#[derive(Clone, Copy, Debug)]
pub struct SideDrop {
    pub cycle: i64,
    pub slot: usize,
    pub at: i64,
    pub speed: f64,
    pub park: usize,
    pub staged: bool,
    pub staged2: bool,
}

impl Default for LoopTiming {
    fn default() -> LoopTiming {
        LoopTiming { release_for: 30, advance_at: 40, advance_every: 20, pulse: 9, depart_every: 12, stand_brake: None, stand_detect: None, stand_detect_each: None, intake_brake: None, inject_at: None, depart_at: 4, depart_period: 16, open_after: 8, wired: None, golem_for: 10, stand_reset: None, shift_depart: false, s1_at: 0, tail0_late: 0, bench_stand: false, stand_offset: 0, release_stagger: 0, tail_check_at: None, stand_detect_slots: None, release_start: 0, adv_shift: [0; 3], adv_width: [0; 3], stand_shift: [0; 3], entry_speed: 0.0, skew: [0; 24], ideal_stand_at: None, tail_check_rest: false, tail2_at: None, placed_raw: false, build_gap: None, build_priced: false, stagger_off: false, side_drop: None, stop_open: 0, stop_close: 40, pocket_release: 10, pocket_reset: 52, stop_brake_delay: 0, stop_cv: 8, pocket_close_delay: 0, rel_for: 40, rel_on: 0, visit: None, build_last: [None; 64], build_fills_last: false, watch_cells: [None; 8], pocket_trigger: None, no_summons: false, record_positions: false, golem_miss: [-1; 8], golem_jitter: 0, golem_late: 0, day_only: false, slept: false, pulse_at: None, summon_at: None, acts: [None; 8], reset_queue_at: None, reset_stragglers: false, reset_stand_carts: false, reset_queue_every: false, no_inject: false, fifo_intake: (2, 6), esc_b: (6, 6), verbose_tail: false, tail_trace: None, esc_g0: (2, 0), esc_adm: (8, 3), adm_latch: false, p_always: false, adm_g1_w: 0, adm_k2: -1, c_prime: None, rel_ext: None, rel_min: 0, late_hold: [None; 3], rel_gate: None, start_random: false, calm: false }
    }
}

/// Runs the loop for `cycles` slots of 100 gt with ideal controls: at each slot the stand's batch
/// leaves as a train (its slots powered) and the queue's front 3 leave as a train for the stand
/// (their slots powered `release_for`), the stand braked as the train's front cart reaches S1's
/// slope; the queue advanced 3 slots; the queue's 3 empty tail slots powered for the returning
/// train, braked as its front cart reaches the third. Checks the loop order (every cart 3
/// places on per slot) and prints each slot's times.
/// The approach's rest check (`queue-loop-run --place-rest`): for each cell, a lone ridden cart set
/// on it at each speed (b/t along the travel there; negative: backward), the stand's REL pulsed as
/// the bench does (+10..+50 of every 100 gt, from 100 gt in); where the cart first rests (20 gt
/// without moving) and where it is after the next REL. `cells` empty: every path cell from the
/// queue head's slope up to pocket 3's rest slope (exclusive), the loop's frame.
pub fn rest_check(spec: &FarmSpec, o: &LoopOpts, cells: &[[i32; 3]], speeds: &[f64], ticks: i64) -> Result<String, String> {
    let lay = loop_layout(o);
    let (sx, sz) = (o.shift[0], o.shift[1]);
    let n = lay.path.len();
    let head = lay.slot_cells[lay.slot_cells.len() - 1];
    let i0 = lay.path.iter().position(|(p, _)| *p == head).ok_or("no head slot in the path")?;
    let p3 = [-o.row_gap + 2 + sx, 0, sz];
    let i1 = lay.path.iter().position(|(p, _)| *p == p3).ok_or("no pocket 3 rest slope in the path")?;
    let todo: Vec<[i32; 3]> = if cells.is_empty() { (i0 + 1..i1).map(|i| lay.path[i].0).collect() } else { cells.iter().map(|c| [c[0] + sx, c[1], c[2] + sz]).collect() };
    let rel = lay.stand_rel.ok_or("not a wired pocket stand")?;
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let pocket_of = |p: redsim::entity::Vec3| -> Option<usize> {
        if (p.z - 0.5 - sz as f64).abs() >= 0.6 || p.y >= 1.0 {
            return None;
        }
        match p.x - sx as f64 - (o.row_gap - 3) as f64 {
            x if (7.5..8.0).contains(&x) => Some(1),
            x if (3.5..4.0).contains(&x) => Some(2),
            x if (-0.5..0.0).contains(&x) => Some(3),
            _ => None,
        }
    };
    let mut out = Vec::new();
    let (mut ok, mut bad) = (0, 0);
    for c in &todo {
        let i = lay.path.iter().position(|(p, _)| p == c).ok_or_else(|| format!("{c:?} is not a path cell"))?;
        let nx = lay.path[(i + 1) % n].0;
        let (dx, dz) = ((nx[0] - c[0]) as f64, (nx[2] - c[2]) as f64);
        let shape = {
            let ro = RunOpts { ai: true, ..RunOpts::steady(1, 10, WalkMode::Random) };
            let run = Run::start(spec, &ro)?;
            run.world.get(BlockPos::new(c[0], c[1], c[2])).to_string().replace("minecraft:", "")
        };
        let mut res: Vec<String> = Vec::new();
        let mut all_ok = true;
        for v in speeds {
            let ro = RunOpts { ai: true, ..RunOpts::steady(1, (ticks + 400) as u32, WalkMode::Random) };
            let mut run = Run::start(spec, &ro)?;
            while run.settle_tick() {}
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [c[0] as f64 + 0.5, c[1] as f64 + 0.5, c[2] as f64 + 0.5], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?;
            if let Some(k) = redsim::entity::index_of(&run.world, id) {
                run.world.entities.list[k].delta_movement = redsim::entity::Vec3::new(dx * v, 0.0, dz * v);
            }
            let (mut still, mut first): (i64, Option<(i64, redsim::entity::Vec3)>) = (0, None);
            let mut after: Option<(i64, redsim::entity::Vec3)> = None;
            for t in 0..ticks {
                if t >= 100 && t % 100 == 10 {
                    run.world.set_block(rel, on, redsim::world::flags::ALL);
                }
                if t >= 100 && t % 100 == 50 {
                    run.world.set_block(rel, off, redsim::world::flags::ALL);
                }
                run.tick();
                let Some(k) = redsim::entity::index_of(&run.world, id) else { break };
                let e = &run.world.entities.list[k];
                if e.delta_movement.horizontal_distance() < 1e-4 && (e.pos.x - e.xo).abs() < 1e-6 && (e.pos.z - e.zo).abs() < 1e-6 {
                    still += 1;
                } else {
                    still = 0;
                }
                if still == 20 {
                    if first.is_none() {
                        first = Some((t - 19, e.pos));
                    } else if t > 160 && after.is_none() {
                        after = Some((t - 19, e.pos));
                    }
                }
            }
            let fmt = |r: Option<(i64, redsim::entity::Vec3)>| match r {
                Some((t, p)) => match pocket_of(p) {
                    Some(k) => format!("pocket {k} at +{t}"),
                    None if p.z - sz as f64 > 19.0 => format!("left with the REL, at rest at {:.2},{:.2},{:.2} (+{t}, the queue's tail)", p.x - sx as f64, p.y, p.z - sz as f64),
                    None => format!("RESTS at {:.2},{:.2},{:.2} (+{t})", p.x - sx as f64, p.y, p.z - sz as f64),
                },
                None => "never at rest".to_string(),
            };
            let good = first.is_some_and(|(_, p)| pocket_of(p).is_some());
            all_ok &= good;
            res.push(format!("v {v:+.2}: {}{}", fmt(first), after.map(|a| format!(", after REL {}", fmt(Some(a)))).unwrap_or_default()));
        }
        if all_ok { ok += 1 } else { bad += 1 }
        out.push(format!("{} {},{},{} {}: {}", if all_ok { "ok  " } else { "REST" }, c[0] - sx, c[1], c[2] - sz, shape, res.join("; ")));
    }
    out.insert(0, format!("rest check: {} cells x {} speeds ({}), a lone ridden cart, REL at +10..+50 of every 100 gt from 100 gt: {ok} cells end in a pocket every time, {bad} hold a cart elsewhere", todo.len(), speeds.len(), speeds.iter().map(|v| format!("{v:+.2}")).collect::<Vec<_>>().join(" ")));
    Ok(out.join("\n"))
}

/// Golem spawn cells around the loop: for a villager riding at each rail cell of the loop, every
/// cell where its golem search (`golem_spawn_cells_from`, the game's LEGACY_IRON_GOLEM scan of the
/// 17 x 17 columns with `checkSpawnObstruction`) would spawn a golem, with the design built and
/// its carts in place. A summon anywhere but the pad must find none.
pub fn loop_spots(spec: &FarmSpec, o: &LoopOpts) -> Result<String, String> {
    golem_spots(spec, Some(o), &[], false, 40, None, &[])
}

/// The golem-spots check (the brief's "no golem spots off the pad"): the game's summon search
/// (`golem_spawn_cells_from`) from every position a villager can occupy: the loop's rail cells
/// (`o`, a loop scene), plus `extra` (positions recorded over a full bench night, or a design's
/// `positions:` list: the siding's branch, the cage, the bed, the dismount and stand-up cells, the
/// way back). Block-only unless `with_entities`: a cell whose only obstruction is an entity in the
/// golem's box (a cart, a rider, the zombie: they move) counts as a spot. With a `pad` (a box): a
/// spot inside it is allowed only when every position that sees it is a stand position (`stand`,
/// plus a loop scene's own stand cells and pockets); a pad cell seen from any other position (the
/// arriving or returning batch, the queue, the napper's path) is an out-of-turn summon, listed on
/// its own line.
pub fn golem_spots(spec: &FarmSpec, o: Option<&LoopOpts>, extra: &[BlockPos], with_entities: bool, list: usize, pad: Option<(BlockPos, BlockPos)>, stand: &[BlockPos]) -> Result<String, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 400, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    for _ in 0..20 {
        run.tick();
    }
    let path: Vec<BlockPos> = o.map(|o| loop_layout(o).path.iter().map(|(c, _)| BlockPos::new(c[0], c[1], c[2])).collect()).unwrap_or_default();
    let mut starts: Vec<(BlockPos, bool)> = path.iter().map(|p| (*p, false)).collect();
    let mut seen: std::collections::HashSet<BlockPos> = path.iter().copied().collect();
    for p in extra {
        if seen.insert(*p) {
            starts.push((*p, true));
        }
    }
    // (each spot: the positions it is seen from, whether any is a rail cell, whether an entity
    // covered it at the check)
    let mut found: std::collections::BTreeMap<[i32; 3], (Vec<BlockPos>, bool, bool)> = std::collections::BTreeMap::new();
    for (start, is_extra) in &starts {
        for (cell, ok) in redsim::entity::mob::spawn::golem_spawn_cells_from(&run.world, *start) {
            let entity_only = ok == Err("an entity is in the golem's box");
            if ok.is_ok() || (entity_only && !with_entities) {
                let f = found.entry([cell.x, cell.y, cell.z]).or_insert((Vec::new(), false, false));
                f.0.push(*start);
                f.1 |= !is_extra;
                f.2 |= entity_only;
            }
        }
    }
    let only_extra = found.values().filter(|f| !f.1).count();
    let covered = found.values().filter(|f| f.2).count();
    // (the pad: its cells allowed from the stand positions only)
    let lay_stand: Vec<BlockPos> = o.map(|o| { let l = loop_layout(o); l.stand_cells.iter().map(|c| BlockPos::new(c[0], c[1], c[2])).chain(l.pockets.iter().map(|(_, c)| BlockPos::new(c[0], c[1], c[2]))).collect::<Vec<_>>() }).unwrap_or_default();
    let stands: std::collections::HashSet<BlockPos> = stand.iter().copied().chain(lay_stand.iter().copied()).collect();
    let in_pad = |c: &[i32; 3]| pad.is_some_and(|(lo, hi)| c[0] >= lo.x && c[0] <= hi.x && c[1] >= lo.y && c[1] <= hi.y && c[2] >= lo.z && c[2] <= hi.z);
    let off_pad: Vec<(&[i32; 3], &(Vec<BlockPos>, bool, bool))> = found.iter().filter(|(c, _)| !in_pad(c)).collect();
    let pad_ok: Vec<&[i32; 3]> = found.iter().filter(|(c, f)| in_pad(c) && f.0.iter().all(|p| stands.contains(p))).map(|(c, _)| c).collect();
    let out_of_turn: Vec<(&[i32; 3], Vec<BlockPos>)> = found.iter().filter(|(c, _)| in_pad(c)).map(|(c, f)| (c, f.0.iter().filter(|p| !stands.contains(p)).copied().collect::<Vec<_>>())).filter(|(_, v)| !v.is_empty()).collect();
    let mut out = vec![format!("{} golem spawn cells off the pad seen from {} positions ({} rail cells of the loop, {} more listed or recorded){}; {} of them only from the extra positions, {} covered by an entity at the check", off_pad.len(), starts.len(), path.len(), starts.len() - path.len(), if with_entities { ", entities counted" } else { ", block-only (a cell only an entity covers counts)" }, only_extra, covered)];
    if pad.is_some() {
        out.push(format!("pad: {} pad cells seen from the {} stand positions only (allowed); {} pad cells seen from another position (out-of-turn summons onto the pad){}", pad_ok.len(), stands.len(), out_of_turn.len(), if out_of_turn.is_empty() { String::new() } else { format!(": {}", out_of_turn.iter().take(list.max(1)).map(|(c, v)| format!("{} {} {} from {} positions, e.g. {} {} {}", c[0], c[1], c[2], v.len(), v[0].x, v[0].y, v[0].z)).collect::<Vec<_>>().join("; ")) }));
    }
    let found: std::collections::BTreeMap<[i32; 3], (Vec<BlockPos>, bool, bool)> = off_pad.into_iter().map(|(c, f)| (*c, f.clone())).collect();
    for (cell, (from, _, cov)) in found.iter().take(list) {
        out.push(format!("  {} {} {} (from {} positions, e.g. {} {} {}){}", cell[0], cell[1], cell[2], from.len(), from[0].x, from[0].y, from[0].z, if *cov { " [an entity covered it]" } else { "" }));
    }
    Ok(out.join("\n"))
}

/// One control's timing shifted by `s` gt (see `loop_sweep`).
pub fn loop_shifted(tm: &LoopTiming, control: &str, s: i64) -> Result<LoopTiming, String> {
    let mut t = tm.clone();
    let k = |name: &str| ["1", "2", "3"].iter().position(|d| name.ends_with(d)).unwrap_or(0);
    match control {
        "release-start" => t.release_start += s,
        "release-end" => t.release_for += s,
        c if c.starts_with("adv") && c.ends_with("-time") => t.adv_shift[k(&c[..4])] += s,
        c if c.starts_with("adv") && c.ends_with("-width") => t.adv_width[k(&c[..4])] += s,
        "s1" | "s2" | "s3" => t.stand_shift[k(control)] += s,
        "stand-brake" => match (&mut t.stand_detect, &mut t.stand_detect_slots) {
            (_, Some(ds)) => ds.iter_mut().for_each(|d| *d += s),
            (Some(d), None) => *d += s,
            (None, None) => return Err("stand-brake: give the brake as --stand-detect or --stand-detect-slots".into()),
        },
        "tail12" | "tail0" => match &mut t.intake_brake {
            Some((a, b)) => {
                if control == "tail12" {
                    *a += s
                } else {
                    *b += s
                }
            }
            None => return Err("tail brakes: give them as --intake-brake A,B".into()),
        },
        "pocket-release" => t.pocket_release += s,
        "pocket-reset" => t.pocket_reset += s,
        // (a counting gate shuts on its count, never before: a negative delay never shuts it, so
        // the run fails, which is the line's early end)
        "gate-close" => t.pocket_close_delay += s,
        // (the wired stand's input: REL's start, its end moving with it (a pulse of fixed length
        // shifted), its length, and its start alone, the end at its own time)
        "rel-start" => t.pocket_release += s,
        "rel-for" => t.rel_for += s,
        "rel-on" => t.rel_on += s,
        // (the nap visits' junction timings, bench-driven: the divert's 10 gt window, T1/T2's set,
        // the holds' kick (every cart's release tick); a design whose controller drives them is
        // shifted by moving its own delays instead)
        "divert" | "t-set" | "kick" => {
            let v = t.visit.as_mut().ok_or_else(|| format!("{control}: the visit timings need --visit or --visits-nightly"))?;
            match control {
                "divert" => v.divert_at += s,
                "t-set" => v.t_at += s,
                _ => {
                    v.release_at += s;
                    for r in v.release_k.iter_mut().flatten() {
                        *r += s;
                    }
                }
            }
        }
        _ => return Err(format!("unknown control {control}: release-start, release-end, adv1-time .. adv3-time, adv1-width .. adv3-width, s1, s2, s3, stand-brake, tail12, tail0, pocket-release, pocket-reset, gate-close, rel-start, rel-for, rel-on, divert, t-set, kick, speed")),
    }
    Ok(t)
}

/// Whether the loop bench passes (0 problems) on every seed.
pub fn loop_passes(spec: &FarmSpec, o: &LoopOpts, t: &LoopTiming, seed: i64, seeds: i64, cycles: i64) -> Result<bool, String> {
    for s in seed..seed + seeds {
        let r = run_loop(spec, o, t, cycles, s, false, None, None)?;
        if !r.lines().any(|l| l == "0 problems") {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A control's tolerance: its earliest and latest passing shift (gt; b/t for `speed`).
pub struct SweepLine {
    pub control: String,
    pub early: f64,
    pub late: f64,
    pub text: String,
}

/// The tolerance line of one timed control (ironspec's rule): the bench at the given timing, then
/// the control's time shifted one gt at a time early and late to the first failure (`seeds` seeds
/// x `cycles` slots each, pass = 0 problems); `speed` sweeps the release group's entry speed kick
/// in 0.02 b/t steps instead.
pub fn loop_sweep(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, control: &str, range: i64, seed: i64, seeds: i64, cycles: i64) -> Result<SweepLine, String> {
    loop_sweep_jobs(spec, o, tm, control, range, seed, seeds, cycles, 1)
}

/// [`loop_sweep`] with up to `jobs` shifts tried at once on their own threads (the next `jobs`
/// shifts outward, the first failure in order ending the side), for slow benches (a wired night).
#[allow(clippy::too_many_arguments)]
pub fn loop_sweep_jobs(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, control: &str, range: i64, seed: i64, seeds: i64, cycles: i64, jobs: usize) -> Result<SweepLine, String> {
    let passes = |t: &LoopTiming| loop_passes(spec, o, t, seed, seeds, cycles);
    let shifted = |s: i64| loop_shifted(tm, control, s);
    // (several shifts at once: each its own thread, the answers in the shifts' order)
    let passes_many = |shifts: &[i64]| -> Result<Vec<bool>, String> {
        let ts: Vec<LoopTiming> = shifts.iter().map(|s| shifted(*s)).collect::<Result<_, _>>()?;
        std::thread::scope(|sc| {
            let hs: Vec<_> = ts.iter().map(|t| sc.spawn(move || loop_passes(spec, o, t, seed, seeds, cycles))).collect();
            hs.into_iter().map(|h| h.join().map_err(|_| "a sweep thread panicked".to_string())?).collect()
        })
    };
    let line = |early: f64, late: f64, text: String| SweepLine { control: control.to_string(), early, late, text };
    if control == "speed" {
        let at = |v: f64| -> Result<bool, String> { passes(&LoopTiming { entry_speed: v, ..tm.clone() }) };
        if !at(0.0)? {
            return Ok(line(0.0, 0.0, "speed: the bench fails with no kick".into()));
        }
        let (mut lo, mut hi) = (0.0, 0.0);
        let mut v = -0.02;
        while v >= -0.6 && at(v)? {
            lo = v;
            v -= 0.02;
        }
        v = 0.02;
        while v <= 0.6 && at(v)? {
            hi = v;
            v += 0.02;
        }
        return Ok(line(lo, hi, format!("speed: the release group's carts kicked from {lo:+.2} to {hi:+.2} b/t along their slopes all pass ({seeds} seeds x {cycles} slots each){}", if lo == 0.0 || hi == 0.0 { "; FLAGGED: no slack on one side" } else { "" })));
    }
    if !passes(&shifted(0)?)? {
        return Ok(line(0.0, 0.0, format!("{control}: the bench fails at the nominal timing")));
    }
    let (mut early, mut late) = (0, 0);
    if jobs > 1 {
        for side in [-1i64, 1] {
            let mut at = 0i64;
            'side: loop {
                let batch: Vec<i64> = (1..=jobs as i64).map(|i| at + side * i).filter(|s| s.abs() <= range).collect();
                if batch.is_empty() {
                    break;
                }
                for (s, ok) in batch.iter().zip(passes_many(&batch)?) {
                    if !ok {
                        break 'side;
                    }
                    at = *s;
                }
            }
            if side < 0 { early = at } else { late = at }
        }
    } else {
        while early > -range && passes(&shifted(early - 1)?)? {
            early -= 1;
        }
        while late < range && passes(&shifted(late + 1)?)? {
            late += 1;
        }
    }
    let edge = |v: i64| if v.abs() == range { format!("{v:+} (the sweep's edge)") } else { format!("{v:+}") };
    Ok(line(early as f64, late as f64, format!("{control}: passes from {} to {} gt ({seeds} seeds x {cycles} slots each){}", edge(early), edge(late), if early == 0 || late == 0 { "; FLAGGED: no slack on one side" } else { "" })))
}

/// The corner runs (the reviewer's note on the tolerance rule): every swept control at its
/// measured early limit together, then every one at its late limit, each with the entry speed
/// kick at its slow and its fast extreme (0 when `speed` was not swept).
pub fn loop_corners(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, lines: &[SweepLine], seed: i64, seeds: i64, cycles: i64) -> Result<String, String> {
    let speed = lines.iter().find(|l| l.control == "speed").map(|l| (l.early, l.late)).unwrap_or((0.0, 0.0));
    let mut out = Vec::new();
    for (name, late) in [("early", false), ("late", true)] {
        let mut t = tm.clone();
        for l in lines.iter().filter(|l| l.control != "speed") {
            t = loop_shifted(&t, &l.control, if late { l.late } else { l.early } as i64)?;
        }
        for v in [speed.0, speed.1] {
            let ok = loop_passes(spec, o, &LoopTiming { entry_speed: v, ..t.clone() }, seed, seeds, cycles)?;
            out.push(format!("corner: every control at its {name} limit, entry speed {v:+.2} b/t: {} ({seeds} seeds x {cycles} slots each)", if ok { "passes" } else { "FAILS" }));
        }
    }
    Ok(out.join("\n"))
}

/// The default build order (the brief's "built in order", for a design that lists none): supports
/// first, bottom up; then containers, each filled as it is placed (before power can reach it);
/// then rails, bottom up; then redstone from its sources outward (breadth-first over cells that
/// touch, wire's up and down steps included; a cell no source reaches starts a new search, the
/// lowest first). Each entry: the cell, its block, the container's fill (SNBT).
pub fn build_order(spec: &FarmSpec) -> Vec<(BlockPos, Block, Option<String>)> {
    build_order_of(&spec.blocks, &spec.block_entities)
}

/// [`build_order`] on any design's blocks and block entities (`nbt:` lines), for a circuit with no
/// farm spec (day-1's scheduler and release benches).
pub fn build_order_of(blocks: &[(BlockPos, Block)], block_entities: &[(BlockPos, String)]) -> Vec<(BlockPos, Block, Option<String>)> {
    fn class(b: &Block) -> u8 {
        match b {
            Block::Air => 9,
            Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } => 2,
            Block::Chest { .. } | Block::Hopper { .. } | Block::Dispenser { .. } | Block::Furnace { .. } | Block::ShulkerBox { .. } | Block::BrewingStand { .. } | Block::Crafter { .. } | Block::Jukebox { .. } | Block::Composter { .. } | Block::ChiseledBookshelf { .. } => 1,
            Block::Wire { .. } | Block::RedstoneBlock | Block::Torch { .. } | Block::WallTorch { .. } | Block::Lever { .. } | Block::Button { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Observer { .. } | Block::Lamp { .. } | Block::Bulb { .. } | Block::Note { .. } | Block::Target { .. } | Block::Piston { .. } | Block::PistonHead { .. } | Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::DaylightDetector { .. } | Block::Tnt { .. } | Block::Trapdoor { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Sculk { .. } => 3,
            _ => 0,
        }
    }
    let source = |b: &Block| matches!(b, Block::RedstoneBlock | Block::Torch { .. } | Block::WallTorch { .. } | Block::Lever { .. } | Block::Button { .. } | Block::DaylightDetector { .. } | Block::Plate { .. } | Block::TripwireHook { .. } | Block::Observer { .. } | Block::Target { .. });
    // (a block that holds up a rail, a wire, a repeater, a comparator, a torch or a plate is a
    // support whatever it is, a redstone block under a booster included: placed first, or what it
    // holds up would pop off)
    let needs_floor = |b: &Block| matches!(b, Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } | Block::Wire { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Torch { .. } | Block::Plate { .. });
    let holds: std::collections::HashSet<BlockPos> = blocks.iter().filter(|(_, b)| needs_floor(b)).map(|(p, _)| BlockPos::new(p.x, p.y - 1, p.z)).chain(blocks.iter().filter_map(|(p, b)| match b { Block::WallTorch { facing, .. } => Some(p.rel(facing.opposite())), _ => None })).collect();
    let class_at = |p: &BlockPos, b: &Block| if holds.contains(p) && !needs_floor(b) && class(b) == 3 { 0 } else { class(b) };
    let fills: std::collections::HashMap<BlockPos, String> = block_entities.iter().cloned().collect();
    let key = |p: &BlockPos| (p.y, p.x, p.z);
    let mut out = Vec::new();
    // (rails: each goes down only when no rail already down could take it as a link it was not
    // meant to have: a rail with fewer than two of its own links down takes a newcomer beside it,
    // as a queue slot's slope took a drop end placed before its peak; among the safe ones, by
    // (y, x, z); if none is safe, the first)
    let rail_links = |p: &BlockPos, b: &Block| -> Vec<(i32, i32)> {
        use redsim::block::RailShape as S;
        let sh = match b { Block::Rail { shape, .. } | Block::PlainRail { shape } | Block::DetectorRail { shape, .. } => *shape, _ => return Vec::new() };
        let (x, z) = (p.x, p.z);
        match sh {
            S::NorthSouth | S::AscendingNorth | S::AscendingSouth => vec![(x, z - 1), (x, z + 1)],
            S::EastWest | S::AscendingEast | S::AscendingWest => vec![(x - 1, z), (x + 1, z)],
            S::SouthEast => vec![(x, z + 1), (x + 1, z)],
            S::SouthWest => vec![(x, z + 1), (x - 1, z)],
            S::NorthWest => vec![(x, z - 1), (x - 1, z)],
            S::NorthEast => vec![(x, z - 1), (x + 1, z)],
        }
    };
    let rails_in_order = |mut left: Vec<(BlockPos, Block)>| -> Vec<(BlockPos, Block)> {
        left.sort_by_key(|(p, _)| key(p));
        let mut placed: Vec<(BlockPos, Block)> = Vec::new();
        while !left.is_empty() {
            let safe = |r: &BlockPos| placed.iter().all(|(p, pb)| {
                if (p.x - r.x).abs() + (p.z - r.z).abs() != 1 || (p.y - r.y).abs() > 1 {
                    return true;
                }
                let pl = rail_links(p, pb);
                if pl.contains(&(r.x, r.z)) {
                    return true;
                }
                pl.iter().filter(|(x, z)| placed.iter().any(|(q, _)| q.x == *x && q.z == *z && (q.y - p.y).abs() <= 1)).count() >= 2
            });
            let pick = left.iter().position(|(r, _)| safe(r)).unwrap_or(0);
            placed.push(left.remove(pick));
        }
        placed
    };
    for c in 0..3u8 {
        let mut v: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, b)| class_at(p, b) == c).cloned().collect();
        v.sort_by_key(|(p, _)| key(p));
        if c == 2 {
            v = rails_in_order(v);
        }
        out.extend(v.into_iter().map(|(p, b)| (p, b, fills.get(&p).cloned())));
    }
    let reds: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, b)| class_at(p, b) == 3).cloned().collect();
    let idx: std::collections::HashMap<BlockPos, usize> = reds.iter().enumerate().map(|(i, (p, _))| (*p, i)).collect();
    let mut seen = vec![false; reds.len()];
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut starts: Vec<usize> = (0..reds.len()).filter(|&i| source(&reds[i].1)).collect();
    starts.sort_by_key(|&i| key(&reds[i].0));
    for i in starts {
        seen[i] = true;
        queue.push_back(i);
    }
    let mut order = Vec::new();
    loop {
        while let Some(i) = queue.pop_front() {
            order.push(i);
            let p = reds[i].0;
            for dx in -1..=1i32 {
                for dy in -1..=1i32 {
                    for dz in -1..=1i32 {
                        let m = dx.abs() + dy.abs() + dz.abs();
                        // (face neighbours, and the up and down steps of a wire)
                        if m == 0 || m > 2 || (m == 2 && dy == 0) {
                            continue;
                        }
                        if let Some(&j) = idx.get(&BlockPos::new(p.x + dx, p.y + dy, p.z + dz)) {
                            if !seen[j] {
                                seen[j] = true;
                                queue.push_back(j);
                            }
                        }
                    }
                }
            }
        }
        match (0..reds.len()).filter(|&i| !seen[i]).min_by_key(|&i| key(&reds[i].0)) {
            Some(i) => {
                seen[i] = true;
                queue.push_back(i);
            }
            None => break,
        }
    }
    out.extend(order.into_iter().map(|i| (reds[i].0, reds[i].1, fills.get(&reds[i].0).cloned())));
    out
}

/// The cells the build bill prices as `place:edge` (prices.rs's rule, approximated on the design's
/// own blocks): more than 2 above the design's lowest level, with no standable surface (a block a
/// player can stand on, two air above it) within 3 horizontally and 1 or 2 below. A player takes
/// an extra second to place each (sneaking to an edge), so the in-order build gives it a longer gap.
pub fn edge_cells(spec: &FarmSpec) -> std::collections::HashSet<BlockPos> {
    edge_cells_of(&spec.blocks)
}

/// [`edge_cells`] on any design's blocks.
pub fn edge_cells_of(blocks: &[(BlockPos, Block)]) -> std::collections::HashSet<BlockPos> {
    let cells: std::collections::HashSet<BlockPos> = blocks.iter().filter(|(_, b)| !matches!(b, Block::Air)).map(|(p, _)| *p).collect();
    let standable = |b: &Block| !matches!(b, Block::Air | Block::Wire { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } | Block::Button { .. } | Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::Water { .. } | Block::Lava { .. } | Block::Fire { .. } | Block::BubbleColumn { .. });
    let up = |p: &BlockPos, d: i32| BlockPos::new(p.x, p.y + d, p.z);
    let surfaces: std::collections::HashSet<BlockPos> = blocks.iter().filter(|(p, b)| standable(b) && !cells.contains(&up(p, 1)) && !cells.contains(&up(p, 2))).map(|(p, _)| *p).collect();
    let low = blocks.iter().map(|(p, _)| p.y).min().unwrap_or(0);
    cells
        .iter()
        .filter(|c| c.y - low > 2 && !(-3..=3).any(|dx| (-3..=3).any(|dz| (1..=2).any(|dy| surfaces.contains(&BlockPos::new(c.x + dx, c.y - dy, c.z + dz))))))
        .copied()
        .collect()
}

/// A rail's `shape` turned `turn` quarter turns clockwise seen from above (north to east), named
/// the game's way (a curve's north/south word first). `BlockState::rotate_y` leaves `shape` alone.
fn rail_shape_turned(shape: &str, turn: i32) -> String {
    let rot = |d: &str| -> String {
        let order = ["north", "east", "south", "west"];
        match order.iter().position(|o| *o == d) {
            Some(i) => order[(i + turn.rem_euclid(4) as usize) % 4].to_string(),
            None => d.to_string(),
        }
    };
    match shape {
        "north_south" | "east_west" => {
            if turn.rem_euclid(2) == 1 {
                if shape == "north_south" { "east_west" } else { "north_south" }.to_string()
            } else {
                shape.to_string()
            }
        }
        s if s.starts_with("ascending_") => format!("ascending_{}", rot(&s["ascending_".len()..])),
        s => {
            let Some((a, b)) = s.split_once('_') else { return s.to_string() };
            let (a, b) = (rot(a), rot(b));
            if a == "north" || a == "south" { format!("{a}_{b}") } else { format!("{b}_{a}") }
        }
    }
}

/// A block turned `turn` quarter turns clockwise seen from above (+x to +z).
fn block_turned(b: &Block, turn: i32) -> Result<Block, String> {
    if turn.rem_euclid(4) == 0 {
        return Ok(*b);
    }
    let mut st = mctas::blockstate::BlockState::parse(&b.to_string()).map_err(|e| e.to_string())?.rotate_y(turn);
    if st.name.ends_with("rail") {
        if let Some(s) = st.props.get("shape").cloned() {
            st.props.insert("shape".into(), rail_shape_turned(&s, turn));
        }
    }
    Block::parse(&st.to_string()).map_err(|e| e.to_string())
}

/// Pastes a part (farmopt's bed siding, ...) into the loop's spec: its blocks, block entities and
/// entities that lie in its local box `bx` (lo, hi; the part's bench parts outside it are left
/// out), minecarts dropped (the loop brings its own 21), turned `turn` quarter turns clockwise
/// seen from above (+x to +z) about its origin, then moved to `at`. A cell the loop already uses
/// is an error. Returns the blocks pasted.
pub fn paste_part(spec: &mut FarmSpec, part: &FarmSpec, at: [i32; 3], turn: i32, bx: ([i32; 3], [i32; 3])) -> Result<usize, String> {
    let t = turn.rem_euclid(4);
    let inside = |x: i32, y: i32, z: i32| (bx.0[0]..=bx.1[0]).contains(&x) && (bx.0[1]..=bx.1[1]).contains(&y) && (bx.0[2]..=bx.1[2]).contains(&z);
    let cell = |p: BlockPos| -> BlockPos {
        let (mut x, mut z) = (p.x, p.z);
        for _ in 0..t {
            (x, z) = (-z, x);
        }
        BlockPos::new(x + at[0], p.y + at[1], z + at[2])
    };
    let used: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| !matches!(b, Block::Air)).map(|(p, _)| *p).collect();
    let mut n = 0;
    for (p, b) in part.blocks.iter().filter(|(p, b)| inside(p.x, p.y, p.z) && !matches!(b, Block::Air)) {
        let q = cell(*p);
        if used.contains(&q) {
            return Err(format!("paste: the part's {} at {} {} {} lands on the loop's cell {} {} {}", b, p.x, p.y, p.z, q.x, q.y, q.z));
        }
        spec.blocks.push((q, block_turned(b, t)?));
        spec.lo = BlockPos::new(spec.lo.x.min(q.x), spec.lo.y.min(q.y), spec.lo.z.min(q.z));
        spec.hi = BlockPos::new(spec.hi.x.max(q.x), spec.hi.y.max(q.y), spec.hi.z.max(q.z));
        n += 1;
    }
    for (p, s) in part.block_entities.iter().filter(|(p, _)| inside(p.x, p.y, p.z)) {
        spec.block_entities.push((cell(*p), s.clone()));
    }
    for (ty, pos, nbt) in part.entities.iter().filter(|(ty, pos, _)| !ty.ends_with("minecart") && inside(pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32)) {
        // (a continuous point turned about the origin block's centre: (x, z) to (1 - z, x))
        let (mut x, mut z) = (pos[0], pos[2]);
        for _ in 0..t {
            (x, z) = (1.0 - z, x);
        }
        spec.entities.push((ty.clone(), [x + at[0] as f64, pos[1] + at[1] as f64, z + at[2] as f64], nbt.clone()));
    }
    Ok(n)
}

pub fn run_loop(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, cycles: i64, seed: i64, verbose: bool, watch: Option<&[usize]>, drive_out: Option<&std::path::Path>) -> Result<String, String> {
    run_loop_hdr(spec, o, tm, &Headers::default(), cycles, seed, verbose, watch, drive_out)
}

/// `run_loop` with the design file's own declared states and flights (`Headers`) checked too.
#[allow(clippy::too_many_arguments)]
pub fn run_loop_hdr(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, hdr: &Headers, cycles: i64, seed: i64, verbose: bool, watch: Option<&[usize]>, drive_out: Option<&std::path::Path>) -> Result<String, String> {
    let ticks = 200 + cycles * 100 + 200;
    let ro = RunOpts { ai: true, raw: tm.placed_raw, ..RunOpts::steady(seed, ticks as u32, WalkMode::Random) };
    // (`build_gap`, the brief's "built in order": the world starts empty and the design is placed
    // cell by cell in its build order, each placement with the game's updates, `build_gap` ticks
    // between placements, the carts spawned after the last)
    let empty = FarmSpec { blocks: Vec::new(), block_entities: Vec::new(), ..spec.clone() };
    let mut run = Run::start(if tm.build_gap.is_some() { &empty } else { spec }, &ro)?;
    // (`start_random`: the run starts at a day time drawn from the seed, 0..24,000: the power-on
    // sweep (day-1 19:02: from any time of day, how long to the first golem and to the grid))
    let start_day = if tm.start_random {
        let mut z = (seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5DEE_CE66_D1CE_4E5B;
        z = (z ^ (z >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= z >> 29;
        let d = (z % 24000) as i64;
        run.world.mob.day_offset = d - run.world.time;
        Some(d)
    } else {
        None
    };
    // (`placed_raw`: the file as written, no settle either: it must start at rest by itself)
    if !tm.placed_raw && tm.build_gap.is_none() {
        while run.settle_tick() {}
    }
    // (the daylight detectors: `Run::start` holds them back for `Run::arrive`, which this runner
    // never calls (it spawns its own carts): placed here, after the settle, as `arrive` would;
    // the build in order places them with the rest. day-1 found every run's detector missing.
    // With `wired`, placed at slot 0 +0 with the first golem pulse instead (day-1, 19:03): a seed
    // starting at night would arm the controller's fallback before the bench's first pulse)
    let dets_at_slot0 = tm.wired.is_some() && tm.build_gap.is_none();
    let dets: Vec<(BlockPos, Block)> = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::DaylightDetector { .. })).cloned().collect();
    if tm.build_gap.is_none() && !dets_at_slot0 {
        if !dets.is_empty() {
            run.world.place_raw(&dets);
            if !tm.placed_raw {
                run.world.place_updates(&dets);
            }
        }
    }
    let drive: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    let mut build_start: Option<i64> = None;
    let mut built_diff: Option<String> = None;
    if let Some(g) = tm.build_gap {
        let t_b = run.world.time;
        build_start = Some(t_b);
        // (`build_priced`: each cell's own priced gap, prices.tsv's place 1 s = 20 gt, and 2 s =
        // 40 gt for a cell the bill charges place:edge, the longest priced gap; else `g` for all)
        let edges = edge_cells(spec);
        // (a design's `build: last x,y,z ...`: those cells after every other one, in its order, and
        // after the `fills-last` stocks too, as placing.rs's build steps list them (day-1, 20:55:
        // detectors listed last went down before the rings were stocked))
        let mut order = build_order(spec);
        let mut last: Vec<(BlockPos, Block, Option<String>)> = Vec::new();
        for p in tm.build_last.iter().flatten() {
            if let Some(i) = order.iter().position(|(q, _, _)| q == p) {
                last.push(order.remove(i));
            }
        }
        let mut fills: Vec<(BlockPos, String)> = Vec::new();
        let place = |run: &mut Run, p: BlockPos, b: Block, fill: Option<String>, defer: bool, fills: &mut Vec<(BlockPos, String)>| -> Result<(), String> {
            run.world.set_block(p, b, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_b, p.x, p.y, p.z, b));
            if let Some(f) = fill {
                if defer {
                    fills.push((p, f));
                } else {
                    redsim::redstone::container::apply_block_entities(&mut run.world, [(p, f.as_str())])?;
                    drive.borrow_mut().push(format!("act {} nbt {} {} {} {}", run.world.time - t_b, p.x, p.y, p.z, f));
                }
            }
            let gap = if tm.build_priced { if edges.contains(&p) { 40 } else { 20 } } else { g };
            for _ in 0..gap.max(1) {
                run.tick();
            }
            Ok(())
        };
        for (p, b, fill) in order {
            place(&mut run, p, b, fill, tm.build_fills_last, &mut fills)?;
        }
        // (`build: fills-last`: the containers' stocks after every block, in the build's order)
        for (p, f) in std::mem::take(&mut fills) {
            redsim::redstone::container::apply_block_entities(&mut run.world, [(p, f.as_str())])?;
            drive.borrow_mut().push(format!("act {} nbt {} {} {} {}", run.world.time - t_b, p.x, p.y, p.z, f));
            for _ in 0..(if tm.build_priced { 20 } else { g }).max(1) {
                run.tick();
            }
        }
        for (p, b, fill) in last {
            place(&mut run, p, b, fill, false, &mut fills)?;
        }
        // (the cells the build left different from the file: a design that needs a listed order
        // or a "place this last" step shows up here)
        let diff: Vec<String> = spec.blocks.iter().filter(|(p, b)| run.world.get(*p) != *b).map(|(p, b)| format!("{} {} {}: {} (file: {})", p.x, p.y, p.z, run.world.get(*p), b)).collect();
        if !diff.is_empty() {
            // (by kind, and every cell whose kind or shape differs, not only its power: a rail
            // that took another shape, a block that is not the file's)
            let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            let mut shaped: Vec<String> = Vec::new();
            for (p, b) in spec.blocks.iter().filter(|(p, b)| run.world.get(*p) != *b) {
                let now = run.world.get(*p);
                let name = |s: &str| s.split('[').next().unwrap_or("").to_string();
                *kinds.entry(name(&b.to_string())).or_insert(0) += 1;
                let strip = |s: String| s.split(['[', ',', ']']).filter(|t| !t.starts_with("power=") && !t.starts_with("powered=") && !t.starts_with("lit=") && !t.starts_with("triggered=") && !t.starts_with("enabled=")).collect::<Vec<_>>().join(",");
                if strip(now.to_string()) != strip(b.to_string()) {
                    shaped.push(format!("{} {} {}: {} (file: {})", p.x, p.y, p.z, now, b));
                }
            }
            built_diff = Some(format!("built in order: {} cells differ from the file after the build ({}); {} differ in more than power: {}", diff.len(), kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "), shaped.len(), if shaped.is_empty() { "none".to_string() } else { shaped.iter().take(12).cloned().collect::<Vec<_>>().join("; ") }));
        }
    }
    // (`side_drop`'s side rail, placed like a player places it (each block with its updates) after
    // the loop is built: three plain rails west of the slot's slope at the peaks' height on glass,
    // a stone stop east of it; any loop cell it reshapes is a problem)
    let mut side_changed: Vec<String> = Vec::new();
    if let Some(sd) = tm.side_drop {
        let slot = sd.slot;
        let lay0 = loop_layout(o);
        let before: Vec<([i32; 3], Block)> = lay0.path.iter().map(|(c, _)| (*c, run.world.get(BlockPos::new(c[0], c[1], c[2])))).collect();
        let s = lay0.slot_cells[slot];
        let glass = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
        let rail = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").map_err(|e| e.to_string())?;
        let stone = Block::parse("minecraft:stone").map_err(|e| e.to_string())?;
        if sd.staged {
            // (day-1's staged drop: two plain rails one block higher, then a braked powered rail
            // ascending west on the end cell, its support a control the bench switches; with
            // `staged2` the upper rail's east cell is a braked powered rail too, powered through
            // the slope)
            let stage = Block::parse("minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]").map_err(|e| e.to_string())?;
            let brake = Block::parse("minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]").map_err(|e| e.to_string())?;
            for dx in [3, 2] {
                run.world.set_block(BlockPos::new(s[0] - dx, s[1] + 1, s[2]), glass, redsim::world::flags::ALL);
                run.world.set_block(BlockPos::new(s[0] - dx, s[1] + 2, s[2]), if sd.staged2 && dx == 2 { brake } else { rail }, redsim::world::flags::ALL);
            }
            run.world.set_block(BlockPos::new(s[0] - 1, s[1], s[2]), glass, redsim::world::flags::ALL);
            run.world.set_block(BlockPos::new(s[0] - 1, s[1] + 1, s[2]), stage, redsim::world::flags::ALL);
        } else {
            for dx in [3, 2, 1] {
                run.world.set_block(BlockPos::new(s[0] - dx, s[1], s[2]), glass, redsim::world::flags::ALL);
                run.world.set_block(BlockPos::new(s[0] - dx, s[1] + 1, s[2]), rail, redsim::world::flags::ALL);
            }
        }
        run.world.set_block(BlockPos::new(s[0] + 1, s[1] + 1, s[2]), stone, redsim::world::flags::ALL);
        for (c, b) in before {
            let now = run.world.get(BlockPos::new(c[0], c[1], c[2]));
            if now != b {
                side_changed.push(format!("{} {} {}: {} (was {})", c[0], c[1], c[2], now, b));
            }
        }
    }
    let mut side_cart: Option<u32> = None;
    let mut side_rest: Option<(i64, f64, usize)> = None;
    let mut side_next: Vec<(usize, i64)> = Vec::new();
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let lay = loop_layout(o);
    // (`shift`: the bench's own coordinates below, in the loop's frame, moved with the design)
    let (fx, fz) = (o.shift[0] as f64, o.shift[1] as f64);
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    // (a nap visit's T controls are off as a target block: see the visit's T's)
    let target = Block::parse("minecraft:target[power=0]").map_err(|e| e.to_string())?;
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
    let mut carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 21 {
        return Err(format!("{} carts, not 21", carts.len()));
    }
    // (each cart's rider)
    let mut riders: Vec<u32> = carts.iter().map(|c| run.world.entities.list.iter().find(|e| e.id == *c).and_then(|e| e.passengers.first().copied()).unwrap_or(0)).collect();
    let mut night_ticks = vec![0u64; riders.len()];
    let mut panic_ticks = vec![0u64; riders.len()];
    let mut calm_at: std::collections::BTreeMap<[i32; 3], u64> = std::collections::BTreeMap::new();
    // the loop's places in travel order: slot 0 .. slot q-1, S3, S2, S1
    let q = o.slots as usize;
    let mut places: Vec<[i32; 3]> = lay.slot_cells.clone();
    places.extend(lay.stand_cells.iter().rev());
    // (a place's span along x: a queue slot its slope ±1, a V stand slot its slope, valley and
    // slope up)
    let wide = !lay.stand_b.is_empty();
    let place_of = |w: &redsim::world::World, id: u32| -> Option<usize> {
        let e = w.entities.list.iter().find(|e| e.id == id)?;
        // (the stop stand's three places by x, the batch at rest against the gate and each
        // other: S1 the front)
        // (the pocket stand's three places: a cart at rest on a pocket's rest slope, S1 the front)
        if o.stand_stop && (e.pos.z - 0.5 - fz).abs() < 0.6 && e.pos.y < 1.0 && e.pos.x - fx > -0.6 && e.pos.x - fx < 8.5 {
            if e.delta_movement.horizontal_distance_sqr() > 1e-6 {
                return None;
            }
            let s = match e.pos.x - fx {
                x if (7.5..8.0).contains(&x) => 0,
                x if (3.5..4.0).contains(&x) => 1,
                x if (-0.5..0.0).contains(&x) => 2,
                _ => return None,
            };
            return Some(q + 2 - s);
        }
        places.iter().enumerate().position(|(k, c)| {
            // (along and across the place's travel direction, from its slope cell's centre)
            let d = if k < q { lay.slot_dir[k] } else { [1, 0] };
            let (ox, oz) = (e.pos.x - (c[0] as f64 + 0.5), e.pos.z - (c[2] as f64 + 0.5));
            let along = ox * d[0] as f64 + oz * d[1] as f64;
            let across = (ox * d[1] as f64).abs() + (oz * d[0] as f64).abs();
            let tail = !lay.tail_b.is_empty() && k < 3;
            let deep = (o.tail_deep || o.tail_desc) && k < 3;
            let span = if (wide && k >= q) || tail { (-1.0, 2.5) } else if deep || ((o.stand_desc || o.stand_deep) && k >= q) { (-2.0, 1.0) } else { (-1.0, 1.0) };
            let e_y_ok = if deep { e.pos.y < 1.5 } else { true };
            along > span.0 && along < span.1 && across < 0.6 && e.pos.y < c[1] as f64 + 1.5 && e.pos.y > c[1] as f64 - 1.0 && e.delta_movement.horizontal_distance_sqr() < 1e-6
        })
    };
    let pos_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id).map(|e| e.pos);
    // the drive log (mobviewer's replay format): every control change as `act <t> set x y z
    // <state>`, t = gt after the entities' spawn, applied before the tick that brings the time
    // to spawn + t
    let t_spawn = run.world.time;
    // (the declared states, `crate::declared`: the design's rail cells, gates and boosters as its
    // options declare them, at tick 0 (a design built in order is checked once built, below) and
    // the boosters again at t 20; a mismatch is a problem: the run fails)
    let (declared, declared_twice) = merge_declared(declared_states(o), &hdr.declared);
    let mut declared_bad: Vec<String> = declared_twice;
    if build_start.is_none() {
        declared_bad.extend(crate::declared::check(&run.world, &declared, false));
    }
    // (the drive log's times count from the build's start when the bench builds the design, from
    // the spawn otherwise)
    let t_origin = build_start.unwrap_or(t_spawn);
    // (`wired`: the controls are the design's own redstone; the bench only pulses the golem input
    // cell each slot and checks where the carts go)
    let wired = tm.wired;
    let stand_cells_ctl: Vec<BlockPos> = lay.stand.clone();
    let set = |w: &mut redsim::world::World, ps: &[BlockPos], p: bool| {
        for c in ps {
            if wired.is_some() && !(tm.bench_stand && stand_cells_ctl.contains(c)) {
                continue;
            }
            w.set_block(*c, if p { on.clone() } else { off.clone() }, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", w.time - t_origin, c.x, c.y, c.z, if p { "redstone_block" } else { "glass" }));
        }
    };
    let input_was = wired.map(|p| run.world.get(p));
    // (`day_only`: the day time held in the day (back 10,000 once it reaches 11,000): no night,
    // so a file's night window (its siding visits) never opens; the loop and the stand alone)
    let hold_day = |w: &mut redsim::world::World| {
        if tm.day_only && w.day_time().rem_euclid(24000) >= 11000 {
            w.mob.day_offset -= 10000;
        }
    };
    // (each slot's golem pulse delay, `golem_jitter`: 0..=J gt, from the seed and the slot)
    let golem_jit = |c: i64| -> i64 {
        if tm.golem_jitter <= 0 {
            return 0;
        }
        let mut z = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (c as u64).wrapping_add(0x632B_E59B_D9B4_E019).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) % (tm.golem_jitter as u64 + 1)) as i64
    };
    // (the stop stand's fence gates: shut or open, facing the travel)
    let set_gate = |w: &mut redsim::world::World, p: BlockPos, facing: &str, open: bool| {
        let s = format!("minecraft:oak_fence_gate[facing={facing},in_wall=false,open={open},powered=false]");
        if let Ok(b) = Block::parse(&s)
            && w.get(p) != b
        {
            w.set_block(p, b, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {s}", w.time - t_spawn, p.x, p.y, p.z));
        }
    };
    // (the re-spacer's pockets: the carts each gate has seen leave it while it holds)
    let mut pocket_seen: Vec<Vec<u32>> = vec![Vec::new(); lay.pockets.len()];
    let mut pocket_hold = true;
    // (when each counting gate shuts: its count reached, plus `pocket_close_delay`)
    let mut pocket_close_at: Vec<Option<i64>> = vec![None; lay.pockets.len()];
    let mut pocket_released_at: Option<i64> = None;
    // (the wired stand: when the bench turns REL off again)
    let mut rel_off_at: Option<i64> = None;
    // (`late_hold`: the controller's own REL end (before `rel_min`'s extension), the last time a
    // cart entered the arrival cell, and which carts are in it)
    let mut raw_rel_off_at: Option<i64> = None;
    let mut late_edge_at: Vec<Option<i64>> = vec![None; 3];
    let mut late_edge_at_rise: Vec<Option<i64>> = vec![None; 3];
    let mut late_in: Vec<Vec<bool>> = vec![vec![false; 64]; 3];
    let mut late_rise_at: Option<i64> = None;
    let mut late_raw_was = false;
    let mut late_holds: Vec<i64> = vec![0; 3];
    let mut late_was_holding: Vec<bool> = vec![false; 3];
    let mut late_win_before: Vec<Option<i64>> = vec![None; 3];
    let mut late_win_after: Vec<Option<i64>> = vec![None; 3];
    let mut late_first_after: Vec<bool> = vec![false; 3];
    // (`rel_gate`: an act's raw input on the REL cell (on, until), the latch's set time, the raw
    // REL's rise, the gate armed for this raw pulse, REL_eff's end, each release (slot, dt, delay
    // after the raw rise, by the latch, the latch's age))
    let mut gate_act: Option<(bool, Option<i64>)> = None;
    let mut gate_q_at: Option<i64> = None;
    let mut gate_q_was = false;
    let mut gate_raw_rise: Option<i64> = None;
    let mut gate_raw_was = false;
    let mut gate_armed = false;
    let mut gate_eff_until: Option<i64> = None;
    let mut gate_log: Vec<(i64, i64, i64, bool, Option<i64>)> = Vec::new();
    let mut gate_rest_after_q: Vec<i64> = Vec::new();
    let mut gate_age_at_rise: Vec<i64> = Vec::new();
    // (`acts`' restores due: (gt, cell, the block the act replaced)); the bench's own REL state
    let mut act_restores: Vec<(i64, BlockPos, Block)> = Vec::new();
    let mut pending_loads: Vec<(i64, Option<[i32; 4]>)> = Vec::new();
    let mut rel_want = false;
    // (the lap order: carts in the order they leave the pocket stand, up its departure climb)
    let mut laps: Vec<usize> = Vec::new();
    let mut on_climb: Vec<bool> = vec![false; 64];
    // (the stand's state per slot and the queue reset, `verbose` / `reset_queue_at`)
    let mut stand_log: Vec<String> = Vec::new();
    let mut unload_log: Vec<String> = Vec::new();
    // (carts back through a gate; departures from the stand: (gt, cart, touching the one before))
    let mut strip_x: Vec<Option<f64>> = vec![None; 64];
    let mut back_through: Vec<String> = Vec::new();
    // (each back-crossing's cause: the stand-and-climb track coordinate s of every cart over its
    // last 80 gt (the strip's x, then 9.5 + the climb's z past the corner); a back-crossing looks
    // back for the cart's furthest point (its reversal), the rail it was on there, and the
    // nearest other cart then)
    let mut s_hist: Vec<std::collections::VecDeque<(i64, f64, String, Option<(usize, f64)>)>> = vec![std::collections::VecDeque::new(); 64];
    let mut back_causes: Vec<String> = Vec::new();
    let mut back_kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut departs: Vec<(i64, usize, bool)> = Vec::new();
    let mut top_v: Vec<Option<f64>> = Vec::new();
    // (the tail's entry: each cart reaching the return run's end, x 1 westward along z 20 at the
    // run's level, with its gap to the one before, contact and speed: what the tail sees)
    let mut tail_in: Vec<(i64, bool, f64)> = Vec::new();
    let mut tail_in_cart: Vec<usize> = Vec::new();
    // (every cart's state each tick: off the rails (no rail at its cell or the one under it) for
    // 3 ticks or more, its rider not seated in it; and the latch's sets after +55 (T2 going out:
    // D1's set, or C's), each a slot offset)
    let mut rail_watch = crate::flight::RailWatch::new(merge_flights(lay.flights.clone(), &hdr.flights));
    let mut unseated: Vec<bool> = vec![false; 21];
    let mut unseat_log: Vec<String> = Vec::new();
    let mut latch_sets: Vec<i64> = Vec::new();
    // (the departure corner (9,-3,0), day-1's divert T: each cart's centre entering and leaving
    // its cell, and G0's openings (REL's onset at the stand), for the divert's contract)
    let mut corner_in: Vec<Option<i64>> = vec![None; 21];
    let mut corner_pass: Vec<(i64, i64)> = Vec::new();
    let mut g0_opens: Vec<i64> = Vec::new();
    let mut g0_was_open = false;
    let mut t2_was_lit: Option<bool> = None;
    let mut latch_all: Vec<(i64, i64, bool)> = Vec::new();
    let mut t3_was_lit = false;
    let mut t3_rises: Vec<(i64, i64)> = Vec::new();
    let mut ret_x: Vec<Option<f64>> = vec![None; 21];
    // (the slot-boundary order check: the last slot's queue, train and stand batch; clean slots)
    let mut order_prev: Option<(Vec<Option<u32>>, Vec<u32>, Vec<u32>)> = None;
    let mut order_dep2: Option<Vec<u32>> = None;
    let mut order_ok = 0;
    // (the pocket stand's measured margins, per slot: [cart 1 at D2 (x 2), cart 2 at D2, cart 2's
    // tail past G2 (x 1.29), cart 3 at G2 (x -0.2), G2 shut]; then (cart 1->2 spacing at D2, G2
    // shut after cart 2 cleared it, G2 shut before cart 3 reached it))
    let mut margin_rec: [Option<i64>; 6] = [None; 6];
    let mut margins: Vec<(i64, i64, i64, i64)> = Vec::new();
    let mut g2_was_open = false;
    // (the approach's arrival marks per slot: the train's cart k first at G2 (x -0.2), G1 (x 3.8),
    // G0 (x 7.8); the latch's set time (T2 going out after +55); C''s edges and F pulses)
    let mut arr_rec: [[Option<i64>; 3]; 3] = [[None; 3]; 3];
    let mut arr: Vec<[[Option<i64>; 3]; 3]> = Vec::new();
    let mut cp_was = false;
    let mut cp_edges: Vec<i64> = Vec::new();
    let mut cp_due: Vec<(i64, bool)> = Vec::new();
    let mut cp_saved: Option<Block> = None;
    let mut cp_ext: Vec<i64> = Vec::new();
    // (a nap visit's state: the carts off the loop, the visiting cart and its batch, what it did)
    let mut away: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut visit_lines_pre: Vec<String> = Vec::new();
    // (the wired stand's REL: the bench drives it only where the file leaves it glass, a bench
    // cell; a file whose controller drives it (stone, a repeater into it) keeps it)
    let rel_bench: Option<BlockPos> = lay.stand_rel.filter(|r| run.world.get(*r).to_string() == "minecraft:glass");
    if let (Some(r), None) = (lay.stand_rel, rel_bench) {
        visit_lines_pre.push(format!("the stand's REL cell {} {} {} is the file's own ({}): the bench leaves it", r.x, r.y, r.z, run.world.get(r)));
    }
    // (the stand's batch at the latest slot start: an observed visit's batch and place in it)
    let mut dep_now: Vec<u32> = Vec::new();
    // (carts back from a visit since the last slot's order check)
    let mut returned: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut visits: Vec<VisitRun> = Vec::new();
    // (the nightly schedule: visits started tonight, the next slot one may start, the nights)
    let mut tonight = 0usize;
    let mut next_visit = 0i64;
    let mut night_reports: Vec<String> = Vec::new();
    let mut slept_tonight: Vec<bool> = vec![false; riders.len()];
    let mut tod_was = -1i64;
    let mut home_at_start: Option<Vec<Option<BlockPos>>> = None;
    let mut visit_lines: Vec<String> = Vec::new();
    let mut home_was: Vec<Option<BlockPos>> = vec![None; riders.len()];
    let mut rider_gone: Vec<bool> = vec![false; riders.len()];
    let mut rider_last: Vec<([f64; 3], f32, i64)> = vec![([0.0; 3], 20.0, 0); riders.len()];
    let mut rider_hurt: Vec<String> = Vec::new();
    let mut golems_seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut villager_cells: std::collections::BTreeSet<BlockPos> = std::collections::BTreeSet::new();
    let mut golem_log: Vec<String> = Vec::new();
    // (with a pad, a Nether part in the design: every golem's summon (its id, tick, summoner,
    // cell, slot and offset), its crossing and death in the Nether, the pulse, the iron)
    let has_nether = run.world.nether.is_some();
    let mut ev_seen = run.world.mob.events.len();
    // (tickexact 2026-11-11, ironspec: the one-bed claim's second failure path) per rider, the
    // oldest CANT_REACH_WALK_TARGET_SINCE at a tick SetWalkTargetFromBlockMemory(HOME) can run
    // (REST, no WALK_TARGET, HOME held: past 1200 gt it releases HOME), and every POI release
    // (MobEvent::Released: a rider's is its HOME); `calm`: every zombie killed first (the control)
    let mut reach_worst: Option<(i64, String)> = None;
    let mut home_releases: Vec<String> = Vec::new();
    let mut rel_seen = run.world.mob.events.len();
    if tm.calm {
        let zombies: Vec<u32> = run.world.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Zombie)).map(|e| e.id).collect();
        for id in &zombies {
            redsim::entity::kill(&mut run.world, *id).map_err(|e| format!("--calm: {e}"))?;
        }
        rel_seen = run.world.mob.events.len();
        eprintln!("--calm: {} zombies killed at the start (a diagnostic, never a design check)", zombies.len());
    }
    let mut crossed_seen = run.world.crossings.len();
    let mut pad_golems: Vec<(u32, i64, u32, [i32; 3], i64, i64)> = Vec::new();
    let mut out_of_turn: Vec<String> = Vec::new();
    // (the power-on view: each golem pulse's rise, each release (G0 opening) and whether it came
    // 1..=20 gt after the pad's pulse; G0 the front pocket's gate, the one with the largest x)
    let mut pulse_rises: Vec<i64> = Vec::new();
    let mut releases: Vec<(i64, bool)> = Vec::new();
    let g0: Option<BlockPos> = lay.pockets.iter().map(|(g, _)| *g).max_by_key(|g| g.x);
    let mut g0_was = g0.is_some_and(|g| matches!(run.world.get(g), Block::FenceGate { open: true, .. }));
    // (per summon tick: how many stand riders were willing, panicking, at the stand (0..=3);
    // the unwilling ones: never slept, slept 24,000 gt ago or more, a golem seen lately)
    let (mut willing, mut panicking, mut stand_count) = ([0u32; 4], [0u32; 4], [0u32; 4]);
    let mut unwilling = [0u32; 3];
    let mut nether_golems: Vec<(u32, i64, Option<i64>, u32)> = Vec::new();
    let (mut pulses, mut spurious_pulses, mut pulse_was) = (0u32, 0u32, false);
    let mut slept_night: Option<i64> = None;
    let mut slept_acts = 0u32;
    let nether_iron = |w: &redsim::world::World| -> i64 {
        let Some(n) = w.nether.as_deref() else { return 0 };
        use redsim::redstone::container as rc;
        let (ingot, block) = (rc::item_id("iron_ingot"), rc::item_id("iron_block"));
        let mut iron = 0i64;
        for c in n.containers.map.values() {
            for s in c.slots.iter().flatten() {
                if Some(s.item) == ingot {
                    iron += s.count as i64;
                } else if Some(s.item) == block {
                    iron += 9 * s.count as i64;
                }
            }
        }
        iron
    };
    let iron_start = nether_iron(&run.world);
    let mut bed_claims: Vec<String> = Vec::new();
    // (with visits: T1's and T2's rails each time their shape changes: absolute time, which, shape)
    let mut t_switches: Vec<(i64, &'static str, String)> = Vec::new();
    let mut t_shapes: [String; 2] = [String::new(), String::new()];
    // (the stop stand's brake: armed with a batch's third cart, set `stop_brake_delay` gt after
    // that cart is on the braked slopes; the design's first batch arms it from the start)
    let mut stop_arm: Option<(u32, Option<i64>)> = None;
    let stop_brake = |w: &mut redsim::world::World, arm: &mut Option<(u32, Option<i64>)>| {
        let Some((id, at)) = *arm else { return };
        let now = w.time as i64;
        let entered = at.or_else(|| w.entities.list.iter().find(|e| e.id == id).filter(|e| e.pos.x - fx >= 2.0 && e.pos.x - fx < 4.7 && (e.pos.z - 0.5 - fz).abs() < 0.6 && e.pos.y < 1.5).map(|_| now));
        *arm = Some((id, entered));
        if let Some(t_in) = entered
            && now >= t_in + tm.stop_brake_delay
        {
            for c in &lay.stand {
                w.set_block(*c, off.clone(), redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:glass", w.time - t_spawn, c.x, c.y, c.z));
            }
            *arm = None;
        }
    };
    let reset_was = tm.stand_reset.map(|p| run.world.get(p));
    let mut reset_off_at: Option<i64> = None;
    // (who is where at the start: the design's order)
    for s in 0..40 {
        run.tick();
        stop_brake(&mut run.world, &mut stop_arm);
        hold_day(&mut run.world);
        if let Some(w) = watch {
            let ps: Vec<String> = w.iter().filter_map(|k| carts.get(*k)).filter_map(|id| pos_of(&run.world, *id)).map(|p| format!("{:.3},{:.3}", p.x, p.y)).collect();
            println!("settle {s}: {}", ps.join(" | "));
        }
    }
    // (`summon_at`: the slots' phase: the bench waits until slot +K falls on the villagers' summon
    // tick (gameTime % 100 == 0): with a pad, the stand batch at rest then; 99 for a controller
    // that takes the golem pulse (summon +1) as its slot's start)
    if let Some(k) = tm.summon_at {
        // (a tick's summon happens at the time it ticks to: the slot's offset k + 1 on it)
        while (run.world.time + k + 1).rem_euclid(100) != 0 {
            run.tick();
            stop_brake(&mut run.world, &mut stop_arm);
            hold_day(&mut run.world);
        }
    }
    let mut late_total: i64 = 0;
    let mut out = Vec::new();
    let mut problems = Vec::new();
    // (the bed's nearest awake villager while it is taken: its distance and where; see the tick
    // loop)
    let mut bed_near: Option<(f64, String)> = None;
    // (merged carts: the shared check, `crate::merge`, every tick; a hard fail)
    let mut merges = crate::merge::MergeTally::default();
    let release_group: Vec<BlockPos> = lay.slots[q - 3..].to_vec();
    // (slots from a departure to the slot whose advance must find that batch in the queue's
    // tail: the line loop's long return takes two)
    let lag: i64 = if o.line { 2 } else { 1 };
    // (the stand's slope-up controls lead its slopes down by this much at the departure)
    let lead: i64 = if lay.stand_b.is_empty() { 0 } else { 5 };
    let intake: Vec<BlockPos> = lay.slots[..3].to_vec();
    // (the re-packing tail: the FIFO's gates, not timed tail brakes; the carts it held at each
    // slot's start, and each intake that found its pocket empty)
    let fifo_on = !lay.fifo.is_empty() || lay.esc.is_some();
    let mut fifo_held: Vec<usize> = Vec::new();
    let mut fifo_empty: Vec<String> = Vec::new();
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
    let mut rise_at: Option<i64> = None;
    let det_cell: Option<BlockPos> = lay.path.iter().find(|(_, k)| *k == Cell::Detector).map(|(c, _)| BlockPos::new(c[0], c[1], c[2]));
    let mut each_at: [Option<i64>; 3] = [None; 3];
    let mut each_done = [false; 3];
    let mut intake_trains: Vec<(i64, Vec<u32>, u8)> = Vec::new();
    let mut arrivals: Vec<String> = Vec::new();
    let mut stand_braked_at: Vec<i64> = Vec::new();
    let mut intake_braked_at: Vec<(i64, i64)> = Vec::new();
    let mut tail_rest_at: Vec<i64> = Vec::new();
    for t in 0..cycles * 100 + 150 {
        let (c, dt) = (t / 100, t % 100);
        // (`acts`, the unload/reload act: a load due now, before this gt's other acts; a whole
        // part's entities come back under new numbers, every record of them renumbered)
        let due: Vec<Option<[i32; 4]>> = pending_loads.iter().filter(|(at, _)| *at == t).map(|(_, u)| *u).collect();
        pending_loads.retain(|(at, _)| *at != t);
        for b in due.into_iter().flatten() {
            redsim::reload::load_some(&mut run.world, b);
            drive.borrow_mut().push(format!("act {} load {},{},{},{}", run.world.time - t_origin, b[0], b[1], b[2], b[3]));
        }
        // (`acts`, fault injection: each block set at its slot offset, then the block it replaced
        // put back after its hold, before the bench's own controls this gt)
        for a in tm.acts.iter().flatten() {
            if a.slot == c && a.dt == dt {
                if let Some((k, [x, y, z])) = a.place {
                    // (a lab act: cart k and its rider set at a loop-frame point, at rest)
                    let p = redsim::entity::Vec3::new(x + fx, y, z + fz);
                    let k = if k == usize::MAX {
                        // (`head`: the cart nearest the queue's head slot)
                        let h = lay.slot_cells[lay.slot_cells.len() - 1];
                        let hp = redsim::entity::Vec3::new(h[0] as f64 + 0.5 + fx, h[1] as f64, h[2] as f64 + 0.5 + fz);
                        let d = |id: &u32| redsim::entity::index_of(&run.world, *id).map(|i| { let q = run.world.entities.list[i].pos; (q.x - hp.x).powi(2) + (q.y - hp.y).powi(2) + (q.z - hp.z).powi(2) }).unwrap_or(f64::MAX);
                        (0..carts.len()).min_by(|a, b| d(&carts[*a]).total_cmp(&d(&carts[*b]))).unwrap_or(0)
                    } else {
                        k
                    };
                    if let Some(i) = carts.get(k).and_then(|id| redsim::entity::index_of(&run.world, *id)) {
                        run.world.entities.set_pos(i, p);
                        run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                        for r in run.world.entities.list[i].passengers.clone() {
                            if let Some(j) = redsim::entity::index_of(&run.world, r) {
                                run.world.entities.set_pos(j, p);
                                run.world.entities.list[j].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                            }
                        }
                        drive.borrow_mut().push(format!("act {} tp e{k} {:.3} {:.3} {:.3}", run.world.time - t_origin, p.x, p.y, p.z));
                    }
                    continue;
                }
                if let Some(u) = a.unload {
                    let n = a.hold.unwrap_or(1).max(1);
                    match u {
                        None => {
                            // (the whole part away between two ticks: its redstone and carts
                            // frozen with it, so no bench control either; the level's time runs
                            // on; back after n ticks, its entities under new numbers, every record
                            // of them renumbered; then this gt goes on)
                            redsim::reload::unload_all(&mut run.world);
                            drive.borrow_mut().push(format!("act {} unload", run.world.time - t_origin));
                            for _ in 0..n {
                                run.tick();
                            }
                            redsim::reload::load_all_ordered(&mut run.world, a.order);
                            drive.borrow_mut().push(match a.order {
                                redsim::reload::Order::Save => format!("act {} load", run.world.time - t_origin),
                                o => format!("act {} load order={}", run.world.time - t_origin, o.name()),
                            });
                            let m = std::mem::take(&mut run.world.renumbered);
                            let r = |id: &mut u32| {
                                if let Some(n) = m.get(id) {
                                    *id = *n;
                                }
                            };
                            let rs = |s: &mut std::collections::HashSet<u32>| *s = s.iter().map(|id| m.get(id).copied().unwrap_or(*id)).collect();
                            carts.iter_mut().for_each(r);
                            riders.iter_mut().for_each(r);
                            parked.iter_mut().for_each(r);
                            pocket_seen.iter_mut().for_each(|v| v.iter_mut().for_each(r));
                            if let Some((a, b, d)) = order_prev.as_mut() {
                                a.iter_mut().flatten().for_each(r);
                                b.iter_mut().for_each(r);
                                d.iter_mut().for_each(r);
                            }
                            if let Some(d) = order_dep2.as_mut() {
                                d.iter_mut().for_each(r);
                            }
                            dep_now.iter_mut().for_each(r);
                            rs(&mut away);
                            rs(&mut returned);
                            rs(&mut golems_seen);
                            if let Some((_, v, _)) = stand_train.as_mut() {
                                v.iter_mut().for_each(r);
                            }
                            intake_trains.iter_mut().for_each(|(_, v, _)| v.iter_mut().for_each(r));
                            // (tickexact 2026-11-11) the visits' records too (each visit's cart,
                            // napper and batch: a visit spanning the reload read the old numbers,
                            // "the napper is not back in its own cart" / "the cart never came back
                            // to rest in a slot" whatever the world did), the stop brake's armed
                            // cart and the pad's golems
                            for vr in visits.iter_mut() {
                                if let Some(c) = vr.cart.as_mut() {
                                    r(c);
                                }
                                if let Some(x) = vr.rider.as_mut() {
                                    r(x);
                                }
                                vr.batch.iter_mut().for_each(r);
                            }
                            if let Some((id, _)) = stop_arm.as_mut() {
                                r(id);
                            }
                            for g in pad_golems.iter_mut() {
                                r(&mut g.0);
                                r(&mut g.2);
                            }
                            for g in nether_golems.iter_mut() {
                                r(&mut g.0);
                                r(&mut g.3);
                            }
                            unload_log.push(format!("slot {c} +{dt}: the whole part away {n} gt, {} entities back under new numbers", m.len()));
                        }
                        Some(b) => {
                            // (a bench world has no forced chunk box: the chunks its box meets)
                            if run.world.entity_chunks.is_none() {
                                let (mn, sz) = (run.world.min, run.world.size);
                                run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                            }
                            redsim::reload::unload_some(&mut run.world, b);
                            drive.borrow_mut().push(format!("act {} unload {},{},{},{}", run.world.time - t_origin, b[0], b[1], b[2], b[3]));
                            pending_loads.push((t + n, u));
                            unload_log.push(format!("slot {c} +{dt}: chunks {},{}..{},{} unforced {n} gt (the part's chunk box {:?})", b[0], b[1], b[2], b[3], run.world.entity_chunks));
                        }
                    }
                    continue;
                }
                let p = BlockPos::new(a.cell[0], a.cell[1], a.cell[2]);
                // (`rel_gate`: an act on the REL cell is a raw input to the gate (a redstone block an
                // extra controller pulse, anything else the controller's pulse held off), never
                // written to the stand's cell)
                if tm.rel_gate.is_some() && Some(p) == rel_bench {
                    gate_act = Some((a.block.to_string().contains("redstone_block"), a.hold.map(|h| t + h.max(1))));
                    continue;
                }
                let was = run.world.get(p);
                run.world.set_block(p, a.block, redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, p.x, p.y, p.z, a.block));
                if let Some(h) = a.hold {
                    act_restores.push((t + h.max(1), p, was));
                }
            }
        }
        act_restores.retain(|(at, p, was)| {
            if *at != t {
                return true;
            }
            // (the bench's REL cell back to what the bench's own schedule wants now: an extra
            // pulse is ORed with the bench's, a held-off one misses it)
            let back = if Some(*p) == rel_bench { if rel_want { on } else { off } } else { *was };
            run.world.set_block(*p, back, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, p.x, p.y, p.z, back));
            false
        });
        let rel_held = |p: BlockPos| act_restores.iter().any(|(_, q, _)| *q == p);
        // (`reset_queue_at`, a diagnostic hand reset of everything but the stand: every cart not at
        // rest in a stand pocket put at rest in the queue's slots as the design places them, in
        // their loop order, the release group first; the stand's carts and every block left as
        // they are; the bench's tracking restarted as at slot 0. A clean run after it means the
        // stand was back by itself at that slot; what needed the reset was the rest of the loop)
        if o.stand_stop && c < cycles && dt == 0 && tm.reset_queue_at.is_some_and(|r| c == r || (tm.reset_queue_every && c > r)) {
            let n = lay.path.len();
            let stand_lo = lay.path.iter().position(|(p, _)| p[2] == 0 && p[0] == lay.stand_cells[2][0] - 1).unwrap_or(n);
            let stand_hi = lay.path.iter().position(|(p, _)| p[2] == 0 && p[0] == lay.stand_cells[0][0] + 1).unwrap_or(n);
            let index_of = |p: redsim::entity::Vec3| (0..n).min_by(|a, b| {
                let d = |i: usize| { let c = lay.path[i].0; (p.x - c[0] as f64 - 0.5).powi(2) + (p.y - c[1] as f64 - 0.5).powi(2) + (p.z - c[2] as f64 - 0.5).powi(2) };
                d(*a).partial_cmp(&d(*b)).unwrap_or(std::cmp::Ordering::Equal)
            }).unwrap_or(0);
            // (the stand's region: its strip at z 0 and the release run below the queue's head; a
            // cart there not at rest in a pocket is a straggler, the stand's own state: left where
            // it is unless `reset_stragglers`)
            let xr = -o.row_gap as f64 + 0.5 + fx;
            let head_z = lay.slot_cells[q - 1][2] as f64;
            let ytop = if o.approach > 0 { 2.7 } else { 1.5 };
            let in_region = |p: redsim::entity::Vec3| ((p.z - 0.5 - fz).abs() < 0.6 && p.y < ytop && p.x > xr - 0.6 && p.x < 9.0 + fx) || ((p.x - xr).abs() < 0.6 && p.z < head_z - 0.5 && p.y > 0.5);
            // (a cart in a chunk whose entities don't tick (a partial unload's level 32 or 33) is
            // kept where it is: frozen, it is no tail's to move; no cart is put on a spot a kept cart
            // holds)
            let frozen = |id: u32| !run.world.unforced.is_empty() && pos_of(&run.world, id).is_some_and(|p| run.world.chunk_level(p.x.floor() as i32 >> 4, p.z.floor() as i32 >> 4) > 31);
            let keep = |id: u32| frozen(id) || (!tm.reset_stand_carts && (place_of(&run.world, id).is_some_and(|k| k >= q) || (!tm.reset_stragglers && pos_of(&run.world, id).is_some_and(in_region))));
            // (front first: carts before the stand (the queue, the release run) by their place along
            // the loop, then those past it (the departure, the return), likewise)
            let mut others: Vec<(usize, u32)> = carts.iter().copied().filter(|id| !keep(*id)).filter_map(|id| pos_of(&run.world, id).map(|p| (index_of(p), id))).collect();
            others.sort_by_key(|(i, _)| std::cmp::Reverse(if *i <= stand_lo { *i + n } else if *i >= stand_hi { *i } else { *i + n }));
            // (`reset_stand_carts`: the stand's three pockets filled first, front first, at the
            // design's rest positions, then the queue: a hand reset of every cart, no block touched)
            let mut spots: Vec<redsim::entity::Vec3> = Vec::new();
            if tm.reset_stand_carts {
                let xb = -o.row_gap as f64 + fx;
                spots.extend([(10.885, -1.8225), (6.885, -0.8225), (2.885, 0.1775)].map(|(dx, y)| redsim::entity::Vec3::new(xb + dx, y, 0.5 + fz)));
            }
            for s in (0..q).rev() {
                let (cell, d, off) = (lay.slot_cells[s], lay.slot_dir[s], 0.32);
                spots.push(redsim::entity::Vec3::new(cell[0] as f64 + 0.5 + d[0] as f64 * (off - 0.5), cell[1] as f64 + 1.0 - off + 0.05, cell[2] as f64 + 0.5 + d[1] as f64 * (off - 0.5)));
            }
            let held: Vec<redsim::entity::Vec3> = carts.iter().copied().filter(|id| keep(*id)).filter_map(|id| pos_of(&run.world, id)).collect();
            spots.retain(|s| held.iter().all(|h| (h.x - s.x).abs() > 0.6 || (h.z - s.z).abs() > 0.6 || (h.y - s.y).abs() > 0.7));
            let placed = others.len().min(spots.len());
            for ((_, id), p) in others.iter().zip(spots.iter()) {
                let p = *p;
                if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                    run.world.entities.set_pos(i, p);
                    run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                    // (its rider too: left behind for a tick, a rider overlaps whatever cart now
                    // sits where it was and pushes it)
                    for r in run.world.entities.list[i].passengers.clone() {
                        if let Some(j) = redsim::entity::index_of(&run.world, r) {
                            run.world.entities.set_pos(j, p);
                            run.world.entities.list[j].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                        }
                    }
                }
                drive.borrow_mut().push(format!("act {} tp e{} {:.3} {:.3} {:.3}", run.world.time - t_origin, carts.iter().position(|x| x == id).unwrap_or(0), p.x, p.y, p.z));
            }
            // (every queue slot braked, as the queue holds its carts at rest: the tail's slopes
            // were open for the returning batch the reset put in it)
            set(&mut run.world, &lay.slots, false);
            stand_log.push(format!("slot {c}: {} reset by hand{}: {placed} carts placed, {} left in the stand's region, {} left where they were", if tm.reset_stand_carts { "every cart (the stand's pockets, then the queue)" } else { "queue" }, if tm.reset_stragglers && !tm.reset_stand_carts { " (the stand's stragglers too)" } else { "" }, carts.len() - others.len(), others.len() - placed));
            order_prev = None;
            order_dep2 = None;
            intake_trains.clear();
            laps.clear();
            on_climb.iter_mut().for_each(|b| *b = false);
        }
        // (`verbose`, the pocket stand's own state at each slot's start: carts at rest in each
        // pocket, any other cart in its strip (the release run's last cells to the last gate), and
        // the latch (its torch T2 unlit: set))
        if verbose && o.stand_stop && c < cycles && dt == 0 {
            let mut pk = [0usize; 3];
            let mut loose: Vec<String> = Vec::new();
            for id in &carts {
                let Some(p) = pos_of(&run.world, *id) else { continue };
                if (p.z - 0.5 - fz).abs() >= 0.6 || p.y >= (if o.approach > 0 { 2.7 } else { 1.5 }) || p.x - fx < -2.6 || p.x - fx >= 9.0 {
                    continue;
                }
                match place_of(&run.world, *id) {
                    Some(k) if k >= q => pk[q + 2 - k] += 1,
                    _ => loose.push(format!("{:.2}", p.x - fx)),
                }
            }
            let latch = if o.stand_wired { let t2 = BlockPos::new(-o.row_gap + 9 + o.shift[0], 0, 2 + o.shift[1]); if run.world.get(t2).to_string().contains("lit=false") { ", latch set" } else { ", latch reset" } } else { "" };
            stand_log.push(format!("slot {c} start: pockets S1 {} S2 {} S3 {}, {} more in the stand's strip{}{latch}", pk[0], pk[1], pk[2], loose.len(), if loose.is_empty() { String::new() } else { format!(" (x {})", loose.join(" ")) }));
        }
        // (`wired`: the daylight detectors placed now, at slot 0 +0, with the first golem pulse)
        if t == 0 && dets_at_slot0 && !dets.is_empty() {
            run.world.place_raw(&dets);
            if !tm.placed_raw {
                run.world.place_updates(&dets);
            }
        }
        if c < cycles && dt == 0 {
            // (who sits in a slot's slope cell, moving or not)
            let in_slot = |id: u32, cell: [i32; 3]| pos_of(&run.world, id).is_some_and(|p| p.x.floor() as i32 == cell[0] && p.z.floor() as i32 == cell[2] && p.y < 1.5);
            let by_cell = |cell: [i32; 3]| carts.iter().copied().find(|id| in_slot(*id, cell));
            let rel: Vec<u32> = (0..3).filter_map(|k| by_cell(lay.slot_cells[q - 1 - k])).collect();
            let dep: Vec<u32> = if o.stand_stop {
                // (the pocket stand: the carts at rest in its 3 pockets, front first)
                let mut v: Vec<(f64, u32)> = carts.iter().filter_map(|id| {
                    let k = place_of(&run.world, *id)?;
                    (k >= q).then(|| (pos_of(&run.world, *id).map_or(0.0, |p| p.x), *id))
                }).collect();
                v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                v.into_iter().map(|(_, id)| id).collect()
            } else {
                (0..3).filter_map(|k| { let c = lay.stand_cells[k]; let span = if lay.stand_b.is_empty() { 1.0 } else { 3.0 }; let back = if o.stand_desc || o.stand_deep { 1.0 } else { 0.0 }; carts.iter().copied().find(|id| pos_of(&run.world, *id).is_some_and(|p| p.x >= c[0] as f64 - back && p.x < c[0] as f64 + span && (p.z - (c[2] as f64 + 0.5)).abs() < 0.6 && p.y < 1.5)) }).collect()
            };
            if rel.len() != 3 || dep.len() != 3 {
                problems.push(format!("slot {c}: the release group holds {} carts and the stand {} at the slot's start", rel.len(), dep.len()));
            }
            // (a nap visit starts: its cart is the stand's cart k now; a single visit at its slot,
            // or the nightly schedule's next)
            let visit_sets: Vec<(BlockPos, Block)> = Vec::new();
            if tm.visit.is_some() {
                dep_now = dep.clone();
            }
            if let Some(v) = tm.visit.filter(|v| !v.observe) {
                let tod = run.world.day_time().rem_euclid(24000);
                let start = if v.every == 0 {
                    (c == v.slot).then_some(v.k)
                } else if tod >= v.from && tod < 23000 && tonight < riders.len() && c >= next_visit {
                    Some(if v.k_mod3 { tonight % 3 } else { tonight / 7 })
                } else {
                    None
                };
                if let Some(k) = start {
                    let cart = dep.get(k).copied();
                    let idx = cart.and_then(|id| carts.iter().position(|x| *x == id));
                    visits.push(VisitRun { slot: c, k, cart, rider: idx.map(|i| riders[i]), batch: dep.clone(), ..Default::default() });
                    if v.every > 0 {
                        tonight += 1;
                        next_visit = c + v.every;
                    }
                    visit_lines.push(format!("visit: slot {c} (day time {tod}), the stand's cart {k} (e{}) to the siding, its drop slot {}", idx.map_or(-1, |i| i as i64), q - 4 - k));
                }
            }
            // the order at each slot's start, cart by cart (no moment has all 21 at rest with a
            // pocket stand): every queue cart 3 slots on from the last slot's start, the tail's
            // 3 the batch that left the stand (its front in slot 2), the stand's 3 the train that
            // left the queue's head (front first)
            if o.stand_stop {
                let queue_now: Vec<Option<u32>> = (0..q).map(|s| by_cell(lay.slot_cells[s])).collect();
                if fifo_on && tm.verbose_tail {
                    arrivals.push(format!("slot {c} start: queue slots 0..6 {:?}", &queue_now[..7.min(q)]));
                }
                if let Some((pq, prel, _)) = &order_prev {
                    // (the tail's 3 at the last slot's start, or, when it was empty there, the
                    // batch that reached it during the last slot: the stand's batch of the slot
                    // before, its front in tail slot 2)
                    let mut bad = Vec::new();
                    for s in 0..q - 3 {
                        let want = pq[s].or_else(|| if s < 3 { order_dep2.as_ref().and_then(|d: &Vec<u32>| d.get(2 - s).copied()) } else { None });
                        // (slot 1: the design's own tail carts, raised out of `by_cell`'s reach, and
                        // no stand batch two slots back to stand in for them)
                        if want.is_none() && s < 3 && order_dep2.is_none() {
                            continue;
                        }
                        // (a cart away on a nap visit leaves a hole that rides the queue; back from
                        // it, it fills the hole)
                        if want.is_some_and(|w| away.contains(&w)) && queue_now[s + 3].is_none() {
                            continue;
                        }
                        if want.is_none() && queue_now[s + 3].is_some_and(|id| returned.contains(&id) || away.contains(&id)) {
                            continue;
                        }
                        if queue_now[s + 3] != want {
                            bad.push(format!("queue slot {} holds {:?}, not {:?}", s + 3, queue_now[s + 3], want));
                        }
                    }
                    if &dep != prel {
                        bad.push(format!("the stand holds {dep:?}, not the train {prel:?}"));
                    }
                    if !bad.is_empty() {
                        problems.push(format!("slot {c}: order: {}", bad.join("; ")));
                    } else {
                        order_ok += 1;
                    }
                }
                order_dep2 = order_prev.as_ref().map(|(_, _, d)| d.clone());
                order_prev = Some((queue_now, rel.clone(), dep.clone()));
                returned.clear();
            }
            stand_train = Some((c, rel, if wired.is_some() && tm.stand_reset.is_none() && !tm.bench_stand { 2 } else { 0 }));
            // (the pocket stand's margins of the slot before, then a fresh record for this one)
            if let [Some(a), Some(b), Some(fr), Some(cl), Some(ar), Some(g)] = margin_rec {
                margins.push((b - a, g - fr, g - cl, ar - g));
            }
            margin_rec = [None; 6];
            if c > 0 {
                arr.push(arr_rec);
            }
            arr_rec = [[None; 3]; 3];
            detect_at = None;
            rise_at = None;
            each_at = [None; 3];
            each_done = [false; 3];
            intake_trains.push((c, dep, 0));
            for (cell, b) in visit_sets {
                run.world.set_block(cell, b.clone(), redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, cell.x, cell.y, cell.z, b));
            }
            if let Some(p) = wired
                && golem_jit(c) == 0
                && !tm.golem_miss.contains(&c)
                && !tm.no_inject
            {
                // the golem pulse into the design's input cell (none at a `--golem-miss` slot: the
                // design's own expiry must release it; none with `no_inject`: the design's pad
                // gives it)
                run.world.set_block(p, on.clone(), redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_origin, p.x, p.y, p.z));
            }
        }
        // (`golem_jitter`: the slot's golem pulse `golem_jit(c)` gt late, 0..=J from the seed:
        // the gaps between pulses vary over 100 - J ..= 100 + J)
        if let (Some(p), true) = (wired, c < cycles && dt > 0 && dt == golem_jit(c) && !tm.golem_miss.contains(&c) && !tm.no_inject) {
            run.world.set_block(p, on.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_origin, p.x, p.y, p.z));
        }
        if let (Some(p), Some(was), true) = (wired, &input_was, c < cycles && dt == golem_jit(c) + tm.golem_for && !tm.golem_miss.contains(&c) && !tm.no_inject) {
            run.world.set_block(p, was.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, p.x, p.y, p.z, was));
        }
        if c < cycles && dt == tm.release_start {
            set(&mut run.world, if tm.release_stagger > 0 { &release_group[2..] } else { &release_group[..] }, true);
            // (`entry_speed`: a speed kick along each release cart's slope, the tolerance probe)
            if tm.entry_speed != 0.0 {
                if let Some((_, tr, _)) = &stand_train {
                    for (k, id) in tr.iter().enumerate() {
                        let d = lay.slot_dir[q - 1 - k];
                        if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                            let m = &mut run.world.entities.list[i].delta_movement;
                            *m = redsim::entity::Vec3::new(m.x + d[0] as f64 * tm.entry_speed, m.y, m.z + d[1] as f64 * tm.entry_speed);
                        }
                    }
                }
            }
        }
        if let (Some(r), Some(was), true) = (tm.stand_reset, &reset_was, reset_off_at == Some(t)) {
            run.world.set_block(r, was.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, r.x, r.y, r.z, was));
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
                drive.borrow_mut().push(format!("act {} tp e{} {:.3} {:.3} {:.3}", run.world.time - t_origin, carts.iter().position(|c| c == id).unwrap_or(0), p.x, p.y, p.z));
            }
        }
        // (`side_drop`, day-1's merge (b) for the bed siding: a cart that left the queue comes back
        // by rolling off the end of a side rail beside a slot's braked slope, onto it. At cycle C,
        // +4 (the queue at rest), the cart in slot `park` leaves the loop (its hole then moves one
        // slot per advance); at +D it comes back: put on the side rail moving toward `slot` at V
        // b/t, or (`staged`) put on the upper side rail at V 20 gt earlier to roll onto the braked
        // staging slope, which is powered for 10 gt at +D)
        if let Some(sd) = tm.side_drop {
            let (cy, slot, d, v, park) = (sd.cycle, sd.slot, sd.at, sd.speed, sd.park);
            if c == cy && dt == 4 && side_cart.is_none() {
                side_cart = carts.iter().copied().find(|id| place_of(&run.world, *id) == Some(park));
                match side_cart {
                    Some(id) => {
                        // (off the loop until it rests in a slot again: the order check's hole)
                        away.insert(id);
                        let stone = Block::parse("minecraft:stone").map_err(|e| e.to_string())?;
                        run.world.set_block(BlockPos::new(70, -1, 70), stone, redsim::world::flags::ALL);
                        if let Some(i) = redsim::entity::index_of(&run.world, id) {
                            run.world.entities.set_pos(i, redsim::entity::Vec3::new(70.5, 0.0, 70.5));
                            run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                        }
                    }
                    None => problems.push(format!("side drop: no cart at rest in slot {park} at slot {c}'s +4")),
                }
            }
            let cell = lay.slot_cells[slot];
            let put_at = if sd.staged { (d - 20).max(5) } else { d };
            // (times past +99 fall in the next slot: +100 is the next release's start)
            let now = c * 100 + dt;
            if let (Some(id), true) = (side_cart, now == cy * 100 + put_at) {
                // (the side rail runs toward the slot along +x from the west; the cart starts on
                // its middle cell, or on the upper rail's west cell when staged)
                let p = if sd.staged { redsim::entity::Vec3::new(cell[0] as f64 - 2.5, cell[1] as f64 + 2.0625, cell[2] as f64 + 0.5) } else { redsim::entity::Vec3::new(cell[0] as f64 - 1.5, cell[1] as f64 + 1.0625, cell[2] as f64 + 0.5) };
                if let Some(i) = redsim::entity::index_of(&run.world, id) {
                    run.world.entities.set_pos(i, p);
                    run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(v, 0.0, 0.0);
                }
            }
            if sd.staged && (c * 100 + dt == cy * 100 + d || c * 100 + dt == cy * 100 + d + 10) {
                // (the kick: a redstone block for 10 gt under the upper rail's east cell, beside
                // the staging slope: two blocks from the slot's own powered rail, which a block
                // under the staging slope would power too)
                let support = BlockPos::new(cell[0] - 2, cell[1] + 1, cell[2]);
                run.world.set_block(support, if c * 100 + dt == cy * 100 + d { on } else { off }, redsim::world::flags::ALL);
            }
            if let (Some(id), true) = (side_cart, now > cy * 100 + put_at && c <= cy + 2) {
                if side_rest.is_none() {
                    if let Some(s) = place_of(&run.world, id) {
                        let along = pos_of(&run.world, id).map(|p| p.z - (places[s][2] as f64)).unwrap_or(0.0);
                        side_rest = Some((dt + 100 * (c - cy), along, s));
                        away.remove(&id);
                        returned.insert(id);
                    }
                } else if let Some(s) = place_of(&run.world, id) {
                    // (then each slot it moves on to, first at rest there: the next advances)
                    if side_next.last().map_or(side_rest.map(|r| r.2), |l| Some(l.0)) != Some(s) {
                        side_next.push((s, dt + 100 * (c - cy)));
                    }
                }
                if c == cy + 2 && dt == 99 {
                    match side_rest {
                        Some((at, along, s)) => arrivals.push(format!("side drop: the cart from slot {park}, back by the side rail at slot {slot} at +{d} ({} {v} b/t), first at rest in slot {s} at +{at} (z offset {along:.2} in the cell); then at rest in {}", if sd.staged { "staged, arriving at" } else { "at" }, side_next.iter().map(|(s, t)| format!("slot {s} at +{t}")).collect::<Vec<_>>().join(", "))),
                        None => problems.push(format!("side drop: the cart from slot {park} (back at slot {slot} at +{d}) never at rest in a queue or stand slot by slot {}'s end", cy + 2)),
                    }
                }
            }
        }
        // (each nap visit's timed controls: the divert at the departure corner while its cart
        // crosses (a T: off as a target block, set one tick before its window: a T reshapes only
        // on an update from a signal source, and a block placed over glass updates its neighbours
        // as glass); T1/T2 set for its drop slot the tick before its siding release, the same way;
        // the siding's release, glass and a redstone block; the cart is off the loop from the
        // divert on)
        if let Some(v) = tm.visit.filter(|v| v.observe) {
            // (observed: the file drives its own controls; a cart on the divert's branch rail is
            // on a visit, the stand's cart k of this slot's batch)
            for (j, id) in carts.iter().enumerate() {
                if away.contains(id) || visits.iter().any(|vr| vr.cart == Some(*id) && vr.back.is_none()) {
                    continue;
                }
                let Some(p) = pos_of(&run.world, *id) else { continue };
                if p.x.floor() as i32 == v.branch.x && (p.y + 0.01).floor() as i32 == v.branch.y && p.z.floor() as i32 == v.branch.z {
                    let k = dep_now.iter().position(|x| x == id).unwrap_or(0);
                    visits.push(VisitRun { slot: c, k, cart: Some(*id), rider: Some(riders[j]), batch: dep_now.clone(), ..Default::default() });
                    away.insert(*id);
                    visit_lines.push(format!("visit (observed): slot {c} +{dt} (day time {}), the stand's cart {k} (e{j}) on the divert's branch, its drop slot {}", run.world.day_time().rem_euclid(24000), q - 4 - k));
                }
            }
        }
        if let Some(v) = tm.visit.filter(|v| !v.observe) {
            let now = c * 100 + dt;
            let mut sets: Vec<(BlockPos, Block)> = Vec::new();
            for vr in visits.iter() {
                let d0 = vr.slot * 100 + v.divert_at + 10 * vr.k as i64;
                let r0 = vr.slot * 100 + v.release_k[vr.k.min(2)].unwrap_or(v.release_at);
                if now == d0 - 1 {
                    sets.push((v.divert, target.clone()));
                }
                if now == d0 {
                    sets.push((v.divert, on.clone()));
                    if let Some(id) = vr.cart {
                        away.insert(id);
                    }
                }
                if now == d0 + 10 {
                    sets.push((v.divert, target.clone()));
                }
                // (T1/T2 for its drop line: with a hold on each line the cart passes them on its
                // way from the siding to its hold, so they are set as it is diverted (the visit
                // before it has passed them by then, 4 slots on); with one release past them
                // (--visit-t-at-release), the tick before that release)
                if now == if v.t_at_release { r0 - 1 } else { d0 + 1 } + v.t_at {
                    let drop = q - 4 - vr.k;
                    for (cell, powered) in [(v.t1, drop != q - 4), (v.t2, drop == q - 6)] {
                        sets.push((cell, target.clone()));
                        if powered {
                            sets.push((cell, on.clone()));
                        }
                    }
                }
                if now == r0 {
                    sets.push((v.release_cells[vr.k.min(2)].unwrap_or(v.release), on.clone()));
                }
                if now == r0 + 10 {
                    sets.push((v.release_cells[vr.k.min(2)].unwrap_or(v.release), off.clone()));
                }
            }
            for (cell, b) in sets {
                run.world.set_block(cell, b.clone(), redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, cell.x, cell.y, cell.z, b));
            }
        }
        if c < cycles && dt == tm.tail_check_at.unwrap_or(tm.advance_at) && !fifo_on {
            // the queue's tail must hold every earlier returning train before the advance
            for (c0, _, st) in &intake_trains {
                if *c0 + lag <= c && (*st < 2 || (o.plain && *st != 9) || (tm.tail_check_rest && *st == 2)) {
                    problems.push(format!("slot {c}: slot {c0}'s returning train not in the queue's tail at the advance"));
                }
            }
            intake_trains.retain(|(c0, _, st)| *c0 + lag > c || (*st < 2 || (o.plain && *st != 9) || (tm.tail_check_rest && *st == 2)));
        }
        if c < cycles {
            // the release group's window per slot (k: 0 = slot q-3, 2 = the head): on from the
            // release start (with `release_stagger`, the head first and the others that many gt
            // apart), off at `release_for` (with `stagger_off`, each as late as it went on). A
            // control is the OR of its lines: the release's off waits for an advance pulse in
            // progress, and the advance's off (below) for a release window still open
            let st_on = |k: usize| tm.release_start + if tm.release_stagger > 0 { (2 - k as i64) * tm.release_stagger } else { 0 };
            let st_off = |k: usize| tm.release_for + if tm.stagger_off && tm.release_stagger > 0 { (2 - k as i64) * tm.release_stagger } else { 0 };
            let adv_on = |s: usize, dt: i64| (0..3usize).any(|k| { let a = tm.advance_at + k as i64 * tm.advance_every + tm.adv_shift[k] + tm.skew.get(s).copied().unwrap_or(0); dt >= a && dt < a + tm.pulse + tm.adv_width[k] });
            for k in 0..3usize {
                if tm.release_stagger > 0 && k < 2 && dt == st_on(k) {
                    set(&mut run.world, &release_group[k..k + 1], true);
                }
                if dt == st_off(k) && !adv_on(q - 3 + k, dt) {
                    set(&mut run.world, &release_group[k..k + 1], false);
                }
            }
            // the stand's batch leaves one slot at a time, front first (S1 at the slot's start,
            // S2 and S3 `depart_every` gt apart; a cart behind crosses more powered cells and
            // would catch the one ahead), each slot's slope up `lead` gt before its slope down
            let shift = o.plain || tm.shift_depart;
            // the stop stand's departure: its gate open and S1's braked slope powered (the batch
            // rolls out bunched), then shut and braked for the next batch; the re-spacer's pockets
            // all opened at once (each cart its own pocket), then reset
            if o.stand_stop {
                let so = tm.stand_offset;
                if dt == so + tm.stop_open || dt == so + tm.stop_close {
                    let open = dt == so + tm.stop_open;
                    // (the gate opens first and the front rolls off by gravity; the braked slope
                    // is powered `stop_cv` gt later, once the front's rider no longer touches the
                    // second cart: powered while a rider's push has the second moving back, it
                    // boosts it back into the third)
                    if !open {
                        set(&mut run.world, &lay.stand, false);
                    }
                    if !open && tm.stop_brake_delay < 0 {
                        if let Some((_, tr, _)) = &stand_train
                            && tr.len() == 3
                        {
                            stop_arm = Some((tr[2], None));
                        }
                    }
                    if let Some(g) = lay.stop_gate {
                        set_gate(&mut run.world, g, "east", open);
                    }
                }
                if dt == so + tm.stop_cv {
                    set(&mut run.world, &lay.stand, true);
                }
                // (a release before the slot's start is the end of the slot before: the batch
                // caught there leaves for this one; slot 0's own batch leaves at its start)
                let rel = so + tm.pocket_release + if o.stand_wired { tm.rel_on } else { 0 };
                let fire = if rel >= 0 { dt == rel } else { (c + 1 < cycles && dt == 100 + rel) || (c == 0 && dt == 0) };
                if fire && o.stand_wired {
                    // the wired stand: REL on (the design's latch and gates do the rest), off
                    // `rel_for` gt later; a file whose REL cell is not glass drives it itself; an
                    // injected act holding the cell keeps it
                    if let Some(r) = rel_bench {
                        rel_want = true;
                        if !rel_held(r) && tm.rel_gate.is_none() {
                            run.world.set_block(r, on.clone(), redsim::world::flags::ALL);
                            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_origin, r.x, r.y, r.z));
                        }
                        // (`rel_on` moves the start alone: the end stays at the nominal start +
                        // `rel_for`)
                        rel_off_at = Some((t - tm.rel_on + tm.rel_for).max(t + 1));
                        raw_rel_off_at = rel_off_at;
                        // (`rel_min` L: the stand's REL held at least L gt from its rise, a pulse
                        // extender on REL; no rise (REL held off by an act), no extension; the
                        // gate's REL_eff has its own length)
                        if tm.rel_min > 0 && !rel_held(r) && tm.rel_gate.is_none() {
                            rel_off_at = rel_off_at.map(|x| x.max(t + tm.rel_min));
                        }
                    }
                    pocket_released_at = Some(t);
                } else if fire {
                    for (g, _) in &lay.pockets {
                        set_gate(&mut run.world, *g, "east", true);
                    }
                    pocket_hold = false;
                    pocket_released_at = Some(t);
                }
                // (the reset at its own time of the slot, not counted from the release: the two
                // controls' windows then stack, the release late and the reset late together)
                if !o.stand_wired && pocket_released_at.is_some() && dt == so + tm.pocket_reset {
                    for (k, (g, _)) in lay.pockets.iter().enumerate() {
                        set_gate(&mut run.world, *g, "east", k > 0);
                    }
                    pocket_seen = vec![Vec::new(); lay.pockets.len()];
                    pocket_close_at = vec![None; lay.pockets.len()];
                    pocket_hold = true;
                }
            }
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
                if shift || o.stand_stop {
                    break;
                }
                let at = tm.stand_offset + tm.stand_shift[k] + if k == 0 { tm.s1_at } else { k as i64 * tm.depart_every };
                if let (true, Some(b)) = (dt == at, lay.stand_b.get(k)) {
                    set(&mut run.world, &[*b], true);
                }
                if dt == at + lead {
                    set(&mut run.world, &[lay.stand[k]], true);
                }
            }
            for k in 0..3 {
                let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                let w = tm.pulse + tm.adv_width[k as usize];
                // (the tail's slopes up go on 12 gt before the first pulse and stay on through
                // the advance and the intake's pass-through: a cart that stopped on one rolls
                // back onto its braked slope down first, the leaving carts climb out boosted,
                // and an arriving queue cart, slow, stops on the slope down before it)
                if k == 0 && !lay.tail_b.is_empty() && dt == a - 12 {
                    set(&mut run.world, &lay.tail_b, true);
                }
                // (each slot's pulse, `skew` gt late for slot s: day-1's wiring reaches the head
                // first; a tail slot whose window has opened stays on through its pulse's end)
                for s in 0..q {
                    let sk = tm.skew.get(s).copied().unwrap_or(0);
                    if dt == a + sk {
                        set(&mut run.world, &lay.slots[s..s + 1], true);
                    }
                    let open_at = a + w + if s == 0 { tm.tail0_late } else { 0 };
                    let release_open = s + 3 >= q && { let k = s + 3 - q; dt >= st_on(k) && dt < st_off(k) };
                    if dt == a + w + sk && !(k == 2 && s < 3 && open_at <= a + w + sk && !fifo_on) && !release_open {
                        set(&mut run.world, &lay.slots[s..s + 1], false);
                    }
                }
                if dt == a + w && !fifo_on {
                    if k == 2 {
                        // the tail open for the returning train: slopes down powered too (slot 0
                        // `tail0_late` gt after slots 1-2)
                        set(&mut run.world, if tm.tail0_late > 0 { &intake[1..] } else { &intake[..] }, true);
                    }
                }
                if k == 2 && tm.tail0_late > 0 && dt == a + w + tm.tail0_late && !fifo_on {
                    set(&mut run.world, &intake[..1], true);
                }
            }
        }
        // the re-packing tail's gates (bench-driven): each pocket's gate open while the next
        // pocket's detector rail is off, the last pocket's (the intake) open `fifo_intake` after
        // each advance's start; an intake that finds its pocket empty leaves a hole in slot 0
        if fifo_on {
            let nf = lay.fifo.len();
            // (a pocket is occupied while a cart's centre is over its rest cell; slot 0 while one
            // is over its braked slopes C and A or the slope up after them)
            let occupied = |w: &redsim::world::World, r: [i32; 3]| carts.iter().any(|id| pos_of(w, *id).is_some_and(|p| p.x.floor() as i32 == r[0] && p.z.floor() as i32 == r[2] && p.y >= r[1] as f64 - 0.2 && p.y < r[1] as f64 + 1.2));
            let s0 = lay.slot_cells[0];
            let slot0_full = |w: &redsim::world::World| carts.iter().any(|id| pos_of(w, *id).is_some_and(|p| p.x.floor() as i32 == s0[0] && (s0[2] - 1..=s0[2] + 1).contains(&(p.z.floor() as i32)) && p.y < s0[1] as f64 + 2.2));
            let slot0_on = run.world.get(lay.slots[0]) == on;
            for j in 0..nf {
                // (the last pocket's gate, the intake: opened `fifo_intake` D gt after each
                // advance's start for W gt, its cart reaching slot 0 just after the pulse)
                let open = if j + 1 < nf {
                    !occupied(&run.world, lay.fifo[j + 1].1)
                } else {
                    let (d, w) = tm.fifo_intake;
                    let _ = (slot0_on, &slot0_full);
                    c < cycles && (0..3).any(|k| {
                        let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                        dt >= a + d && dt < a + d + w
                    })
                };
                set_gate(&mut run.world, lay.fifo[j].0, "south", open);
            }
            // (admission control, `esc_adm`: G0 opened `esc_adm` D gt after the gate opens for its W
            // gt, the third advance's after `adm_k2` gt when set; P always on from the first gate
            // opening with `p_always`, else powered with the gate)
            if let (true, Some(g0)) = (o.esc_adm, lay.esc_g0) {
                let (d, w) = tm.fifo_intake;
                let (dl, wl) = tm.esc_adm;
                let want = c < cycles && (0..3).any(|k| {
                    let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                    let dk = if k == 2 && tm.adm_k2 >= 0 { tm.adm_k2 } else { dl };
                    dt >= a + d + dk && dt < a + d + dk + wl
                });
                let s = format!("minecraft:oak_fence_gate[facing=south,in_wall=false,open={want},powered=false]");
                if let Ok(blk) = Block::parse(&s)
                    && run.world.get(g0) != blk
                {
                    run.world.set_block(g0, blk, redsim::world::flags::ALL);
                    drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, g0.x, g0.y, g0.z, s));
                }
                if let Some(pc) = lay.esc_p {
                    let gate_now = c < cycles && (0..3).any(|k| {
                        let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                        dt >= a + d && dt < a + d + w
                    });
                    let want_p = if tm.p_always { c > 0 || dt >= tm.advance_at + tm.adv_shift[0] + d } else { gate_now };
                    let b = if want_p { on.clone() } else { off.clone() };
                    if run.world.get(pc) != b {
                        run.world.set_block(pc, b, redsim::world::flags::ALL);
                        drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", run.world.time - t_origin, pc.x, pc.y, pc.z, if want_p { "redstone_block" } else { "glass" }));
                    }
                }
            }
            // (the escapement: B powered `esc_b` K gt after the gate opens (`fifo_intake` D gt after
            // each advance's start), for its W gt; the flat escapement's G_B open in that window)
            if lay.esc.is_some() || lay.esc_gb.is_some() {
                let (d, _) = tm.fifo_intake;
                let (kb, wb) = tm.esc_b;
                let want = c < cycles && (0..3).any(|k| {
                    let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                    dt >= a + d + kb && dt < a + d + kb + wb
                });
                if let Some(e) = lay.esc {
                    let b = if want { on.clone() } else { off.clone() };
                    if run.world.get(e) != b {
                        run.world.set_block(e, b, redsim::world::flags::ALL);
                        drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", run.world.time - t_origin, e.x, e.y, e.z, if want { "redstone_block" } else { "glass" }));
                    }
                }
                if let Some(gb) = lay.esc_gb {
                    let s = format!("minecraft:oak_fence_gate[facing=south,in_wall=false,open={want},powered=false]");
                    if let Ok(blk) = Block::parse(&s)
                        && run.world.get(gb) != blk
                    {
                        run.world.set_block(gb, blk, redsim::world::flags::ALL);
                        drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, gb.x, gb.y, gb.z, s));
                    }
                }
                // (the B door, `esc_door`: open with B's pulse)
                if let Some(dp) = lay.esc_door {
                    for (half, y) in [("lower", 0), ("upper", 1)] {
                        let s = format!("minecraft:iron_door[facing=south,half={half},hinge=left,open={want},powered=false]");
                        if let Ok(blk) = Block::parse(&s) {
                            let p = BlockPos::new(dp.x, dp.y + y, dp.z);
                            if run.world.get(p) != blk {
                                run.world.set_block(p, blk, redsim::world::flags::ALL);
                                drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, p.x, p.y, p.z, s));
                            }
                        }
                    }
                }
                // (P powered with the gate: the same window)
                if let Some(pc) = lay.esc_p {
                    let (d, w) = tm.fifo_intake;
                    let want = c < cycles && (0..3).any(|k| {
                        let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                        dt >= a + d && dt < a + d + w
                    });
                    let b = if want { on.clone() } else { off.clone() };
                    if run.world.get(pc) != b {
                        run.world.set_block(pc, b, redsim::world::flags::ALL);
                        drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", run.world.time - t_origin, pc.x, pc.y, pc.z, if want { "redstone_block" } else { "glass" }));
                    }
                }
            }
            if c < cycles && dt == 0 {
                let held = if lay.esc.is_some() || lay.esc_gb.is_some() {
                    carts.iter().filter(|id| pos_of(&run.world, **id).is_some_and(|p| lay.queue_cells.iter().any(|r| p.x.floor() as i32 == r[0] && p.z.floor() as i32 == r[2] && p.y >= r[1] as f64 - 0.2 && p.y < r[1] as f64 + 1.2))).count()
                } else {
                    lay.fifo.iter().filter(|(_, r)| occupied(&run.world, *r)).count()
                };
                fifo_held.push(held);
            }
            // (an advance that finds slot 0 empty: a hole into the register)
            if c < cycles && (0..3).any(|k| dt == tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize]) && !slot0_full(&run.world) {
                fifo_empty.push(format!("slot {c} +{dt}"));
            }
        }
        // (the wired stand's REL back to glass, whatever slot that falls in)
        if rel_off_at == Some(t) {
            if let Some(r) = rel_bench {
                rel_want = false;
                if !rel_held(r) && tm.rel_gate.is_none() {
                    run.world.set_block(r, off.clone(), redsim::world::flags::ALL);
                    drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:glass", run.world.time - t_origin, r.x, r.y, r.z));
                }
            }
            rel_off_at = None;
        }
        // (`ideal_stand`, a diagnostic: the batch put at rest at the stand's nominal offsets once it
        // has arrived, as a stand that always delivers would leave it: what the rest of the loop
        // tolerates on its own)
        if let (Some(at), true) = (tm.ideal_stand_at, c < cycles && dt == tm.ideal_stand_at.unwrap_or(-1)) {
            let _ = at;
            if let Some((_, tr, _)) = &stand_train {
                for (k, id) in tr.iter().enumerate() {
                    let cell = lay.stand_cells[k];
                    let p = redsim::entity::Vec3::new(cell[0] as f64 + 0.5 + (0.40 - 0.5), cell[1] as f64 + 1.0 - 0.40 + 0.05, cell[2] as f64 + 0.5);
                    if let Some(i) = redsim::entity::index_of(&run.world, *id) {
                        let e = &mut run.world.entities.list[i];
                        if (e.pos.x - p.x).abs() < 2.0 && (e.pos.z - p.z).abs() < 0.6 {
                            e.pos = p;
                            e.delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                        }
                    }
                }
            }
        }
        // (`no_summons`, a diagnostic: every rider remembers a golem, so none summons one)
        if tm.no_summons {
            for r in &riders {
                if let Some(i) = redsim::entity::index_of(&run.world, *r) {
                    if matches!(&run.world.entities.list[i].kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) {
                        redsim::entity::mob::villager::golem_detected(&mut run.world, i);
                    }
                }
            }
        }
        // (`rel_gate`, emulated: the settled-release gate. Raw REL: the bench's own pulse, or an act
        // on the REL cell (a redstone block ORed in, anything else holding it off). REL_eff: one
        // L-gt pulse per raw pulse, triggered when the latch (T2 out) has been set S gt or the raw
        // REL has been on T gt; the stand's REL cell carries REL_eff)
        if let (Some((gs, gt, gl)), Some(r)) = (tm.rel_gate, rel_bench) {
            if gate_act.is_some_and(|(_, until)| until.is_some_and(|u| t >= u)) {
                gate_act = None;
            }
            let raw = match gate_act {
                Some((true, _)) => true,
                Some((false, _)) => false,
                None => rel_want,
            };
            let q = !run.world.get(BlockPos::new(-o.row_gap + 9 + o.shift[0], 0, 2 + o.shift[1])).to_string().contains("lit=true");
            if q && !gate_q_was {
                // (a latch set as the run starts belongs to the batch the design places at rest:
                // long settled)
                gate_q_at = Some(if t < 40 { t - 10_000 } else { t });
            }
            gate_q_was = q;
            if raw && !gate_raw_was {
                gate_raw_rise = Some(t);
                gate_armed = true;
                if q && let Some(qa) = gate_q_at.filter(|qa| t - qa < 1_000) {
                    gate_age_at_rise.push(t - qa);
                }
            }
            if !raw {
                gate_armed = false;
            }
            gate_raw_was = raw;
            if gate_eff_until.is_some_and(|u| t >= u) {
                gate_eff_until = None;
            }
            if gate_armed && gate_eff_until.is_none() {
                let by_q = q && gate_q_at.is_some_and(|qa| t - qa >= gs);
                let by_t = gate_raw_rise.is_some_and(|rr| t - rr >= gt);
                if by_q || by_t {
                    gate_eff_until = Some(t + gl);
                    gate_armed = false;
                    gate_log.push((c, dt, t - gate_raw_rise.unwrap_or(t), by_q, if q { gate_q_at.map(|qa| t - qa) } else { None }));
                }
            }
            let want = gate_eff_until.is_some();
            if want != run.world.get(r).to_string().contains("redstone_block") {
                run.world.set_block(r, if want { on.clone() } else { off.clone() }, redsim::world::flags::ALL);
                drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, r.x, r.y, r.z, if want { "minecraft:redstone_block" } else { "minecraft:glass" }));
            }
        }
        // (`late_hold`, emulated: per held gate Gk, a cart entering its arrival cell within the
        // last N gt, within W gt of the controller's REL rising (the pocket carts still starting
        // from rest), holds Gk shut; never shut on a cart inside the gate's block)
        if tm.late_hold.iter().any(|h| h.is_some()) {
            // (the controller's REL: the REL cell a redstone block within the bench's own pulse
            // (before `rel_min`'s extension) or held so by an act)
            let block_on = rel_bench.is_some_and(|r| run.world.get(r).to_string().contains("redstone_block"));
            let raw_on = block_on && (raw_rel_off_at.is_some_and(|x| t < x) || rel_bench.is_some_and(|r| act_restores.iter().any(|(_, q, _)| *q == r)));
            if raw_on && !late_raw_was {
                late_rise_at = Some(t);
                late_edge_at_rise = late_edge_at.clone();
                // (the crossing windows: each held gate's cell, its latest crossing before this
                // rise and, below, its first crossing after it)
                for gk in 0..3 {
                    if let Some(e) = late_edge_at[gk] {
                        let d = t - e;
                        late_win_before[gk] = Some(late_win_before[gk].map_or(d, |m: i64| m.min(d)));
                    }
                    late_first_after[gk] = true;
                }
            }
            late_raw_was = raw_on;
            let ps: Vec<Option<redsim::entity::Vec3>> = carts.iter().map(|id| pos_of(&run.world, *id)).collect();
            for (cell, n, w, gk) in tm.late_hold.iter().flatten().copied() {
                let Some(g) = lay.pockets.get(gk).map(|p| p.0) else { continue };
                let (cx, cy, cz) = (cell[0] + o.shift[0], cell[1], cell[2] + o.shift[1]);
                for (k, p) in ps.iter().enumerate().take(64) {
                    let inside = p.is_some_and(|p| p.x.floor() as i32 == cx && p.y.floor() as i32 == cy && p.z.floor() as i32 == cz);
                    if inside && !late_in[gk][k] {
                        late_edge_at[gk] = Some(t);
                        if late_first_after[gk] && let Some(r) = late_rise_at {
                            let d = t - r;
                            late_win_after[gk] = Some(late_win_after[gk].map_or(d, |m: i64| m.min(d)));
                            late_first_after[gk] = false;
                        }
                    }
                    late_in[gk][k] = inside;
                }
                // (a cart inside the gate's closed shape: an east-facing gate shuts as a thin wall
                // at x 0.375..0.625 of its block, a cart 0.98 long)
                let in_gate = ps.iter().flatten().any(|p| (p.x - (g.x as f64 + 0.5)).abs() < 0.6 && (p.z - (g.z as f64 + 0.5)).abs() < 0.99 && p.y > g.y as f64 - 1.7 && p.y < g.y as f64 + 1.0);
                // (G2: an arrival before or after REL's rise (nothing but arrivals crosses its
                // cell); G1: only a cart that crossed its cell within N gt before the rise, still
                // rolling into pocket 2 then (after the rise, pocket 3's own departing cart crosses
                // it), held for W gt from the rise)
                let hold = if gk == 2 { late_rise_at.is_some_and(|r| t - r < w) && late_edge_at[gk].is_some_and(|e| t - e < n) } else { late_rise_at.is_some_and(|r| t - r < w && late_edge_at_rise[gk].is_some_and(|e| r - e < n)) };
                // (the gate as the hold wants it: shut while holding (its powered flag kept at
                // the signal, so the design's own updates leave it shut), never shut on a cart
                // inside its block; on the hold's end, back to what its signal says)
                let cur = run.world.get(g).to_string();
                let sig = run.world.has_neighbor_signal(g);
                let want_open = if hold { if cur.contains("open=true") && in_gate { Some(true) } else { Some(false) } } else if late_was_holding[gk] { Some(sig) } else { None };
                if let Some(open) = want_open {
                    let s = cur.replace("open=true", &format!("open={open}")).replace("open=false", &format!("open={open}")).replace("powered=true", &format!("powered={sig}")).replace("powered=false", &format!("powered={sig}"));
                    if s != cur
                        && let Ok(b) = Block::parse(&s)
                    {
                        run.world.set_block(g, b, redsim::world::flags::ALL);
                    }
                    if hold && !open {
                        late_holds[gk] += 1;
                    }
                }
                late_was_holding[gk] = hold;
            }
        }
        run.tick();
        merges.observe(&run.world, &format!("t {t}"));
        // (the boosters still powered after the warm-up)
        if t == 20 && build_start.is_none() {
            for b in crate::declared::check(&run.world, &declared, true) {
                if !declared_bad.contains(&b) {
                    declared_bad.push(format!("{b} (at t 20)"));
                }
            }
        }
        stop_brake(&mut run.world, &mut stop_arm);
        hold_day(&mut run.world);
        // (the lap order: each cart onto the pocket stand's departure climb, its first cell)
        if o.stand_stop {
            // (the approach's arrival marks: the slot's train, cart k at G2, G1, G0)
            if let Some((c0, tr, _)) = &stand_train
                && *c0 == c
            {
                for (k, id) in tr.iter().enumerate().take(3) {
                    if let Some(p) = pos_of(&run.world, *id).filter(|p| (p.z - 0.5 - fz).abs() < 0.6 && p.y < 1.5) {
                        for (g, gx) in [-0.2, 3.8, 7.8].iter().enumerate() {
                            if arr_rec[k][g].is_none() && p.x - fx >= *gx {
                                arr_rec[k][g] = Some(dt);
                            }
                        }
                    }
                }
            }
            // (C', emulated: the detector's rising edge, then F's stone a redstone block DELAY gt
            // later for FOR gt)
            if let Some((dc, delay, hold, lock)) = tm.c_prime {
                let det = BlockPos::new(dc[0] + o.shift[0], dc[1], dc[2] + o.shift[1]);
                let on_now = matches!(run.world.get(det), Block::DetectorRail { powered: true, .. });
                if on_now && !cp_was && cp_edges.last().is_none_or(|l| t - l >= lock) {
                    cp_edges.push(t);
                    cp_due.push((t + delay, true));
                    cp_due.push((t + delay + hold.max(1), false));
                    // (`rel_ext` E: REL held on to the edge + E gt when it is on at the edge, a
                    // train-synchronous end for a REL that would end sooner; never starts one)
                    if let (Some(e), Some(r)) = (tm.rel_ext, rel_bench)
                        && run.world.get(r) == on
                        && rel_off_at.is_some_and(|x| x < t + e)
                    {
                        rel_off_at = Some(t + e);
                        cp_ext.push(t + e);
                    }
                }
                cp_was = on_now;
                let fstone = BlockPos::new(-o.row_gap + 3 + 5 + o.shift[0], -2, 2 + o.shift[1]);
                for (_, put) in cp_due.iter().filter(|(at, _)| *at == t) {
                    if *put {
                        if cp_saved.is_none() {
                            cp_saved = Some(run.world.get(fstone));
                        }
                        run.world.set_block(fstone, on, redsim::world::flags::ALL);
                    } else if let Some(b) = cp_saved.take() {
                        run.world.set_block(fstone, b, redsim::world::flags::ALL);
                    }
                    drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, fstone.x, fstone.y, fstone.z, run.world.get(fstone)));
                }
                cp_due.retain(|(at, _)| *at > t);
            }
            // (backward through a gate: a cart on the stand's strip crossing a gate's wall westward;
            // also each cart off the rails, a rider not seated, and the latch's sets)
            for (k, id) in carts.iter().enumerate().take(21) {
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) else { continue };
                // (off the rails, with the design's declared flights: `crate::flight`)
                rail_watch.observe(&run.world, k, *id, Some(riders[k]), &format!("slot {c} +{dt}"));
                let seated = e.passengers.contains(&riders[k]);
                if !seated && !unseated[k] {
                    unseat_log.push(format!("slot {c} +{dt}: cart {k}'s rider not seated (cart at {:.2},{:.2},{:.2})", e.pos.x - fx, e.pos.y, e.pos.z - fz));
                }
                unseated[k] = !seated;
            }
            if o.stand_wired {
                let lit = run.world.get(BlockPos::new(-o.row_gap + 9 + o.shift[0], 0, 2 + o.shift[1])).to_string().contains("lit=true");
                if t2_was_lit == Some(true) && !lit && dt >= 55 && c < cycles {
                    latch_sets.push(dt);
                }
                // (every set, whenever it comes, and whether G0 (= REL) was open then: a set while
                // REL is on is overridden by it; one with REL off before +55 is out of turn)
                if t2_was_lit == Some(true) && !lit && c < cycles {
                    let rel_on = lay.pockets.first().is_some_and(|g| run.world.get(g.0).to_string().contains("open=true"));
                    latch_all.push((c, dt, rel_on));
                }
                t2_was_lit = Some(lit);
            }
            // (the guard's set path: T3 lit (C's timer or C''s pulse), each rise)
            if o.stand_guard {
                let t3 = BlockPos::new(-o.row_gap + 8 + o.shift[0], -3, 2 + o.shift[1]);
                let lit3 = run.world.get(t3).to_string().contains("lit=true");
                if lit3 && !t3_was_lit && c < cycles {
                    t3_rises.push((c, dt));
                }
                t3_was_lit = lit3;
            }
            if lay.pockets.len() == 3 {
                let g0_open = run.world.get(lay.pockets[0].0).to_string().contains("open=true");
                if g0_open && !g0_was_open {
                    g0_opens.push(t);
                }
                g0_was_open = g0_open;
                let (cx, cz) = (-o.row_gap + 12 + o.shift[0], o.shift[1]);
                for (k, id) in carts.iter().enumerate().take(21) {
                    let inside = pos_of(&run.world, *id).is_some_and(|p| p.x.floor() as i32 == cx && p.z.floor() as i32 == cz && p.y < -1.5);
                    match (inside, corner_in[k]) {
                        (true, None) => corner_in[k] = Some(t),
                        (false, Some(t0)) => {
                            corner_pass.push((t0, t));
                            corner_in[k] = None;
                        }
                        _ => {}
                    }
                }
            }
            // the departure: each cart past x 9 (the curve) eastward, its gap to the one before)
            let gx = [-o.row_gap as f64 + 3.5, -o.row_gap as f64 + 7.5, -o.row_gap as f64 + 11.5];
            {
                let xd = -o.row_gap as f64 + 12.5;
                let ps: Vec<Option<redsim::entity::Vec3>> = carts.iter().map(|id| pos_of(&run.world, *id)).collect();
                for (k, p) in ps.iter().enumerate().take(64) {
                    let s = p.and_then(|p| {
                        let (x, z) = (p.x - fx, p.z - fz);
                        if (z - 0.5).abs() < 0.6 && p.y < 1.5 && x > -o.row_gap as f64 - 1.0 && x < xd + 0.5 {
                            Some(x)
                        } else if (x - xd).abs() < 0.6 && z > 0.5 && z < 30.0 && p.y < 12.0 {
                            Some(xd + (z - 0.5))
                        } else {
                            None
                        }
                    });
                    let h = &mut s_hist[k];
                    match (s, p) {
                        (Some(s), Some(p)) => {
                            let rail = run.world.get(BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)).to_string();
                            let rail = rail.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string() + if rail.contains("powered=true") { "+" } else if rail.contains("powered=false") { "-" } else { "" };
                            let near = ps.iter().enumerate().filter(|(j, q)| *j != k && q.is_some()).map(|(j, q)| (j, q.unwrap())).map(|(j, q)| (j, ((q.x - p.x).powi(2) + (q.y - p.y).powi(2) + (q.z - p.z).powi(2)).sqrt())).min_by(|a, b| a.1.total_cmp(&b.1)).filter(|(_, d)| *d < 1.3);
                            h.push_back((t, s, rail, near));
                            if h.len() > 80 {
                                h.pop_front();
                            }
                        }
                        _ => h.clear(),
                    }
                }
            }
            for (k, id) in carts.iter().enumerate() {
                let p = pos_of(&run.world, *id);
                let now = p.filter(|p| (p.z - 0.5 - fz).abs() < 0.6 && p.y < 1.5).map(|p| p.x - fx);
                if let (Some(x0), Some(x1)) = (strip_x[k], now) {
                    for (g, gxx) in gx.iter().enumerate() {
                        if x0 >= *gxx && x1 < *gxx {
                            back_through.push(format!("slot {c} +{dt}: cart {k} back through G{}", 2 - g));
                            // (its cause: the furthest point over its history, the rail there, the
                            // nearest cart then; the kind: reversed on the climb or in the strip)
                            if let Some((ti, si, rail, near)) = s_hist[k].iter().max_by(|a, b| a.1.total_cmp(&b.1)).cloned() {
                                let xd = -o.row_gap as f64 + 12.5;
                                let kind = if si > xd + 0.6 { "climb" } else if si > *gxx + 1.0 { "strip" } else { "near the gate" };
                                *back_kinds.entry(kind.to_string()).or_default() += 1;
                                let at = if si > xd { format!("climb z {:.2}", si - xd + 0.5) } else { format!("strip x {si:.2}") };
                                back_causes.push(format!("cart {k} through G{} at slot {c} +{dt}: reversed {} gt before at {at} on {rail}{}", 2 - g, t - ti, near.map(|(j, d)| format!(", cart {j} {d:.2} away")).unwrap_or_default()));
                            }
                        }
                    }
                    let xd = -o.row_gap as f64 + 12.0;
                    if x0 < xd && x1 >= xd {
                        // (contact: the cart before it, still within 1.05 of it along the track)
                        let touching = departs.last().and_then(|(_, kp, _)| pos_of(&run.world, carts[*kp])).zip(p).is_some_and(|(a, b)| ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt() < 1.05);
                        departs.push((t, k, touching));
                        top_v.push(None);
                    }
                }
                strip_x[k] = now;
            }
            // (the tail's entry: the return run along z 20 at the run's level, crossing x 1 west)
            let ry = if o.tail_esc > 0 { esc_ye(o) as f64 } else if o.tail_fifo > 0 { 2.0 + o.tail_fifo as f64 } else if o.tail_desc { 4.0 } else { 1.0 };
            for (k, id) in carts.iter().enumerate() {
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) else { continue };
                let now = ((e.pos.z - fz - 20.5).abs() < 0.6 && (e.pos.y - ry).abs() < 0.9).then_some(e.pos.x - fx);
                if let (Some(x0), Some(x1)) = (ret_x[k.min(20)], now) {
                    if x0 >= 1.0 && x1 < 1.0 {
                        let touching = tail_in.last().is_some() && carts.iter().any(|o2| *o2 != *id && pos_of(&run.world, *o2).is_some_and(|p| (p.z - e.pos.z).abs() < 0.6 && (p.y - e.pos.y).abs() < 0.9 && (p.x - e.pos.x).abs() < 1.05 && p.x < e.pos.x));
                        tail_in.push((t, touching, e.delta_movement.horizontal_distance()));
                        tail_in_cart.push(k);
                    }
                }
                ret_x[k.min(20)] = now;
            }
            // (each departed cart's speed where the climb ends, on the run north's first cell)
            let (run_x, run_y) = (-o.row_gap as f64 + 12.5 + fx, if o.tail_desc { 4.0 } else { 1.0 });
            for (i, (t0, k, _)) in departs.iter().enumerate() {
                if top_v[i].is_none() && t - t0 < 80 {
                    if let Some(e) = run.world.entities.list.iter().find(|e| e.id == carts[*k]) {
                        if (e.pos.x - run_x).abs() < 0.3 && e.pos.y > run_y - 0.1 && e.pos.z - fz >= 8.0 {
                            top_v[i] = Some(e.delta_movement.horizontal_distance());
                        }
                    }
                }
            }
            let xd = (-o.row_gap + 12) as f64 + 0.5 + fx;
            for (k, id) in carts.iter().enumerate() {
                let up = pos_of(&run.world, *id).is_some_and(|p| (p.x - xd).abs() < 0.6 && p.z - fz >= 1.5 && p.z - fz < 3.0);
                if up && !on_climb[k] {
                    laps.push(k);
                }
                on_climb[k] = up;
            }
        }
        // 13,000: the bench's stand-in for the siding's naps, as the station's linear bench does)
        if tm.slept {
            let night = (run.world.day_time() - 13000).div_euclid(24000);
            if slept_night.is_none() || (run.world.day_time().rem_euclid(24000) >= 13000 && slept_night != Some(night)) {
                slept_night = Some(night);
                let now = run.world.time;
                for r in &riders {
                    if let Some(i) = redsim::entity::index_of(&run.world, *r) {
                        redsim::entity::mob::ai::brain::brain_mut(&mut run.world, i).last_slept = Some(now);
                    }
                }
                slept_acts += 1;
            }
        }
        // (with a pad: every golem's summon, its crossing into the Nether and its death there,
        // and the pulse at the design's `output.pulse`)
        if has_nether {
            let evs: Vec<redsim::entity::mob::MobEvent> = run.world.mob.events[ev_seen.min(run.world.mob.events.len())..].to_vec();
            ev_seen = run.world.mob.events.len();
            for ev in evs {
                if let redsim::entity::mob::MobEvent::GolemTry { t: et, villager, golem: Some(id), at: Some(cell), .. } = ev {
                    pad_golems.push((id, et, villager, cell, c, dt));
                    // (in turn: the summoner on a stand cell or its pocket; any other position
                    // is an out-of-turn summon onto the pad, the brief's flag, here as it happened)
                    let at = redsim::entity::index_of(&run.world, villager).map(|i| run.world.entities.list[i].pos);
                    let in_turn = at.is_some_and(|p| {
                        let (bx, by, bz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                        lay.stand_cells.iter().chain(lay.pockets.iter().map(|(_, q)| q)).any(|q| q[0] == bx && q[2] == bz && (q[1] - by).abs() <= 1)
                    });
                    if !in_turn {
                        out_of_turn.push(format!("slot {c} +{dt}: villager e#{villager} at {}", at.map_or("gone".to_string(), |p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z))));
                    }
                }
            }
            for &(ct, from, to) in &run.world.crossings[crossed_seen.min(run.world.crossings.len())..] {
                if pad_golems.iter().any(|g| g.0 == from) {
                    nether_golems.push((to, ct, None, from));
                }
            }
            crossed_seen = run.world.crossings.len();
            if let Some(n) = run.world.nether.as_deref() {
                for g in nether_golems.iter_mut().filter(|g| g.2.is_none()) {
                    if !n.entities.list.iter().any(|e| e.id == g.0 && !e.removed) {
                        g.2 = Some(run.world.time);
                    }
                }
            }
            // (the stand batch on the tick before the summon tick: each rider in a stand pocket
            // willing (slept within 24,000 gt, no golem seen lately) and panicking, and why not)
            if (run.world.time + 1).rem_euclid(100) == 0 {
                let t_next = run.world.time + 1;
                let (mut want, mut panic, mut at_stand) = (0usize, 0usize, 0usize);
                for r in &riders {
                    let Some(i) = redsim::entity::index_of(&run.world, *r) else { continue };
                    let p = run.world.entities.list[i].pos;
                    let (bx, by, bz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                    if !lay.stand_cells.iter().chain(lay.pockets.iter().map(|(_, q)| q)).any(|q| q[0] == bx && q[2] == bz && (q[1] - by).abs() <= 1) {
                        continue;
                    }
                    at_stand += 1;
                    if let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind
                        && let Some(b) = m.brain.as_deref()
                    {
                        if b.wants_golem(t_next) {
                            want += 1;
                        } else if b.last_slept.is_none() {
                            unwilling[0] += 1;
                        } else if b.last_slept.is_some_and(|s| t_next - s >= 24000) {
                            unwilling[1] += 1;
                        } else {
                            unwilling[2] += 1;
                        }
                    }
                    if redsim::entity::mob::ai::brain::has_exact(&run.world, i) && redsim::entity::mob::ai::brain::ex(&run.world, i).active == redsim::entity::mob::ai::brain::Act::Panic {
                        panic += 1;
                    }
                }
                willing[want.min(3)] += 1;
                panicking[panic.min(3)] += 1;
                stand_count[at_stand.min(3)] += 1;
            }
            // (each release: G0 opening; locked to the grid when it opens 1..=20 gt after the
            // pad's pulse rose: pulse-triggered (day-1's stand REL comes about 14 gt after the
            // pulse; an EXPIRE release has none that near))
            if let Some(g) = g0 {
                let open = matches!(run.world.get(g), Block::FenceGate { open: true, .. });
                if open && !g0_was {
                    let t = run.world.time;
                    let locked = pulse_rises.last().is_some_and(|r| t - r >= 1 && t - r <= 20);
                    releases.push((t, locked));
                }
                g0_was = open;
            }
            if let Some(p) = tm.pulse_at {
                let on = matches!(run.world.get(p), Block::Wire { power, .. } if power > 0);
                if on && !pulse_was {
                    pulse_rises.push(run.world.time);
                }
                if on && !pulse_was {
                    let t = run.world.time;
                    if pad_golems.iter().any(|g| t - g.1 >= 0 && t - g.1 <= 5) {
                        pulses += 1;
                    } else {
                        spurious_pulses += 1;
                    }
                }
                pulse_was = on;
            }
        }
        // rider's bed claim, when it starts and ends, and where it rode then)
        for e in run.world.entities.list.iter() {
            if let redsim::entity::EntityKind::Mob(m) = &e.kind {
                if tm.record_positions && m.ty == redsim::entity::mob::types::MobType::Villager {
                    villager_cells.insert(e.block_position());
                }
                if m.ty == redsim::entity::mob::types::MobType::IronGolem && golems_seen.insert(e.id) {
                    golem_log.push(format!("e#{} at slot {c} +{dt} (day time {}) at {:.2},{:.2},{:.2}", e.id, run.world.day_time().rem_euclid(24000), e.pos.x, e.pos.y, e.pos.z));
                    // (a golem on the pad, its box meeting a portal block, is the farm's; any
                    // other spawned off the pad, a spawn cell `queue-loop-spots` lists; a villager
                    // that has slept (a napper back from its visit) summons one when it panics
                    // with two more such villagers near)
                    let bb = redsim::entity::make_bounding_box(e.pos, 1.4, 2.7).inflate(0.01, 0.01, 0.01);
                    let mut on_pad = false;
                    for y in bb.min_y.floor() as i32..=bb.max_y.floor() as i32 {
                        for z in bb.min_z.floor() as i32..=bb.max_z.floor() as i32 {
                            for x in bb.min_x.floor() as i32..=bb.max_x.floor() as i32 {
                                on_pad |= run.world.get(BlockPos::new(x, y, z)).to_string().contains("nether_portal");
                            }
                        }
                    }
                    if !on_pad {
                        problems.push(format!("slot {c} +{dt}: an iron golem spawned at {:.2},{:.2},{:.2}, off the pad (see queue-loop-spots)", e.pos.x, e.pos.y, e.pos.z));
                    }
                }
            }
        }
        {
            for (k, r) in riders.iter().enumerate() {
                let Some(i) = redsim::entity::index_of(&run.world, *r) else {
                    if !rider_gone[k] {
                        rider_gone[k] = true;
                        let p = pos_of(&run.world, carts[k]).map_or("gone".to_string(), |p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z));
                        let (lp, lh, lt) = rider_last[k];
                        problems.push(format!("slot {c} +{dt}: rider e{k} gone from the world (its cart at {p}; the rider last at {:.2},{:.2},{:.2} with health {lh} at t {lt})", lp[0], lp[1], lp[2]));
                    }
                    continue;
                };
                if let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind {
                    let e = &run.world.entities.list[i];
                    if m.health < rider_last[k].1 {
                        // (and the damage's source, from the world's mob events; for a wall,
                        // the suffocating cells the eye's box meets)
                        let src = run.world.mob.events.iter().rev().find_map(|ev| match ev { redsim::entity::mob::MobEvent::Hurt { id, src, amount, .. } if *id == *r => Some(format!(" ({src:?}, {amount})")), _ => None }).unwrap_or_default();
                        let eye = e.pos.y + 1.62;
                        let mut walls = Vec::new();
                        // (the eye's box as `is_in_wall` makes it, at the rider's position now and
                        // at its last tick's: the suffocating cells whose boxes it meets)
                        for (tag, q) in [("now", [e.pos.x, e.pos.y, e.pos.z]), ("last", rider_last[k].0)] {
                            let ey = q[1] + 1.62;
                            let bb = redsim::entity::Aabb::new(q[0] - 0.24, ey - 5.0e-7, q[2] - 0.24, q[0] + 0.24, ey + 5.0e-7, q[2] + 0.24);
                            for y in bb.min_y.floor() as i32..=bb.max_y.floor() as i32 {
                                for z in bb.min_z.floor() as i32..=bb.max_z.floor() as i32 {
                                    for x in bb.min_x.floor() as i32..=bb.max_x.floor() as i32 {
                                        let p = BlockPos::new(x, y, z);
                                        let s = run.world.get(p);
                                        if !s.is_air() && redsim::entity::mob::blocks::is_suffocating(&s) && redsim::entity::collide::boxes_at(&run.world, &s, p).iter().any(|b| b.intersects(bb)) {
                                            walls.push(format!("{tag} {:.2},{:.2},{:.2}: {},{},{} {}", q[0], q[1], q[2], p.x, p.y, p.z, s.to_string().trim_start_matches("minecraft:")));
                                        }
                                    }
                                }
                            }
                        }
                        rider_hurt.push(format!("e{k} hurt at slot {c} +{dt}: health {} -> {}{src} at {:.2},{:.2},{:.2} (eye {eye:.2}; suffocating near: {})", rider_last[k].1, m.health, e.pos.x, e.pos.y, e.pos.z, walls.join(", ")));
                    }
                    rider_last[k] = ([e.pos.x, e.pos.y, e.pos.z], m.health, run.world.time - t_start);
                }
                let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind else { continue };
                let h = m.brain.as_deref().and_then(|b| b.home);
                if h != home_was[k] {
                    let p = run.world.entities.list[i].pos;
                    bed_claims.push(format!("e{k} {} at slot {c} +{dt} riding at {:.1},{:.1},{:.1}", h.map_or("let its bed go".to_string(), |h| format!("took the bed ({},{},{})", h.x, h.y, h.z)), p.x, p.y, p.z));
                    home_was[k] = h;
                }
            }
        }
        // (the nap visits: each rider's sleep, each visiting cart rolling off its drop rail and back
        // at rest in a slot; its batch's order before the first advance of slot `slot`+5; the
        // single visit's divert rail and bed holders; the nightly checks at dawn and midday)
        if let Some(v) = tm.visit {
            let tod = run.world.day_time().rem_euclid(24000);
            // (T1's and T2's rails: a shape change logged with its absolute time)
            for (j, (cell, name)) in [(v.t1, "T1"), (v.t2, "T2")].into_iter().enumerate() {
                let s = run.world.get(BlockPos::new(cell.x, cell.y + 1, cell.z)).to_string();
                if s != t_shapes[j] {
                    if !t_shapes[j].is_empty() {
                        let shape = s.split("shape=").nth(1).and_then(|t| t.split([',', ']']).next()).unwrap_or("?").to_string();
                        t_switches.push((c * 100 + dt, name, shape));
                    }
                    t_shapes[j] = s;
                }
            }
            let asleep_now: Vec<bool> = riders.iter().map(|r| redsim::entity::index_of(&run.world, *r).is_some_and(|i| matches!(&run.world.entities.list[i].kind, redsim::entity::EntityKind::Mob(_)) && redsim::entity::mob::mob(&run.world, i).sleeping_pos.is_some())).collect();
            for (k, a) in asleep_now.iter().enumerate() {
                if *a {
                    slept_tonight[k] = true;
                }
            }
            // the bed's nearest awake villager while it is taken (ironspec 2026-11-11: a calm
            // villager within 16 of an occupied bed loses HOME for good; traced, every tick): from
            // each sleeper's bed block centre to every rider not asleep
            for (k, a) in asleep_now.iter().enumerate() {
                if !*a {
                    continue;
                }
                let Some(bed) = redsim::entity::index_of(&run.world, riders[k]).and_then(|i| redsim::entity::mob::mob(&run.world, i).sleeping_pos) else { continue };
                for (j, r) in riders.iter().enumerate() {
                    if j == k || asleep_now[j] {
                        continue;
                    }
                    let Some(p) = pos_of(&run.world, *r) else { continue };
                    let d = ((p.x - (bed.x as f64 + 0.5)).powi(2) + (p.y - (bed.y as f64 + 0.5)).powi(2) + (p.z - (bed.z as f64 + 0.5)).powi(2)).sqrt();
                    if bed_near.as_ref().is_none_or(|b: &(f64, String)| d < b.0) {
                        bed_near = Some((d, format!("rider e{j} at {:.2},{:.2},{:.2}, bed {},{},{}, slot {c} +{dt}", p.x, p.y, p.z, bed.x, bed.y, bed.z)));
                    }
                }
            }
            let homes_now: Vec<Option<BlockPos>> = riders.iter().map(|r| redsim::entity::index_of(&run.world, *r).and_then(|i| match &run.world.entities.list[i].kind { redsim::entity::EntityKind::Mob(m) => m.brain.as_deref().and_then(|b| b.home), _ => None })).collect();
            // (the `cant-reach:` line: each rider's streak where HOME's release can run, and the
            // releases)
            for (k, r) in riders.iter().enumerate() {
                let Some(i) = redsim::entity::index_of(&run.world, *r) else { continue };
                let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind else { continue };
                let Some(b) = m.brain.as_deref() else { continue };
                let Some(x) = b.ex.as_deref() else { continue };
                if b.home.is_some()
                    && x.active == redsim::entity::mob::ai::brain::Act::Rest
                    && x.mem.walk_target.is_none()
                    && let Some(since) = x.mem.cant_reach_since
                {
                    let age = run.world.time - since;
                    if reach_worst.as_ref().is_none_or(|w| age > w.0) {
                        let p = run.world.entities.list[i].pos;
                        reach_worst = Some((age, format!("rider e{k} at {:.2},{:.2},{:.2}, slot {c} +{dt}", p.x, p.y, p.z)));
                    }
                }
            }
            let n_ev = run.world.mob.events.len();
            for ev in &run.world.mob.events[rel_seen.min(n_ev)..] {
                if let redsim::entity::mob::MobEvent::Released { id, .. } = ev
                    && let Some(k) = riders.iter().position(|r| r == id)
                {
                    home_releases.push(format!("rider e{k} slot {c} +{dt}"));
                }
            }
            rel_seen = n_ev;
            if home_at_start.is_none() {
                home_at_start = Some(homes_now.clone());
            }
            // (dawn: tonight's sleepers and the schedule reset; dawn and midday: every HOME as at
            // the start)
            let dawn = tod_was >= 0 && tod < tod_was;
            let midday = tod_was >= 0 && tod_was < 6000 && tod >= 6000;
            if dawn || midday {
                if let Some(h0) = &home_at_start {
                    let moved: Vec<String> = (0..riders.len()).filter(|k| homes_now[*k] != h0[*k]).map(|k| format!("e{k} {:?} (was {:?})", homes_now[k].map(|p| (p.x, p.y, p.z)), h0[k].map(|p| (p.x, p.y, p.z)))).collect();
                    if !moved.is_empty() {
                        problems.push(format!("t {}: {}: HOME changed: {}", run.world.time - t_start, if dawn { "dawn" } else { "midday" }, moved.join(", ")));
                    }
                }
            }
            if dawn && (v.every > 0 || v.observe) {
                let n = slept_tonight.iter().filter(|s| **s).count();
                night_reports.push(format!("night {}: {n} of {} riders asleep, {tonight} visits", night_reports.len() + 1, riders.len()));
                if n < riders.len() {
                    let missed: Vec<usize> = (0..riders.len()).filter(|k| !slept_tonight[*k]).collect();
                    problems.push(format!("night {}: riders never asleep: {missed:?}", night_reports.len()));
                }
                slept_tonight = vec![false; riders.len()];
                tonight = 0;
            }
            tod_was = tod;
            for vr in visits.iter_mut() {
                let Some(id) = vr.cart else { continue };
                let now = c * 100 + dt - vr.slot * 100;
                let drop = q - 4 - vr.k;
                let cell = lay.slot_cells[drop];
                if v.every == 0 && !v.observe && (now == v.divert_at + 10 * vr.k as i64 + 1 || now == v.divert_at + 10 * vr.k as i64 + 12 || now == 250) {
                    // (the single visit: who holds a bed, the divert's rail, the cart then)
                    let homes: Vec<String> = homes_now.iter().enumerate().filter_map(|(k, h)| h.map(|h| format!("e{k} ({},{},{})", h.x, h.y, h.z))).collect();
                    visit_lines.push(format!("visit: +{now}: riders holding a bed: {}", if homes.is_empty() { "none".to_string() } else { homes.join(", ") }));
                    if now != 250 {
                        let rail = BlockPos::new(v.divert.x, v.divert.y + 1, v.divert.z);
                        let p = pos_of(&run.world, id).map_or("gone".to_string(), |p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z));
                        visit_lines.push(format!("visit: +{now}: the divert's rail ({},{},{}) is {} over {}; the cart at {p}", rail.x, rail.y, rail.z, run.world.get(rail), run.world.get(v.divert)));
                    }
                }
                // (an entry back from its visit tracks no more: the same cart visited again later
                // has its own entry)
                if away.contains(&id) && vr.back.is_none() {
                    // (the rider's last seen state)
                    if let Some(i) = vr.rider.and_then(|r| redsim::entity::index_of(&run.world, r)) {
                        let e = &run.world.entities.list[i];
                        let h = match &e.kind { redsim::entity::EntityKind::Mob(m) => m.health, _ => 0.0 };
                        vr.seen = Some((now, [e.pos.x, e.pos.y, e.pos.z], h));
                    }
                    if now == 300 && vr.sleep.0.is_none() {
                        if vr.rider.and_then(|r| redsim::entity::index_of(&run.world, r)).is_none() {
                            visit_lines.push(format!("visit slot {} cart {}: not asleep by +300: the rider is gone from the world (last seen {})", vr.slot, vr.k, vr.seen.map_or("never".to_string(), |(t, p, h)| format!("at +{t} at {:.2},{:.2},{:.2}, health {h}", p[0], p[1], p[2]))));
                        }
                        if let Some(i) = vr.rider.and_then(|r| redsim::entity::index_of(&run.world, r)) {
                            let e = &run.world.entities.list[i];
                            let act = if redsim::entity::mob::ai::brain::has_exact(&run.world, i) { format!("{:?}", redsim::entity::mob::ai::brain::ex(&run.world, i).active) } else { "-".to_string() };
                            let home = match &e.kind { redsim::entity::EntityKind::Mob(m) => m.brain.as_deref().and_then(|b| b.home).map(|h| (h.x, h.y, h.z)), _ => None };
                            visit_lines.push(format!("visit slot {} cart {}: not asleep by +300: the rider at {:.2},{:.2},{:.2}, riding {:?}, activity {act}, home {home:?}, day time {}", vr.slot, vr.k, e.pos.x, e.pos.y, e.pos.z, e.vehicle, run.world.day_time().rem_euclid(24000)));
                        }
                    }
                    if let Some(r) = vr.rider.and_then(|r| riders.iter().position(|x| *x == r)) {
                        if asleep_now[r] {
                            vr.sleep.0.get_or_insert(now);
                            vr.sleep.1 = Some(now);
                        }
                    }
                    // (the first tick on T1's rail: the coupling with the next visit's T switch)
                    if vr.t1_pass.is_none() {
                        if let Some(p) = pos_of(&run.world, id) {
                            if p.x.floor() as i32 == v.t1.x && p.z.floor() as i32 == v.t1.z && (p.y - (v.t1.y as f64 + 1.0)).abs() < 0.9 {
                                vr.t1_pass = Some(now);
                            }
                        }
                    }
                    // (ready: at rest on the siding's exit slot, over its release control)
                    if vr.ready.is_none() {
                        if let Some(i) = redsim::entity::index_of(&run.world, id) {
                            let e = &run.world.entities.list[i];
                            let rc = v.release_cells[vr.k.min(2)].unwrap_or(v.release);
                            if e.pos.x.floor() as i32 == rc.x && e.pos.z.floor() as i32 == rc.z && (e.pos.y - (rc.y as f64 + 1.0)).abs() < 1.0 && e.delta_movement.horizontal_distance() < 0.01 {
                                vr.ready = Some(now);
                            }
                        }
                    }
                    if vr.rolled.is_none() && now > v.release_k[vr.k.min(2)].unwrap_or(v.release_at) {
                        if let Some(i) = redsim::entity::index_of(&run.world, id) {
                            let e = &run.world.entities.list[i];
                            // (on the drop rail first: west of the slot's cell at the peaks'
                            // height; then into the slot's column)
                            let on_rail = e.pos.x >= cell[0] as f64 - 3.0 && e.pos.x < cell[0] as f64 && e.pos.y >= cell[1] as f64 + 0.9 && (e.pos.z - (cell[2] as f64 + 0.5)).abs() < 0.6;
                            if on_rail {
                                vr.on_rail = true;
                            }
                            if vr.on_rail && e.pos.x >= cell[0] as f64 && e.pos.x < cell[0] as f64 + 1.0 && (e.pos.z - (cell[2] as f64 + 0.5)).abs() < 0.6 {
                                vr.rolled = Some((now, e.delta_movement.horizontal_distance()));
                            }
                        }
                    }
                    if vr.rolled.is_some() {
                        if let Some(s) = place_of(&run.world, id) {
                            vr.back = Some((now, s));
                            away.remove(&id);
                            returned.insert(id);
                        }
                    }
                }
                if c == vr.slot + 5 && dt == tm.advance_at - 1 {
                    let at: Vec<Option<usize>> = vr.batch.iter().map(|b| place_of(&run.world, *b)).collect();
                    let want: Vec<Option<usize>> = (0..vr.batch.len()).map(|j| Some(q - 4 - j)).collect();
                    if v.every == 0 && !v.observe {
                        visit_lines.push(format!("visit: the batch at slots {at:?} at slot {c}'s +{dt} (want {want:?})"));
                    }
                    if at != want {
                        problems.push(format!("visit (slot {}, cart {}): the batch at slots {at:?} at slot {c}'s +{dt}, not {want:?}", vr.slot, vr.k));
                    }
                    let aboard = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied()) == vr.rider;
                    if !aboard {
                        problems.push(format!("visit (slot {}, cart {}): the napper is not back in its own cart", vr.slot, vr.k));
                    }
                }
            }
        }
        // PANIC at night (the deep night, day time 13,670..22,330): every rider, and where the
        // calm ones ride
        let tod = run.world.day_time().rem_euclid(24000);
        if (13670..22330).contains(&tod) {
            for (k, r) in riders.iter().enumerate() {
                // (the napper on its visit is meant to sleep, not panic)
                if away.contains(&carts[k]) {
                    continue;
                }
                let Some(i) = redsim::entity::index_of(&run.world, *r) else { continue };
                night_ticks[k] += 1;
                let panicking = redsim::entity::mob::ai::brain::has_exact(&run.world, i) && redsim::entity::mob::ai::brain::ex(&run.world, i).active == redsim::entity::mob::ai::brain::Act::Panic;
                if panicking {
                    panic_ticks[k] += 1;
                } else if let Some(p) = pos_of(&run.world, carts[k]) {
                    *calm_at.entry([p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]).or_insert(0) += 1;
                }
            }
        }
        if let Some((c0, c1)) = tm.tail_trace {
            if c >= c0 && c <= c1 && (lay.esc.is_some() || !lay.fifo.is_empty()) {
                // (`--tail-trace`: every cart in the tail's cells, x 0, z 20..slot 0, each tick, with
                // B, P and the gate)
                let z1 = lay.slot_cells.first().map_or(40, |s| s[2]) + 1;
                let ps: Vec<String> = carts.iter().enumerate().filter_map(|(k, id)| pos_of(&run.world, *id).filter(|p| p.x > -0.5 && p.x < 1.5 && p.z > 19.5 && p.z < z1 as f64).map(|p| {
                    let v = run.world.entities.list.iter().find(|e| e.id == *id).map_or(0.0, |e| e.delta_movement.horizontal_distance());
                    format!("{k}@{:.2},{:.2} v{:.3}", p.y, p.z, v)
                })).collect();
                let on = |b: Option<BlockPos>| b.map_or('?', |b| if matches!(run.world.get(b), Block::RedstoneBlock) { 'P' } else { '-' });
                let gate = lay.fifo.last().map_or('?', |(g, _)| if matches!(run.world.get(*g), Block::FenceGate { open: true, .. }) { 'o' } else { '-' });
                println!("tt c{c} +{dt}: B{} P{} G{} | {}", on(lay.esc), on(lay.esc_p), gate, ps.join(" "));
            }
        }
        if let Some(w) = watch {
            let ps: Vec<String> = w.iter().filter_map(|k| carts.get(*k)).map(|id| run.world.entities.list.iter().find(|e| e.id == *id).map(|e| {
                // (and its rider: where, its box's x span)
                let r = e.passengers.first().and_then(|p| run.world.entities.list.iter().find(|x| x.id == *p)).map(|x| format!(" r{:.2},{:.2} [{:.2}..{:.2} y {:.2}..{:.2}]{}", x.pos.x, x.pos.y, x.bb.min_x, x.bb.max_x, x.bb.min_y, x.bb.max_y, if redsim::entity::minecart::is_pushable(x) { "" } else { " unpushable" })).unwrap_or_default();
                // (and the rail it is on: its cell's block, shape and power only)
                let cell = BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32);
                let rail = run.world.get(cell).to_string().replace("minecraft:", "").replace(",waterlogged=false", "");
                // (and any other entity within 1.5 of it, its own rider aside: what could push it)
                let near: Vec<String> = run.world.entities.list.iter().filter(|x| x.id != e.id && !e.passengers.contains(&x.id) && (x.pos.x - e.pos.x).abs() < 1.5 && (x.pos.y - e.pos.y).abs() < 2.5 && (x.pos.z - e.pos.z).abs() < 1.5).map(|x| format!("e#{} {} {:.2},{:.2},{:.2}", x.id, match &x.kind { redsim::entity::EntityKind::Mob(m) => format!("{:?}", m.ty), k => format!("{:?}", std::mem::discriminant(k)) }, x.pos.x, x.pos.y, x.pos.z)).collect();
                let near = if near.is_empty() { String::new() } else { format!(" near [{}]", near.join("; ")) };
                format!("{:.2},{:.2},{:.2} v{:.3} on {rail}{r}{near}", e.pos.x, e.pos.y, e.pos.z, e.delta_movement.horizontal_distance())
            }).unwrap_or_default()).collect();
            // (the pocket stand: each gate open or shut, G2 G1 G0, and the wired stand's REL and
            // detector D1)
            let gates: String = lay.pockets.iter().rev().map(|(g, _)| if matches!(run.world.get(*g), Block::FenceGate { open: true, .. }) { 'o' } else { '-' }).collect();
            let wired_cells = lay.stand_rel.map(|r| format!(" REL {} D1 {}", if matches!(run.world.get(r), Block::RedstoneBlock) || run.world.has_neighbor_signal(r) { "on" } else { "-" }, if lay.path.iter().any(|(c, k)| *k == Cell::Detector && c[2] == 0 && matches!(run.world.get(BlockPos::new(c[0], c[1], c[2])), Block::DetectorRail { powered: true, .. })) { "on" } else { "-" })).unwrap_or_default();
            let stand = if o.stand_stop { format!(" | gates {gates}{wired_cells}") } else { String::new() };
            // (`--watch-cells`: those cells' block states too, e.g. a wiring join's line)
            let cells: String = tm.watch_cells.iter().flatten().map(|c| format!(" | {},{},{} {}", c[0], c[1], c[2], run.world.get(BlockPos::new(c[0], c[1], c[2])))).collect();
            // (and the design's daylight detectors, with the day time and the weather)
            let dets: String = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::DaylightDetector { .. })).map(|(p, _)| format!(" | det {},{},{} {}", p.x, p.y, p.z, run.world.get(*p).to_string().replace("minecraft:daylight_detector", ""))).collect();
            let wx = if dets.is_empty() { String::new() } else { format!(" | day {} rain {:.2} thunder {:.2} darken {}", run.world.day_time().rem_euclid(24000), run.world.rain_level(1.0), run.world.thunder_level(1.0), run.world.sky_darken()) };
            println!("c{c} +{dt}: {}{stand}{cells}{dets}{wx}", ps.join(" | "));
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
        } else if o.stand_stop {
            // the pocket stand's gates while they hold: gate k shuts once k carts have passed it (a
            // cart's centre 1.29 past the gate slope's start: its tail past the gate); the wired
            // stand's gates are the design's own
            if pocket_hold && !o.stand_wired {
                for k in 1..lay.pockets.len() {
                    let (g, cell) = lay.pockets[k];
                    for id in &carts {
                        // (`pocket_trigger`: the first cart past the trigger position, whatever
                        // count)
                        if let (Some(trg), Some(p)) = (tm.pocket_trigger, pos_of(&run.world, *id)) {
                            let (x, d) = trg[k - 1];
                            if pocket_close_at[k].is_none() && (p.z - (cell[2] as f64 + 0.5)).abs() < 0.6 && p.x >= x && p.x < x + 2.0 && p.y < 1.5 {
                                pocket_close_at[k] = Some(t + d);
                            }
                            continue;
                        }
                        if let Some(p) = pos_of(&run.world, *id)
                            && (p.z - (cell[2] as f64 + 0.5)).abs() < 0.6
                            && p.x >= cell[0] as f64 + 1.29
                            && p.x < cell[0] as f64 + 3.0
                            && !pocket_seen[k].contains(id)
                        {
                            pocket_seen[k].push(*id);
                            if pocket_seen[k].len() == k {
                                pocket_close_at[k] = Some(t + tm.pocket_close_delay);
                            }
                        }
                    }
                    if pocket_close_at[k] == Some(t) {
                        set_gate(&mut run.world, g, "east", false);
                    }
                }
            }
            // (the margins: each crossing the first tick past its mark, on the stand's line; G2's
            // shutting the tick it goes from open to shut)
            if let Some((_, tr, _)) = &stand_train
                && tr.len() == 3
                && lay.pockets.len() == 3
            {
                let x_of = |id: u32| pos_of(&run.world, id).filter(|p| (p.z - 0.5 - fz).abs() < 0.6 && p.y < 1.5).map(|p| p.x - fx);
                // (cart 2 "in" the gate once its front is past the gate's near face (a shutting
                // gate no longer stops a cart it already overlaps), "clear" once its tail is past)
                let marks = [(0usize, 0usize, 2.0), (1, 1, 2.0), (2, 1, -0.1), (3, 1, 1.29), (4, 2, -0.2)];
                for (slot, cart, x) in marks {
                    if margin_rec[slot].is_none() && x_of(tr[cart]).is_some_and(|px| px >= x) {
                        margin_rec[slot] = Some(t);
                    }
                }
                let g2_open = format!("{}", run.world.get(lay.pockets[2].0)).contains("open=true");
                if g2_was_open && !g2_open {
                    margin_rec[5] = Some(t);
                }
                g2_was_open = g2_open;
            }
            // the pocket stand catches with no rail control: the batch is in once its three carts
            // rest in S1, S2, S3 (each against its own gate)
            if let Some((c0, tr, st)) = &mut stand_train
                && *st == 0
                && tr.len() == 3
                && (0..3).all(|k| place_of(&run.world, tr[k]) == Some(q + 2 - k))
            {
                arrivals.push(format!("slot {c0}: batch at rest at the stand +{}", t - *c0 * 100));
                stand_braked_at.push(t - *c0 * 100);
                // (`rel_gate`'s band: the batch at rest this many gt after the latch set)
                if let Some(qa) = gate_q_at.filter(|qa| t - qa < 100) {
                    gate_rest_after_q.push(t - qa);
                }
                *st = 3;
            }
        } else if let Some((c0, tr, st)) = &mut stand_train {
            let (s1, s2, s3) = (lay.stand_cells[0], lay.stand_cells[1], lay.stand_cells[2]);
            let trig = if lay.stand_b.is_empty() { [s2[0] + 1, 0, s2[2]] } else { [s2[0] + 3, 1, s2[2]] };
            // (catch slots: braked once the front cart is 0.45 into its catch cell, S2's peak; it
            // then stops ~0.4 down S1's slope, the train's other two likewise in theirs)
            let caught = |w: &redsim::world::World| if o.stand_desc || o.stand_deep { in_cell(w, tr[0], [s2[0] + 1, s2[1], s2[2]]) } else if lay.stand_b.is_empty() { pos_of(w, tr[0]).is_some_and(|p| p.x >= s2[0] as f64 + 2.45 && (p.z - (s2[2] as f64 + 0.5)).abs() < 0.6) } else { in_cell(w, tr[0], trig) };
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
            // (per-slot brakes off the detector: S1, S2, S3 braked d1, d2, d3 gt after its rise)
            if let (Some(ds), 0, 3) = (tm.stand_detect_slots, *st, tr.len()) {
                let fired = det_cell.is_some_and(|dp| matches!(run.world.get(dp), Block::DetectorRail { powered: true, .. }));
                if rise_at.is_none() && fired {
                    rise_at = Some(t);
                }
                if let Some(r) = rise_at {
                    for k in 0..3usize {
                        if !each_done[k] && t == r + ds[k] {
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
            }
            let due = tr.len() == 3 && tm.stand_detect_each.is_none() && tm.stand_detect_slots.is_none() && match (tm.stand_brake, tm.stand_detect) { (Some(b), _) => t - *c0 * 100 == b, (None, Some(_)) => detected, (None, None) => caught(&run.world) };
            if *st == 0 && tr.len() == 3 && due {
                set(&mut run.world, &lay.stand, false);
                set(&mut run.world, &lay.stand_b, false);
                if let (Some(r), Some(_)) = (tm.stand_reset, wired) {
                    // the wired stand's brake: the bench pulses the latch's reset input
                    run.world.set_block(r, on.clone(), redsim::world::flags::ALL);
                    drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_origin, r.x, r.y, r.z));
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
            if fifo_on {
                break;
            }
            if wired.is_some() {
                // the design's own redstone brakes the tail: done once the train's 3 carts rest
                // in slots 2, 1, 0
                if *st < 2 && tr.len() == 3 && (0..3).all(|k| away.contains(&tr[k]) || place_of(&run.world, tr[k]) == Some(2 - k)) {
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
                // (`tail2_at`: slot 2, the front cart's, braked on its own, any time before that
                // cart arrives: the front cart stops in it whenever the brake came)
                if let (Some(b2), 0) = (tm.tail2_at, *st) {
                    if rel_t == b2 {
                        set(&mut run.world, &[intake[2]], false);
                    }
                }
                let due1 = match tm.intake_brake { Some((b, _)) => rel_t == b, None => in_cell(&run.world, tr[0], lay.slot_peak[1]) };
                if *st == 0 && due1 {
                    set(&mut run.world, if tm.tail2_at.is_some() { &intake[1..2] } else { &intake[1..3] }, false);
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
                // (and at rest: the 3 carts still in slots 2, 1, 0)
                if *st == 2 && (0..3).all(|k| place_of(&run.world, tr[k]) == Some(2 - k)) {
                    tail_rest_at.push(t - *c0 * 100);
                    *st = 3;
                }
            }
        }
        // the drive log's expectations: every cart's position at each slot's end
        if dt == 99 {
            for (k, id) in carts.iter().enumerate() {
                if let Some(p) = pos_of(&run.world, *id) {
                    drive.borrow_mut().push(format!("expect {} pos e{k} {:.3} {:.3} {:.3}", run.world.time - t_origin, p.x, p.y, p.z));
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
        // (`golem_late`: the next golem comes up to J gt late: the world ticks that many more gt
        // at the slot's end before the next pulse, so the gaps between pulses vary over
        // 100 ..= 100 + J; the bench's own per-tick work skips them, its checks stay pulse-relative)
        if dt == 99 && c + 1 < cycles && tm.golem_late > 0 {
            let mut z = (seed as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93) ^ (c as u64).wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            let extra = ((z ^ (z >> 31)) % (tm.golem_late as u64 + 1)) as i64;
            for _ in 0..extra {
                run.tick();
                stop_brake(&mut run.world, &mut stop_arm);
                hold_day(&mut run.world);
            }
            late_total += extra;
        }
    }
    if tm.golem_late > 0 {
        out.push(format!("golem pulses up to {} gt late: {late_total} gt added over {cycles} slots (the gaps between pulses 100..={})", tm.golem_late, 100 + tm.golem_late));
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
    if o.stand_stop {
        out.push(format!("slot-boundary order: {order_ok} clean slots (every queue cart 3 slots on, the tail the stand's last batch, the stand the last train)"));
        // the lap order: the last 21 departures the same cycle as the first 21 (a swap or a lost
        // cart changes it for good: a phase inversion)
        let n = carts.len();
        if laps.len() >= 2 * n {
            let (first, last) = (&laps[..n], &laps[laps.len() - n..]);
            let distinct = |v: &[usize]| v.iter().collect::<std::collections::BTreeSet<_>>().len() == n;
            let kept = distinct(first) && distinct(last) && first.iter().position(|k| *k == last[0]).is_some_and(|p0| (0..n).all(|i| last[i] == first[(p0 + i) % n]));
            out.push(format!("lap order: {} ({} departures; the last {n} {} the first {n}'s cycle)", if kept { "kept" } else { "CHANGED" }, laps.len(), if kept { "are" } else { "are not" }));
        }
        if fifo_on {
            let mut hist = [0usize; 8];
            for h in &fifo_held {
                hist[(*h).min(7)] += 1;
            }
            let what = if lay.esc.is_some() || lay.esc_gb.is_some() { format!("escapement{}, a queue of {} cells", if lay.esc_gb.is_some() { " (flat B, gate G_B)" } else { "" }, lay.queue_cells.len()) } else { format!("{} pockets", lay.fifo.len()) };
            out.push(format!("the re-packing tail ({what}): carts held at the slots' starts {} (0..7); {} advances found slot 0 empty (a hole into the register){}", hist.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("/"), fifo_empty.len(), if fifo_empty.is_empty() { String::new() } else { format!(": {}", fifo_empty.iter().take(12).cloned().collect::<Vec<_>>().join(", ")) }));
        }
        // (one line in a fixed form, read back by `queue-loop-run --seeds`: min and max of each)
        let mm = |f: &dyn Fn(&(i64, i64, i64, i64)) -> i64| (margins.iter().map(f).min().unwrap_or(0), margins.iter().map(f).max().unwrap_or(0));
        let ((a0, a1), (f0, f1), (b0, b1), (c0, c1)) = (mm(&|m| m.0), mm(&|m| m.1), mm(&|m| m.2), mm(&|m| m.3));
        out.push(format!("stand margins: gap12 {a0} {a1} in {f0} {f1} clear {b0} {b1} arrive {c0} {c1} slots {}", margins.len()));
        // (the approach's arrivals over the slots: each train cart first at G2, G1, G0)
        let span_of = |k: usize, g: usize| {
            let v: Vec<i64> = arr.iter().filter_map(|r| r[k][g]).collect();
            if v.is_empty() { "-".to_string() } else { format!("+{}..+{}", v.iter().min().unwrap(), v.iter().max().unwrap()) }
        };
        out.push(format!("approach arrivals ({} slots): cart 1 at G2 {} G1 {} G0 {}; cart 2 at G2 {} G1 {}; cart 3 at G2 {}", arr.len(), span_of(0, 0), span_of(0, 1), span_of(0, 2), span_of(1, 0), span_of(1, 1), span_of(2, 0)));
        if let Some((dc, delay, hold, lock)) = tm.c_prime {
            let rel: Vec<String> = cp_edges.iter().map(|t| format!("{}+{}", t / 100, t % 100)).collect();
            out.push(format!("C' (bench-emulated): detector {},{},{} rising edges taken {} ({}; lockout {lock} gt), F's stone a redstone block {delay} gt after each for {hold} gt{}", dc[0], dc[1], dc[2], cp_edges.len(), rel.join(" "), tm.rel_ext.map(|e| format!("; REL held to the edge + {e} gt {} times (ends {})", cp_ext.len(), cp_ext.iter().map(|t| format!("{}+{}", t / 100, t % 100)).collect::<Vec<_>>().join(" "))).unwrap_or_default()));
        }
    }
    let span = |v: &[i64]| if v.is_empty() { "-".to_string() } else { format!("+{}..+{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
    out.push(format!("stand braked at {} after its slot's start; tail slots 1-2 braked at {}, slot 0 at {} after the train's departure", span(&stand_braked_at), span(&intake_braked_at.iter().filter(|x| x.1 == 1).map(|x| x.0).collect::<Vec<_>>()), span(&intake_braked_at.iter().filter(|x| x.1 == 0).map(|x| x.0).collect::<Vec<_>>())));
    if !tail_rest_at.is_empty() {
        out.push(format!("returning trains at rest in the tail at {} after their departure (the next slot's first advance reaches the tail at +{})", span(&tail_rest_at), 100 + tm.advance_at + tm.adv_shift[0] + tm.skew[0].min(tm.skew[1]).min(tm.skew[2])));
    }
    let nights: u64 = night_ticks.iter().sum();
    // (every rider alive and still in its own cart)
    let lost: Vec<usize> = (0..riders.len()).filter(|k| !away.contains(&carts[*k]) && !run.world.entities.list.iter().any(|e| e.id == carts[*k] && e.passengers.first() == Some(&riders[*k]))).collect();
    if !lost.is_empty() {
        problems.push(format!("riders no longer in their carts at the end: carts {lost:?}"));
    }
    let zombies = run.world.entities.list.iter().filter(|e| matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if redsim::entity::mob::natural::is_zombie_class(m.ty))).count();
    if o.zombie_every > 0 {
        out.push(format!("zombies alive at the end: {zombies}"));
    }
    if nights > 0 {
        let fr: Vec<f64> = night_ticks.iter().zip(&panic_ticks).map(|(n, p)| if *n > 0 { *p as f64 / *n as f64 } else { 1.0 }).collect();
        let min = fr.iter().cloned().fold(1.0, f64::min);
        let mean = panic_ticks.iter().sum::<u64>() as f64 / nights as f64;
        let mut calm: Vec<([i32; 3], u64)> = calm_at.into_iter().collect();
        calm.sort_by(|a, b| b.1.cmp(&a.1));
        let top: Vec<String> = calm.iter().take(6).map(|(c, n)| format!("{} {} {} ({n})", c[0], c[1], c[2])).collect();
        out.push(format!("PANIC at night (day time 13,670..22,330, {} rider-ticks): {:.2}% of rider-ticks, the least-panicking rider {:.2}%; calm most at {}", nights, 100.0 * mean, 100.0 * min, if top.is_empty() { "-".to_string() } else { top.join(", ") }));
    }
    if let Some(path) = drive_out {
        let mut text = format!("seed {seed}\ndays 0\nwalk random\n# ticks {}\n# t0 {t_spawn}\n# mctas mobfarm queue-loop-run <design> --cycles {cycles} --seed {seed} --release-for {} --advance-at {} --advance-every {} --pulse {}\n# start: Run::start(spec, RunOpts {{ ai: true, ..steady(seed, ticks, Random) }}); settle_tick until false; the daylight detectors placed (raw, then updates), as Run::arrive does; spawn the design's entity lines in order (e0 .. e20, each a minecart with its villager e<N>.p0); t0 = the time right after that spawn; act t applied when the time is t0 + t, before the next tick\n", run.world.time - t_origin, tm.release_for, tm.advance_at, tm.advance_every, tm.pulse);
        if let (Some(b), Some(g)) = (build_start, tm.build_gap) {
            let gaps = if tm.build_priced { "each cell's priced gap between placements (20 gt; 40 gt at a cell the bill charges place:edge)".to_string() } else { format!("{g} gt between placements") };
            text.push_str(&format!("# build: built in order instead: Run::start with the design's blocks and block entities removed (no settle), then each `act t set x y z <state>` a placement with the game's updates (flags 3) and each `act t nbt x y z <snbt>` a container's fill, in the build order, {gaps}; every time here counts from the build's start (time {b}), not from t0\n# spawn {}\n", t_spawn - b));
        }
        for l in drive.borrow().iter() {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::write(path, text).map_err(|e| e.to_string())?;
        out.push(format!("drive log: {}", path.display()));
    }
    if let Some(d) = built_diff {
        out.push(d);
    }
    out.extend(visit_lines_pre.iter().cloned());
    if tm.record_positions {
        out.push(format!("villager positions ({}): {}", villager_cells.len(), villager_cells.iter().map(|p| format!("{},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ")));
    }
    if tm.no_summons {
        out.push("diagnostic (--no-summons): every rider held GOLEM_DETECTED_RECENTLY, so no golem was summoned anywhere; not a design check".to_string());
    }
    if has_nether {
        let iron = nether_iron(&run.world) - iron_start;
        let with: std::collections::BTreeSet<i64> = pad_golems.iter().map(|g| g.4).collect();
        let offsets: Vec<i64> = pad_golems.iter().map(|g| g.5).collect();
        let died = nether_golems.iter().filter(|g| g.2.is_some()).count();
        let dwell: Vec<i64> = nether_golems.iter().filter_map(|g| pad_golems.iter().find(|p| p.0 == g.3).map(|p| g.1 - p.1)).collect();
        out.push(format!(
            "the pad: {} golems over {cycles} slots ({} slots with a golem: {:.2}%), summoned at slot +{}..+{}; {} through the portal{}, {} died in the Nether ({} alive at the end); pulses at {}: {pulses} with a golem, {spurious_pulses} without; iron into the Nether part's containers: {iron:+} ingots",
            pad_golems.len(),
            with.len(),
            100.0 * with.len() as f64 / cycles.max(1) as f64,
            offsets.iter().min().copied().unwrap_or(0),
            offsets.iter().max().copied().unwrap_or(0),
            nether_golems.len(),
            if dwell.is_empty() { String::new() } else { format!(" ({}..{} gt after the summon)", dwell.iter().min().unwrap(), dwell.iter().max().unwrap()) },
            died,
            nether_golems.len() - died,
            tm.pulse_at.map_or("(no output.pulse port)".to_string(), |p| format!("{},{},{}", p.x, p.y, p.z)),
        ));
        if tm.slept {
            out.push(format!("LAST_SLEPT bench-driven (--slept): every rider's set {slept_acts} times (the start and each night from day time 13,000), the station bench's stand-in for the siding's naps"));
        }
        out.push(format!("summons onto the pad out of turn (the summoner not on a stand cell or its pocket): {} of {}{}", out_of_turn.len(), pad_golems.len(), if out_of_turn.is_empty() { String::new() } else { format!(": {}", out_of_turn.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }));
        out.push(format!("pad stats: golems {} with {} slots {cycles} crossed {} died {} alive {} pulses {pulses} spurious {spurious_pulses} iron {iron} out_of_turn {}", pad_golems.len(), with.len(), nether_golems.len(), died, nether_golems.len() - died, out_of_turn.len()));
        out.push(format!("summon ticks: stand riders willing 0/1/2/3: {} {} {} {}; panicking 0/1/2/3: {} {} {} {}; riders at the stand 0/1/2/3: {} {} {} {}; unwilling: never slept {}, slept 24,000 gt ago or more {}, a golem seen lately {}", willing[0], willing[1], willing[2], willing[3], panicking[0], panicking[1], panicking[2], panicking[3], stand_count[0], stand_count[1], stand_count[2], stand_count[3], unwilling[0], unwilling[1], unwilling[2]));
        out.push(format!("summon stats: {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}", willing[0], willing[1], willing[2], willing[3], panicking[0], panicking[1], panicking[2], panicking[3], stand_count[0], stand_count[1], stand_count[2], stand_count[3], unwilling[0], unwilling[1], unwilling[2]));
        // (power-on: the first golem, the first release, the releases before the first locked one)
        let first_golem = pad_golems.first().map(|g| g.1 - t_start);
        let first_release = releases.first().map(|r| r.0 - t_start);
        let before_lock = releases.iter().position(|r| r.1);
        out.push(format!(
            "power-on: day time at the start {}; the first golem at +{} gt; the first release (G0 open) at +{} gt; {} releases before the first locked one (G0 open 1..20 gt after the pad's pulse){}",
            start_day.map_or("(as the bench's)".to_string(), |d| d.to_string()),
            first_golem.map_or("never".to_string(), |t| t.to_string()),
            first_release.map_or("never".to_string(), |t| t.to_string()),
            before_lock.map_or("all".to_string(), |n| n.to_string()),
            if releases.is_empty() { String::new() } else { format!(", {} of {} releases locked", releases.iter().filter(|r| r.1).count(), releases.len()) },
        ));
        out.push(format!("power stats: {} {} {} {} {} {}", start_day.unwrap_or(-1), first_golem.unwrap_or(-1), first_release.unwrap_or(-1), before_lock.map_or(-1, |n| n as i64), releases.iter().filter(|r| r.1).count(), releases.len()));
        for s in out_of_turn.iter().take(3) {
            problems.push(format!("a summon onto the pad out of turn: {s}"));
        }
    }
    if !golem_log.is_empty() {
        out.push(format!("iron golems: {} first seen: {}", golem_log.len(), golem_log.iter().take(12).cloned().collect::<Vec<_>>().join("; ")));
    }
    if !rider_hurt.is_empty() {
        out.push(format!("riders hurt: {}", rider_hurt.join("; ")));
    }
    out.push(format!(
        "cant-reach: {}; HOME releases: {}",
        match &reach_worst {
            Some((age, at)) => format!("the oldest CANT_REACH_WALK_TARGET_SINCE where HOME's release can run (REST, no WALK_TARGET, HOME held) {age} gt ({at}); margin to 1200: {:+}", 1200 - age),
            None => "never set where HOME's release can run (REST, no WALK_TARGET, HOME held)".to_string(),
        },
        if home_releases.is_empty() { "none".to_string() } else { format!("{} ({})", home_releases.len(), home_releases.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    if let Some(v) = tm.visit {
        out.extend(visit_lines.iter().cloned());
        out.push(format!("visit: bed claims: {}", if bed_claims.is_empty() { "none".to_string() } else { bed_claims.join("; ") }));
        // (each visit: asleep, rolled off, back; a visit whose 6 slots outran the run is left out)
        out.push(match &bed_near {
            Some((d, at)) => format!("bed: the nearest awake villager while a bed was taken {d:.2} ({at}); margin over 16: {:+.2}", d - 16.0),
            None => "bed: never taken".into(),
        });
        let mut done = 0;
        for vr in &visits {
            if vr.slot + 6 > cycles {
                continue;
            }
            done += 1;
            out.push(format!("visit slot {} cart {}: rider asleep {}; ready at the siding's exit {}; passed T1 {}; rolled off its drop rail {}; back at rest {}", vr.slot, vr.k, match vr.sleep { (Some(a), Some(b)) => format!("+{a}..+{b}"), _ => "never".into() }, vr.ready.map_or("never".into(), |t| format!("+{t}")), vr.t1_pass.map_or("never".into(), |t| format!("at +{t}")), vr.rolled.map_or("never".into(), |(t, s)| format!("at +{t} at {s:.2} b/t")), vr.back.map_or("never".into(), |(t, s)| format!("in slot {s} at +{t}"))));
            if vr.sleep.0.is_none() {
                problems.push(format!("visit (slot {}, cart {}): the napper never slept", vr.slot, vr.k));
            }
            if vr.back.is_none() {
                problems.push(format!("visit (slot {}, cart {}): the cart never came back to rest in a slot", vr.slot, vr.k));
            }
        }
        // (the coupling "the previous visit's cart clears the T's before they switch for the next":
        // for each visit, the first T switch after it passed T1 and the last one before it did,
        // relative to its pass; the smallest gaps over the run)
        let mut after: Vec<i64> = Vec::new();
        let mut before: Vec<i64> = Vec::new();
        for vr in &visits {
            let Some(p) = vr.t1_pass.map(|t| vr.slot * 100 + t) else { continue };
            if let Some((t, _, _)) = t_switches.iter().find(|(t, _, _)| *t > p) {
                after.push(t - p);
            }
            if let Some((t, _, _)) = t_switches.iter().rev().find(|(t, _, _)| *t <= p) {
                before.push(p - t);
            }
        }
        if !after.is_empty() {
            out.push(format!("visit: T1/T2 switches {} times; after a visiting cart passed T1 the next switch came {}..{} gt later; before it passed, the last switch was {}..{} gt earlier", t_switches.len(), after.iter().min().unwrap_or(&0), after.iter().max().unwrap_or(&0), before.iter().min().unwrap_or(&0), before.iter().max().unwrap_or(&0)));
        }
        if v.every > 0 || v.observe {
            out.push(format!("visits: {done} complete; {}", if night_reports.is_empty() { "no dawn reached".to_string() } else { night_reports.join("; ") }));
        }
    }
    if !side_changed.is_empty() {
        problems.insert(0, format!("side drop: the side rail reshaped {} loop cells: {}", side_changed.len(), side_changed.iter().take(4).cloned().collect::<Vec<_>>().join("; ")));
    }
    out.extend(arrivals);
    // (the hard count, ironspec 21:24: every cart and rider at the start and at the end; a cart
    // short is a derail)
    {
        let alive = |id: u32| run.world.entities.list.iter().any(|e| e.id == id && !e.removed && e.pos.y > -6.0);
        let c_end = carts.iter().filter(|id| alive(**id)).count();
        let r_end = riders.iter().filter(|id| alive(**id)).count();
        let seated = carts.iter().zip(riders.iter()).filter(|(c, r)| run.world.entities.list.iter().any(|e| e.id == **c && e.passengers.contains(r))).count();
        out.push(format!("carts and riders: {} and {} at the start, {c_end} and {r_end} at the end ({seated} seated)", carts.len(), riders.len()));
        if c_end < carts.len() || r_end < riders.len() {
            problems.push(format!("the count: {c_end} of {} carts and {r_end} of {} riders at the end", carts.len(), riders.len()));
        }
        out.push(merges.line());
        out.push(format!("declared states (rails, gates, boosters as the options declare them; tick 0 and boosters at t 20): {} mismatches{}", declared_bad.len(), if declared_bad.is_empty() { String::new() } else { format!(" ({})", declared_bad.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
        if !declared_bad.is_empty() {
            problems.push(format!("declared states: {} mismatches ({})", declared_bad.len(), declared_bad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
        }
        if !merges.pairs.is_empty() {
            problems.push(format!("merged carts: {} pairs ({})", merges.pairs.len(), merges.first.join("; ")));
        }
    }
    if verbose {
        let ps: Vec<String> = carts.iter().map(|id| pos_of(&run.world, *id).map(|p| format!("{:.1},{:.1},{:.1}", p.x, p.y, p.z)).unwrap_or_default()).collect();
        out.push(format!("  carts: {}", ps.join(" ")));
        for c in lay.stand_cells.iter() {
            let p = BlockPos::new(c[0], c[1], c[2]);
            out.push(format!("  stand rail {:?}: {:?} on {:?}", c, run.world.get(p), run.world.get(BlockPos::new(c[0], c[1] - 1, c[2]))));
        }
    }
    // (the stand's per-slot state and any hand reset)
    out.extend(stand_log);
    out.extend(unload_log);
    if !run.world.errors.is_empty() {
        out.push(format!("model errors: {}", run.world.errors.join("; ")));
    }
    // (the pocket stand's departures and backward crossings: one fixed-form line each, read back
    // by `--seeds`; the gaps per slot, a slot's own departures from its +0 to the next slot's +0)
    if o.stand_stop {
        let gaps: Vec<(i64, i64, bool)> = departs.windows(2).map(|w| (w[1].0 / 100, w[1].0 - w[0].0, w[1].2)).collect();
        let normal: Vec<i64> = gaps.iter().filter(|(_, g, _)| *g < 60).map(|(_, g, _)| *g).collect();
        let contact = departs.iter().filter(|(_, _, tch)| *tch).count();
        out.push(format!("stand departures: {} carts, gap min {} max {} gt (gaps under 60 gt), {contact} in contact with the one before", departs.len(), normal.iter().min().map_or("-".to_string(), |v| v.to_string()), normal.iter().max().map_or("-".to_string(), |v| v.to_string())));
        let small: Vec<String> = gaps.iter().filter(|(_, g, tch)| *g < 8 || *tch).take(12).map(|(s, g, tch)| format!("slot {s} {g} gt{}", if *tch { " touching" } else { "" })).collect();
        // (the departure corner (9, -3, 0), entered (x 9 crossed): each slot's carts 0, 1, 2 in
        // their departure order, and their margins in day-1's nap-visit divert windows shifted
        // +4 (golem +21+10k ..= +30+10k))
        let per_k: Vec<Vec<i64>> = (0..3).map(|k| (0..cycles).filter_map(|c| departs.iter().filter(|(t, _, _)| t / 100 == c).nth(k).map(|(t, _, _)| t - 100 * c)).collect()).collect();
        let lo = |v: &Vec<i64>| v.iter().min().copied();
        let hi = |v: &Vec<i64>| v.iter().max().copied();
        let corner: Vec<String> = per_k.iter().enumerate().map(|(k, v)| match (lo(v), hi(v)) {
            (Some(a), Some(b)) => format!("cart {k} +{a}..+{b} (margins {}/{})", a - (21 + 10 * k as i64), (30 + 10 * k as i64) - b),
            _ => format!("cart {k} -"),
        }).collect();
        out.push(format!("departure corner (9,-3,0) entered: {}", corner.join(", ")));
        if !small.is_empty() {
            out.push(format!("stand departures under 8 gt apart or touching: {}", small.join(", ")));
        }
        out.push(format!("back through a gate: {}{}", back_through.len(), if back_through.is_empty() { String::new() } else { format!(" ({})", back_through.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }));
        if !back_causes.is_empty() {
            out.push(format!("back-crossing causes: {} ({})", back_kinds.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "), back_causes.iter().take(8).cloned().collect::<Vec<_>>().join("; ")));
        }
        if let Some((gs, gt, gl)) = tm.rel_gate {
            let delays: Vec<i64> = gate_log.iter().map(|g| g.2).collect();
            let byq: Vec<i64> = gate_log.iter().filter(|g| g.3).map(|g| g.2).collect();
            let byt = gate_log.iter().filter(|g| !g.3).count();
            let ages: Vec<i64> = gate_log.iter().filter_map(|g| g.4).collect();
            let late = gate_log.iter().max_by_key(|g| g.1).map(|g| format!("slot {} +{}", g.0, g.1)).unwrap_or_default();
            let delayed: Vec<String> = gate_log.iter().filter(|g| g.2 > 0).map(|g| format!("slot {} +{} (+{} after REL{})", g.0, g.1, g.2, if g.3 { "" } else { ", timeout" })).collect();
            out.push(format!("rel gate (S {gs}, T {gt}, L {gl}): {} releases, {} by the latch (delay after REL's rise {}), {byt} by the timeout; the latch's age at a release {}; latest release {late}; delayed: {}", gate_log.len(), byq.len(), if byq.is_empty() { "-".to_string() } else { format!("{}..{} gt", byq.iter().min().unwrap(), byq.iter().max().unwrap()) }, if ages.is_empty() { "-".to_string() } else { format!("{}..{} gt", ages.iter().min().unwrap(), ages.iter().max().unwrap()) }, if delayed.is_empty() { "none".to_string() } else { delayed.iter().take(8).cloned().collect::<Vec<_>>().join(", ") }));
            let _ = delays;
            let mm = |v: &Vec<i64>| if v.is_empty() { "-".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
            out.push(format!("rel gate band: the batch at rest {} gt after the latch sets (S at least the top); the latch's age at REL's rise {} gt (S at most the bottom keeps a release undelayed)", mm(&gate_rest_after_q), mm(&gate_age_at_rise)));
        }
        if tm.late_hold.iter().any(|h| h.is_some()) {
            out.push(format!("late hold: {}", tm.late_hold.iter().flatten().map(|h| format!("G{} held shut {} gt", h.3, late_holds[h.3.min(2)])).collect::<Vec<_>>().join(", ")));
            out.push(format!("late hold windows (each REL rise): {}", tm.late_hold.iter().flatten().map(|h| { let g = h.3.min(2); format!("G{}'s cell {},{},{}: latest crossing before a rise >= {} gt before it, first crossing after a rise >= {} gt after it", h.3, h.0[0], h.0[1], h.0[2], late_win_before[g].map_or("-".to_string(), |v| v.to_string()), late_win_after[g].map_or("-".to_string(), |v| v.to_string())) }).collect::<Vec<_>>().join("; ")));
        }
        out.push(format!("{}; riders not seated: {}{}", rail_watch.line(), unseat_log.len(), if unseat_log.is_empty() { String::new() } else { format!(" ({})", unseat_log.iter().take(6).cloned().collect::<Vec<_>>().join(", ")) }));
        if o.stand_wired && !latch_sets.is_empty() {
            out.push(format!("latch set (T2 out after +55): +{}..+{} over {} slots", latch_sets.iter().min().unwrap(), latch_sets.iter().max().unwrap(), latch_sets.len()));
        }
        if o.stand_wired {
            // (every set: out of turn is one with REL off before +55, or a second one with REL off
            // in a slot; a set while REL is on (a departing cart over D1, C''s late pulse) is held
            // off by REL's reset and only flickers T2)
            let early: Vec<String> = latch_all.iter().filter(|(_, d, r)| *d < 55 && !*r).map(|(c, d, _)| format!("slot {c} +{d}")).collect();
            let under_rel = latch_all.iter().filter(|(_, _, r)| *r).count();
            let mut per: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
            for (c, _, r) in &latch_all {
                if !*r {
                    *per.entry(*c).or_default() += 1;
                }
            }
            let doubles: Vec<String> = per.iter().filter(|(_, n)| **n > 1).map(|(c, n)| format!("slot {c} x{n}")).collect();
            out.push(format!("latch sets: {} with REL off in {} slots ({under_rel} more under REL, overridden); out of turn (REL off, before +55) {}{}; more than one with REL off in a slot {}{}", latch_all.len() - under_rel, per.len(), early.len(), if early.is_empty() { String::new() } else { format!(" ({})", early.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }, doubles.len(), if doubles.is_empty() { String::new() } else { format!(" ({})", doubles.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }));
        }
        if o.stand_guard && !t3_rises.is_empty() {
            let offs: Vec<i64> = t3_rises.iter().map(|(_, d)| *d).collect();
            let mut per: std::collections::BTreeMap<i64, Vec<i64>> = std::collections::BTreeMap::new();
            for (c, d) in &t3_rises {
                per.entry(*c).or_default().push(*d);
            }
            let multi: Vec<String> = per.iter().filter(|(_, v)| v.len() > 1).map(|(c, v)| format!("slot {c} +{}", v.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("/+"))).collect();
            out.push(format!("T3 (the guard's set: C's timer or C') lit {} times at +{}..+{}; slots with more than one {}{}", t3_rises.len(), offs.iter().min().unwrap(), offs.iter().max().unwrap(), multi.len(), if multi.is_empty() { String::new() } else { format!(" ({})", multi.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }));
        }
        // (the departure corner, day-1's divert T: each departure of 3 (passes under 30 gt apart),
        // cart k's centre in and out of the corner cell, from the slot's start and from G0's last
        // opening before it (REL's onset), min..max over the slots)
        {
            corner_pass.sort();
            let mut groups: Vec<Vec<(i64, i64)>> = Vec::new();
            for p in &corner_pass {
                match groups.last_mut() {
                    Some(g) if p.0 - g.last().unwrap().0 < 30 => g.push(*p),
                    _ => groups.push(vec![*p]),
                }
            }
            let mut rng = [[(i64::MAX, i64::MIN); 4]; 3];
            let mut n = 0;
            for g in groups.iter().filter(|g| g.len() == 3) {
                let Some(open) = g0_opens.iter().rev().find(|o| **o <= g[0].0).copied() else { continue };
                n += 1;
                let base = g[0].0.div_euclid(100) * 100;
                for (k, (i, o2)) in g.iter().enumerate() {
                    for (j, v) in [i - base, o2 - base, i - open, o2 - open].into_iter().enumerate() {
                        rng[k][j] = (rng[k][j].0.min(v), rng[k][j].1.max(v));
                    }
                }
            }
            if n > 0 {
                let r = |p: (i64, i64)| if p.0 == p.1 { format!("{}", p.0) } else { format!("{}..{}", p.0, p.1) };
                out.push(format!("divert corner (departures of 3, {n} slots): {}", (0..3).map(|k| format!("cart {k} in +{} out +{} (from G0's opening: in {} out {})", r(rng[k][0]), r(rng[k][1]), r(rng[k][2]), r(rng[k][3]))).collect::<Vec<_>>().join("; ")));
            }
        }
        // (bursts: departures under 30 gt apart; each burst's size, when it leaves in its slot,
        // its gaps, the pairs in contact, and the speeds where the climb ends; the biggest first)
        let mut bursts: Vec<Vec<usize>> = Vec::new();
        for i in 0..departs.len() {
            match bursts.last_mut() {
                Some(b) if departs[i].0 - departs[*b.last().unwrap()].0 < 30 => b.push(i),
                _ => bursts.push(vec![i]),
            }
        }
        let fmt = |b: &Vec<usize>| {
            let t0 = departs[b[0]].0;
            let gaps: Vec<String> = b.windows(2).map(|w| format!("{}{}", departs[w[1]].0 - departs[w[0]].0, if departs[w[1]].2 { "t" } else { "" })).collect();
            let vs: Vec<String> = b.iter().map(|i| top_v[*i].map_or("-".to_string(), |v| format!("{v:.2}"))).collect();
            format!("{} carts (slot {} +{}..+{}, gaps {}, top {})", b.len(), t0.div_euclid(100), t0.rem_euclid(100), departs[*b.last().unwrap()].0 - t0.div_euclid(100) * 100, if gaps.is_empty() { "-".to_string() } else { gaps.join("/") }, vs.join("/"))
        };
        let mut sizes: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for b in &bursts {
            *sizes.entry(b.len()).or_default() += 1;
        }
        let mut order: Vec<&Vec<usize>> = bursts.iter().collect();
        order.sort_by_key(|b| std::cmp::Reverse((b.len(), b.windows(2).filter(|w| departs[w[1]].2).count())));
        if let Some(b) = order.first() {
            out.push(format!("stand bursts: {} (size x count); the biggest: {}", sizes.iter().map(|(s, n)| format!("{s}x{n}")).collect::<Vec<_>>().join(" "), fmt(b)));
        }
        if verbose {
            for b in bursts.iter().filter(|b| b.len() != 3) {
                out.push(format!("  burst: {}", fmt(b)));
            }
        }
        // (the tail's entry: the same grouping, gaps under 30 gt one group)
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in 0..tail_in.len() {
            match groups.last_mut() {
                Some(g) if tail_in[i].0 - tail_in[*g.last().unwrap()].0 < 30 => g.push(i),
                _ => groups.push(vec![i]),
            }
        }
        let gaps: Vec<i64> = groups.iter().flat_map(|g| g.windows(2).map(|w| tail_in[w[1]].0 - tail_in[w[0]].0)).collect();
        let big = groups.iter().max_by_key(|g| (g.len(), std::cmp::Reverse(g.windows(2).map(|w| tail_in[w[1]].0 - tail_in[w[0]].0).min().unwrap_or(99))));
        if let Some(g) = big {
            let t0 = tail_in[g[0]].0;
            let gg: Vec<String> = g.windows(2).map(|w| format!("{}{}", tail_in[w[1]].0 - tail_in[w[0]].0, if tail_in[w[1]].1 { "t" } else { "" })).collect();
            let vs: Vec<String> = g.iter().map(|i| format!("{:.2}", tail_in[*i].2)).collect();
            let mut sizes: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
            for g in &groups {
                *sizes.entry(g.len()).or_default() += 1;
            }
            out.push(format!("tail entry: {} carts, gap min {} gt within groups, {} touching; groups {}; the biggest: {} carts (slot {} +{}, gaps {}, speeds {})", tail_in.len(), gaps.iter().min().map_or("-".to_string(), |v| v.to_string()), tail_in.iter().filter(|x| x.1).count(), sizes.iter().map(|(s, n)| format!("{s}x{n}")).collect::<Vec<_>>().join(" "), g.len(), t0.div_euclid(100), t0.rem_euclid(100), if gg.is_empty() { "-".to_string() } else { gg.join("/") }, vs.join("/")));
        }
        // (`--verbose`: every crossing of the tail's entry, for a tail bench to replay: slot +gt,
        // cart, speed, touching the one ahead)
        if verbose && !tail_in.is_empty() {
            out.push(format!("tail entry trace (slot +gt cart speed): {}", tail_in.iter().zip(tail_in_cart.iter()).map(|((t, touch, v), k)| format!("{}+{} c{k} {v:.2}{}", t.div_euclid(100), t.rem_euclid(100), if *touch { "t" } else { "" })).collect::<Vec<_>>().join(", ")));
        }
    }
    // (which slots had a problem, and how many each: a fault's reach at a glance; a returning train
    // still missing from the tail is counted once, at its first report, not again every slot)
    if !problems.is_empty() {
        let mut by: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        let mut trains_seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for p in &problems {
            if let Some((_, rest)) = p.split_once(": ")
                && rest.ends_with("returning train not in the queue's tail at the advance")
                && !trains_seen.insert(rest.to_string())
            {
                continue;
            }
            if let Some(n) = p.strip_prefix("slot ").and_then(|r| r.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|d| d.parse::<i64>().ok()) {
                *by.entry(n).or_default() += 1;
            }
        }
        out.push(format!("problem slots: {}", by.iter().map(|(s, n)| format!("{s}x{n}")).collect::<Vec<_>>().join(" ")));
    }
    Ok(format!("{}\n{} problems{}", out.join("\n"), problems.len(), if problems.is_empty() { String::new() } else { format!(":\n  {}", problems.iter().take(if verbose { 2000 } else { 20 }).cloned().collect::<Vec<_>>().join("\n  ")) }))
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    /// Every checked-in loop design regenerates without a requested booster placed plain (the
    /// generator refuses one: the qboost hazard, ironspec Nov 11 01:04), and its layout's declared
    /// flights parse back from their header form.
    #[test]
    fn checked_in_loops_have_no_downgraded_boosters() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/parts");
        let mut seen = 0;
        let mut stack = vec![root];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let Some(q) = text.lines().find_map(|l| l.strip_prefix("queue: ")) else { continue };
                if !q.starts_with("scene=Loop") {
                    continue;
                }
                let mut meta = std::collections::BTreeMap::new();
                meta.insert("queue".to_string(), q.to_string());
                let o = loop_opts_of(&meta).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                let lay = loop_layout_frame(&o);
                assert!(lay.downgraded.is_empty(), "{}: boosters placed plain: {:?}", p.display(), lay.downgraded);
                assert_eq!(crate::flight::parse(&crate::flight::format(&lay.flights)).unwrap(), lay.flights, "{}", p.display());
                seen += 1;
            }
        }
        assert!(seen >= 1, "no checked-in loop design found");
    }

    #[test]
    fn a_rail_turned_once_runs_the_turned_way() {
        // (+x to +z: an east-west rail becomes north-south, a curve's ends turn with it, a slope
        // rises toward the turned side)
        assert_eq!(rail_shape_turned("east_west", 1), "north_south");
        assert_eq!(rail_shape_turned("south_west", 1), "north_west");
        assert_eq!(rail_shape_turned("north_east", 1), "south_east");
        assert_eq!(rail_shape_turned("ascending_east", 1), "ascending_south");
        assert_eq!(rail_shape_turned("north_west", 2), "south_east");
    }

    #[test]
    fn four_turns_give_the_block_back() {
        for s in ["minecraft:rail[shape=south_west,waterlogged=false]", "minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]", "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]", "minecraft:repeater[delay=2,facing=east,locked=false,powered=false]", "minecraft:comparator[facing=north,mode=subtract,powered=false]", "minecraft:redstone_wire[east=side,north=up,power=0,south=none,west=side]", "minecraft:observer[facing=down,powered=false]"] {
            let b = Block::parse(s).unwrap();
            let mut t = b;
            for _ in 0..4 {
                t = block_turned(&t, 1).unwrap();
            }
            assert_eq!(t, b, "{s}");
            assert_eq!(block_turned(&block_turned(&b, 1).unwrap(), 3).unwrap(), b, "{s}");
        }
    }
}
