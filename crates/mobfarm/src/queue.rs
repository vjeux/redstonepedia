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
    /// Admission control: R (the slope under G_P into slot 0) a powered rail pulsed at each
    /// launch (`LoopTiming::adm_rpulse`), its control beside it (east); braked between pulses.
    pub adm_rboost: bool,
    /// Admission control: P a sloped detector rail (it senses its cart; no booster), the launch on
    /// gravity (with `adm_rboost`, R's pulse speeds it).
    pub adm_pdet: bool,
    /// Admission control's entry-hold sensors (with `adm_ledge`): the plain landing flat and the
    /// third queue slope detector rails, read by the stand's REL (ironspec Nov 11 03:35).
    pub adm_sensors: bool,
    /// The room detector's queue slope (0 = the top one, by the landing; default 2).
    pub adm_room_k: u32,
    /// The top queue slope a detector too: the landing latch's reset (the cart moved on).
    pub adm_latch_reset: bool,
    /// Admission control's x-queue (ironspec's R1): N plain slopes down west along z 24 from the
    /// return run's take-off, the curve north and one flat transition cell upstream of z 25|26,
    /// then the `tail_esc` slopes north into G0 and P (0: the column queue).
    pub adm_xq: u32,
}

impl Default for LoopOpts {
    fn default() -> LoopOpts {
        LoopOpts { slots: 18, run: 20, depart: 3, boost_every: 0, boost_first: 3, straight: false, drop: false, vstand: false, tail_v: true, bridge: true, row_gap: 2, uturn_boost: false, line: false, tail_deep: false, plain: false, stand_desc: false, detector: false, zombie_every: 0, zombie_x: 3, stand_deep: false, tail_desc: false, peak_pad: false, short_depart: false, stand_stop: false, stand_wired: false, stand_guard: false, shift: [0, 0], tail_fifo: 0, tail_esc: 0, climb_boost: None, tail_brake: false, tail_plain: false, climb_every: 0, ledge_slope: false, esc_door: false, esc_flat: false, landing_boost2: false, esc_g0: false, esc_adm: false, adm_ledge: false, adm_g1: 0, adm_short: false, adm_qboost: false, adm_flatb: false, adm_rboost: false, adm_pdet: false, adm_sensors: false, adm_room_k: 2, adm_latch_reset: false, adm_xq: 0 }
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
        // (the escapement tail's cells and controls)
        lay.fifo.iter_mut().for_each(|(g, p)| {
            b(g);
            c(p);
        });
        lay.esc.iter_mut().for_each(b);
        lay.esc_p.iter_mut().for_each(b);
        lay.esc_door.iter_mut().for_each(b);
        lay.esc_gb.iter_mut().for_each(b);
        lay.esc_g0.iter_mut().for_each(b);
        lay.esc_g1.iter_mut().for_each(b);
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
/// The entry hold's per-slot record (`run_loop`, `adm_sensors` designs).
#[derive(Clone, Debug, Default)]
struct SlotHold {
    /// The cart-evidence latch's longest set time (gt).
    latch_max: i64,
    /// The landing detector's longest unbroken on time (gt).
    land_on_max: i64,
    /// The room detector at REL: -1 on, else gt since it was last on (`i64::MAX`: never).
    room_slack: Option<i64>,
    land_edges: usize,
    next_edges: usize,
    /// Carts crossing the landing flat's centre forward.
    crossings: usize,
    /// The progress input at REL: carts that left slot 0 forward since the last release.
    progress: Option<usize>,
}

/// Admission control's R: the slope under G_P into slot 0, from P's rail cell (`esc_adm_a1`).
pub fn adm_r_cell(a1: [i32; 3]) -> [i32; 3] {
    [a1[0], a1[1] - 1, a1[2] + 1]
}

/// The build's ground under the escapement tail (ironspec Nov 11 03:23: "real ground under the
/// ledge, so a derail shows the build's actual outcome"): a stone floor one below the design's
/// lowest block, under the tail's column (x −2..2, z 18..35, moved by dx, dz), where nothing is.
pub fn with_tail_ground(spec: &FarmSpec, o: &LoopOpts, dx: i32, dz: i32) -> FarmSpec {
    let mut s = spec.clone();
    if o.tail_esc == 0 {
        return s;
    }
    let Some(y0) = spec.blocks.iter().map(|(p, _)| p.y).min() else { return s };
    let taken: std::collections::HashSet<(i32, i32, i32)> = spec.blocks.iter().map(|(p, _)| (p.x, p.y, p.z)).collect();
    if let Ok(stone) = Block::parse("minecraft:stone") {
        for x in -2..=2 {
            for z in 18..=35 {
                let p = (x + dx, y0 - 1, z + dz);
                if !taken.contains(&p) {
                    s.blocks.push((BlockPos::new(p.0, p.1, p.2), stone.clone()));
                }
            }
        }
    }
    s
}

/// A rider's walk, the brief's pressed-cart print (tick-exact Nov 11: a rider whose walk has a
/// reachable path pushes the cart next to it; the reviewer: MoveToTargetSink also runs on a
/// partial path): `Some(0)` a walk target with a path that reaches it, `Some(1)` a partial path,
/// `Some(2)` a walk target with no path; `None` no walk target.
pub fn rider_walk_kind(w: &redsim::world::World, rider: u32) -> Option<u8> {
    let r = w.entities.list.iter().find(|e| e.id == rider)?;
    let redsim::entity::EntityKind::Mob(m) = &r.kind else { return None };
    let ex = m.brain.as_ref().and_then(|b| b.ex.as_ref())?;
    ex.mem.walk_target?;
    Some(match ex.mem.path.as_ref() {
        Some(p) if p.can_reach() => 0,
        Some(_) => 1,
        None => 2,
    })
}

/// A rider with a walk target whose path can reach it (`rider_walk_kind` 0).
pub fn rider_walking(w: &redsim::world::World, rider: u32) -> bool {
    rider_walk_kind(w, rider) == Some(0)
}

/// Does the rider's box touch another cart's box (tick-exact Nov 11, scene rider-push: a walking
/// rider moves a cart only through contact, whatever its path; pressed slope pairs sit 0.21
/// short and never move)?
pub fn rider_touches_cart(w: &redsim::world::World, rider: u32, own: u32) -> bool {
    rider_touch(w, rider, own).is_some()
}

/// The cart a rider's box touches (`rider_touches_cart`): its z and the overlap along z.
pub fn rider_touch(w: &redsim::world::World, rider: u32, own: u32) -> Option<(f64, f64)> {
    let r = w.entities.list.iter().find(|e| e.id == rider)?;
    w.entities.list.iter().find(|o| o.id != own && o.id != rider && !o.removed && matches!(o.kind, redsim::entity::EntityKind::Minecart(_)) && o.bb.intersects(r.bb)).map(|o| (o.pos.z, (o.bb.max_z.min(r.bb.max_z) - o.bb.min_z.max(r.bb.min_z))))
}

/// The detector rails' on-edges against the carts that entered each one's search box (the game's
/// AABB(x+0.2, y, z+0.2, x+0.8, y+0.8, z+0.8)): a cart entering within the 20-gt recheck of the
/// one before gives no on-edge of its own, and every latch set by on-edges depends on that not
/// happening (ironspec Nov 11 03:57: a per-detector line in every runner, equal in normal flow).
#[derive(Default)]
pub struct DetectorEdges {
    cells: Vec<BlockPos>,
    on: Vec<bool>,
    edges: Vec<usize>,
    entries: Vec<usize>,
    /// Per detector, the carts inside its search box last tick.
    inside: Vec<std::collections::HashSet<u32>>,
    /// Per detector, the game time it last turned off.
    last_off: Vec<Option<i64>>,
    /// Per detector, the shortest time from its turning off to the next cart's entry (the edge
    /// margin: the next cart's own on-edge needs the detector off before it enters).
    min_gap: Vec<Option<i64>>,
    /// Per detector, the carts that entered while it was still on from the one before (merged).
    merged: Vec<usize>,
}

impl DetectorEdges {
    pub fn new(cells: Vec<BlockPos>) -> DetectorEdges {
        let n = cells.len();
        DetectorEdges { cells, on: vec![false; n], edges: vec![0; n], entries: vec![0; n], inside: vec![Default::default(); n], last_off: vec![None; n], min_gap: vec![None; n], merged: vec![0; n] }
    }

    /// One tick: each detector's power and the carts in its search box.
    pub fn observe(&mut self, w: &redsim::world::World, carts: &[u32]) {
        for (i, p) in self.cells.iter().enumerate() {
            let on = w.get(*p).to_string().contains("powered=true");
            let was_on = self.on[i];
            if on && !was_on {
                self.edges[i] += 1;
            }
            if !on && was_on {
                self.last_off[i] = Some(w.time);
            }
            self.on[i] = on;
            let mut now = std::collections::HashSet::new();
            for e in w.entities.list.iter().filter(|e| carts.contains(&e.id)) {
                let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
                let hit = x + 0.49 > p.x as f64 + 0.2 && x - 0.49 < p.x as f64 + 0.8 && z + 0.49 > p.z as f64 + 0.2 && z - 0.49 < p.z as f64 + 0.8 && y + 0.7 > p.y as f64 && y < p.y as f64 + 0.8;
                if hit {
                    now.insert(e.id);
                    if !self.inside[i].contains(&e.id) {
                        self.entries[i] += 1;
                        if was_on && self.inside[i].is_empty() {
                            self.merged[i] += 1;
                        } else if let Some(off) = self.last_off[i].filter(|_| !was_on) {
                            let g = w.time - off;
                            self.min_gap[i] = Some(self.min_gap[i].map_or(g, |m| m.min(g)));
                        }
                    }
                }
            }
            self.inside[i] = now;
        }
    }

    /// The timing line (the advance-spacing measurement, ironspec 2026-11-11 04:15): each
    /// detector's shortest gap from turning off to the next cart's entry, and the carts that entered
    /// while it was still on from the one before.
    pub fn timing_line(&self, dx: i32, dz: i32) -> String {
        format!(
            "detector timing (off -> next cart's entry, the edge margin; merged = entered while still on): {}",
            self.cells.iter().enumerate().map(|(i, p)| format!("{},{},{} min gap {} merged {}", p.x - dx, p.y, p.z - dz, self.min_gap[i].map_or("-".to_string(), |g| format!("{g} gt")), self.merged[i])).collect::<Vec<_>>().join(", ")
        )
    }

    /// The line: each detector's on-edges against cart entries, unequal ones flagged.
    pub fn line(&self, dx: i32, dz: i32) -> String {
        let short = self.cells.iter().enumerate().filter(|(i, _)| self.edges[*i] < self.entries[*i]).count();
        format!(
            "detector edges against carts entering (equal in normal flow for every edge-latched detector): {} detectors, {short} short{}",
            self.cells.len(),
            if self.cells.is_empty() { String::new() } else { format!(": {}", self.cells.iter().enumerate().map(|(i, p)| format!("{},{},{} {}:{}{}", p.x - dx, p.y, p.z - dz, self.edges[i], self.entries[i], if self.edges[i] < self.entries[i] { " SHORT" } else { "" })).collect::<Vec<_>>().join(", ")) }
        )
    }
}

/// Train gaps at a plane (the stand's reference cell, 51b2aacd 2026-11-11 05:28: each cart's centre
/// crossing loop x 0.5 eastward on the strip, z 0.5, y about 0; ironspec 05:28: in the shared
/// runner, min/max per cart pair over the run, a pair a gt off its normal flagged with its slot). A
/// crossing's tick is interpolated from the cart's positions on either side of the plane; crossings
/// within 50 gt of the one before are one train.
pub struct PlaneGaps {
    x: f64,
    y: f64,
    z: f64,
    last: std::collections::HashMap<u32, f64>,
    /// (game time of the crossing, slot, cart)
    crossings: Vec<(f64, i64, u32)>,
}

impl PlaneGaps {
    pub fn new(x: f64, y: f64, z: f64) -> PlaneGaps {
        PlaneGaps { x, y, z, last: Default::default(), crossings: Vec::new() }
    }

    /// One tick (after it): every cart's centre against the plane.
    pub fn observe(&mut self, w: &redsim::world::World, carts: &[u32], slot: i64) {
        for e in w.entities.list.iter().filter(|e| carts.contains(&e.id) && !e.removed) {
            let px = e.pos.x;
            if let Some(&lx) = self.last.get(&e.id) {
                if lx < self.x && px >= self.x && (e.pos.z - self.z).abs() < 0.5 && (e.pos.y - self.y).abs() < 1.0 {
                    let f = (self.x - lx) / (px - lx);
                    self.crossings.push((w.time as f64 - 1.0 + f, slot, e.id));
                }
            }
            self.last.insert(e.id, px);
        }
    }

    /// The line: per pair (1->2, 2->3, .., the last of a train to the first of the next), min..max
    /// over the run and its normal (the median), every gap 1 gt or more off it flagged with its slot.
    pub fn line(&self, dx: i32, dz: i32) -> String {
        let mut trains: Vec<Vec<(f64, i64)>> = Vec::new();
        for &(t, s, _) in &self.crossings {
            match trains.last_mut() {
                Some(tr) if t - tr.last().unwrap().0 < 30.0 => tr.push((t, s)),
                _ => trains.push(vec![(t, s)]),
            }
        }
        // pairs: index i (0 = 1->2, ..) inside trains; "train" between trains
        let mut pairs: std::collections::BTreeMap<String, Vec<(f64, i64)>> = Default::default();
        for (k, tr) in trains.iter().enumerate() {
            for (i, w) in tr.windows(2).enumerate() {
                pairs.entry(format!("{}->{}", i + 1, i + 2)).or_default().push((w[1].0 - w[0].0, w[1].1));
            }
            if let Some(next) = trains.get(k + 1) {
                pairs.entry("train->train".into()).or_default().push((next[0].0 - tr.last().unwrap().0, next[0].1));
            }
        }
        let mut sizes: std::collections::BTreeMap<usize, usize> = Default::default();
        for tr in &trains {
            *sizes.entry(tr.len()).or_default() += 1;
        }
        let body: Vec<String> = pairs
            .iter()
            .map(|(name, v)| {
                let mut g: Vec<f64> = v.iter().map(|x| x.0).collect();
                g.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let med = g[g.len() / 2];
                let off: Vec<String> = v.iter().filter(|x| (x.0 - med).abs() >= 1.0).map(|x| format!("slot {} {:.1}", x.1, x.0)).collect();
                format!(
                    "{name} {:.1}..{:.1} (normal {:.1}, {} of {}{}{})",
                    g[0],
                    g[g.len() - 1],
                    med,
                    off.len(),
                    g.len(),
                    if off.is_empty() { " within 1 gt" } else { " a gt or more off: " },
                    off.iter().take(6).cloned().collect::<Vec<_>>().join(", ")
                )
            })
            .collect();
        format!(
            "train gaps at the plane x {:.1} (z {:.1}, y {:.1}; crossings eastward, gt): {} trains ({}){}",
            self.x - dx as f64,
            self.z - dz as f64,
            self.y,
            trains.len(),
            sizes.iter().map(|(n, c)| format!("{n}x{c}")).collect::<Vec<_>>().join(" "),
            if body.is_empty() { String::new() } else { format!("; {}", body.join("; ")) }
        )
    }
}

/// The stand batch's panic on the slot ticks (ironspec 2026-11-11 05:49: the measure for any change
/// to approach timing): on each gameTime % 100 == 0 tick, the riders at rest in the stand's pocket
/// row (loop z 0.5, y below 1, x -1..9) and how many of them panic; counted with 3 there.
pub struct StandPanic {
    fx: f64,
    fz: f64,
    /// Slots with 0, 1, 2, 3 of the 3 panicking.
    counts: [usize; 4],
    /// The slots with fewer than 3 (slot, panicking), the first few.
    short: Vec<(i64, usize)>,
    /// Per villager (its entity id): slot ticks it sat at rest in the pocket row, and how many of
    /// them it was panicking (the forced-phase sweep reads the forced villager's: its id after the
    /// load is a new number, so a rider is any seated villager, not the start's rider list).
    per: std::collections::BTreeMap<u32, (u32, u32)>,
}

impl StandPanic {
    pub fn new(fx: i32, fz: i32) -> StandPanic {
        StandPanic { fx: fx as f64, fz: fz as f64, counts: [0; 4], short: Vec::new(), per: Default::default() }
    }

    pub fn observe(&mut self, w: &redsim::world::World, riders: &[u32], slot: i64, dt: i64) {
        // (the runner's slot start: where it fires the golem input, the farm's %100 tick)
        if dt != 0 {
            return;
        }
        let mut at = Vec::new();
        for (i, e) in w.entities.list.iter().enumerate() {
            if e.removed || !(riders.contains(&e.id) || e.vehicle.is_some()) {
                continue;
            }
            let Some(p) = crate::timeline::panicking(w, i) else { continue };
            let (x, y, z) = (e.pos.x - self.fx, e.pos.y, e.pos.z - self.fz);
            let still = e.delta_movement.x.abs() + e.delta_movement.z.abs() < 0.01;
            if still && (z - 0.5).abs() < 0.3 && y < 1.0 && x > -1.0 && x < 9.0 {
                at.push(p);
                let s = self.per.entry(e.id).or_default();
                s.0 += 1;
                s.1 += p as u32;
            }
        }
        if at.len() == 3 {
            let p = at.iter().filter(|b| **b).count();
            self.counts[p] += 1;
            if p < 3 && self.short.len() < 8 {
                self.short.push((slot, p));
            }
        }
    }

    pub fn line(&self) -> String {
        let n: usize = self.counts.iter().sum();
        format!(
            "stand batch panicking on the slot tick (3 at rest): 3 in {}, 2 in {}, 1 in {}, 0 in {} of {n} slots{}; per villager (panicking of slot ticks at the stand): {}",
            self.counts[3],
            self.counts[2],
            self.counts[1],
            self.counts[0],
            if self.short.is_empty() { String::new() } else { format!(" (short: {})", self.short.iter().map(|(s, p)| format!("slot {s} {p}")).collect::<Vec<_>>().join(", ")) },
            if self.per.is_empty() { "-".to_string() } else { self.per.iter().map(|(id, (seen, p))| format!("e{id} {p}/{seen}")).collect::<Vec<_>>().join(" ") }
        )
    }
}

/// The escapement tail's arrivals (ironspec Nov 11 05:58, R1's harm): every cart's position along
/// the tail's track (cells in travel order from the jump's landing to P), each tick. An impact is a
/// cart's speed along the track dropping by more than 0.04 b/t in one tick, or its forward motion
/// reversing, with another tail cart within 1.3 blocks ahead; an arrival is a cart's first impact
/// after it lands, printed with its free roll (cells from where it landed), the closing speed, the
/// cell and the met cart's state. Also every backward cell crossing (cause-free: the bar is none),
/// the front's readiness at each G0 opening (gt pressed at G0 before it opens; the bar is 2), the
/// jump's spacing (the bar is 5 gt) with the landing's clearance at each jump, and the queue's
/// occupancy downstream of the x-queue's curve and upstream of it.
#[derive(Default)]
pub struct TailWatch {
    cells: Vec<[i32; 3]>,
    /// The first cell downstream of the curve (the x-queue; 0 for the column queue).
    curve_idx: usize,
    g0: Option<BlockPos>,
    /// The front's rest centre (G0's cell, 0.12 short along the track).
    g0_rest: [f64; 3],
    g0_was_open: bool,
    /// Per cart last tick: track position, speed along the track, cell index.
    prev: std::collections::HashMap<u32, (f64, f64, usize)>,
    /// Carts landed and not yet met: the landing tick and track position.
    landed: std::collections::HashMap<u32, (i64, f64)>,
    pressed_since: std::collections::HashMap<u32, i64>,
    /// G0 openings whose front was not pressed yet: (opening tick, label, that front's id).
    pending: Vec<(i64, String, u32)>,
    /// Openings with no cart in the queue, and openings whose front never reached G0.
    empty_openings: usize,
    not_admitted: usize,
    /// Every G0 opening (tick, label), and every landing (tick, cart, settled tick, label).
    opening_ts: Vec<(i64, String)>,
    landings: Vec<(i64, u32, Option<i64>, String)>,
    /// Landed carts not yet settled: (landing tick, ticks in a row under 0.01 b/t).
    await_settle: std::collections::HashMap<u32, (i64, u32)>,
    /// Each G0 opening's front: (cart, opening tick, gt pressed at G0 before it (negative: late),
    /// label).
    admits: Vec<(u32, i64, i64, String)>,
    /// The carts in an impact this tick (the harm line's split).
    pub impacted: std::collections::HashSet<u32>,
    arrivals: usize,
    roll_min: f64,
    roll_max: f64,
    closing_max: (f64, String),
    by_state: [usize; 4],
    arrival_log: Vec<String>,
    impacts: usize,
    back_cross: usize,
    back_log: Vec<String>,
    openings: usize,
    front_min: Option<(i64, String)>,
    front_short: Vec<String>,
    front_short_n: usize,
    jumps: Vec<i64>,
    landing_busy: usize,
    landing_log: Vec<String>,
    occ_down: (usize, String),
    occ_up: (usize, String),
    /// Every arrival printed (`--arrival-log`).
    pub verbose: bool,
}

impl TailWatch {
    pub fn new(lay: &LoopLayout) -> TailWatch {
        let mut w = TailWatch { roll_min: f64::INFINITY, ..Default::default() };
        let (Some(fl), Some(a1), Some(g0)) = (lay.flights.first(), lay.esc_adm_a1, lay.esc_g0) else {
            return w;
        };
        let Some(i0) = lay.path.iter().position(|(c, _)| *c == fl.landing[0]) else {
            return w;
        };
        let Some(i1) = lay.path.iter().skip(i0).position(|(c, _)| *c == a1).map(|k| i0 + k) else {
            return w;
        };
        w.cells = lay.path[i0..=i1].iter().map(|(c, _)| *c).collect();
        // (the x-queue's curve: the first cell after a change of direction)
        w.curve_idx = (1..w.cells.len().saturating_sub(1)).find(|&k| {
            let (a, b, c) = (w.cells[k - 1], w.cells[k], w.cells[k + 1]);
            (b[0] - a[0], b[2] - a[2]) != (c[0] - b[0], c[2] - b[2])
        }).map_or(0, |k| k + 2);
        w.g0 = Some(g0);
        w.g0_rest = [g0.x as f64 + 0.5, g0.y as f64 + 0.18, g0.z as f64 - 0.12];
        w
    }

    /// The cell index and the track position of a point (None: not on the tail's cells).
    fn track(&self, x: f64, y: f64, z: f64) -> Option<(usize, f64)> {
        let i = self.cells.iter().position(|c| x.floor() as i32 == c[0] && z.floor() as i32 == c[2] && y - c[1] as f64 > -0.6 && y - (c[1] as f64) < 1.6)?;
        let n = self.cells.len();
        let (a, b) = if i + 1 < n { (self.cells[i], self.cells[i + 1]) } else { (self.cells[i - 1], self.cells[i]) };
        let (dx, dz) = ((b[0] - a[0]).signum() as f64, (b[2] - a[2]).signum() as f64);
        let c = self.cells[i];
        let f = ((x - (c[0] as f64 + 0.5)) * dx + (z - (c[2] as f64 + 0.5)) * dz + 0.5).clamp(0.0, 0.999);
        Some((i, i as f64 + f))
    }

    /// One tick, after the world's.
    pub fn observe(&mut self, w: &redsim::world::World, carts: &[u32], t: i64, c: i64, dt: i64) {
        self.impacted.clear();
        if self.cells.is_empty() {
            return;
        }
        let n = self.cells.len();
        let p_idx = n - 1;
        // (this tick's tail carts: id, cart index, cell, track position, speed along the track, speed)
        let mut now: Vec<(u32, usize, usize, f64, f64, f64)> = Vec::new();
        for (k, id) in carts.iter().enumerate() {
            let Some(e) = w.entities.list.iter().find(|e| e.id == *id) else { continue };
            let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
            let v = e.delta_movement;
            let Some((i, s)) = self.track(x, y, z) else {
                self.prev.insert(*id, (f64::NAN, 0.0, usize::MAX));
                continue;
            };
            let (a, b) = if i + 1 < n { (self.cells[i], self.cells[i + 1]) } else { (self.cells[i - 1], self.cells[i]) };
            let (dx, dz) = ((b[0] - a[0]).signum() as f64, (b[2] - a[2]).signum() as f64);
            let vs = v.x * dx + v.z * dz;
            now.push((*id, k, i, s, vs, v.horizontal_distance()));
        }
        now.sort_by(|a, b| a.3.total_cmp(&b.3));
        // (the occupancy: downstream of the curve (not P), upstream of it)
        let down = now.iter().filter(|q| q.2 >= self.curve_idx && q.2 < p_idx).count();
        let up = now.iter().filter(|q| q.2 < self.curve_idx).count();
        if down > self.occ_down.0 {
            self.occ_down = (down, format!("slot {c} +{dt}"));
        }
        if up > self.occ_up.0 {
            self.occ_up = (up, format!("slot {c} +{dt}"));
        }
        // (pressed at G0: at the front's rest, under 0.01 b/t)
        let g = self.g0_rest;
        for q in &now {
            let e = w.entities.list.iter().find(|e| e.id == q.0).unwrap();
            let at = (e.pos.x - g[0]).abs() < 0.3 && (e.pos.z - g[2]).abs() < 0.06 && (e.pos.y - g[1]).abs() < 0.3;
            if at && q.5 < 0.01 {
                self.pressed_since.entry(q.0).or_insert(t);
            } else {
                self.pressed_since.remove(&q.0);
            }
        }
        // (an opening whose front was not pressed at it: that front's own arrival at G0's rest,
        // at any speed, or onto P, decides the margin (negative: late); not there within 40 gt:
        // not admitted (ironspec 06:14: the margin only for openings that admit a cart, the
        // rest counted apart))
        let mut keep = Vec::new();
        for (t_open, label, fid) in std::mem::take(&mut self.pending) {
            let there = now.iter().find(|q| q.0 == fid).is_some_and(|q| {
                let e = w.entities.list.iter().find(|e| e.id == q.0).unwrap();
                q.2 == p_idx || ((e.pos.x - g[0]).abs() < 0.3 && (e.pos.z - g[2]).abs() < 0.06 && (e.pos.y - g[1]).abs() < 0.3)
            });
            if there {
                let m = -(t - t_open);
                self.admits.push((fid, t_open, m, label.clone()));
                if self.front_min.as_ref().is_none_or(|f| m < f.0) {
                    self.front_min = Some((m, label.clone()));
                }
                self.front_short_n += 1;
                if self.front_short.len() < 6 {
                    self.front_short.push(format!("{label}: the front reached G0 {} gt after it opened", -m));
                }
            } else if t - t_open > 40 {
                self.not_admitted += 1;
                if self.front_short.len() < 6 {
                    self.front_short.push(format!("{label}: the front not at G0 within 40 gt"));
                }
            } else {
                keep.push((t_open, label, fid));
            }
        }
        self.pending = keep;
        // (the front's readiness at each G0 opening)
        if let Some(g0) = self.g0 {
            let open = matches!(w.get(g0), Block::FenceGate { open: true, .. });
            if open && !self.g0_was_open {
                self.openings += 1;
                let label = format!("slot {c} +{dt}");
                self.opening_ts.push((t, label.clone()));
                let front = now.iter().filter(|q| q.2 < p_idx).max_by(|a, b| a.3.total_cmp(&b.3));
                match front {
                    Some(q) if self.pressed_since.contains_key(&q.0) => {
                        let m = t - self.pressed_since[&q.0];
                        self.admits.push((q.0, t, m, label.clone()));
                        if self.front_min.as_ref().is_none_or(|f| m < f.0) {
                            self.front_min = Some((m, label.clone()));
                        }
                        if m < 2 {
                            self.front_short_n += 1;
                            if self.front_short.len() < 6 {
                                self.front_short.push(format!("{label}: the front pressed {m} gt before"));
                            }
                        }
                    }
                    Some(q) => self.pending.push((t, label, q.0)),
                    None => self.empty_openings += 1,
                }
            }
            self.g0_was_open = open;
        }
        // (the batch's settling: a landed cart's first 3 ticks in a row under 0.01 b/t)
        for q in &now {
            if let Some(st) = self.await_settle.get_mut(&q.0) {
                if q.5 < 0.01 {
                    st.1 += 1;
                    if st.1 >= 3 {
                        let landed = st.0;
                        self.await_settle.remove(&q.0);
                        if let Some(l) = self.landings.iter_mut().rev().find(|l| l.1 == q.0 && l.0 == landed) {
                            l.2 = Some(t - 2);
                        }
                    }
                } else {
                    st.1 = 0;
                }
            }
        }
        // (impacts, arrivals and backward crossings, against last tick)
        for (j, q) in now.iter().enumerate() {
            let (id, k, i, s, vs, _) = *q;
            match self.prev.get(&id).copied() {
                Some((_, pvs, pi)) if pi != usize::MAX => {
                    if i < pi {
                        self.back_cross += 1;
                        if self.back_log.len() < 6 {
                            self.back_log.push(format!("slot {c} +{dt}: cart {k} {:?} back to {:?}", self.cells[pi], self.cells[i]));
                        }
                    }
                    let ahead = now.get(j + 1).copied();
                    let dropped = pvs - vs > 0.04 || (pvs > 0.02 && vs < -0.005);
                    if dropped && let Some(b) = ahead {
                        let eb = w.entities.list.iter().find(|e| e.id == b.0).unwrap();
                        let ea = w.entities.list.iter().find(|e| e.id == id).unwrap();
                        let d = ((ea.pos.x - eb.pos.x).powi(2) + (ea.pos.y - eb.pos.y).powi(2) + (ea.pos.z - eb.pos.z).powi(2)).sqrt();
                        if d < 1.3 {
                            self.impacts += 1;
                            self.impacted.insert(id);
                            self.impacted.insert(b.0);
                            let pb = self.prev.get(&b.0).map_or(0.0, |p| p.1);
                            let closing = pvs - pb;
                            if closing > self.closing_max.0 {
                                self.closing_max = (closing, format!("slot {c} +{dt}: cart {k} into cart {} at {:?}", b.1, self.cells[i]));
                            }
                            if let Some((t_land, s_land)) = self.landed.remove(&id) {
                                self.arrivals += 1;
                                let roll = s - s_land;
                                self.roll_min = self.roll_min.min(roll);
                                self.roll_max = self.roll_max.max(roll);
                                let st = if self.pressed_since.contains_key(&b.0) { 0 } else if b.2 == p_idx { 3 } else if b.4 > 0.01 { 1 } else { 2 };
                                self.by_state[st] += 1;
                                if self.verbose {
                                    self.arrival_log.push(format!(
                                        "arrival slot {c} +{dt}: cart {k} (landed {} gt before) met cart {} at {:?} after a free roll of {roll:.2} cells, closing {closing:.3} b/t; the met cart {}; the queue {} downstream, {} upstream",
                                        t - t_land, b.1, self.cells[i], ["pressed at G0", "rolling", "at rest short of G0", "on P"][st], down, up
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {
                    // (a cart entering the tail's cells: landed, if on the landing cells)
                    if i <= 1 {
                        self.landed.insert(id, (t, s));
                        // (the landing's clearance: any cart at rest on the landing cells)
                        let busy: Vec<usize> = now.iter().filter(|o| o.0 != id && o.2 <= 1 && o.5 < 0.01).map(|o| o.1).collect();
                        if !busy.is_empty() {
                            self.landing_busy += 1;
                            if self.landing_log.len() < 4 {
                                self.landing_log.push(format!("slot {c} +{dt}: cart {k} landed with carts {busy:?} at rest there"));
                            }
                        }
                        self.jumps.push(t);
                        self.landings.push((t, id, None, format!("slot {c} +{dt}")));
                        self.await_settle.insert(id, (t, 0));
                    }
                }
            }
        }
        for q in &now {
            self.prev.insert(q.0, (q.3, q.4, q.2));
        }
    }

    pub fn lines(&self) -> Vec<String> {
        if self.cells.is_empty() {
            return Vec::new();
        }
        let gaps: Vec<i64> = self.jumps.windows(2).map(|p| p[1] - p[0]).collect();
        let gmin = gaps.iter().min().copied().unwrap_or(0);
        let under = gaps.iter().filter(|g| **g < 5).count();
        let mut out = vec![
            format!(
                "the jump onto the landing (the bar: 5 gt apart): {} landings, the closest {gmin} gt apart, {under} under 5 gt; landings with a cart at rest on the landing cells: {}{}",
                self.jumps.len(), self.landing_busy, if self.landing_log.is_empty() { String::new() } else { format!(" ({})", self.landing_log.join(", ")) }
            ),
            format!(
                "arrivals (a landed cart's first impact): {} (met a cart pressed at G0 {}, rolling {}, at rest short of G0 {}, on P {}); the free roll {:.2}..{:.2} cells; the hardest impact {:.3} b/t closing{}; impacts in all {}",
                self.arrivals, self.by_state[0], self.by_state[1], self.by_state[2], self.by_state[3],
                if self.arrivals > 0 { self.roll_min } else { 0.0 }, self.roll_max, self.closing_max.0,
                if self.closing_max.1.is_empty() { String::new() } else { format!(" ({})", self.closing_max.1) }, self.impacts
            ),
            format!("carts pushed back across a cell boundary in the tail (the bar: 0, whatever the cause): {}{}", self.back_cross, if self.back_log.is_empty() { String::new() } else { format!(" ({})", self.back_log.join(", ")) }),
            format!(
                "the front at each G0 opening (gt pressed at G0 before it opens, negative when it reached G0 after; the bar: 2): {} openings, the least {}; {} under 2{}{}; {} with the queue empty, {} whose front never reached G0 within 40 gt",
                self.openings,
                self.front_min.as_ref().map_or("-".to_string(), |f| format!("{} gt ({})", f.0, f.1)),
                self.front_short_n,
                if self.front_short.is_empty() { String::new() } else { format!(" ({})", self.front_short.join(", ")) },
                if self.pending.is_empty() { String::new() } else { format!("; {} pending at the end", self.pending.len()) },
                self.empty_openings,
                self.not_admitted
            ),
            format!(
                "the queue's occupancy: at most {} carts downstream of the curve ({}) and {} upstream ({})",
                self.occ_down.0, self.occ_down.1, self.occ_up.0, if self.occ_up.1.is_empty() { "-" } else { &self.occ_up.1 }
            ),
        ];
        // (each batch against the openings that admit its carts, ironspec 06:14: every cart of a
        // batch pressed at G0 at least 2 gt before the first opening that can admit it, the one
        // that finds it the front; a batch is the landings less than 30 gt apart; per batch the
        // carts' rest times after the first landing, and each one's admitting opening)
        let mut batches: Vec<Vec<&(i64, u32, Option<i64>, String)>> = Vec::new();
        for l in &self.landings {
            match batches.last_mut() {
                Some(b) if l.0 - b.last().unwrap().0 < 30 => b.push(l),
                _ => batches.push(vec![l]),
            }
        }
        let mut least: Option<(i64, String)> = None;
        let mut under = 0usize;
        let mut unadmitted = 0usize;
        let mut blog: Vec<String> = Vec::new();
        for b in &batches {
            let t0 = b[0].0;
            let mut parts: Vec<String> = Vec::new();
            for l in b {
                let rest = l.2.map_or("never at rest".to_string(), |s| format!("at rest +{}", s - t0));
                match self.admits.iter().find(|a| a.0 == l.1 && a.1 > l.0) {
                    Some(a) => {
                        if least.as_ref().is_none_or(|x| a.2 < x.0) {
                            least = Some((a.2, format!("the batch landing {}, admitted {}", b[0].3, a.3)));
                        }
                        if a.2 < 2 {
                            under += 1;
                        }
                        parts.push(format!("{rest}, admitted {} ({} gt after the landing) pressed {} gt before", a.3, a.1 - t0, a.2));
                    }
                    None => {
                        unadmitted += 1;
                        parts.push(format!("{rest}, not admitted by the end"));
                    }
                }
            }
            if self.verbose {
                blog.push(format!("batch landing {}: {}", b[0].3, parts.join("; ")));
            }
        }
        out.push(format!(
            "each batch's carts at the openings that admit them (the bar: pressed at G0 2 gt before): {} batches, the least {}; {under} carts under 2; {unadmitted} not admitted by the end",
            batches.len(), least.as_ref().map_or("-".to_string(), |x| format!("{} gt ({})", x.0, x.1))
        ));
        out.extend(blog);
        out.extend(self.arrival_log.iter().cloned());
        out
    }
}

/// Two release traces (`MCTAS_RELEASE_TRACE` lines `t T c C dt D R bits | k:x,y,z ...`) compared on
/// the ticks both have: the release rails' power and the carts on the release run (the same carts,
/// every axis within `tol`); the first difference of each and the counts.
pub fn trace_diff(a: &str, b: &str, tol: f64) -> String {
    type Row = (String, String, Vec<(usize, [f64; 3])>);
    let parse = |s: &str| -> std::collections::BTreeMap<i64, Row> {
        let mut m = std::collections::BTreeMap::new();
        for line in s.lines() {
            let Some((head, carts)) = line.split_once('|') else { continue };
            let w: Vec<&str> = head.split_whitespace().collect();
            if w.len() < 8 || w[0] != "t" {
                continue;
            }
            let Ok(t) = w[1].parse::<i64>() else { continue };
            let label = format!("slot {} +{}", w[3], w[5]);
            let cs: Vec<(usize, [f64; 3])> = carts
                .split_whitespace()
                .filter_map(|c| {
                    let (k, p) = c.split_once(':')?;
                    let v: Vec<f64> = p.split(',').filter_map(|x| x.parse().ok()).collect();
                    (v.len() == 3).then(|| (k.parse().ok(), [v[0], v[1], v[2]])).and_then(|(k, p)| k.map(|k| (k, p)))
                })
                .collect();
            m.insert(t, (label, w[7].to_string(), cs));
        }
        m
    };
    let (ma, mb) = (parse(a), parse(b));
    let mut ticks = 0usize;
    let (mut pdiff, mut cdiff) = (0usize, 0usize);
    let (mut pfirst, mut cfirst): (Option<String>, Option<String>) = (None, None);
    let mut gap = 0.0f64;
    for (t, ra) in &ma {
        let Some(rb) = mb.get(t) else { continue };
        ticks += 1;
        if ra.1 != rb.1 {
            pdiff += 1;
            pfirst.get_or_insert_with(|| format!("t {t} ({}): rails {} against {}", ra.0, ra.1, rb.1));
        }
        let ka: Vec<usize> = ra.2.iter().map(|c| c.0).collect();
        let kb: Vec<usize> = rb.2.iter().map(|c| c.0).collect();
        let mut differs = ka != kb;
        for (ca, cb) in ra.2.iter().zip(rb.2.iter()) {
            let g = (0..3).map(|i| (ca.1[i] - cb.1[i]).abs()).fold(0.0, f64::max);
            if ca.0 == cb.0 {
                gap = gap.max(g);
            }
            if g > tol {
                differs = true;
            }
        }
        if differs {
            cdiff += 1;
            cfirst.get_or_insert_with(|| {
                let show = |r: &Row| r.2.iter().map(|c| format!("{}:{:.3},{:.3},{:.3}", c.0, c.1[0], c.1[1], c.1[2])).collect::<Vec<_>>().join(" ");
                format!("t {t} ({}): [{}] against [{}]", ra.0, show(ra), show(rb))
            });
        }
    }
    format!(
        "trace-diff: {ticks} ticks in both ({} and {} in each); the release rails' power differs on {pdiff}{}; the carts on the release run differ on {cdiff}{} (the largest gap between the same cart {gap:.4} b); {}",
        ma.len(),
        mb.len(),
        pfirst.map_or(String::new(), |f| format!(", first {f}")),
        cfirst.map_or(String::new(), |f| format!(", first {f}")),
        if pdiff == 0 && cdiff == 0 && ticks > 0 { "IDENTICAL" } else { "DIFFERENT" }
    )
}

/// A design file's own declared states and flights (its `declared:` and `flights:` header lines,
/// a pasted part's carried in by `mobfarm paste`), which the runners check on top of the ones
/// derived from the loop's options.
#[derive(Clone, Debug, Default)]
pub struct Headers {
    pub declared: Vec<crate::declared::Decl>,
    pub flights: Vec<crate::flight::Flight>,
    /// Cells over a rail the design allows a block in (`rider_space_ok:`).
    pub rider_space_ok: Vec<[i32; 3]>,
    /// The rider-space check's positive control (`--rider-space-control`): stones placed at these
    /// cells before the check.
    pub rider_space_control: Vec<[i32; 3]>,
    /// The declared sight stretches (`sight:`): riders on these rails keep every zombie within 8 in
    /// sight; `sight_ok:` the blocks allowed to stop a line.
    pub sight: Vec<([i32; 3], [i32; 3])>,
    pub sight_ok: Vec<[i32; 3]>,
}

/// The design's `declared:`, `flights:` and `rider_space_ok:` headers.
pub fn headers_of(meta: &std::collections::BTreeMap<String, String>) -> Result<Headers, String> {
    Ok(Headers {
        declared: match meta.get("declared") { Some(s) => crate::declared::parse(s)?, None => Vec::new() },
        flights: match meta.get("flights") { Some(s) => crate::flight::parse(s)?, None => Vec::new() },
        rider_space_ok: match meta.get("rider_space_ok") { Some(s) => crate::declared::parse_cells(s).map_err(|e| format!("rider_space_ok: {e}"))?, None => Vec::new() },
        rider_space_control: Vec::new(),
        sight: match meta.get("sight") { Some(s) => crate::declared::parse_boxes(s).map_err(|e| format!("sight: {e}"))?, None => Vec::new() },
        sight_ok: match meta.get("sight_ok") { Some(s) => crate::declared::parse_cells(s).map_err(|e| format!("sight_ok: {e}"))?, None => Vec::new() },
    })
}

/// The rider-space check on a run's world at tick 0: every rail block of the spec (moved by dx, dz),
/// the declared cells, the generator's own gates (the stand's pockets, its stop gate, the tail's)
/// and the header's `rider_space_ok:` allowed; the positive control's stones placed first.
pub fn rider_space_check(w: &mut redsim::world::World, spec: &FarmSpec, declared: &[crate::declared::Decl], hdr: &Headers, o: &LoopOpts, dx: i32, dz: i32) -> Vec<String> {
    if let Ok(stone) = Block::parse("minecraft:stone") {
        for c in &hdr.rider_space_control {
            w.set_block(BlockPos::new(c[0] + dx, c[1], c[2] + dz), stone.clone(), redsim::world::flags::ALL);
        }
    }
    let ok = &hdr.rider_space_ok;
    let lay = loop_layout(o);
    let gates: Vec<[i32; 3]> = lay.pockets.iter().map(|(g, _)| [g.x + dx, g.y, g.z + dz]).chain(lay.stop_gate.map(|g| [g.x + dx, g.y, g.z + dz])).chain(lay.fifo.iter().map(|(g, _)| [g.x + dx, g.y, g.z + dz])).collect();
    let rails: Vec<[i32; 3]> = spec.blocks.iter().filter(|(_, b)| redsim::rails::is_rail(b)).map(|(p, _)| [p.x, p.y, p.z]).collect();
    let allowed: std::collections::HashSet<[i32; 3]> = declared.iter().map(|d| d.cell).chain(gates).chain(ok.iter().map(|c| [c[0] + dx, c[1], c[2] + dz])).collect();
    let mut bad: Vec<String> = crate::declared::rider_space(w, &rails, &allowed).into_iter().map(|b| format!("rider space: {b}")).collect();
    // (the declared zombies' own cells, ironspec Nov 11 04:47: no suffocating or collision block
    // in a zombie's feet cell or its eye cell (eyes 1.74 over its feet), else it sees no rider and
    // panics none (day-1's B2 route: a support stone in the station zombie's head, no panic, no
    // golem); a zombie riding a boat or a cart sits at its vehicle's height)
    for (ty, p, nbt) in &spec.entities {
        let zombie = ty.contains("zombie") || nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie"));
        if !zombie || ty.contains("minecart") && !nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie")) {
            continue;
        }
        let (x, y, z) = (p[0].floor() as i32, p[1], p[2].floor() as i32);
        for cy in [y.floor() as i32, (y + 1.74).floor() as i32] {
            let c = [x, cy, z];
            if allowed.contains(&c) {
                continue;
            }
            let b = w.get(BlockPos::new(c[0], c[1], c[2]));
            if redsim::entity::mob::blocks::is_suffocating(&b) || !redsim::behavior::collision_boxes(&b).is_empty() {
                bad.push(format!("zombie space: {},{},{}: {b} in the zombie at ({:.2},{:.2},{:.2})", c[0], c[1], c[2], p[0], p[1], p[2]));
            }
        }
    }
    // (the sight lines: the declared stretches' riders against the declared zombies, ironspec Nov
    // 11 04:31; day-1's release-gen check with its eye heights)
    if !hdr.sight.is_empty() {
        let in_box = |p: &BlockPos| hdr.sight.iter().any(|(a, b)| p.x >= a[0] + dx && p.x <= b[0] + dx && p.y >= a[1] && p.y <= b[1] && p.z >= a[2] + dz && p.z <= b[2] + dz);
        let srails: Vec<[i32; 3]> = spec.blocks.iter().filter(|(p, b)| redsim::rails::is_rail(b) && in_box(p)).map(|(p, _)| [p.x, p.y, p.z]).collect();
        let zombies: Vec<[f64; 3]> = spec.entities.iter().filter(|(ty, _, nbt)| ty.contains("zombie") && !ty.contains("minecart") || nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie"))).map(|(_, p, _)| *p).collect();
        let sok: std::collections::HashSet<[i32; 3]> = hdr.sight_ok.iter().map(|c| [c[0] + dx, c[1], c[2] + dz]).collect();
        let (n, cuts) = crate::declared::sight_cuts(w, &srails, &zombies, &sok);
        if !cuts.is_empty() {
            bad.push(format!("sight lines: {} of {n} cut ({})", cuts.len(), cuts.iter().take(4).cloned().collect::<Vec<_>>().join("; ")));
        }
    }
    bad
}

/// The derived declared states with a header's added: an identical entry once, a cell declared two
/// different ways a mismatch of its own (returned with the merged list).
pub fn merge_declared(mut ds: Vec<crate::declared::Decl>, hdr: &[crate::declared::Decl]) -> (Vec<crate::declared::Decl>, Vec<String>) {
    let mut bad = Vec::new();
    for h in hdr {
        // (a control named on one side only is no conflict: the other side's older form)
        let same = |d: &crate::declared::Decl| d.cell == h.cell && d.id == h.id && d.props == h.props && d.always == h.always && (d.ctl.is_none() || h.ctl.is_none() || d.ctl == h.ctl);
        match ds.iter().position(|d| d.cell == h.cell) {
            Some(i) if same(&ds[i]) => {
                if ds[i].ctl.is_none() {
                    ds[i].ctl = h.ctl;
                }
                ds[i].unpowered |= h.unpowered;
            }
            Some(i) => bad.push(format!("{},{},{}: declared two ways: {} (options) and {} (header)", h.cell[0], h.cell[1], h.cell[2], crate::declared::format(std::slice::from_ref(&ds[i])), crate::declared::format(std::slice::from_ref(h)))),
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
    // (a braked rail's control block, for the runtime check: each register slot's, P's)
    let mut ctl_of: std::collections::HashMap<[i32; 3], [i32; 3]> = lay.slot_cells.iter().zip(lay.slots.iter()).map(|(r, c)| (*r, [c.x, c.y, c.z])).collect();
    if let (Some(a1), Some(pc), false) = (lay.esc_adm_a1, lay.esc_p, o.adm_pdet) {
        ctl_of.insert(a1, [pc.x, pc.y, pc.z]);
    }
    for (c, k) in &lay.path {
        ds.push(match k {
            Cell::Boost => Decl::new(*c, "powered_rail", &[("powered", "true")], true),
            Cell::Slot | Cell::Linked => {
                let d = Decl::new(*c, "powered_rail", &[("powered", "false")], false);
                match ctl_of.get(c) {
                    Some(ctl) => d.with_ctl(*ctl),
                    None => d,
                }
            }
            Cell::Plain => Decl::new(*c, "rail", &[], false),
            Cell::Detector => Decl::new(*c, "detector_rail", &[], false),
        });
    }
    let gate = |p: BlockPos, open: bool| Decl::new([p.x, p.y, p.z], "oak_fence_gate", &[("open", if open { "true" } else { "false" })], false).bench_set();
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
    // (`adm_rboost`: R braked at rest, its control glass)
    if let (true, Some(a1)) = (o.esc_adm && o.adm_rboost, lay.esc_adm_a1) {
        let r = adm_r_cell(a1);
        ds.retain(|d| d.cell != r);
        ds.push(Decl::new(r, "powered_rail", &[("powered", "false")], false).with_ctl([r[0] + 1, r[1], r[2]]));
        ds.push(Decl::new([r[0] + 1, r[1], r[2]], "glass", &[], false));
    }
    ds
}

/// The escapement tail's level (`tail_esc`): the return run's and the ledge's y (the queue of Q
/// slopes, B, X, P and R stepping down to slot 0 at y 0; 2 more with `ledge_slope`'s landing).
pub fn esc_ye(o: &LoopOpts) -> i32 {
    if o.esc_adm {
        // (slot 0 at y 0, R 1, P 2, G0's level 3, the Q queue slopes up to the curve at 3 + Q;
        // with the ledge the return run (the pre-ledge) one higher; with the x-queue (`adm_xq` N)
        // its N slopes and the drop over the gap on top)
        if o.adm_xq > 0 {
            return o.tail_esc as i32 + 3 + o.adm_xq as i32 + 1;
        }
        return o.tail_esc as i32 + 3 + if o.adm_ledge { 1 } else { 0 };
    }
    if o.esc_flat {
        // (slot 0 at y 0, R 1, P 2, Y 3, X 4, the flat B 5, then the Q queue slopes)
        return o.tail_esc as i32 + 5;
    }
    o.tail_esc as i32 + 6 + if o.ledge_slope && !o.tail_brake { if o.tail_esc >= 5 { 1 } else { 2 } } else { 0 }
}

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
    LoopLayout { slots: slot_cells.iter().map(ctrl).collect(), stand: stand_cells.iter().map(ctrl).collect(), stand_b, tail_b, slot_dir, slot_peak, path, slot_cells, stand_cells, stop_gate: None, pockets: Vec::new(), stand_rel: None, fifo: Vec::new(), esc: None, esc_p: None, esc_door: None, esc_gb: None, esc_g0: None, esc_adm_a1: None, esc_g1: None, esc_flat_b: None, queue_cells: Vec::new(), downgraded, flights: Vec::new() }
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
    // the ledge sits on the return run and the curve is the landing's level, one lower; with the
    // x-queue (`adm_xq` N, ironspec's R1, Nov 11 05:39) the return run turns west at z 24 instead:
    // take-off flats at y ye from x xd-1 down to N + 2, a gap at x N + 1, N plain slopes down west
    // (x N..1, the first the landing, a block below the take-off), the curve north (0, y_f, 24) and
    // one flat transition cell (0, y_f, 25), all upstream of the stand's forced chunk border z
    // 25|26; the gravity queue's slopes then go on north from z 26 into G0 and P as before
    let mut z = zr + 1;
    let xq_top = if o.adm_xq > 0 {
        let n = o.adm_xq as i32;
        let xd = -o.row_gap + 12;
        for x in (n + 2..xd).rev() {
            path.push(([x, ye, 24], Cell::Plain));
        }
        for k in 0..n {
            let x = n - k;
            let cell = [x, ye - 2 - k, 24];
            path.push((cell, if o.adm_sensors && k == 1 { Cell::Detector } else { Cell::Plain }));
        }
        let yf = ye - 1 - n;
        path.push(([0, yf, 24], Cell::Plain));
        path.push(([0, yf, 25], Cell::Plain));
        z = 26;
        Some(yf)
    } else {
        path.push(([0, ye, zr], Cell::Plain));
        None
    };
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
        let top = if let Some(yf) = xq_top {
            // (the x-queue's flight: off the last take-off flat over the gap onto the first or
            // second slope down, within 6 gt; its queue cells the x-queue's slopes)
            let n = o.adm_xq as i32;
            flights.push(crate::flight::Flight { take_off: [n + 2, ye, 24], landing: vec![[n, ye - 2, 24], [n - 1, ye - 3, 24]], max_air: 6 });
            for k in 0..n {
                queue_cells.push([n - k, ye - 2 - k, 24]);
            }
            yf
        } else if o.adm_ledge && o.adm_short {
            // (`adm_short`: the entry curve itself is the pre-ledge; the gap, then a one-cell
            // booster landing: two cells for two more queue slopes)
            path.push(([0, ye - 1, z + 1], Cell::Boost));
            z += 2;
            ye - 1
        } else if o.adm_ledge {
            path.push(([0, ye, z], Cell::Plain));
            path.push(([0, ye - 1, z + 2], Cell::Boost));
            // (`adm_sensors`: the plain landing flat a detector rail, "landing occupied", one of the
            // entry hold's two inputs (the stand's REL waits while it is on))
            path.push(([0, ye - 1, z + 3], if o.adm_sensors { Cell::Detector } else { Cell::Plain }));
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
            // (`adm_sensors`: the `adm_room_k`-th queue slope (default the third) a detector rail,
            // "no room": on while the queue's last cart reaches it; the entry hold's other input)
            path.push(([0, top - 1 - k, z], if o.adm_sensors && (k == o.adm_room_k as i32 || (o.adm_latch_reset && k == 0)) { Cell::Detector } else { Cell::Plain }));
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
        // (`adm_pdet`: P a sloped detector rail, sensing its cart, no booster)
        path.push(([0, lg - 1, pz], if o.adm_pdet { Cell::Detector } else { Cell::Linked }));
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
    while z >= 1 {
        let k = if i < o.boost_first { Cell::Boost } else if o.detector && z == 4 { Cell::Detector } else { Cell::Plain };
        path.push(([xb, 1, z], k));
        z -= 1;
        i += 1;
    }
    // the curve into the stand, the catch cell before S3's slope
    let xs = xb + 2;
    path.push(([xb, 1, 0], Cell::Plain));
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
        for y in -3..top + lip {
            // (`climb_boost` N: only the climb's first N cells boosted, the rest plain slopes;
            // `climb_every` K: a booster on every Kth cell)
            let boosted = o.climb_boost.is_none_or(|n| (y + 3) < n as i32) && (o.climb_every <= 1 || (y + 3) % o.climb_every as i32 == 0);
            path.push(([xd, y, z], if boosted { Cell::Boost } else { Cell::Plain }));
            z += 1;
        }
        if lip > 0 {
            path.push(([xd, top + lip, z], Cell::Plain));
            z += 1;
            for k in 1..=lip {
                path.push(([xd, top + lip - k, z], Cell::Plain));
                z += 1;
            }
        }
        let run_start = z;
        // (`adm_xq`: the run goes on north to z 24 and turns west there into the x-queue's take-off;
        // the tail lays the rest)
        let zq = if o.adm_xq > 0 { 24 } else { zr };
        while z < zq {
            // (the guard: the run's first cell a booster, so the stand's departures, slower out of
            // the guard's braked gate slopes, reach the tail as fast as before)
            path.push(([xd, top, z], if o.stand_guard && z == run_start { Cell::Boost } else { Cell::Plain }));
            z += 1;
        }
        path.push(([xd, top, zq], Cell::Plain));
        if o.adm_xq == 0 {
            for x in (1..xd).rev() {
                path.push(([x, top, zr], Cell::Plain));
            }
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
    // (`adm_rboost`: R, the slope under G_P into slot 0, a powered rail (braked between the
    // launch pulses), its control a glass block beside it (east), set to a redstone block by the
    // bench for `adm_rpulse` gt from each G_P opening)
    if let (true, Some(a1)) = (o.esc_adm && o.adm_rboost, lay.esc_adm_a1) {
        let r = adm_r_cell(a1);
        let cur = b.d.blocks.get(&r).cloned().unwrap_or_default();
        let Some(shape) = cur.strip_prefix("minecraft:rail[").or_else(|| cur.strip_prefix("rail[")) else { bail!("adm_rboost: R at {r:?} is {cur}, not a plain rail") };
        let shape = shape.split(',').find(|t| t.starts_with("shape=")).unwrap_or("shape=north_south").trim_end_matches(']').to_string();
        b.set(r[0], r[1], r[2], &format!("powered_rail[powered=false,{shape},waterlogged=false]"));
        let ctl = [r[0] + 1, r[1], r[2]];
        if b.d.blocks.contains_key(&ctl) {
            bail!("adm_rboost: the cell east of R at {ctl:?} is taken");
        }
        b.set(ctl[0], ctl[1], ctl[2], "glass");
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
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("queue".into(), format!("scene=Loop carts=21 spacing=3 release=3 riders=1 slots={} run={} depart={} boost={} boost_first={} straight={} drop={} vstand={} tail_v={} bridge={} row_gap={} uturn_boost={} line={} tail_deep={} plain={} stand_desc={} detector={} zombie_every={} zombie_x={} stand_deep={} tail_desc={} peak_pad={} short_depart={} stand_stop={}{}{}", o.slots, o.run, o.depart, o.boost_every, o.boost_first, o.straight as i32, o.drop as i32, o.vstand as i32, o.tail_v as i32, o.bridge as i32, o.row_gap, o.uturn_boost as i32, o.line as i32, o.tail_deep as i32, o.plain as i32, o.stand_desc as i32, o.detector as i32, o.zombie_every, o.zombie_x, o.stand_deep as i32, o.tail_desc as i32, o.peak_pad as i32, o.short_depart as i32, o.stand_stop as i32, if o.stand_wired { if o.stand_guard { " stand_wired=1 stand_guard=1" } else { " stand_wired=1" } } else { "" }, (if o.tail_fifo > 0 { format!(" tail_fifo={}", o.tail_fifo) } else { String::new() }) + &(if o.tail_esc > 0 { format!(" tail_esc={}", o.tail_esc) } else { String::new() }) + &(o.climb_boost.map(|n| format!(" climb_boost={n}")).unwrap_or_default() + if o.tail_brake { " tail_brake=1" } else { "" } + if o.tail_plain { " tail_plain=1" } else { "" } + &(if o.climb_every > 0 { format!(" climb_every={}", o.climb_every) } else { String::new() }) + if o.ledge_slope { " ledge_slope=1" } else { "" } + if o.esc_door { " esc_door=1" } else { "" } + if o.esc_flat { " esc_flat=1" } else { "" } + if o.landing_boost2 { " landing_boost2=1" } else { "" } + if o.esc_g0 { " esc_g0=1" } else { "" } + if o.esc_adm { " esc_adm=1" } else { "" } + if o.adm_ledge { " adm_ledge=1" } else { "" } + &if o.adm_g1 > 0 { format!(" adm_g1={}", o.adm_g1) } else { String::new() } + if o.adm_short { " adm_short=1" } else { "" } + if o.adm_qboost { " adm_qboost=1" } else { "" } + if o.adm_flatb { " adm_flatb=1" } else { "" } + if o.adm_rboost { " adm_rboost=1" } else { "" } + if o.adm_pdet { " adm_pdet=1" } else { "" } + if o.adm_sensors { " adm_sensors=1" } else { "" } + &if o.adm_sensors && o.adm_room_k != 2 { format!(" adm_room_k={}", o.adm_room_k) } else { String::new() } + if o.adm_sensors && o.adm_latch_reset { " adm_latch_reset=1" } else { "" } + &if o.adm_xq > 0 { format!(" adm_xq={}", o.adm_xq) } else { String::new() })));
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
            "adm_rboost" => o.adm_rboost = v == "1",
            "adm_pdet" => o.adm_pdet = v == "1",
            "adm_sensors" => o.adm_sensors = v == "1",
            "adm_room_k" => o.adm_room_k = v.parse().unwrap_or(2),
            "adm_latch_reset" => o.adm_latch_reset = v == "1",
            "adm_xq" => o.adm_xq = v.parse().unwrap_or(0),
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
    /// With `wired`: the bench still drives the release group's 3 controls (a design without its
    /// release controller, against the wired one: the bench-against-wired check).
    pub bench_release: bool,
    /// Each release slope's off time (q15, q16, q17: the last 3 queue slots, gt into the slot),
    /// overriding the computed windows (day-1's new core at 24 gt: q15 closes with A1's pulse).
    pub release_offs: Option<[i64; 3]>,
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
    /// (`--watch-mob`, farmopt 2026-11-11) mobs printed every gt over slots
    /// `watch_mob_slots.0 ..= watch_mob_slots.1`: place, pose, vehicle, activities, behaviours,
    /// memories, target, and the hostiles within 16 blocks.
    pub watch_mob: [Option<RiderRef>; 4],
    pub watch_mob_slots: (i64, i64),
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
    /// (tickexact 2026-11-11, ironspec's forced-phase mode) After each whole-part load, these
    /// sensor delays forced on one villager (`--force-phase`).
    pub phase_force: Option<PhaseForce>,
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
    /// (tickexact 2026-11-11) A diagnostic: each exposed rider-tick (`freeze exposure:`) also
    /// computes, on a copy of the rider's navigation, whether a path from its cart reaches within 1
    /// of its bed (an open bed top's counterfactual).
    pub reach_probe: bool,
    /// (tickexact 2026-11-11) Every exposed rider-tick (`freeze exposure:`) as an `exposure ..`
    /// line (`--exposure-out FILE`): game time, slot, offset, rider, position, streak, panicked
    /// within 20 gt (1/0), its PATH (reaching / partial / none).
    pub exposure_log: bool,
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
    /// Print the arrival stream (`arrival c<C> +<DT> x<X> v<V>`: each cart crossing x 8.5
    /// westward on the escapement tail's return run), for tail-bench `--arrivals`.
    pub arrivals: bool,
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
    /// Admission faults (bench-driven): (slot, advance 0..2, kind, offset): kind 0 skips the
    /// advance's G_P and G0 pulses (the divert's hole), 1 misses its G0 pulse, 2 misses its G_P
    /// pulse, 3 adds a G0 pulse `offset` gt after G_P's opening, 4 adds a G_P pulse there.
    pub adm_faults: [Option<(i64, i64, u8, i64)>; 8],
    /// Admission control's skip requests (slot, dt): each sets a latch; the next G0 opening after
    /// it is skipped (a hole one advance later), which clears the latch.
    pub adm_skip_req: [Option<(i64, i64)>; 8],
    /// Admission control's R booster (`adm_rboost` designs): R's control on for this many gt from
    /// each G_P opening (0: never).
    pub adm_rpulse: i64,
    /// The entry hold (`adm_sensors` designs): the stand's release waits while either of the
    /// tail's detectors (landing, room) is on.
    pub entry_hold: bool,
    /// The entry hold's landing input counts only after this many gt on without a break (a cart
    /// at rest, not the batch passing; 0: plain on).
    pub hold_landing_rest: i64,
    /// The cart-evidence landing latch (`adm_latch_reset` designs): a strand once it has stayed
    /// set this many gt.
    pub hold_strand_t: i64,
    /// The entry hold's progress input: REL only if a cart left slot 0 forward since the last
    /// release.
    pub hold_progress: bool,
    /// The register's cart interlock, ideal sensors, bench-driven (ironspec Nov 11 04:38): an
    /// advance moves a slot's cart only into an empty slot (or one emptied in the same advance), the
    /// head releases only into an empty stand, the tail launches only into an empty slot 0.
    pub reg_interlock: bool,
    /// A level-33 freeze of the tail's chunk: its controls delayed by the freeze (a pure delay)
    /// instead of snapping to their schedule on the resume tick (the burst).
    pub l33_delay: bool,
    /// Every tail arrival printed (`TailWatch`): its free roll, closing speed, the met cart's state
    /// and the queue's occupancy.
    pub arrival_log: bool,
    /// The front metric's positive control (`--hold-back C,DT,N`): from slot C +DT for N gt the
    /// queue's second cart behind G0 is held where it is (its motion zeroed every tick).
    pub hold_back: Option<[i64; 3]>,
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
    /// (tickexact 2026-11-11, the reload-order rule) A whole-part unload's load order
    /// (`SLOT,DT,unload,N,ORDER`: `save`, `reverse` or `seed:K`; `reload::load_all_ordered`).
    pub order: redsim::reload::Order,
    /// (farmopt 2026-11-11, day-1's positive control for the unload id-remap check) a rider moved:
    /// `SLOT,DT,dismount,R` (R off its cart, to the cart's dismount spot) or `SLOT,DT,swap,R1,R2`
    /// (the two riders exchange carts). `cell` and `block` unused then.
    pub ride: Option<RideAct>,
}

/// A rider named by an act: a visit's napper (`visit:SLOT`, the visit that started at that slot,
/// by its current number: renumbered at a whole-part unload), the rider of that visit's batch's
/// J-th cart (`visit:SLOT/J`), or an entity by its current number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RiderRef {
    Visit(i64),
    Batch(i64, usize),
    Id(u32),
}

/// A rider act (`Act::ride`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RideAct {
    Dismount(RiderRef),
    Swap(RiderRef, RiderRef),
}

pub fn parse_rider(s: &str, t: &str) -> Result<RiderRef, String> {
    match t.strip_prefix("visit:") {
        Some(v) => match v.split_once('/') {
            Some((slot, j)) => Ok(RiderRef::Batch(slot.parse::<i64>().map_err(|e| format!("--act {s}: visit:SLOT/J: {e}"))?, j.parse::<usize>().map_err(|e| format!("--act {s}: visit:SLOT/J: {e}"))?)),
            None => v.parse::<i64>().map(RiderRef::Visit).map_err(|e| format!("--act {s}: visit:SLOT: {e}")),
        },
        None => t.parse::<u32>().map(RiderRef::Id).map_err(|e| format!("--act {s}: a rider is visit:SLOT, visit:SLOT/J or an entity number: {e}")),
    }
}

/// `--reload-orders` (ironspec's reload-order rule): `all:N` = N orders, save, reverse, then
/// seed:1 .. seed:N-2; `all` = `all:40` (ironspec 2026-11-11 05:12: wherever a sensor phase cannot
/// be forced, a reload row runs at least 40 samples with 0 failures, each order one sample of the
/// game's random reload (tick-exact), until tick-exact's forced-phase sweep lands); `seed:A-B` a
/// range, or one order (`save`, `reverse`, `seed:K`; K < 1000).
pub fn parse_reload_orders(names: &[String]) -> Result<Vec<redsim::reload::Order>, String> {
    use redsim::reload::Order;
    let mut orders = Vec::new();
    let bad = |n: &str| format!("--reload-orders {n}: save, reverse, seed:K, seed:A-B (K < 1000), all (40 orders) or all:N (N orders, 1 <= N <= 1000)");
    for n in names {
        if n == "all" || n.starts_with("all:") {
            let k = match n.strip_prefix("all:") {
                Some(k) => k.parse::<u32>().ok().filter(|k| (1..=1000).contains(k)).ok_or_else(|| bad(n))?,
                None => 40,
            };
            orders.extend([Order::Save, Order::Reverse].into_iter().chain((1..).map(Order::Seed)).take(k as usize));
        } else if let Some((a, b)) = n.strip_prefix("seed:").and_then(|r| r.split_once('-')) {
            let (a, b) = (a.parse::<u32>().map_err(|_| bad(n))?, b.parse::<u32>().map_err(|_| bad(n))?);
            if a > b || b >= 1000 {
                return Err(bad(n));
            }
            orders.extend((a..=b).map(Order::Seed));
        } else {
            orders.push(Order::parse(n).ok_or_else(|| bad(n))?);
        }
    }
    Ok(orders)
}

/// The Wilson score interval (95%) of `k` misses in `n` samples, as fractions.
pub fn wilson95(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (z, n, p) = (1.96f64, n as f64, k as f64 / n as f64);
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    ((c - h).max(0.0), (c + h).min(1.0))
}

/// (tickexact 2026-11-11, ironspec's forced-phase mode) A whole-part reload's forced sensor phases:
/// after each whole-part load, `who`'s listed sensors start at the given delays (the load's own
/// draws are still made, so every other draw stays as sampled; a sensor's delay is its
/// `timeToTick`, drawn `nextInt(scanRate)` from the villager's own random when its brain is built,
/// `villager::SENSORS`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseForce {
    pub who: RiderRef,
    /// (sensor index in `villager::SENSORS`, delay)
    pub sensors: [Option<(usize, i64)>; 4],
}

/// A villager sensor by its index or name in `villager::SENSORS` (`nearest_living_entities`,
/// `villager_hostiles`, ...; `living` and `hostiles` for the two that time a sleeper's wake).
pub fn sensor_index(t: &str) -> Option<usize> {
    let names = redsim::entity::mob::villager::SENSORS;
    match t {
        "living" => Some(0),
        "hostiles" => Some(5),
        _ => t.parse::<usize>().ok().filter(|k| *k < names.len()).or_else(|| names.iter().position(|(n, _)| *n == t)),
    }
}

/// Parses `WHO@SENSOR=DELAY[,SENSOR=DELAY...]` (WHO as an act's rider: `visit:SLOT`, `visit:SLOT/J`
/// or an entity number; at most 4 sensors; a delay below the sensor's scan rate).
pub fn parse_phase_force(s: &str) -> Result<PhaseForce, String> {
    let (who, rest) = s.split_once('@').ok_or_else(|| format!("--force-phase {s}: WHO@SENSOR=DELAY[,...]"))?;
    let who = parse_rider(s, who)?;
    let mut sensors = [None; 4];
    for (n, kv) in rest.split(',').enumerate() {
        anyhow_lite(n < 4, || format!("--force-phase {s}: at most 4 sensors"))?;
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("--force-phase {s}: SENSOR=DELAY"))?;
        let k = sensor_index(k).ok_or_else(|| format!("--force-phase {s}: no sensor {k} (an index or a name of villager::SENSORS, living, hostiles)"))?;
        let v: i64 = v.parse().map_err(|e| format!("--force-phase {s}: {e}"))?;
        let rate = redsim::entity::mob::villager::SENSORS[k].1;
        anyhow_lite((0..rate).contains(&v), || format!("--force-phase {s}: {} delays are 0..{}", redsim::entity::mob::villager::SENSORS[k].0, rate - 1))?;
        sensors[n] = Some((k, v));
    }
    Ok(PhaseForce { who, sensors })
}

fn anyhow_lite(ok: bool, why: impl FnOnce() -> String) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why()) }
}

/// Parses `SLOT,DT,X,Y,Z,BLOCK[,FOR]` (the block state may hold commas inside its brackets),
/// `SLOT,DT,unload,N[,ORDER]` (the whole part away N gt, its load in ORDER: save, reverse or
/// seed:K), `SLOT,DT,unload,X0,Z0,X1,Z1,N` (a chunk box), `SLOT,DT,dismount,R` or
/// `SLOT,DT,swap,R1,R2` (R: `visit:SLOT` or an entity number).
pub fn parse_act(s: &str) -> Result<Act, String> {
    let f: Vec<&str> = s.split(',').map(|t| t.trim()).collect();
    if f.len() >= 3 && (f[2] == "dismount" || f[2] == "swap") {
        let num = |t: &str, what: &str| t.parse::<i64>().map_err(|e| format!("--act {s}: {what}: {e}"));
        let (slot, dt) = (num(f[0], "SLOT")?, num(f[1], "DT")?);
        let ride = match (f[2], f.len()) {
            ("dismount", 4) => RideAct::Dismount(parse_rider(s, f[3])?),
            ("swap", 5) => RideAct::Swap(parse_rider(s, f[3])?, parse_rider(s, f[4])?),
            _ => return Err(format!("--act {s}: give SLOT,DT,dismount,R or SLOT,DT,swap,R1,R2 (R: visit:SLOT or an entity number)")),
        };
        let air = Block::parse("minecraft:air").map_err(|e| format!("--act {s}: {e}"))?;
        return Ok(Act { slot, dt, cell: [0; 3], block: air, hold: None, unload: None, order: redsim::reload::Order::Save, ride: Some(ride) });
    }
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
        return Ok(Act { slot, dt, cell: [0; 3], block: air, hold: Some(n.max(1)), unload: Some(unload), order, ride: None });
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
    Ok(Act { slot, dt, cell: [x as i32, y as i32, z as i32], block, hold, unload: None, order: redsim::reload::Order::Save, ride: None })
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
        LoopTiming { release_for: 30, advance_at: 40, advance_every: 20, pulse: 9, depart_every: 12, stand_brake: None, stand_detect: None, stand_detect_each: None, intake_brake: None, inject_at: None, depart_at: 4, depart_period: 16, open_after: 8, wired: None, golem_for: 10, stand_reset: None, shift_depart: false, s1_at: 0, tail0_late: 0, bench_stand: false, bench_release: false, release_offs: None, stand_offset: 0, release_stagger: 0, tail_check_at: None, stand_detect_slots: None, release_start: 0, adv_shift: [0; 3], adv_width: [0; 3], stand_shift: [0; 3], entry_speed: 0.0, skew: [0; 24], ideal_stand_at: None, tail_check_rest: false, tail2_at: None, placed_raw: false, build_gap: None, build_priced: false, stagger_off: false, side_drop: None, stop_open: 0, stop_close: 40, pocket_release: 10, pocket_reset: 52, stop_brake_delay: 0, stop_cv: 8, pocket_close_delay: 0, rel_for: 40, rel_on: 0, visit: None, build_last: [None; 64], build_fills_last: false, watch_cells: [None; 8], watch_mob: [None; 4], watch_mob_slots: (0, -1), pocket_trigger: None, no_summons: false, calm: false, reach_probe: false, exposure_log: false, record_positions: false, golem_miss: [-1; 8], golem_jitter: 0, golem_late: 0, day_only: false, slept: false, pulse_at: None, summon_at: None, acts: [None; 8], phase_force: None, reset_queue_at: None, reset_stragglers: false, reset_stand_carts: false, reset_queue_every: false, no_inject: false, fifo_intake: (2, 6), esc_b: (6, 6), verbose_tail: false, tail_trace: None, arrivals: false, esc_g0: (2, 0), esc_adm: (8, 3), adm_latch: false, p_always: false, adm_g1_w: 0, adm_k2: -1, adm_faults: [None; 8], adm_skip_req: [None; 8], adm_rpulse: 0, entry_hold: false, hold_landing_rest: 0, hold_strand_t: 40, hold_progress: false, reg_interlock: false, l33_delay: false, arrival_log: false, hold_back: None, start_random: false }
    }
}

/// Runs the loop for `cycles` slots of 100 gt with ideal controls: at each slot the stand's batch
/// leaves as a train (its slots powered) and the queue's front 3 leave as a train for the stand
/// (their slots powered `release_for`), the stand braked as the train's front cart reaches S1's
/// slope; the queue advanced 3 slots; the queue's 3 empty tail slots powered for the returning
/// train, braked as its front cart reaches the third. Checks the loop order (every cart 3
/// places on per slot) and prints each slot's times.
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
    // (the build's ground under the escapement tail's ledge: a derail lands as it would)
    let grounded = with_tail_ground(spec, o, 0, 0);
    let mut run = Run::start(if tm.build_gap.is_some() { &empty } else { &grounded }, &ro)?;
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
    let (mut expose_night, mut expose_ticks) = (vec![0u64; riders.len()], vec![0u64; riders.len()]);
    let mut expose_max: (usize, String) = (0, String::new());
    let mut expose_multi = 0u64;
    let mut last_panic = vec![i64::MIN / 2; riders.len()];
    let mut exposure_lines: Vec<String> = Vec::new();
    // (ironspec's contact rule, tickexact 2026-11-11) every rider, every tick, by walk state
    // (reaching, partial, a walk target with no path, no walk target): touching another cart's box,
    // within 0.2 of one; the most touching at once; the cells
    let mut walkers = [0u64; 4];
    let mut rider_touch_ticks = [0u64; 4];
    let mut walk_near = [0u64; 4];
    let mut touch_max: (usize, String) = (0, String::new());
    let mut touch_cells: std::collections::BTreeMap<[i32; 3], u64> = std::collections::BTreeMap::new();
    // (ironspec's push rule, tickexact 2026-11-11: counted at the push itself) every push a rider's
    // pushEntities gives another cart (redsim's log, drained each tick), by the pusher's walk state;
    // the largest; the most in one tick; the cells; other mobs' pushes on carts
    redsim::entity::minecart::log_cart_pushes(&mut run.world);
    let mut pushes = [0u64; 4];
    let mut push_big = 0.0f64;
    let mut push_max: (usize, String) = (0, String::new());
    let mut push_cells: std::collections::BTreeMap<[i32; 3], u64> = std::collections::BTreeMap::new();
    let mut push_other = 0u64;
    // (the pushes' harm: per pushed cart its episode (start, start position, push direction, peak,
    // start cell, crossed a cell backward), and the totals)
    let mut push_eps: std::collections::HashMap<u32, (i64, [f64; 2], [f64; 2], f64, [i32; 2], bool, bool)> = std::collections::HashMap::new();
    let mut harm_impact_eps = 0usize;
    let mut harm_eps = 0usize;
    let mut harm_peak: (f64, String) = (0.0, String::new());
    let mut harm_return_max = 0i64;
    let mut harm_unreturned = 0usize;
    let mut harm_crossed = 0usize;
    let mut harm_cross_log: Vec<String> = Vec::new();
    let mut expose_map: std::collections::BTreeMap<[i32; 3], (u64, i64, u64, u64)> = std::collections::BTreeMap::new();
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
        // (the rider space: no block with a collision box in the two cells over a rail)
        declared_bad.extend(rider_space_check(&mut run.world, spec, &declared, hdr, o, 0, 0));
    }
    // (the drive log's times count from the build's start when the bench builds the design, from
    // the spawn otherwise)
    let t_origin = build_start.unwrap_or(t_spawn);
    // (`wired`: the controls are the design's own redstone; the bench only pulses the golem input
    // cell each slot and checks where the carts go)
    let wired = tm.wired;
    let stand_cells_ctl: Vec<BlockPos> = lay.stand.clone();
    // (`bench_release`: with `wired`, the bench still drives the release group's 3 controls, the
    // last 3 queue slots': the bench-against-wired check runs a design without its release
    // controller this way)
    let rel_cells_ctl: Vec<BlockPos> = if lay.slots.len() >= 3 { lay.slots[lay.slots.len() - 3..].to_vec() } else { Vec::new() };
    let set = |w: &mut redsim::world::World, ps: &[BlockPos], p: bool| {
        for c in ps {
            if wired.is_some() && !(tm.bench_stand && stand_cells_ctl.contains(c)) && !(tm.bench_release && rel_cells_ctl.contains(c)) {
                continue;
            }
            w.set_block(*c, if p { on.clone() } else { off.clone() }, redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", w.time - t_origin, c.x, c.y, c.z, if p { "redstone_block" } else { "glass" }));
        }
    };
    let input_was = wired.map(|p| run.world.get(p));
    // (`bench_release`: the release group's 3 control cells cut to glass before the first slot, so
    // the design's own release lines end there and only the bench drives them)
    if tm.bench_release && wired.is_some() {
        for c in &rel_cells_ctl {
            run.world.set_block(*c, off.clone(), redsim::world::flags::ALL);
            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:glass", run.world.time - t_origin, c.x, c.y, c.z));
        }
    }
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
    // (ironspec's forced-phase mode) the forced villager: (id, the load's game time, asleep at the
    // load), until it wakes (its wake logged in `unload_log`)
    let mut phase_watch: Option<(u32, i64, bool)> = None;
    // (carts back through a gate; departures from the stand: (gt, cart, touching the one before))
    let mut strip_x: Vec<Option<f64>> = vec![None; 64];
    let mut back_through: Vec<String> = Vec::new();
    let mut departs: Vec<(i64, usize, bool)> = Vec::new();
    let mut top_v: Vec<Option<f64>> = Vec::new();
    // (the tail's entry: each cart reaching the return run's end, x 1 westward along z 20 at the
    // run's level, with its gap to the one before, contact and speed: what the tail sees)
    let mut tail_in: Vec<(i64, bool, f64)> = Vec::new();
    // (every cart's state each tick: off the rails (no rail at its cell or the one under it) for
    // 3 ticks or more, its rider not seated in it; and the latch's sets after +55 (T2 going out:
    // D1's set, or C's), each a slot offset)
    let mut rail_watch = crate::flight::RailWatch::new(merge_flights(lay.flights.clone(), &hdr.flights));
    // (the arrival stream and backward ticks on P: see the stand-strip block)
    let mut arr_stream: Vec<String> = Vec::new();
    let mut arr_prev_x = vec![f64::MIN; 21];
    let mut p_back: Vec<String> = Vec::new();
    let mut p_back_ticks = 0usize;
    // (admission control's margins: the fastest front at a G0 opening (speed, distance short,
    // when); the launched cart's rider against the next admitted cart: the closest gap between
    // the rider's box and a cart on P (blocks, when), and per admission the ticks between the
    // rider's box leaving G_P's plane and the admitted cart's front reaching P's end)
    let mut g0_open_vmax: (f64, f64, String) = (-1.0, 0.0, String::new());
    let mut rider_gap_min: (f64, String) = (f64::MAX, String::new());
    let mut rider_clear_t: Option<i64> = None;
    let mut rider_ticks_min: (i64, String) = (i64::MAX, String::new());
    let mut rider_back_prev = vec![f64::MIN; 21];
    let mut front_prev = vec![f64::MIN; 21];
    let mut arrivals_now: Vec<(usize, String)> = Vec::new();
    // (admission control's skip-request latch)
    let mut skip_latch = false;
    let mut skip_active = false;
    // (`reg_interlock`: the advance's allowed slots, the tail's launches held, the record)
    let mut reg_allow: Vec<bool> = Vec::new();
    let mut il_skip = [false; 3];
    let mut il_delay: [Option<i64>; 3] = [None; 3];
    let mut il_tempty: [Option<i64>; 3] = [None; 3];
    let mut il_slack_min: (i64, String) = (i64::MAX, String::new());
    let mut reg_held: Vec<String> = Vec::new();
    let mut reg_stuck_total = 0usize;
    let mut il_skips = 0usize;
    // (the tail's chunk at level 33: the time it spent frozen, its controls' lag with `l33_delay`)
    let mut tail_lag = 0i64;
    // (launches into a full slot 0, interlock or not)
    let mut gp_was_open = false;
    let mut launch_into_full = 0usize;
    let mut launch_full_log: Vec<String> = Vec::new();
    // (the stand holds its batch back: a REL held or dropped by the entry hold, until one fires)
    let mut stand_held = false;
    let mut base_prev = false;
    let mut skip_log: Vec<String> = Vec::new();
    // (the entry hold's detectors and their record)
    let hold_cells: Vec<BlockPos> = if o.esc_adm && o.adm_sensors {
        let mut v = Vec::new();
        if let Some(l) = lay.flights.first().and_then(|f| f.landing.get(1)) {
            v.push(BlockPos::new(l[0] + fx as i32, l[1], l[2] + fz as i32));
        }
        if let Some(q) = lay.queue_cells.get(o.adm_room_k as usize) {
            v.push(BlockPos::new(q[0] + fx as i32, q[1], q[2] + fz as i32));
        }
        // (`adm_latch_reset`: the top queue slope, the latch's reset: the cart moved on)
        if o.adm_latch_reset
            && let Some(q) = lay.queue_cells.first()
        {
            v.push(BlockPos::new(q[0] + fx as i32, q[1], q[2] + fz as i32));
        }
        v
    } else {
        Vec::new()
    };
    let mut hold_last_on: Option<i64> = None;
    let mut hold_blocked = 0usize;
    let mut hold_rels = 0usize;
    let mut hold_log: Vec<String> = Vec::new();
    let mut hold_slack_min: (i64, String) = (i64::MAX, String::new());
    let mut hold_pending: Option<i64> = None;
    let mut hold_delays: Vec<i64> = Vec::new();
    let mut hold_dropped = 0usize;
    let mut landing_since: Option<i64> = None;
    // (the entry hold's per-slot record and the detectors' last states)
    let mut det_prev = (false, false, false);
    let mut latch_set: Option<i64> = None;
    let mut room_last_on: Option<i64> = None;
    let mut slot_hold = SlotHold::default();
    let mut slot_holds: Vec<SlotHold> = Vec::new();
    let mut land_prev_z = vec![f64::MIN; 21];
    // (the progress input: carts leaving slot 0 forward since the last release; the first
    // release is free)
    let mut prog_prev_z = vec![f64::MIN; 21];
    let mut prog_since_rel = 1usize;
    // (the x-queue's flats: each cart's rest there since, the longest)
    let mut flat_rest_since: Vec<Option<i64>> = vec![None; 21];
    let mut flat_rest_max: (i64, String) = (0, String::new());
    let mut walk_kinds = [0usize; 3];
    let mut walk_touch = 0usize;
    let mut touch_idle = 0usize;
    let mut walk_touch_log: Vec<String> = Vec::new();
    let mut walking_log: Vec<String> = Vec::new();
    // (every detector rail of the loop: its on-edges against the carts entering it; with
    // MCTAS_WATCH_DETECTORS "x,y,z;...", more cells watched the same way (design coordinates), the
    // advance-spacing measurement's transit detectors; the layout's own cells are already placed)
    let mut det_cells: Vec<BlockPos> = lay.path.iter().filter(|(_, k)| matches!(k, Cell::Detector)).map(|(c, _)| BlockPos::new(c[0], c[1], c[2])).collect();
    for item in std::env::var("MCTAS_WATCH_DETECTORS").unwrap_or_default().split(';').filter(|s| !s.trim().is_empty()) {
        let p: Vec<i32> = item.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>().map_err(|e| format!("MCTAS_WATCH_DETECTORS {item}: {e}"))?;
        if p.len() != 3 {
            return Err(format!("MCTAS_WATCH_DETECTORS: x,y,z;... not {item}"));
        }
        det_cells.push(BlockPos::new(p[0] + fx as i32, p[1], p[2] + fz as i32));
    }
    let mut det_edges = DetectorEdges::new(det_cells);
    // (the stand's reference plane for train gaps: G2's plane, loop x 0.5 on the strip z 0.5,
    // y 0 (51b2aacd 05:28); MCTAS_GAP_PLANE=x,y,z for another (the crossing is still eastward in x))
    let gp: Vec<f64> = std::env::var("MCTAS_GAP_PLANE").ok().map(|v| v.split(',').filter_map(|t| t.trim().parse().ok()).collect()).filter(|v: &Vec<f64>| v.len() == 3).unwrap_or(vec![0.5, 0.0, 0.5]);
    let mut plane_gaps = PlaneGaps::new(gp[0] + fx as f64, gp[1], gp[2] + fz as f64);
    let mut stand_panic = StandPanic::new(fx as i32, fz as i32);
    // (the tail's arrivals, impacts, backward crossings, the front's readiness at each G0 opening,
    // the jump's spacing and the landing's clearance, the queue's occupancy: escapement designs)
    let mut tail_watch = TailWatch::new(&lay);
    tail_watch.verbose = tm.arrival_log;
    let mut held_back: Option<(u32, redsim::entity::Vec3, i64)> = None;
    // (the release trace, MCTAS_RELEASE_TRACE: the release group's rails and the release run's
    // cells, from the release group's first slot on)
    let rel_rails: Vec<BlockPos> = if lay.slot_cells.len() >= 3 { lay.slot_cells[lay.slot_cells.len() - 3..].iter().map(|c| BlockPos::new(c[0], c[1], c[2])).collect() } else { Vec::new() };
    let rel_run: Vec<[i32; 3]> = lay.slot_cells.len().checked_sub(3).and_then(|i| lay.path.iter().position(|(c, _)| *c == lay.slot_cells[i])).map(|i0| lay.path.iter().skip(i0).take(30).map(|(c, _)| *c).collect()).unwrap_or_default();
    let mut rel_trace: Option<std::io::BufWriter<std::fs::File>> = match std::env::var("MCTAS_RELEASE_TRACE") {
        Ok(p) if !p.is_empty() => Some(std::io::BufWriter::new(std::fs::File::create(&p).map_err(|e| format!("MCTAS_RELEASE_TRACE {p}: {e}"))?)),
        _ => None,
    };
    let mut unseated: Vec<bool> = vec![false; 21];
    let mut unseat_log: Vec<String> = Vec::new();
    let mut latch_sets: Vec<i64> = Vec::new();
    let mut t2_was_lit: Option<bool> = None;
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
    let mut homes_prev: Option<Vec<Option<BlockPos>>> = None;
    let n_riders = riders.len();
    let (mut stamp_prev, mut wt_prev) = (vec![None::<i64>; n_riders], vec![None::<redsim::entity::mob::ai::brain::WalkTarget>; n_riders]);
    let (mut act_prev, mut home_prev) = (vec![redsim::entity::mob::ai::brain::Act::Idle; n_riders], vec![None::<BlockPos>; n_riders]);
    let mut erase_by: std::collections::BTreeMap<String, (u64, i64)> = std::collections::BTreeMap::new();
    let mut erase_at: std::collections::BTreeMap<[i32; 3], std::collections::BTreeMap<String, u64>> = std::collections::BTreeMap::new();
    let mut stamp_oldest: Vec<(i64, String)> = vec![(0, String::new()); n_riders];
    let mut stamp_set_by: std::collections::BTreeMap<&'static str, u64> = std::collections::BTreeMap::new();
    let mut near_ticks = vec![0u64; n_riders];
    let mut near_max: (usize, String) = (0, String::new());
    // (the design's beds, each bed's free tickets logged at the start and at every change)
    let bed_cells: Vec<BlockPos> = {
        let w = &run.world;
        let mut v = Vec::new();
        for x in w.min.x..w.min.x + w.size[0] {
            for y in w.min.y..w.min.y + w.size[1] {
                for z in w.min.z..w.min.z + w.size[2] {
                    if matches!(w.get(BlockPos::new(x, y, z)), redsim::block::Block::Bed { head: true, .. }) {
                        v.push(BlockPos::new(x, y, z));
                    }
                }
            }
        }
        v
    };
    let mut bed_free: Vec<Option<u32>> = bed_cells.iter().map(|p| redsim::entity::mob::ai::poi::free_tickets(&mut run.world, *p)).collect();
    let mut ticket_log: Vec<String> = bed_cells.iter().zip(&bed_free).map(|(p, f)| format!("{},{},{} {} at the start", p.x, p.y, p.z, f.map_or("gone".to_string(), |n| format!("{n} free")))).collect();
    let mut home_log: Vec<String> = Vec::new();
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
                // (a rider act: the napper of a visit, or an entity, by its current number)
                if let Some(ra) = a.ride {
                    let who = |r: RiderRef| -> Option<u32> {
                        match r {
                            RiderRef::Id(id) => Some(id),
                            RiderRef::Visit(s) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.rider),
                            RiderRef::Batch(s, j) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.batch.get(j).copied()).and_then(|cart| redsim::entity::index_of(&run.world, cart)).and_then(|i| run.world.entities.list[i].passengers.first().copied()),
                        }
                    };
                    let vehicle_of = |w: &redsim::World, id: u32| redsim::entity::index_of(w, id).and_then(|i| w.entities.list[i].vehicle);
                    let line = match ra {
                        RideAct::Dismount(r) => match who(r).and_then(|id| redsim::entity::index_of(&run.world, id).map(|i| (id, i))) {
                            Some((id, i)) => {
                                let v = run.world.entities.list[i].vehicle;
                                redsim::entity::stop_riding(&mut run.world, i);
                                format!("slot {c} +{dt}: act: e{id} dismounted (it rode {})", v.map(|v| format!("cart e{v}")).unwrap_or_else(|| "nothing".into()))
                            }
                            None => format!("slot {c} +{dt}: act: dismount {r:?}: no such rider"),
                        },
                        RideAct::Swap(r1, r2) => match (who(r1), who(r2)) {
                            (Some(a1), Some(a2)) => match (vehicle_of(&run.world, a1), vehicle_of(&run.world, a2)) {
                                (Some(v1), Some(v2)) => {
                                    for id in [a1, a2] {
                                        if let Some(i) = redsim::entity::index_of(&run.world, id) {
                                            redsim::entity::stop_riding(&mut run.world, i);
                                        }
                                    }
                                    let m1 = redsim::entity::mob::mount(&mut run.world, a1, v2);
                                    let m2 = redsim::entity::mob::mount(&mut run.world, a2, v1);
                                    format!("slot {c} +{dt}: act: e{a1} (was in cart e{v1}) and e{a2} (was in cart e{v2}) swapped{}", match (m1, m2) { (Ok(()), Ok(())) => String::new(), (a, b) => format!(" (mount: {a:?}, {b:?})") })
                                }
                                (v1, v2) => format!("slot {c} +{dt}: act: swap e{a1}, e{a2}: not both riding ({v1:?}, {v2:?})"),
                            },
                            _ => format!("slot {c} +{dt}: act: swap {r1:?}, {r2:?}: no such rider"),
                        },
                    };
                    drive.borrow_mut().push(format!("# {line}"));
                    unload_log.push(line);
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
                            // (ironspec's forced-phase mode) the chosen villager's sensor delays
                            // set after the load's own draws
                            if let Some(pf) = tm.phase_force {
                                let id = match pf.who {
                                    RiderRef::Id(id) => Some(m.get(&id).copied().unwrap_or(id)),
                                    RiderRef::Visit(s) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.rider),
                                    RiderRef::Batch(s, j) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.batch.get(j).copied()).and_then(|cart| redsim::entity::index_of(&run.world, cart)).and_then(|i| run.world.entities.list[i].passengers.first().copied()),
                                };
                                let mut line = format!("slot {c} +{dt}: phases forced on {:?}: no such villager", pf.who);
                                if let Some((id, i)) = id.and_then(|id| redsim::entity::index_of(&run.world, id).map(|i| (id, i)))
                                    && let redsim::entity::EntityKind::Mob(mm) = &mut run.world.entities.list[i].kind
                                    && let Some(b) = mm.brain.as_deref_mut()
                                {
                                    let drawn = b.sensors;
                                    let mut set = Vec::new();
                                    for (k, v) in pf.sensors.iter().flatten() {
                                        b.sensors[*k] = *v;
                                        set.push(format!("{} {v} (drawn {})", redsim::entity::mob::villager::SENSORS[*k].0, drawn[*k]));
                                    }
                                    line = format!("slot {c} +{dt}: phases forced on e{id} after the load: {}; asleep {}", set.join(", "), mm.sleeping_pos.is_some());
                                    phase_watch = Some((id, run.world.time, mm.sleeping_pos.is_some()));
                                }
                                // (the tick order after the load: the villager's place in the
                                // entity list against its visit's cart's)
                                if let (Some(id), RiderRef::Visit(s)) = (id, pf.who)
                                    && let Some(cart) = visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.cart)
                                {
                                    let (iv, ic) = (redsim::entity::index_of(&run.world, id), redsim::entity::index_of(&run.world, cart));
                                    line.push_str(&format!("; tick order: the villager {iv:?}, its visit's cart e{cart} {ic:?}"));
                                }
                                unload_log.push(line);
                            }
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
            let in_region = |p: redsim::entity::Vec3| ((p.z - 0.5 - fz).abs() < 0.6 && p.y < 1.5 && p.x > xr - 0.6 && p.x < 9.0 + fx) || ((p.x - xr).abs() < 0.6 && p.z < head_z - 0.5 && p.y > 0.5);
            let keep = |id: u32| !tm.reset_stand_carts && (place_of(&run.world, id).is_some_and(|k| k >= q) || (!tm.reset_stragglers && pos_of(&run.world, id).is_some_and(in_region)));
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
                if (p.z - 0.5 - fz).abs() >= 0.6 || p.y >= 1.5 || p.x - fx < -2.6 || p.x - fx >= 9.0 {
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
        // (`reg_interlock`, ideal sensors, bench-driven (ironspec Nov 11 04:38): the head releases
        // only into a stand that isn't holding a batch back (its last REL held or dropped by the
        // entry hold); a held release leaves the head's carts in place, and the register's
        // advances then stop behind them)
        let stand_has_carts = tm.reg_interlock && stand_held;
        if c < cycles && dt == tm.release_start && stand_has_carts {
            reg_held.push(format!("slot {c}: release held (the stand holds carts)"));
        }
        if c < cycles && dt == tm.release_start && !stand_has_carts {
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
            // (each slope `release_for` from its own opening, the release start included: day-1's
            // wired controller keeps a fixed-length window per slope, so a later start moves its
            // end too (the bench-against-wired check, Nov 11 06:51))
            let st_off = |k: usize| tm.release_offs.map(|o| o[k]).unwrap_or(tm.release_start + tm.release_for + if tm.stagger_off && tm.release_stagger > 0 { (2 - k as i64) * tm.release_stagger } else { 0 });
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
                let fire_nominal = if rel >= 0 { dt == rel } else { (c + 1 < cycles && dt == 100 + rel) || (c == 0 && dt == 0) };
                // (the entry hold, `adm_sensors` designs (ironspec Nov 11 03:35, 03:52): the
                // tail's detectors read at REL: room (the `adm_room_k`-th queue slope: no room
                // while on) and the landing; with `adm_latch_reset` (the top queue slope a third
                // detector) the landing input is ironspec's cart-evidence latch: set by the
                // landing detector's on-edge, reset by the next cell's on-edge, a strand when it
                // has stayed set `hold_strand_t` gt; else the landing counts after
                // `hold_landing_rest` gt on without a break. Per slot: the latch's longest set
                // time, the landing's longest on time, the room slack at REL, and the landing's
                // on-edges against the carts crossing it. With `entry_hold` a release waits while
                // either input is on, up to the pockets' reset)
                let det_on = |p: &BlockPos| run.world.get(*p).to_string().contains("powered=true");
                let landing_raw = hold_cells.first().is_some_and(det_on);
                let room_on = hold_cells.get(1).is_some_and(det_on);
                let next_on = hold_cells.get(2).is_some_and(det_on);
                if !hold_cells.is_empty() {
                    if landing_raw && !det_prev.0 {
                        slot_hold.land_edges += 1;
                        if hold_cells.len() > 2 {
                            latch_set.get_or_insert(t);
                        }
                    }
                    if next_on && !det_prev.2 {
                        slot_hold.next_edges += 1;
                        latch_set = None;
                    }
                    det_prev = (landing_raw, room_on, next_on);
                    if room_on {
                        room_last_on = Some(t);
                    }
                    if let Some(s) = latch_set {
                        slot_hold.latch_max = slot_hold.latch_max.max(t - s + 1);
                    }
                }
                if landing_raw {
                    landing_since.get_or_insert(t);
                } else {
                    landing_since = None;
                }
                if let Some(s) = landing_since {
                    slot_hold.land_on_max = slot_hold.land_on_max.max(t - s + 1);
                }
                let landing_on = if hold_cells.len() > 2 { latch_set.is_some_and(|s| t - s >= tm.hold_strand_t) } else { landing_raw && landing_since.is_some_and(|s| t - s >= tm.hold_landing_rest) };
                // (`hold_progress`, ironspec Nov 11 04:16: a fourth input, tail progress: REL only if
                // a cart has left slot 0 forward (crossed z 34.5: the register took the escapement's
                // launch) since the last REL; it sees a frozen tail or register from the first
                // stalled slot, which the room read can't)
                let progress_stalled = tm.hold_progress && !hold_cells.is_empty() && prog_since_rel == 0;
                let sensors_on = landing_on || room_on || progress_stalled;
                if sensors_on {
                    hold_last_on = Some(t);
                }
                if fire_nominal && !hold_cells.is_empty() {
                    slot_hold.room_slack = Some(if room_on { -1 } else { room_last_on.map_or(i64::MAX, |r| t - r) });
                    slot_hold.progress = Some(prog_since_rel);
                    if sensors_on {
                        hold_blocked += 1;
                        if hold_log.len() < 6 {
                            let mut which: Vec<&str> = Vec::new();
                            if landing_on {
                                which.push("landing");
                            }
                            if room_on {
                                which.push("room");
                            }
                            if progress_stalled {
                                which.push("no progress");
                            }
                            hold_log.push(format!("slot {c} +{dt} {}", which.join("+")));
                        }
                    } else if let Some(lo) = hold_last_on {
                        if t - lo < hold_slack_min.0 {
                            hold_slack_min = (t - lo, format!("slot {c} +{dt}"));
                        }
                    }
                    hold_rels += 1;
                }
                if dt == 99 && !hold_cells.is_empty() {
                    slot_holds.push(std::mem::take(&mut slot_hold));
                }
                let fire = if tm.entry_hold && !hold_cells.is_empty() {
                    if fire_nominal && sensors_on {
                        hold_pending = Some(t);
                        stand_held = true;
                        false
                    } else if let (Some(t0), false) = (hold_pending, sensors_on) {
                        hold_pending = None;
                        hold_delays.push(t - t0);
                        true
                    } else if hold_pending.is_some() && dt + 1 >= so + tm.pocket_reset {
                        hold_pending = None;
                        hold_dropped += 1;
                        false
                    } else {
                        fire_nominal
                    }
                } else {
                    fire_nominal
                };
                // (the progress input counts from each release that fires; the stand no longer
                // holds a batch back)
                if fire {
                    prog_since_rel = 0;
                    stand_held = false;
                }
                if fire && o.stand_wired {
                    // the wired stand: REL on (the design's latch and gates do the rest), off
                    // `rel_for` gt later; a file whose REL cell is not glass drives it itself; an
                    // injected act holding the cell keeps it
                    if let Some(r) = rel_bench {
                        rel_want = true;
                        if !rel_held(r) {
                            run.world.set_block(r, on.clone(), redsim::world::flags::ALL);
                            drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:redstone_block", run.world.time - t_origin, r.x, r.y, r.z));
                        }
                        // (`rel_on` moves the start alone: the end stays at the nominal start +
                        // `rel_for`)
                        rel_off_at = Some((t - tm.rel_on + tm.rel_for).max(t + 1));
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
                // first; a tail slot whose window has opened stays on through its pulse's end;
                // with `reg_interlock` a slot's cart moves only into an empty slot or one whose
                // cart moves in the same advance, the head never: decided at the advance's start)
                if tm.reg_interlock && dt == a {
                    let occ: Vec<bool> = lay.slot_cells.iter().take(q).map(|r| carts.iter().any(|id| pos_of(&run.world, *id).is_some_and(|p| p.x.floor() as i32 == r[0] && p.z.floor() as i32 == r[2] && p.y >= r[1] as f64 - 0.2 && p.y < r[1] as f64 + 1.2))).collect();
                    let mut mv = vec![false; q];
                    for s in (0..q.saturating_sub(1)).rev() {
                        mv[s] = occ[s] && (!occ[s + 1] || mv[s + 1]);
                    }
                    reg_allow = (0..q).map(|s| !occ[s] || mv[s]).collect();
                    let stuck = (0..q).filter(|s| occ[*s] && !mv[*s]).count();
                    if stuck > 0 && stuck < q && reg_held.len() < 12 {
                        reg_held.push(format!("slot {c} +{dt}: {stuck} carts held"));
                    } else if stuck == q && reg_held.len() < 12 {
                        reg_held.push(format!("slot {c} +{dt}: the register full and held"));
                    }
                    reg_stuck_total += stuck;
                }
                for s in 0..q {
                    let sk = tm.skew.get(s).copied().unwrap_or(0);
                    if dt == a + sk && (!tm.reg_interlock || reg_allow.get(s).copied().unwrap_or(true)) {
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
        // each advance's start; an intake that finds its pocket empty leaves a hole in slot 0.
        // (A level-33 freeze of the tail's chunk stops its redstone (ironspec Nov 11 04:47): with
        // `l33_delay` the tail's controls run on their own clock, behind by the time spent frozen
        // (a pure delay); else they write nothing while frozen and snap to their schedule on the
        // resume tick (every edge due during the freeze at once, the worst burst).)
        let tail_frozen33 = lay.esc_g0.is_some_and(|g| run.world.chunk_level(g.x.div_euclid(16), g.z.div_euclid(16)) >= 33);
        if tail_frozen33 {
            tail_lag += 1;
        }
        if fifo_on && !tail_frozen33 {
            let (c, dt) = if tm.l33_delay { let tt = t - tail_lag; (tt.div_euclid(100), tt.rem_euclid(100)) } else { (c, dt) };
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
                    // (`reg_interlock`: the launch waits for an empty slot 0 (the cart in it gone
                    // from its cell since the advance's pulse), up to 10 gt, else that advance's
                    // launch and its G0 are skipped, P keeping its cart; the whole admission (G_P
                    // and G0) shifts by the wait; the slack (G_P's timed opening minus the tick
                    // slot 0 read empty) is recorded)
                    if tm.reg_interlock {
                        if dt == 0 {
                            il_skip = [false; 3];
                            il_delay = [None; 3];
                            il_tempty = [None; 3];
                        }
                        let s0 = lay.slot_cells[0];
                        let s0_full = carts.iter().any(|id| pos_of(&run.world, *id).is_some_and(|p| (p.x - (s0[0] as f64 + 0.5)).abs() < 0.5 && p.z >= s0[2] as f64 && p.z < s0[2] as f64 + 1.0 && p.y >= s0[1] as f64 - 0.2 && p.y < s0[1] as f64 + 1.2));
                        for k in 0..3usize {
                            let a = tm.advance_at + k as i64 * tm.advance_every + tm.adv_shift[k];
                            if dt > a && il_tempty[k].is_none() && !s0_full {
                                il_tempty[k] = Some(dt);
                            }
                            if dt >= a + d && il_delay[k].is_none() && !il_skip[k] {
                                if !s0_full {
                                    il_delay[k] = Some(dt - (a + d));
                                    let slack = (a + d) - il_tempty[k].unwrap_or(dt);
                                    if slack < il_slack_min.0 {
                                        il_slack_min = (slack, format!("slot {c} +{}", a + d));
                                    }
                                    if dt > a + d && reg_held.len() < 12 {
                                        reg_held.push(format!("slot {c} +{}: launch waited {} gt (slot 0 full)", a + d, dt - (a + d)));
                                    }
                                } else if dt >= a + d + 10 {
                                    il_skip[k] = true;
                                    il_skips += 1;
                                    if reg_held.len() < 12 {
                                        reg_held.push(format!("slot {c} +{}: launch held (slot 0 still full at +10)", a + d));
                                    }
                                }
                            }
                        }
                    }
                    let il_shift = |k: usize| if tm.reg_interlock { il_delay[k].unwrap_or(0) } else { 0 };
                    let il_open = |k: usize| !tm.reg_interlock || il_delay[k].is_some();
                    // (admission faults, `adm_faults`: this advance's G_P pulse skipped or
                    // missed, or an extra one OFF gt after its opening)
                    let fault = |k: i64| tm.adm_faults.iter().flatten().find(|f| f.0 == c && f.1 == k).map(|f| (f.2, f.3));
                    c < cycles && (0..3).any(|k| {
                        let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize] + il_shift(k as usize);
                        let base = dt >= a + d && dt < a + d + w && !matches!(fault(k), Some((0, _)) | Some((2, _))) && il_open(k as usize);
                        let extra = matches!(fault(k), Some((4, off)) if dt >= a + d + off && dt < a + d + off + w);
                        base || extra
                    })
                };
                // (every launch into a full slot 0, interlock or not: G_P opening while a cart rests
                // on slot 0's cell, at rest (under 0.02 b/t), not the one leaving; the interlock's
                // positive control)
                if j + 1 == nf {
                    if open && !gp_was_open {
                        let s0 = lay.slot_cells[0];
                        let full = run.world.entities.list.iter().any(|e| carts.contains(&e.id) && (e.pos.x - (s0[0] as f64 + 0.5)).abs() < 0.5 && e.pos.z >= s0[2] as f64 && e.pos.z < s0[2] as f64 + 1.0 && e.pos.y >= s0[1] as f64 - 0.2 && e.pos.y < s0[1] as f64 + 1.2 && e.delta_movement.horizontal_distance() < 0.02);
                        if full {
                            launch_into_full += 1;
                            if launch_full_log.len() < 4 {
                                launch_full_log.push(format!("slot {c} +{dt}"));
                            }
                        }
                    }
                    gp_was_open = open;
                }
                set_gate(&mut run.world, lay.fifo[j].0, "south", open);
            }
            // (admission control, `esc_adm`: G0 opened `esc_adm` D gt after the gate opens for its W
            // gt, the third advance's after `adm_k2` gt when set; P always on from the first gate
            // opening with `p_always`, else powered with the gate)
            if let (true, Some(g0)) = (o.esc_adm, lay.esc_g0) {
                let (d, w) = tm.fifo_intake;
                let (dl, wl) = tm.esc_adm;
                // (admission faults: this advance's G0 pulse skipped (with G_P) or missed, or an
                // extra one OFF gt after G_P's opening)
                let fault = |k: i64| tm.adm_faults.iter().flatten().find(|f| f.0 == c && f.1 == k).map(|f| (f.2, f.3));
                let base_now = c < cycles && (0..3).any(|k| {
                    let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize] + if tm.reg_interlock { il_delay[k as usize].unwrap_or(0) } else { 0 };
                    let dk = if k == 2 && tm.adm_k2 >= 0 { tm.adm_k2 } else { dl };
                    dt >= a + d + dk && dt < a + d + dk + wl && !matches!(fault(k), Some((0, _)) | Some((1, _))) && !(tm.reg_interlock && (il_skip[k as usize] || il_delay[k as usize].is_none()))
                });
                // (the skip request, `adm_skip_req`: a latch set by a request; the next G0 opening
                // after it is skipped, which clears it)
                if tm.adm_skip_req.iter().flatten().any(|r| r.0 == c && r.1 == dt) {
                    skip_latch = true;
                    skip_log.push(format!("slot {c} +{dt}: skip requested"));
                }
                if base_now && !base_prev && skip_latch {
                    skip_latch = false;
                    skip_active = true;
                    skip_log.push(format!("slot {c} +{dt}: G0's opening skipped"));
                }
                if !base_now {
                    skip_active = false;
                }
                base_prev = base_now;
                let want = (base_now && !skip_active) || (c < cycles && (0..3).any(|k| {
                    let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                    matches!(fault(k), Some((3, off)) if dt >= a + d + off && dt < a + d + off + wl)
                }));
                let s = format!("minecraft:oak_fence_gate[facing=south,in_wall=false,open={want},powered=false]");
                if let Ok(blk) = Block::parse(&s)
                    && run.world.get(g0) != blk
                {
                    // (G0 opening: the queue's front cart then, its distance short of G0's box and
                    // its speed, the worst kept: the fastest arrival an admission must survive)
                    if want {
                        let near = g0.z as f64 + 0.375 - fz;
                        let front = run.world.entities.list.iter().filter(|e| matches!(e.kind, redsim::entity::EntityKind::Minecart(_)) && (e.pos.x - fx - (g0.x as f64 + 0.5)).abs() < 0.3 && e.pos.z - fz + 0.49 <= near + 0.02 && e.pos.z - fz > near - 6.0).max_by(|a, b| a.pos.z.total_cmp(&b.pos.z));
                        if let Some(f) = front {
                            let v = f.delta_movement.horizontal_distance();
                            let short = near - (f.pos.z - fz + 0.49);
                            if v > g0_open_vmax.0 {
                                g0_open_vmax = (v, short, format!("slot {c} +{dt}"));
                            }
                        }
                    }
                    run.world.set_block(g0, blk, redsim::world::flags::ALL);
                    drive.borrow_mut().push(format!("act {} set {} {} {} {}", run.world.time - t_origin, g0.x, g0.y, g0.z, s));
                }
                if let (Some(pc), false) = (lay.esc_p, o.adm_pdet) {
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
                // (`adm_rboost`: R's control on for `adm_rpulse` gt from each G_P opening)
                if let (true, Some(a1)) = (o.adm_rboost && tm.adm_rpulse > 0, lay.esc_adm_a1) {
                    let r = adm_r_cell(a1);
                    let rc = BlockPos::new(r[0] + 1 + fx as i32, r[1], r[2] + fz as i32);
                    let want_r = c < cycles && (0..3).any(|k| {
                        let a = tm.advance_at + k * tm.advance_every + tm.adv_shift[k as usize];
                        dt >= a + d && dt < a + d + tm.adm_rpulse
                    });
                    let b = if want_r { on.clone() } else { off.clone() };
                    if run.world.get(rc) != b {
                        run.world.set_block(rc, b, redsim::world::flags::ALL);
                        drive.borrow_mut().push(format!("act {} set {} {} {} minecraft:{}", run.world.time - t_origin, rc.x, rc.y, rc.z, if want_r { "redstone_block" } else { "glass" }));
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
                if !rel_held(r) {
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
        // (the runtime check: a braked rail powered while its control is off, once per cell;
        // bench-driven controls only)
        if build_start.is_none() && tm.wired.is_none() {
            for b in crate::declared::check_runtime(&run.world, &declared) {
                let key = b.split(':').next().unwrap_or("").to_string();
                if !declared_bad.iter().any(|x| x.starts_with(&key)) {
                    declared_bad.push(format!("{b} (slot {c} +{dt})"));
                }
            }
        }
        // (every detector rail: on-edges against carts entering)
        if build_start.is_none() {
            det_edges.observe(&run.world, &carts);
            plane_gaps.observe(&run.world, &carts, c as i64);
            stand_panic.observe(&run.world, &riders, c as i64, dt as i64);
            tail_watch.observe(&run.world, &carts, t, c as i64, dt as i64);
            // (MCTAS_RELEASE_TRACE=path, the bench-against-wired check: each tick the release
            // group's 3 rails' power and every cart within the release group and the 30 cells
            // after its first slot, for `trace-diff`)
            if let Some(f) = rel_trace.as_mut().filter(|_| c < cycles) {
                use std::io::Write;
                let bits: String = rel_rails.iter().map(|p| if run.world.get(*p).to_string().contains("powered=true") { '1' } else { '0' }).collect();
                let mut line = format!("t {} c {c} dt {dt} R {bits} |", run.world.time - t_origin);
                for (k, id) in carts.iter().enumerate() {
                    if let Some(p) = pos_of(&run.world, *id)
                        && rel_run.iter().any(|c3| p.x.floor() as i32 == c3[0] && p.z.floor() as i32 == c3[2] && (p.y - c3[1] as f64) > -0.6 && (p.y - (c3[1] as f64)) < 1.6)
                    {
                        line.push_str(&format!(" {k}:{:.4},{:.4},{:.4}", p.x, p.y, p.z));
                    }
                }
                let _ = writeln!(f, "{line}");
            }
        }
        // (`--hold-back`: the front metric's positive control, the queue's second cart behind G0
        // held in place)
        if let (Some([hc, hdt, hn]), Some(g0)) = (tm.hold_back, lay.esc_g0) {
            if c as i64 == hc && dt as i64 == hdt {
                let mut q: Vec<(u32, f64)> = carts.iter().filter_map(|id| pos_of(&run.world, *id).filter(|p| (p.x - (g0.x as f64 + 0.5)).abs() < 0.4 && p.z < g0.z as f64 && p.z > g0.z as f64 - 6.0).map(|p| (*id, p.z))).collect();
                q.sort_by(|a, b| b.1.total_cmp(&a.1));
                held_back = q.get(1).and_then(|(id, _)| redsim::entity::index_of(&run.world, *id).map(|i| (*id, run.world.entities.list[i].pos, t + hn)));
            }
            if let Some((id, p, until)) = held_back {
                if t < until {
                    if let Some(i) = redsim::entity::index_of(&run.world, id) {
                        run.world.entities.set_pos(i, p);
                        run.world.entities.list[i].delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                    }
                } else {
                    held_back = None;
                }
            }
        }
        stop_brake(&mut run.world, &mut stop_arm);
        hold_day(&mut run.world);
        // (the lap order: each cart onto the pocket stand's departure climb, its first cell)
        if o.stand_stop {
            // (backward through a gate: a cart on the stand's strip crossing a gate's wall westward;
            // also each cart off the rails, a rider not seated, and the latch's sets)
            for (k, id) in carts.iter().enumerate().take(21) {
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) else { continue };
                // (the arrival stream, `arrivals`: each cart crossing x 8.5 westward on the
                // escapement tail's return run, as tail-bench `--arrivals` replays it; and a cart
                // moving backward on admission control's P while P is powered)
                if o.tail_esc > 0 {
                    let (x, y, z) = (e.pos.x - fx, e.pos.y, e.pos.z - fz);
                    if tm.arrivals && (y - esc_ye(o) as f64).abs() < 0.6 && (z - 20.5).abs() < 0.4 && arr_prev_x[k] > 8.5 && x <= 8.5 {
                        arr_stream.push(format!("arrival c{c} +{dt} x{x:.4} v{:.4}", e.delta_movement.horizontal_distance()));
                    }
                    arr_prev_x[k] = x;
                    // (the entry hold's landing: carts crossing the plain landing flat's centre)
                    if let Some(l) = lay.flights.first().and_then(|f| f.landing.get(1)) {
                        let lz = l[2] as f64 + 0.5;
                        if (x - 0.5).abs() < 0.4 && (y - l[1] as f64).abs() < 0.6 && land_prev_z[k] < lz && z >= lz {
                            slot_hold.crossings += 1;
                        }
                        land_prev_z[k] = z;
                    }
                    // (the x-queue's flats, `adm_xq` (ironspec's R1 note 2): a cart at rest (under
                    // 0.01 b/t) on the curve (0, y_f, 24) or the transition cell (0, y_f, 25), its
                    // longest unbroken rest; nothing may rest there in normal flow)
                    if o.adm_xq > 0 && x.floor() as i32 == 0 && (z.floor() as i32 == 24 || z.floor() as i32 == 25) && y > 7.0 {
                        if e.delta_movement.horizontal_distance() < 0.01 {
                            let s = flat_rest_since[k].get_or_insert(t);
                            if t - *s + 1 > flat_rest_max.0 {
                                flat_rest_max = (t - *s + 1, format!("slot {c} +{dt}: cart {k} at {x:.2},{y:.2},{z:.2}"));
                            }
                        } else {
                            flat_rest_since[k] = None;
                        }
                    } else {
                        flat_rest_since[k] = None;
                    }
                    // (the progress input: a cart leaving slot 0 forward, across z 34.5)
                    if let Some(a1) = lay.esc_adm_a1 {
                        let pz = a1[2] as f64 + 3.5;
                        if (x - 0.5).abs() < 0.4 && prog_prev_z[k] < pz && z >= pz && prog_prev_z[k] > pz - 1.0 {
                            prog_since_rel += 1;
                        }
                        prog_prev_z[k] = z;
                    }
                    // (riders in the pressed queue, the landing to P: walk targets by path
                    // (reaching, partial, none), and every rider whose box touches another cart's,
                    // walking or not (tick-exact Nov 11: LivingEntity.pushEntities runs every tick;
                    // contact alone pushes, bridged in gate-escg6l-v3 to 500 ticks); the brief's
                    // print: touching must be 0)
                    if let (Some(a1), Some(r)) = (lay.esc_adm_a1, e.passengers.first())
                        && (x - 0.5).abs() < 0.4
                        && z > 24.0
                        && z < a1[2] as f64 + 1.0
                        && y < esc_ye(o) as f64 - 0.5
                    {
                        let kind = rider_walk_kind(&run.world, *r);
                        let kname = kind.map_or("not walking", |kd| ["reaching", "partial", "no path"][kd as usize]);
                        if let Some(kd) = kind {
                            walk_kinds[kd as usize] += 1;
                            if walking_log.iter().filter(|l| l.ends_with(kname)).count() < 3 {
                                walking_log.push(format!("slot {c} +{dt}: cart {k} at z {z:.2} {kname}"));
                            }
                        }
                        if let Some((tz, ov)) = rider_touch(&run.world, *r, *id) {
                            walk_touch += 1;
                            if kind.is_none() {
                                touch_idle += 1;
                            }
                            if walk_touch_log.len() < 3 {
                                walk_touch_log.push(format!("slot {c} +{dt}: cart {k} at z {z:.2} ({kname}) touching a cart at z {:.3} by {ov:.3}", tz - fz));
                            }
                        }
                    }
                    if let Some(a1) = lay.esc_adm_a1
                        && x.floor() as i32 == a1[0]
                        && z.floor() as i32 == a1[2]
                        && y >= a1[1] as f64
                        && y < a1[1] as f64 + 1.3
                        && e.delta_movement.z < -0.01
                        && run.world.get(BlockPos::new(a1[0] + fx as i32, a1[1], a1[2] + fz as i32)).to_string().contains("powered=true")
                    {
                        if p_back.len() < 6 {
                            // (who pushed it: the non-cart entities within 1.6 blocks)
                            let near: Vec<String> = run.world.entities.list.iter().filter(|o| o.id != *id && !matches!(o.kind, redsim::entity::EntityKind::Minecart(_)) && (o.pos.x - e.pos.x).abs() < 1.6 && (o.pos.z - e.pos.z).abs() < 1.6 && (o.pos.y - e.pos.y).abs() < 2.5).map(|o| format!("{}#{} at {:.2},{:.2},{:.2}{}", match &o.kind { redsim::entity::EntityKind::Mob(m) => format!("{:?}", m.ty), k => format!("{k:?}").chars().take(12).collect() }, o.id, o.pos.x - fx, o.pos.y, o.pos.z - fz, match o.vehicle { Some(v) => format!(" riding #{v}"), None => String::new() })).collect();
                            p_back.push(format!("slot {c} +{dt}: cart {k} at z {z:.2} vz {:.3}; near: {}", e.delta_movement.z, if near.is_empty() { "none".to_string() } else { near.join(", ") }));
                        }
                        p_back_ticks += 1;
                    }
                    // (the rider margin: this cart on P against every rider ahead of it; this
                    // cart's rider leaving G_P's plane; this cart's front reaching P's end)
                    if let Some(a1) = lay.esc_adm_a1 {
                        let z_gp = (a1[2] + 1) as f64 + 0.375;
                        let tnow = run.world.time - t_origin;
                        if let Some(r) = run.world.entities.list.iter().find(|r| r.id == riders[k]) {
                            let back = r.pos.z - fz - 0.3;
                            if rider_back_prev[k] != f64::MIN && rider_back_prev[k] <= z_gp && back > z_gp {
                                rider_clear_t = Some(tnow);
                            }
                            rider_back_prev[k] = back;
                        }
                        let on_p = x.floor() as i32 == a1[0] && z.floor() as i32 == a1[2] && y >= a1[1] as f64 && y < a1[1] as f64 + 1.3;
                        let front = z + 0.49;
                        if on_p {
                            for (k2, id2) in carts.iter().enumerate().take(21) {
                                if k2 == k {
                                    continue;
                                }
                                let Some(b) = run.world.entities.list.iter().find(|b| b.id == *id2) else { continue };
                                if b.pos.z - fz <= z {
                                    continue;
                                }
                                let Some(r) = run.world.entities.list.iter().find(|r| r.id == riders[k2]) else { continue };
                                if r.pos.y + 1.95 > y && r.pos.y < y + 0.7 && (r.pos.x - e.pos.x).abs() < 0.79 {
                                    let gap = (r.pos.z - fz - 0.3) - front;
                                    if gap < rider_gap_min.0 {
                                        rider_gap_min = (gap, format!("slot {c} +{dt} (cart {k} on P, cart {k2}'s rider)"));
                                    }
                                }
                            }
                        }
                        if on_p && front_prev[k] != f64::MIN && front_prev[k] < z_gp - 0.075 && front >= z_gp - 0.075 {
                            // (judged after every cart's rider this tick: see below the loop)
                            arrivals_now.push((k, format!("slot {c} +{dt} (cart {k})")));
                        }
                        front_prev[k] = front;
                    }
                }
                // (off the rails, with the design's declared flights: `crate::flight`)
                rail_watch.observe(&run.world, k, *id, Some(riders[k]), &format!("slot {c} +{dt}"));
                let seated = e.passengers.contains(&riders[k]);
                if !seated && !unseated[k] {
                    unseat_log.push(format!("slot {c} +{dt}: cart {k}'s rider not seated (cart at {:.2},{:.2},{:.2})", e.pos.x - fx, e.pos.y, e.pos.z - fz));
                }
                unseated[k] = !seated;
            }
            // (admission control's rider ticks: each cart that reached P's end this tick, against
            // the last launched rider to leave G_P's plane, this tick's included)
            for (_, when) in arrivals_now.drain(..) {
                if let Some(tc) = rider_clear_t {
                    let m = run.world.time - t_origin - tc;
                    if m < rider_ticks_min.0 {
                        rider_ticks_min = (m, when);
                    }
                }
            }
            if o.stand_wired {
                let lit = run.world.get(BlockPos::new(-o.row_gap + 9 + o.shift[0], 0, 2 + o.shift[1])).to_string().contains("lit=true");
                if t2_was_lit == Some(true) && !lit && dt >= 55 && c < cycles {
                    latch_sets.push(dt);
                }
                t2_was_lit = Some(lit);
            }
            // the departure: each cart past x 9 (the curve) eastward, its gap to the one before)
            let gx = [-o.row_gap as f64 + 3.5, -o.row_gap as f64 + 7.5, -o.row_gap as f64 + 11.5];
            for (k, id) in carts.iter().enumerate() {
                let p = pos_of(&run.world, *id);
                let now = p.filter(|p| (p.z - 0.5 - fz).abs() < 0.6 && p.y < 1.5).map(|p| p.x - fx);
                if let (Some(x0), Some(x1)) = (strip_x[k], now) {
                    for (g, gxx) in gx.iter().enumerate() {
                        if x0 >= *gxx && x1 < *gxx {
                            back_through.push(format!("slot {c} +{dt}: cart {k} back through G{}", 2 - g));
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
            let ry = if o.tail_desc { 4.0 } else { 1.0 };
            for (k, id) in carts.iter().enumerate() {
                let Some(e) = run.world.entities.list.iter().find(|e| e.id == *id) else { continue };
                let now = ((e.pos.z - fz - 20.5).abs() < 0.6 && (e.pos.y - ry).abs() < 0.9).then_some(e.pos.x - fx);
                if let (Some(x0), Some(x1)) = (ret_x[k.min(20)], now) {
                    if x0 >= 1.0 && x1 < 1.0 {
                        let touching = tail_in.last().is_some() && carts.iter().any(|o2| *o2 != *id && pos_of(&run.world, *o2).is_some_and(|p| (p.z - e.pos.z).abs() < 0.6 && (p.y - e.pos.y).abs() < 0.9 && (p.x - e.pos.x).abs() < 1.05 && p.x < e.pos.x));
                        tail_in.push((t, touching, e.delta_movement.horizontal_distance()));
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
            // (tickexact 2026-11-11, ironspec's recoverability) every HOME lost or gained, by whom
            // and when (a loss's cause: the `cant-reach:` line's releases, ValidateNearbyPoi, ...)
            if let Some(prev) = &homes_prev {
                for (k, (was, now)) in prev.iter().zip(homes_now.iter()).enumerate() {
                    if was != now {
                        home_log.push(match now {
                            None => format!("e{k} lost at slot {c} +{dt}"),
                            Some(p) => format!("e{k} {},{},{} at slot {c} +{dt}", p.x, p.y, p.z),
                        });
                    }
                }
            }
            homes_prev = Some(homes_now.clone());
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
            // (tickexact 2026-11-11, ironspec: what erases the riders' CANT_REACH stamps) each
            // rider's stamp against the last tick's: gone (MoveToTargetSink: a path that can reach,
            // or its target reached) or set; the walk target and activity of the tick before say
            // which walk did it. And the near-release line: REST, HOME held, a stamp >= 1,000 gt,
            // with or without a WALK_TARGET
            let mut near: Vec<String> = Vec::new();
            for (k, r) in riders.iter().enumerate() {
                let Some(i) = redsim::entity::index_of(&run.world, *r) else { continue };
                let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind else { continue };
                let (Some(b), Some(x)) = (m.brain.as_deref(), m.brain.as_deref().and_then(|b| b.ex.as_deref())) else { continue };
                let p = run.world.entities.list[i].pos;
                let cell = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
                let stamp = x.mem.cant_reach_since;
                if let Some(old) = stamp_prev[k] {
                    if stamp.is_none() {
                        use redsim::entity::mob::ai::brain::Tracker;
                        let what = match wt_prev[k] {
                            // (SleepInBed.start erases WALK_TARGET and the stamp: a napper lying down)
                            _ if m.sleeping_pos.is_some() => "lying down (SleepInBed)",
                            Some(w) => match w.target {
                                Tracker::Block(t) if Some(t) == home_prev[k] => "the HOME walk",
                                Tracker::Entity { .. } => "a walk to an entity",
                                _ => "another walk",
                            },
                            None => "no walk target",
                        };
                        let key = format!("{what} in {}", act_prev[k].name());
                        let age = run.world.time - old;
                        let e = erase_by.entry(key.clone()).or_insert((0, 0));
                        e.0 += 1;
                        e.1 = e.1.max(age);
                        *erase_at.entry(cell).or_default().entry(key).or_insert(0) += 1;
                    }
                } else if stamp.is_some() {
                    *stamp_set_by.entry(x.active.name()).or_insert(0) += 1;
                }
                // (each rider's oldest stamp at any tick, whatever it does: one that never clears
                // shows here)
                if let Some(since) = stamp {
                    let age = run.world.time - since;
                    if age > stamp_oldest[k].0 {
                        stamp_oldest[k] = (age, format!("at {:.1},{:.1},{:.1} slot {c} +{dt} in {}", p.x, p.y, p.z, x.active.name()));
                    }
                }
                if x.active == redsim::entity::mob::ai::brain::Act::Rest
                    && b.home.is_some()
                    && let Some(since) = stamp
                    && run.world.time - since >= 1000
                {
                    near_ticks[k] += 1;
                    near.push(format!("e{k} at {:.1},{:.1},{:.1} ({} gt{})", p.x, p.y, p.z, run.world.time - since, if x.mem.walk_target.is_some() { ", walk target set" } else { "" }));
                }
                stamp_prev[k] = stamp;
                wt_prev[k] = x.mem.walk_target;
                act_prev[k] = x.active;
                home_prev[k] = b.home;
            }
            if near.len() > near_max.0 {
                near_max = (near.len(), format!("slot {c} +{dt}: {}", near.join(", ")));
            }
            // (ironspec: the bed's free tickets at the start and at every change)
            for (bi, bp) in bed_cells.iter().enumerate() {
                let f = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, *bp);
                if f != bed_free[bi] {
                    ticket_log.push(format!("{},{},{} {} at slot {c} +{dt}", bp.x, bp.y, bp.z, f.map_or("gone".to_string(), |n| format!("{n} free"))));
                    bed_free[bi] = f;
                }
            }
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
                    let aboard_now = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
                    let aboard = aboard_now == vr.rider;
                    if !aboard {
                        // (farmopt 2026-11-11: the ids as the bench holds them now, renumbered at a
                        // whole-part unload, and where the napper is)
                        let napper_in = vr.rider.and_then(|r| redsim::entity::index_of(&run.world, r)).and_then(|i| run.world.entities.list[i].vehicle);
                        let e = |x: Option<u32>| x.map(|v| format!("e{v}")).unwrap_or_else(|| "nobody".into());
                        problems.push(format!(
                            "visit (slot {}, cart {}): the napper is not back in its own cart (cart e{id} holds {}; napper {} rides {})",
                            vr.slot,
                            vr.k,
                            e(aboard_now),
                            e(vr.rider),
                            napper_in.map(|v| format!("cart e{v}")).unwrap_or_else(|| "nothing".into())
                        ));
                    }
                }
            }
        }
        // PANIC at night (the deep night, day time 13,670..22,330): every rider, and where the
        // calm ones ride
        let tod = run.world.day_time().rem_euclid(24000);
        // (tickexact 2026-11-11, ironspec's contact rule) every rider whose box touches another
        // cart's box, every tick: LivingEntity.pushEntities runs in each rider's tick, so it pushes
        // that cart away from itself (AbstractMinecart.push: 0.05 b/t at 1 apart, nothing closer
        // than 0.01) whether it walks or not; a walk only moves its box within the tick. By walk
        // state (reaching, partial, a walk target with no path, no walk target); within 0.2 too
        {
            // (ironspec's forced-phase mode) the forced villager's wake after the load
            if let Some((id, t_load, true)) = phase_watch {
                let awake = redsim::entity::index_of(&run.world, id).is_none_or(|i| !matches!(&run.world.entities.list[i].kind, redsim::entity::EntityKind::Mob(mm) if mm.sleeping_pos.is_some()));
                if awake {
                    unload_log.push(format!("slot {c} +{dt}: e{id} (phases forced) woke at +{} after the load", run.world.time - t_load));
                    phase_watch = None;
                }
            }
            // the pushes this tick, at the push (redsim's log)
            let now = redsim::entity::minecart::take_cart_pushes(&mut run.world);
            // (the push's harm, ironspec Nov 11 04:53: per pushed cart an episode from the push, its
            // displacement along the push, the peak, and the tick it is back (the displacement back to
            // 0 or below), and whether it crossed a cell boundary backward)
            for p in &now {
                if riders.contains(&p.mob)
                    && !push_eps.contains_key(&p.cart)
                    && let Some(i) = redsim::entity::index_of(&run.world, p.cart)
                {
                    let q = run.world.entities.list[i].pos;
                    let n = p.push[0].hypot(p.push[1]).max(1e-9);
                    push_eps.insert(p.cart, (t, [q.x, q.z], [p.push[0] / n, p.push[1] / n], 0.0f64, [q.x.floor() as i32, q.z.floor() as i32], false, false));
                }
            }
            let mut ended: Vec<u32> = Vec::new();
            for (cart, ep) in push_eps.iter_mut() {
                let Some(i) = redsim::entity::index_of(&run.world, *cart) else {
                    ended.push(*cart);
                    continue;
                };
                // (ironspec 05:58: a cart-on-cart impact during the episode makes it the impact's,
                // counted apart from the rider pushes')
                if tail_watch.impacted.contains(cart) {
                    ep.6 = true;
                }
                let q = run.world.entities.list[i].pos;
                let d = (q.x - ep.1[0]) * ep.2[0] + (q.z - ep.1[1]) * ep.2[1];
                ep.3 = ep.3.max(d);
                let cell = [q.x.floor() as i32, q.z.floor() as i32];
                if cell != ep.4 && ((cell[0] - ep.4[0]) as f64 * ep.2[0] + (cell[1] - ep.4[1]) as f64 * ep.2[1]) > 0.0 {
                    ep.5 = true;
                }
                let age = t - ep.0;
                if (age > 1 && d <= 0.0) || age > 200 {
                    if ep.6 {
                        harm_impact_eps += 1;
                        ended.push(*cart);
                        continue;
                    }
                    harm_eps += 1;
                    if ep.3 > harm_peak.0 {
                        harm_peak = (ep.3, format!("slot {c} +{dt}: cart #{cart} at {:.2},{:.2},{:.2}", q.x, q.y, q.z));
                    }
                    if age > 200 {
                        harm_unreturned += 1;
                    } else {
                        harm_return_max = harm_return_max.max(age);
                    }
                    if ep.5 {
                        harm_crossed += 1;
                        if harm_cross_log.len() < 4 {
                            harm_cross_log.push(format!("slot {c} +{dt}: cart #{cart}"));
                        }
                    }
                    ended.push(*cart);
                }
            }
            for cart in ended {
                push_eps.remove(&cart);
            }
            let mut by_rider: Vec<String> = Vec::new();
            for p in &now {
                let Some(k) = riders.iter().position(|r| *r == p.mob) else {
                    push_other += 1;
                    continue;
                };
                let kind = rider_walk_kind(&run.world, p.mob).map_or(3, |x| x as usize);
                pushes[kind] += 1;
                push_big = push_big.max(p.push[0].hypot(p.push[1]));
                if let Some(i) = redsim::entity::index_of(&run.world, p.mob) {
                    let q = run.world.entities.list[i].pos;
                    *push_cells.entry([q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32]).or_insert(0) += 1;
                    by_rider.push(format!("e{k} at {:.2},{:.2},{:.2} on cart #{} ({:.3} apart, {})", q.x, q.y, q.z, p.cart, p.dist, ["reaching", "partial", "no path", "no walk"][kind]));
                }
            }
            if by_rider.len() > push_max.0 {
                push_max = (by_rider.len(), format!("slot {c} +{dt}: {}", by_rider.join(", ")));
            }
            let mut touching_now: Vec<String> = Vec::new();
            for (k, r) in riders.iter().enumerate() {
                let kind = rider_walk_kind(&run.world, *r).map_or(3, |x| x as usize);
                walkers[kind] += 1;
                if redsim::entity::touching_cart(&run.world, *r, 0.0) {
                    rider_touch_ticks[kind] += 1;
                    if let Some(i) = redsim::entity::index_of(&run.world, *r) {
                        let p = run.world.entities.list[i].pos;
                        *touch_cells.entry([p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]).or_insert(0) += 1;
                        touching_now.push(format!("e{k} at {:.2},{:.2},{:.2} ({})", p.x, p.y, p.z, ["reaching", "partial", "no path", "no walk"][kind]));
                    }
                } else if redsim::entity::touching_cart(&run.world, *r, 0.2) {
                    walk_near[kind] += 1;
                }
            }
            if touching_now.len() > touch_max.0 {
                touch_max = (touching_now.len(), format!("slot {c} +{dt}: {}", touching_now.join(", ")));
            }
        }
        // (tickexact 2026-11-11, ironspec: the freeze exposure) the villagers calm in REST with a
        // live CANT_REACH streak and HOME held, this tick (the napper on its visit included): a long
        // freeze now releases each one's HOME on the resume, and two holders of the one bed losing
        // it together leave one homeless (the ticket caps at 1)
        if (13670..22330).contains(&tod) {
            let mut exposed: Vec<String> = Vec::new();
            let mut probe: Vec<(usize, BlockPos, [i32; 3])> = Vec::new();
            for (k, r) in riders.iter().enumerate() {
                let Some(i) = redsim::entity::index_of(&run.world, *r) else { continue };
                expose_night[k] += 1;
                let redsim::entity::EntityKind::Mob(m) = &run.world.entities.list[i].kind else { continue };
                let (Some(b), Some(x)) = (m.brain.as_deref(), m.brain.as_deref().and_then(|b| b.ex.as_deref())) else { continue };
                if x.active == redsim::entity::mob::ai::brain::Act::Panic {
                    last_panic[k] = run.world.time;
                }
                if let (Some(home), redsim::entity::mob::ai::brain::Act::Rest, Some(since)) = (b.home, x.active, x.mem.cant_reach_since) {
                    expose_ticks[k] += 1;
                    let p = run.world.entities.list[i].pos;
                    let streak = run.world.time - since;
                    exposed.push(format!("e{k} at {:.1},{:.1},{:.1} ({streak} gt)", p.x, p.y, p.z));
                    if tm.exposure_log {
                        let path = match &x.mem.path {
                            Some(pp) if pp.reached => "reaching",
                            Some(_) => "partial",
                            None => "none",
                        };
                        exposure_lines.push(format!("exposure {} {c} {dt} e{k} {:.2} {:.2} {:.2} {streak} {} {path}", run.world.time, p.x, p.y, p.z, (run.world.time - last_panic[k] <= 20) as u8));
                    }
                    // (the position map: per block cell, the exposed rider-ticks, the longest
                    // streak, and how many of those ticks the rider had panicked within 20 gt)
                    let cell = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
                    let e = expose_map.entry(cell).or_insert((0, 0, 0, 0));
                    e.0 += 1;
                    e.1 = e.1.max(streak);
                    if run.world.time - last_panic[k] <= 20 {
                        e.2 += 1;
                    }
                    if tm.reach_probe {
                        probe.push((i, home, cell));
                    }
                }
            }
            // (`--reach-probe`, ironspec's open-top question as a counterfactual: does an exposed
            // rider have a path from its cart to within 1 of its bed (AcquirePoi's reach), on a copy
            // of its navigation: no side effect)
            for (i, home, cell) in probe {
                let reach = redsim::entity::mob::ai::with_nav(&mut run.world, i, |w, nm, nav, arena| {
                    let mut copy = nav.clone();
                    copy.create_path_positions(w, nm, &[home], 1, arena).is_some_and(|p| p.reached)
                });
                if reach && let Some(e) = expose_map.get_mut(&cell) {
                    e.3 += 1;
                }
            }
            if exposed.len() >= 2 {
                expose_multi += 1;
            }
            if exposed.len() > expose_max.0 {
                expose_max = (exposed.len(), format!("slot {c} +{dt}: {}", exposed.join(", ")));
            }
        }
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
                // (with the x-queue, its row along z 24 too, each cart's x printed)
                let xq = o.adm_xq > 0;
                let ps: Vec<String> = carts.iter().enumerate().filter_map(|(k, id)| pos_of(&run.world, *id).filter(|p| (p.x > -0.5 && p.x < 1.5 && p.z > 19.5 && p.z < z1 as f64) || (xq && p.z > 23.5 && p.z < 25.5 && p.x > -0.5 && p.x < 9.5 && p.y > 7.0)).map(|p| {
                    let v = run.world.entities.list.iter().find(|e| e.id == *id).map_or(0.0, |e| e.delta_movement.horizontal_distance());
                    if xq { format!("{k}@{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, v) } else { format!("{k}@{:.2},{:.2} v{:.3}", p.y, p.z, v) }
                })).collect();
                let on = |b: Option<BlockPos>| b.map_or('?', |b| if matches!(run.world.get(b), Block::RedstoneBlock) { 'P' } else { '-' });
                let gate = lay.fifo.last().map_or('?', |(g, _)| if matches!(run.world.get(*g), Block::FenceGate { open: true, .. }) { 'o' } else { '-' });
                println!("tt c{c} +{dt}: B{} P{} G{} | {}", on(lay.esc), on(lay.esc_p), gate, ps.join(" "));
            }
        }
        // (`--watch-mob`, farmopt 2026-11-11: a mob every gt over slots S0..S1: its place, pose,
        // vehicle, activities, behaviours, memories and target, and the hostiles within 16 blocks
        // of it with their distance, the line of sight from its eyes and their target)
        if c >= tm.watch_mob_slots.0 && c <= tm.watch_mob_slots.1 {
            for r in tm.watch_mob.iter().flatten() {
                let id = match *r {
                    RiderRef::Id(id) => Some(id),
                    RiderRef::Visit(s) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.rider),
                    RiderRef::Batch(s, j) => visits.iter().find(|vr| vr.slot == s).and_then(|vr| vr.batch.get(j).copied()).and_then(|cart| redsim::entity::index_of(&run.world, cart)).and_then(|i| run.world.entities.list[i].passengers.first().copied()),
                };
                let Some(i) = id.and_then(|id| redsim::entity::index_of(&run.world, id)) else {
                    println!("mob c{c} +{dt}: {r:?}: not in the world");
                    continue;
                };
                let e = &run.world.entities.list[i];
                let f = redsim::exact::entity_frame(&run.world, e);
                let mem = f.memories.as_ref().map(|m| m.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                let eye = e.pos.add(0.0, e.kind.eye_height() as f64, 0.0);
                const HOSTILE: [&str; 11] = ["drowned", "evoker", "husk", "illusioner", "pillager", "ravager", "vex", "vindicator", "zoglin", "zombie", "zombie_villager"];
                let dist = |a: redsim::entity::vec3::Vec3, b: redsim::entity::vec3::Vec3| a.distance_to_sqr(b).sqrt();
                let hostiles: Vec<String> = run
                    .world
                    .entities
                    .list
                    .iter()
                    .filter(|x| !x.removed && matches!(&x.kind, redsim::entity::EntityKind::Mob(m) if HOSTILE.contains(&m.ty.def().id)) && dist(x.pos, e.pos) < 16.0)
                    .map(|x| {
                        let xf = redsim::exact::entity_frame(&run.world, x);
                        let los = redsim::entity::mob::los::has_line_of_sight(&run.world, eye, x.pos.add(0.0, x.kind.eye_height() as f64, 0.0));
                        format!("e{} {} {:.2},{:.2},{:.2} d{:.2} los {} tgt {}", x.id, xf.ty, x.pos.x, x.pos.y, x.pos.z, dist(x.pos, e.pos), if los { "y" } else { "n" }, xf.target.as_deref().unwrap_or("-"))
                    })
                    .collect();
                println!(
                    "mob c{c} +{dt}: e{} {} {:.2},{:.2},{:.2} pose {} veh {} act {} beh {} tgt {} mem [{mem}] | hostiles: {}",
                    e.id,
                    f.ty,
                    e.pos.x,
                    e.pos.y,
                    e.pos.z,
                    f.pose.as_deref().unwrap_or("-"),
                    f.vehicle.as_deref().unwrap_or("-"),
                    f.activities.as_deref().unwrap_or("-"),
                    f.behaviors.as_deref().unwrap_or("-"),
                    f.target.as_deref().unwrap_or("-"),
                    if hostiles.is_empty() { "-".to_string() } else { hostiles.join("; ") }
                );
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
    out.push(format!("home log: {}", if home_log.is_empty() { "no HOME changed".to_string() } else { format!("{} changes: {}", home_log.len(), home_log.iter().take(40).cloned().collect::<Vec<_>>().join("; ")) }));
    out.push(format!("bed tickets: {}", if ticket_log.is_empty() { "no bed".to_string() } else { ticket_log.iter().take(40).cloned().collect::<Vec<_>>().join("; ") }));
    {
        let mut by: Vec<(&String, &(u64, i64))> = erase_by.iter().collect();
        by.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        let mut cells: Vec<(&[i32; 3], u64, &std::collections::BTreeMap<String, u64>)> = erase_at.iter().map(|(c, m)| (c, m.values().sum::<u64>(), m)).collect();
        cells.sort_by(|a, b| b.1.cmp(&a.1));
        out.push(format!(
            "stamp erasures (a rider's CANT_REACH gone: the walk it had the tick before): {} ({}); stamps set: {}",
            erase_by.values().map(|v| v.0).sum::<u64>(),
            by.iter().map(|(k, (n, a))| format!("{k}: {n}, the oldest {a} gt")).collect::<Vec<_>>().join("; "),
            stamp_set_by.iter().map(|(k, n)| format!("in {k} {n}")).collect::<Vec<_>>().join(", ")
        ));
        out.push(format!(
            "stamp erasures by cell, the 16 most: {}",
            cells.iter().take(16).map(|(c, n, m)| format!("{} {} {}: {n} ({})", c[0], c[1], c[2], m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ")
        ));
        let mut old: Vec<(usize, &(i64, String))> = stamp_oldest.iter().enumerate().filter(|(_, s)| s.0 > 0).collect();
        old.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        // (ironspec's bar: every rider's oldest stamp in normal running at most 1,000 gt)
        let worst = old.first().map_or(0, |(_, (a, _))| *a);
        out.push(format!(
            "stamps' oldest age per rider (any activity): the oldest {worst} gt ({}: the bar is 1,000); the 8 oldest: {}",
            if worst <= 1000 { "within the bar" } else { "OVER the bar" },
            old.iter().take(8).map(|(k, (a, w))| format!("e{k} {a} gt ({w})")).collect::<Vec<_>>().join("; ")
        ));
        let mut per: Vec<(usize, u64)> = near_ticks.iter().copied().enumerate().filter(|(_, n)| *n > 0).collect();
        per.sort_by(|a, b| b.1.cmp(&a.1));
        out.push(format!(
            "near release (REST, HOME held, a CANT_REACH stamp >= 1,000 gt, a WALK_TARGET or not): {} rider-ticks, at most {} at once{}; by rider: {}; HOME releases: {}",
            near_ticks.iter().sum::<u64>(),
            near_max.0,
            if near_max.0 > 0 { format!(" ({})", near_max.1) } else { String::new() },
            if per.is_empty() { "-".to_string() } else { per.iter().take(8).map(|(k, n)| format!("e{k} {n}")).collect::<Vec<_>>().join(", ") },
            home_releases.len()
        ));
    }
    {
        let mut fr: Vec<(usize, f64)> = expose_night.iter().zip(&expose_ticks).enumerate().filter(|(_, (n, _))| **n > 0).map(|(k, (n, e))| (k, *e as f64 / *n as f64)).collect();
        fr.sort_by(|a, b| b.1.total_cmp(&a.1));
        out.push(format!(
            "freeze exposure (deep night: calm in REST, a live CANT_REACH streak, HOME held): at most {} at once{}; ticks with 2 or more: {expose_multi}; the most exposed: {}",
            expose_max.0,
            if expose_max.0 > 0 { format!(" ({})", expose_max.1) } else { String::new() },
            fr.iter().take(6).map(|(k, f)| format!("e{k} {:.2}%", 100.0 * f)).collect::<Vec<_>>().join(", ")
        ));
        let mut cells: Vec<([i32; 3], (u64, i64, u64, u64))> = expose_map.into_iter().collect();
        cells.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        let total: u64 = cells.iter().map(|(_, v)| v.0).sum();
        let reachable: u64 = cells.iter().map(|(_, v)| v.3).sum();
        out.push(format!(
            "freeze exposure map (block cell: exposed rider-ticks, the longest streak, the share panicking within 20 gt before{}), the 16 most: {}",
            if tm.reach_probe { ", the share with a path to within 1 of the bed (--reach-probe)" } else { "" },
            if cells.is_empty() { "-".to_string() } else { cells.iter().take(16).map(|(c, (n, s, p, r))| format!("{} {} {}: {n} ticks, {s} gt, {:.0}%{}", c[0], c[1], c[2], 100.0 * *p as f64 / *n as f64, if tm.reach_probe { format!(", {:.0}% reachable", 100.0 * *r as f64 / *n as f64) } else { String::new() })).collect::<Vec<_>>().join("; ") }
        ));
        if tm.reach_probe {
            out.push(format!("reach probe: of {total} exposed rider-ticks, {reachable} ({:.1}%) had a path from the cart to within 1 of the bed (an open bed top would clear their stamps on the next HOME walk); the rest only panic coverage clears", if total > 0 { 100.0 * reachable as f64 / total as f64 } else { 0.0 }));
        }
        {
            let mut cells: Vec<([i32; 3], u64)> = touch_cells.into_iter().collect();
            cells.sort_by(|a, b| b.1.cmp(&a.1));
            let mut pcells: Vec<([i32; 3], u64)> = push_cells.into_iter().collect();
            pcells.sort_by(|a, b| b.1.cmp(&a.1));
            out.push(format!(
                "rider pushes on other carts (counted at the push: a rider's LivingEntity.pushEntities moving a cart not its own, AbstractMinecart.push past its 0.01): {} (by the pusher's walk state: reaching {}, partial {}, a walk target with no path {}, no walk target {}); the largest {:.4} b/t; the most in one tick {}{}; the cells: {}; other mobs' pushes on carts {}",
                pushes.iter().sum::<u64>(), pushes[0], pushes[1], pushes[2], pushes[3], push_big, push_max.0,
                if push_max.0 > 0 { format!(" ({})", push_max.1) } else { String::new() },
                if pcells.is_empty() { "-".to_string() } else { pcells.iter().take(8).map(|(c, n)| format!("{} {} {}: {n}", c[0], c[1], c[2])).collect::<Vec<_>>().join("; ") },
                push_other
            ));
            out.push(format!(
                "the rider pushes' harm (per pushed cart from the push: its displacement along the push; an episode with a cart-on-cart impact in it is the impact's, counted apart): {harm_eps} episodes; the peak {:.3} b{}; back within {harm_return_max} gt at most, {harm_unreturned} not back within 200 gt; {harm_crossed} crossed a cell boundary backward{}; {harm_impact_eps} episodes with an impact (see the arrivals)",
                harm_peak.0,
                if harm_peak.1.is_empty() { String::new() } else { format!(" ({})", harm_peak.1) },
                if harm_cross_log.is_empty() { String::new() } else { format!(" ({})", harm_cross_log.join(", ")) }
            ));
            out.push(format!(
                "riders touching another cart's box (each pushes that cart away every tick, walking or not; LivingEntity.pushEntities): {} rider-ticks (following a reaching path {}, a partial path {}, a walk target with no path {}, no walk target {}); within 0.2 of one: {} (reaching {}, partial {}, no path {}, no walk {}); the most touching at once {}{}; the cells with the most touching ticks: {}; riders' walk states, every tick: reaching {}, partial {}, a walk target with no path {}, no walk target {}",
                rider_touch_ticks.iter().sum::<u64>(), rider_touch_ticks[0], rider_touch_ticks[1], rider_touch_ticks[2], rider_touch_ticks[3],
                walk_near.iter().sum::<u64>(), walk_near[0], walk_near[1], walk_near[2], walk_near[3], touch_max.0,
                if touch_max.0 > 0 { format!(" ({})", touch_max.1) } else { String::new() },
                if cells.is_empty() { "-".to_string() } else { cells.iter().take(8).map(|(c, n)| format!("{} {} {}: {n}", c[0], c[1], c[2])).collect::<Vec<_>>().join("; ") },
                walkers[0], walkers[1], walkers[2], walkers[3]
            ));
        }
        out.extend(exposure_lines);
    }
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
        if lay.esc_adm_a1.is_some() {
            out.push(format!("riders in the pressed queue (landing to P) with a walk target, rider-ticks by path: reaching {}, partial {}, no path {}{}; riders whose box touches another cart's, walking or not (contact alone pushes: LivingEntity.pushEntities, bridged in gate-escg6l-v3; must be 0): {walk_touch} (not walking {touch_idle}){}", walk_kinds[0], walk_kinds[1], walk_kinds[2], if walking_log.is_empty() { String::new() } else { format!(" ({})", walking_log.join("; ")) }, if walk_touch_log.is_empty() { String::new() } else { format!(" ({})", walk_touch_log.join("; ")) }));
            if !hold_cells.is_empty() {
                let latch = hold_cells.len() > 2;
                let worst_latch = slot_holds.iter().map(|s| s.latch_max).max().unwrap_or(0);
                let worst_on = slot_holds.iter().map(|s| s.land_on_max).max().unwrap_or(0);
                let room_min = slot_holds.iter().filter_map(|s| s.room_slack).min();
                let merged: Vec<String> = slot_holds.iter().enumerate().filter(|(_, s)| s.land_edges < s.crossings).map(|(i, s)| format!("{i} {}:{}", s.land_edges, s.crossings)).collect();
                out.push(format!(
                    "entry hold per slot: {}latch set at most {worst_latch} gt{}, landing on unbroken at most {worst_on} gt, room slack at REL at least {} gt (-1: on); landing on-edges short of the carts crossing it in {} of {} slots{}",
                    if latch { "" } else { "(no latch) " },
                    if latch { format!(" (strand threshold {} gt: slack {})", tm.hold_strand_t, tm.hold_strand_t - worst_latch) } else { String::new() },
                    room_min.map_or("-".to_string(), |v| if v == i64::MAX { "never on".to_string() } else { v.to_string() }),
                    merged.len(),
                    slot_holds.len(),
                    if merged.is_empty() { String::new() } else { format!(" (slot edges:carts {})", merged.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }
                ));
                out.push(format!(
                    "entry hold slots (latch/landing-on/room-slack gt, edges:carts, progress at REL): {}",
                    slot_holds.iter().enumerate().map(|(i, s)| format!("{i} {}/{}/{} {}:{} p{}", s.latch_max, s.land_on_max, s.room_slack.map_or("-".to_string(), |v| if v == i64::MAX { "n".to_string() } else { v.to_string() }), s.land_edges, s.crossings, s.progress.map_or("-".to_string(), |p| p.to_string()))).collect::<Vec<_>>().join(", ")
                ));
                out.push(format!(
                    "entry hold (the tail's landing and room detectors at REL{}): on at {hold_blocked} of {hold_rels} releases{}; slack from both off to REL at least {} gt ({}){}",
                    if tm.entry_hold { ", the release waiting while either is on" } else { ", measured only" },
                    if hold_log.is_empty() { String::new() } else { format!(" ({})", hold_log.join(", ")) },
                    if hold_slack_min.0 == i64::MAX { "-".to_string() } else { hold_slack_min.0.to_string() },
                    hold_slack_min.1,
                    if tm.entry_hold { format!("; releases held {} times (waits {:?} gt), dropped at the reset {hold_dropped}", hold_delays.len(), hold_delays) } else { String::new() }
                ));
            }
            if !skip_log.is_empty() || tm.adm_skip_req.iter().any(|r| r.is_some()) {
                out.push(format!("skip requests (admission control's latch): {}{}", if skip_log.is_empty() { "none acted".to_string() } else { skip_log.join("; ") }, if skip_latch { "; still latched at the end" } else { "" }));
            }
            out.push(format!("backward on P while it is powered (admission control): {p_back_ticks} ticks{}", if p_back.is_empty() { String::new() } else { format!(" ({})", p_back.join("; ")) }));
            out.push(format!(
                "admission margins: the fastest front at a G0 opening {:.3} b/t, {:.3} short of G0's box ({}); the launched cart's rider and the next cart on P at least {} apart ({}); the admitted cart's front reached P's end at least {} gt after the launched cart's rider left G_P's plane ({})",
                g0_open_vmax.0, g0_open_vmax.1, g0_open_vmax.2,
                if rider_gap_min.0 == f64::MAX { "-".to_string() } else { format!("{:.3}", rider_gap_min.0) }, rider_gap_min.1,
                if rider_ticks_min.0 == i64::MAX { "-".to_string() } else { rider_ticks_min.0.to_string() }, rider_ticks_min.1
            ));
            if p_back_ticks > 0 {
                problems.push(format!("backward on P while powered: {p_back_ticks} ticks ({})", p_back.first().cloned().unwrap_or_default()));
            }
        }
        out.extend(arr_stream.iter().cloned());
        out.push(det_edges.line(fx as i32, fz as i32));
        out.extend(tail_watch.lines());
        if lay.esc_adm_a1.is_some() {
            out.push(format!("launches into a full slot 0 (G_P opening while a cart rests on slot 0's cell): {launch_into_full}{}", if launch_full_log.is_empty() { String::new() } else { format!(" ({})", launch_full_log.join(", ")) }));
        }
        if o.adm_xq > 0 {
            out.push(format!("the x-queue's flats (curve and transition cell): the longest rest there {} gt{}", flat_rest_max.0, if flat_rest_max.1.is_empty() { String::new() } else { format!(" ({})", flat_rest_max.1) }));
        }
        if tm.reg_interlock {
            out.push(format!("register interlock (ideal sensors, bench-driven, advances {} gt apart): carts held at advances {reg_stuck_total} (cart-advances), launches held {il_skips}; launch slack (G_P's timed opening minus slot 0 read empty) at least {} gt ({}){}", tm.advance_every, if il_slack_min.0 == i64::MAX { "-".to_string() } else { il_slack_min.0.to_string() }, il_slack_min.1, if reg_held.is_empty() { String::new() } else { format!(" ({})", reg_held.join("; ")) }));
        }
        out.push(det_edges.timing_line(fx as i32, fz as i32));
        out.push(plane_gaps.line(fx as i32, fz as i32));
        out.push(stand_panic.line());
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
        if !small.is_empty() {
            out.push(format!("stand departures under 8 gt apart or touching: {}", small.join(", ")));
        }
        out.push(format!("back through a gate: {}{}", back_through.len(), if back_through.is_empty() { String::new() } else { format!(" ({})", back_through.iter().take(8).cloned().collect::<Vec<_>>().join(", ")) }));
        out.push(format!("{}; riders not seated: {}{}", rail_watch.line(), unseat_log.len(), if unseat_log.is_empty() { String::new() } else { format!(" ({})", unseat_log.iter().take(6).cloned().collect::<Vec<_>>().join(", ")) }));
        if o.stand_wired && !latch_sets.is_empty() {
            out.push(format!("latch set (T2 out after +55): +{}..+{} over {} slots", latch_sets.iter().min().unwrap(), latch_sets.iter().max().unwrap(), latch_sets.len()));
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

    /// `--reload-orders`' forms (ironspec 2026-11-11 05:12: `all` samples 40 orders, save, reverse
    /// and seed:1..38; `all:N` N orders) and the row's Wilson interval (tick-exact's 6 of 32:
    /// 9-35%; 0 of 40: 0-9%).
    #[test]
    fn reload_orders_and_their_rate() {
        use redsim::reload::Order;
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let all = parse_reload_orders(&s(&["all"])).unwrap();
        assert_eq!((all.len(), all[0], all[1], all[2], all[39]), (40, Order::Save, Order::Reverse, Order::Seed(1), Order::Seed(38)));
        let ten = parse_reload_orders(&s(&["all:10"])).unwrap();
        assert_eq!((ten.len(), ten[9]), (10, Order::Seed(8)));
        assert_eq!(parse_reload_orders(&s(&["all:1"])).unwrap(), vec![Order::Save]);
        assert_eq!(parse_reload_orders(&s(&["save", "seed:3-5"])).unwrap(), vec![Order::Save, Order::Seed(3), Order::Seed(4), Order::Seed(5)]);
        assert!(parse_reload_orders(&s(&["seed:5-3"])).is_err() && parse_reload_orders(&s(&["all:x"])).is_err() && parse_reload_orders(&s(&["all:0"])).is_err() && parse_reload_orders(&s(&["sideways"])).is_err());
        let (lo, hi) = wilson95(0, 40);
        assert!(lo == 0.0 && (hi * 100.0).round() == 9.0);
        // (tick-exact's row 300,50: 6 misses in 32 orders, Wilson 95% 9-35%)
        let (lo, hi) = wilson95(6, 32);
        assert_eq!(((lo * 100.0).round(), (hi * 100.0).round()), (9.0, 35.0));
        let (lo, hi) = wilson95(0, 32);
        assert!(lo == 0.0 && (hi * 100.0).round() == 11.0);
    }

    /// The rider acts parse (farmopt 2026-11-11: day-1's positive control for the unload id-remap
    /// check): a visit's napper or an entity number, dismount or swap; the old forms unchanged.
    #[test]
    fn rider_acts_parse() {
        let a = parse_act("302,10,dismount,visit:298").unwrap();
        assert_eq!((a.slot, a.dt, a.ride, a.unload.is_none()), (302, 10, Some(RideAct::Dismount(RiderRef::Visit(298))), true));
        let a = parse_act("302,10,swap,visit:298,1042").unwrap();
        assert_eq!(a.ride, Some(RideAct::Swap(RiderRef::Visit(298), RiderRef::Id(1042))));
        assert_eq!(parse_act("303,25,swap,visit:298,visit:298/0").unwrap().ride, Some(RideAct::Swap(RiderRef::Visit(298), RiderRef::Batch(298, 0))));
        assert!(parse_act("302,10,swap,visit:298").is_err() && parse_act("302,10,dismount,cart:3").is_err());
        let u = parse_act("300,80,unload,4000,seed:3").unwrap();
        assert!(u.ride.is_none() && u.unload == Some(None) && u.hold == Some(4000));
        let b = parse_act("5,10,1,2,3,minecraft:stone,4").unwrap();
        assert!(b.ride.is_none() && b.unload.is_none() && b.hold == Some(4));
    }

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
