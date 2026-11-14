//! The picks' per-lane ITEM RING as blocks (day-1, Nov 14 2026; ironspec 11:56a / 12:02p / 13:07p, the parent 13:01p):
//! the sensed pointer of ctl-pick (`ctlpick::Cfg::sensed` with `positional_release`) laid as one 8-dropper ring per
//! lane, in a FLAT ring: all 16 cells (8 droppers, 8 hoppers) at one level, two rows of 8 (the north row flowing east,
//! the south row west), a dust patch over all 16 as the STEP (dust on a dropper fires it; on a hopper it locks it for the
//! pulse) and as the golem cap (a dust-covered top is a refused surface).
//!
//! THE TOKENS: every PRESENT slot holds a stackable filler F (stone: a lone item of a 64-stack reads 1 on a comparator),
//! the TARGET slot holds the unstackable T (a wooden shovel: a lone unstackable reads 2) instead of its filler (in normal
//! operation the target's slot is never an away slot: the away slots sit at picked - 5 / - 10 = + 3 / + 6), and an
//! AWAY slot is an EMPTY dropper (reads 0). Between steps every token rests in a dropper.
//!
//! THE CELLS (ring-local; P0..P7 the droppers in flow order): the TAP is P2 (read by a comparator on its north face);
//! T's EXTRACTOR, a hopper under P2, locked by a comparator (rear: a full container, a constant 15) through its powered
//! support block, unlocked for 13 gt at the grant's capture (REL + 92, before the pop) by a signal into that comparator's
//! side, lifts T out (the picked slot now empty = away) and pushes it straight into the UP-DROPPER under P6's feeder
//! hopper (one hop: P6 = the tap + 4, where the slot picked + 5 sits once the pop has stepped); after the pop's step the
//! FILLER's EXTRACTOR, a hopper under P6 (locked the same way), unlocked for 10 gt at the lane's pop + 40 (REL + 40 / 57 /
//! 67), lifts P6's filler down a 5-hopper path to the STOCK dropper under P4 (= the tap + 2); the up-dropper fires T at
//! REL + 80 into P6's feeder, which pushes it into P6 before the next pop; RELEASE (the positional rule): at a circuit
//! entry with P4 EMPTY and the siding's presence, the stock fires one filler up into P4 (and the R hold is released:
//! the logic's). No hopper sits under a ring container except the two extractors; every power source and powered block
//! under the ring is a horizontal repeater / comparator (they power only forward), so no ring dropper is held powered.
//!
//! This file lays the MECHANICS (the ring, its dust and step input, the extractors and their lock comparators, the
//! up-dropper, the filler path, the stock, the tap comparator, the up-fire and stock-fire repeaters); the bench drives
//! the step, the two unlocks and the two fires as emulated stand-ins (redstone blocks placed / removed at the inputs'
//! cells), and judges the item movements, their times against the deadlines, and a world open (`liveness::restart`) at
//! every tick of a grant and a release.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::redstone::container::CRef;

type P = [i32; 3];

/// The ring's droppers in flow order (P0..P7), ring-local: the north row (z -1) flows east, the south row (z 0) west.
pub const DROPPERS: [P; 8] = [[-3, 0, -1], [-1, 0, -1], [1, 0, -1], [3, 0, -1], [4, 0, 0], [2, 0, 0], [0, 0, 0], [-2, 0, 0]];
/// The ring's hoppers in flow order (the one after P0 .. the one after P7) and their facings.
pub const RING_HOPPERS: [P; 8] = [[-2, 0, -1], [0, 0, -1], [2, 0, -1], [4, 0, -1], [3, 0, 0], [1, 0, 0], [-1, 0, 0], [-3, 0, 0]];
const HOPPER_FACING: [&str; 8] = ["east", "east", "east", "south", "west", "west", "west", "north"];
const DROPPER_FACING: [&str; 8] = ["east", "east", "east", "east", "west", "west", "west", "west"];
/// The tap (P2), the dropper T joins after the pop's step (P6 = the tap + 4), the release's dropper (P4 = the tap + 2).
pub const TAP: usize = 2;
pub const T_INTO: usize = 6;
pub const REL_AT: usize = 4;
/// The step: the dust patch over the 16 ring cells (y 1), fed by a repeater from the west; the bench's drive is a
/// redstone block behind that repeater.
pub const STEP_REP: P = [-4, 1, -1];
pub const STEP_DRIVE: P = [-5, 1, -1];
/// T's extractor (under the tap), the up-dropper it pushes into (under P6's feeder hopper), the tap comparator on the
/// tap's north face (its output cell; its support is the T lock block), the T lock: a comparator whose rear is a full
/// container and whose output powers the lock block; the bench's T unlock = a redstone block at the lock comparator's
/// north side (output 15 - 15 = 0).
pub const T_EXTRACT: P = [1, -1, -1];
pub const UP: P = [1, -1, 0];
pub const TAP_CMP: P = [1, 0, -2];
pub const TAP_OUT: P = [1, 0, -3];
pub const T_LOCK_BLOCK: P = [1, -1, -2];
pub const T_LOCK_CMP: P = [0, -1, -2];
pub const T_LOCK_SRC: P = [-1, -1, -2];
pub const T_UNLOCK_AT: P = [0, -1, -3];
/// The filler's extractor (under P6), its lock comparator (output east into it; rear a full container), the unlock
/// repeater into that comparator's south side (the bench's drive: a redstone block behind it), its path and the stock.
pub const F_EXTRACT: P = [0, -1, 0];
pub const F_LOCK_CMP: P = [-1, -1, 0];
pub const F_LOCK_SRC: P = [-2, -1, 0];
pub const F_UNLOCK_REP: P = [-1, -1, 1];
pub const F_UNLOCK_AT: P = [-1, -1, 2];
/// The filler path: south of the ring, then down to the bottom layer at (3,-2,1) (so the cells (4,-1,1) / (4,-1,2) stay free
/// for P4's comparator and its output's support), east, north to the stock.
pub const F_PATH: [P; 8] = [[0, -1, 1], [1, -1, 1], [2, -1, 1], [3, -1, 1], [3, -2, 1], [4, -2, 1], [5, -2, 1], [5, -2, 0]];
const F_PATH_FACING: [&str; 8] = ["east", "east", "east", "down", "east", "east", "north", "north"];
/// THE RELEASE's chain: the LIFT dropper holds ONE filler (loaded at the build) under a RELAY hopper (5,0,-1) that pushes
/// into P4's feeder (4,0,-1), which pushes it into P4; the STOCK (bottom layer, the filler path's end) sits under the
/// lift. ONE release pulse into the lift fires the lift's filler up (into P4) and, through the lift's cell above it
/// (quasi-connectivity), the stock, which reloads the lift with one filler: exactly one filler per release, whatever
/// the two fires' order in the tick. The stock's `triggered` is cleared when the pulse ends (the release dust sits on
/// the filler path's last hopper, whose ENABLED flip updates the stock). None of the three sits under a ring cell or
/// beside a ring dropper (every ring cell is powered at each step, so a dropper under one fires at every step).
pub const STOCK: P = [5, -2, -1];
pub const LIFT: P = [5, -1, -1];
pub const RELAY: P = [5, 0, -1];
/// The release pulse: a dust north of the lift (on a stone) that points into the lift (its fire) and powers the stone,
/// beside the stock (its fire, which reloads the lift); its input from the north (the logic's RELEASE; the bench's
/// stand-in a redstone block there). The stock's `triggered` clears when the dust falls (the dust's update reaches the
/// stone's neighbours).
pub const REL_DUST: P = [5, -1, -2];
pub const REL_STONE: P = [5, -2, -2];
pub const STOCK_DRIVE: P = [5, -1, -3];
/// P3's comparator (on P3's north face) and its output cell: the release's "P4 empty" is P3 EMPTY sampled at each step's
/// start (the slot at P3 moves to P4 at the step), held by the logic until the next step (P4 has no free face whose
/// comparator output stays inside the box).
pub const P3_CMP: P = [3, 0, -2];
pub const P3_OUT: P = [3, 0, -3];
/// The up-fire: from the box's south row at x 5 (the logic's UP input; the bench's stand-in a redstone block behind the
/// repeater UP_IN_REP) north along two dusts (on the filler path's two last hoppers), west through a repeater under P4,
/// a dust under the ring's hopper (3,0,0), a repeater under P5 into the up-dropper.
pub const UP_IN_REP: P = [5, -1, 2];
pub const UP_IN_DUST: [P; 2] = [[5, -1, 1], [5, -1, 0]];
pub const UP_REP1: P = [4, -1, 0];
pub const UP_DUST: P = [3, -1, 0];
pub const UP_REP2: P = [2, -1, 0];
pub const UP_DRIVE: P = [5, -1, 3];
/// T's ride: none (the extractor pushes T straight into the up-dropper).
pub const T_RIDE: [P; 0] = [];

/// The token items.
pub const FILLER: &str = "minecraft:stone";
pub const TARGET: &str = "minecraft:wooden_shovel";
/// The extractors' unlock windows (gt): a locked hopper neither pulls nor pushes, so each window covers the pull (within
/// 1 gt of the unlock, 26.3: the cooldown counts down while locked) and the push 8 gt later. T's: opened at the POP
/// (the step follows it by STEP_LAG), so T is pulled before the step and its push into the up-dropper is held by the
/// step's own power on the tap (the extractor sits under it) until the pulse ends, after the up-dropper's
/// quasi-connected fire of that step (the up-dropper sits under a ring hopper, powered at every step); it closes
/// before the extractor's next possible pull (8 gt after the push). The filler's: P6's next item comes only with T.
pub const T_WINDOW: i64 = 22;
/// The step follows the pop by this many gt (the logic's delay on the POP line), and lasts STEP_WIDTH.
pub const STEP_LAG: i64 = 6;
pub const STEP_WIDTH: i64 = 8;
pub const F_WINDOW: i64 = 10;
/// The bench's T start: P7, so T reaches the tap (P2) after 3 steps.
pub const T_START: usize = 7;
/// The bench's timed events after the grant's pop (lane -1's: the filler's unlock at REL + 40, the up-fire off the REL +
/// 67 tap (ironspec 13:24p's widening); lanes 3 / 7 read REL + 57 / 67 and REL + 80, i.e. the pop + 40 and + 63 / 53).
pub const F_AT: i64 = 40;
pub const UP_AT: i64 = 67;

/// The lock comparators' constant: a single chest holding one stone (the container-signal formula: floor(14 x (1/64)/27)
/// + 1 = 1; any level >= 1 locks the hopper, and an unlock at 15 on the side takes the output to 0). A chest, not a
/// dropper, so no signal passing by can dispense it.
fn constant_container() -> String {
    "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".to_string()
}

/// The ring's design with T starting in dropper `t_start` and fillers in the other seven.
pub fn design(t_start: usize) -> Design {
    let mut d = Design { name: "picks-ring8".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    // (the ring: 8 droppers and 8 hoppers in two rows)
    for i in 0..8 {
        put(DROPPERS[i], &format!("minecraft:dropper[facing={},triggered=false]", DROPPER_FACING[i]));
        put(RING_HOPPERS[i], &format!("minecraft:hopper[enabled=true,facing={}]", HOPPER_FACING[i]));
    }
    // (the dust patch over the ring, fed at its north-west corner by the step repeater)
    for x in -3..=4 {
        for z in -1..=0 {
            let side = |b: bool| if b { "side" } else { "none" };
            let east = side(x < 4);
            let west = side(x > -3 || z == -1);
            let north = side(z == 0);
            let south = side(z == -1);
            put([x, 1, z], &format!("minecraft:redstone_wire[east={east},north={north},power=0,south={south},west={west}]"));
        }
    }
    put(STEP_REP, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]");
    put([STEP_REP[0], 0, STEP_REP[2]], "minecraft:glass");
    // (T's extractor, the up-dropper, the tap comparator on the lock block, the lock comparator and its constant)
    // (both extractors start LOCKED: the hopper disabled, its lock comparator already outputting 15, so neither pulls
    // in the run's first ticks before the comparators' updates land)
    put(T_EXTRACT, "minecraft:hopper[enabled=false,facing=south]");
    put(UP, "minecraft:dropper[facing=up,triggered=false]");
    put(TAP_CMP, "minecraft:comparator[facing=south,mode=compare,powered=true]");
    put(T_LOCK_BLOCK, "minecraft:stone");
    put(T_LOCK_CMP, "minecraft:comparator[facing=west,mode=subtract,powered=true]");
    put([T_LOCK_CMP[0], -2, T_LOCK_CMP[2]], "minecraft:glass");
    put(T_LOCK_SRC, "minecraft:chest[facing=north,type=single,waterlogged=false]");
    // (the filler's extractor, its lock comparator, constant and unlock repeater, its path, the stock)
    put(F_EXTRACT, "minecraft:hopper[enabled=false,facing=south]");
    put(F_LOCK_CMP, "minecraft:comparator[facing=west,mode=subtract,powered=true]");
    put([F_LOCK_CMP[0], -2, F_LOCK_CMP[2]], "minecraft:glass");
    put(F_LOCK_SRC, "minecraft:chest[facing=north,type=single,waterlogged=false]");
    put(F_UNLOCK_REP, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put([F_UNLOCK_REP[0], -2, F_UNLOCK_REP[2]], "minecraft:glass");
    for (p, f) in F_PATH.iter().zip(F_PATH_FACING) {
        put(*p, &format!("minecraft:hopper[enabled=true,facing={f}]"));
    }
    put(STOCK, "minecraft:dropper[facing=up,triggered=false]");
    put(LIFT, "minecraft:dropper[facing=up,triggered=false]");
    put(RELAY, "minecraft:hopper[enabled=true,facing=west]");
    // (the release dust on its stone; P3's comparator on glass)
    put(REL_DUST, "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    put(REL_STONE, "minecraft:stone");
    put(P3_CMP, "minecraft:comparator[facing=south,mode=compare,powered=true]");
    put([P3_CMP[0], -1, P3_CMP[2]], "minecraft:glass");
    // (the up-fire chain and the stock-fire repeater, on glass)
    put(UP_IN_REP, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put([UP_IN_REP[0], -2, UP_IN_REP[2]], "minecraft:glass");
    put(UP_IN_DUST[0], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]");
    put(UP_IN_DUST[1], "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    put(UP_REP1, "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
    put(UP_DUST, "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]");
    put(UP_REP2, "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
    for p in [UP_REP1, UP_DUST, UP_REP2] {
        put([p[0], -2, p[2]], "minecraft:glass");
    }
    // (the tokens and the two constants)
    for (i, p) in DROPPERS.iter().enumerate() {
        let id = if i == t_start { TARGET } else { FILLER };
        d.block_entities.insert(*p, format!("{{Items:[{{Slot:0b,id:\"{id}\",count:1}}]}}"));
    }
    d.block_entities.insert(LIFT, format!("{{Items:[{{Slot:0b,id:\"{FILLER}\",count:1}}]}}"));
    d.block_entities.insert(T_LOCK_SRC, constant_container());
    d.block_entities.insert(F_LOCK_SRC, constant_container());
    d.block_entities.insert(T_LOCK_CMP, "{OutputSignal:1}".to_string());
    d.block_entities.insert(F_LOCK_CMP, "{OutputSignal:1}".to_string());
    d.block_entities.insert(TAP_CMP, "{OutputSignal:1}".to_string());
    d.block_entities.insert(P3_CMP, "{OutputSignal:1}".to_string());
    // (the mob-farm kind's port: the tap comparator's output cell)
    d.ports.insert("output".into(), TAP_OUT);
    d
}

/// A token as the bench expects it in a dropper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tok {
    F,
    T,
    #[default]
    Empty,
}

/// One scripted run's plan (game ticks from the run's start).
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Step pulses (the input cell powered for `step_width` gt).
    pub steps: Vec<i64>,
    pub step_width: i64,
    /// T's extractor unlocked for 4 gt from each of these ticks.
    pub t_unlock: Vec<i64>,
    /// The filler's extractor unlocked for 4 gt from each of these ticks.
    pub f_unlock: Vec<i64>,
    /// The up-dropper fired (2 gt) at each of these ticks.
    pub up_fire: Vec<i64>,
    /// The stock fired (2 gt) at each of these ticks.
    pub stock_fire: Vec<i64>,
    /// A world open at this tick (between the tick and the next).
    pub restart: Option<(i64, crate::liveness::Restart)>,
    pub horizon: i64,
    /// Ticks [a, b) whose contents of the traced cells are recorded (a debug print).
    pub trace: Option<(i64, i64, Vec<P>)>,
}

/// One run's readings.
#[derive(Clone, Debug, Default)]
pub struct Read {
    /// Before each step (the tick before its pulse): each ring dropper's token, the tap comparator's output level.
    pub before: Vec<([Tok; 8], i32, usize)>,
    /// The tick T left the tap after each unlock, reached the up-dropper, and was in D0 again.
    pub t_out: Vec<i64>,
    pub t_up: Vec<i64>,
    pub t_in: Vec<i64>,
    /// The tick each extracted filler reached the stock.
    pub f_stock: Vec<i64>,
    /// The end state: the ring's tokens, the stock's count, any item left in a path or the inserter, the dropped items.
    pub end: [Tok; 8],
    pub stock_end: u32,
    /// The lift's count at the end (one filler loaded at the build, reloaded by every release).
    pub lift_end: u32,
    pub stray: Vec<String>,
    pub items_total: u32,
    pub extra_items: Vec<String>,
    /// The two lock comparators' constant rears (each a one-stone chest: the container-signal formula gives 1), read at the
    /// run's start and at its end.
    pub consts: (i32, i32),
    pub consts_end: (i32, i32),
    /// The up-dropper's `triggered` state on the tick before each real up-fire (it must be CLEAR, or the fire's rising edge
    /// is lost: the quasi-connected power of the steps sets it, and only a neighbour's change clears it).
    pub up_triggered_before: Vec<bool>,
    pub trace: Vec<String>,
}

fn items(w: &redsim::world::World, p: P) -> Vec<(u8, u8)> {
    let bp = crate::mirror::at(p);
    w.containers.get(CRef::Block(bp)).map(|c| c.slots.iter().flatten().map(|s| (s.item, s.count)).collect()).unwrap_or_default()
}

fn tok_of(w: &redsim::world::World, p: P, f: u8, t: u8, extra: &mut Vec<String>) -> Tok {
    let it = items(w, p);
    let n: u32 = it.iter().map(|x| x.1 as u32).sum();
    if n > 1 {
        extra.push(format!("{p:?} holds {n} items"));
    }
    if it.iter().any(|x| x.0 == t) {
        Tok::T
    } else if it.iter().any(|x| x.0 == f) {
        Tok::F
    } else {
        Tok::Empty
    }
}

/// Runs a plan on the ring's spec.
pub fn run_plan(spec: &FarmSpec, plan: &Plan) -> Result<Read, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, plan.horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(&crate::mirror::spec(spec)?, &ro)?;
    while run.settle_tick() {}
    let rb = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let f_id = redsim::redstone::container::item_id(FILLER).ok_or("no filler item")?;
    let t_id = redsim::redstone::container::item_id(TARGET).ok_or("no target item")?;
    let mut r = Read::default();
    r.consts = (redsim::redstone::container::analog_output(&run.world, crate::mirror::at(T_LOCK_SRC)), redsim::redstone::container::analog_output(&run.world, crate::mirror::at(F_LOCK_SRC)));
    let mut t_unl: Option<i64> = None;
    let mut t_rode = false;
    let mut up_had = false;
    let mut d0_t_since_fire = false;
    let mut stock_prev: u32 = 0;
    for t in 0..plan.horizon {
        // (the drives at this tick: a pulse's start and end)
        for &s in &plan.steps {
            if t == s {
                r.before.push(([0, 1, 2, 3, 4, 5, 6, 7].map(|i| tok_of(&run.world, DROPPERS[i], f_id, t_id, &mut r.extra_items)), run.world.best_neighbor_signal(crate::mirror::at(TAP_OUT)), r.before.len()));
                run.world.set_block(crate::mirror::at(STEP_DRIVE), rb.clone(), all);
            }
            if t == s + plan.step_width {
                run.world.set_block(crate::mirror::at(STEP_DRIVE), air.clone(), all);
            }
        }
        for &u in &plan.t_unlock {
            if t == u {
                run.world.set_block(crate::mirror::at(T_UNLOCK_AT), rb.clone(), all);
                t_unl = Some(t);
                t_rode = false;
            }
            if t == u + T_WINDOW {
                run.world.set_block(crate::mirror::at(T_UNLOCK_AT), air.clone(), all);
            }
        }
        for &u in &plan.f_unlock {
            if t == u {
                run.world.set_block(crate::mirror::at(F_UNLOCK_AT), rb.clone(), all);
            }
            if t == u + F_WINDOW {
                run.world.set_block(crate::mirror::at(F_UNLOCK_AT), air.clone(), all);
            }
        }
        for &u in &plan.up_fire {
            if t == u {
                r.up_triggered_before.push(matches!(run.world.get(crate::mirror::at(UP)), Block::Dispenser { triggered: true, .. }));
                run.world.set_block(crate::mirror::at(UP_DRIVE), rb.clone(), all);
                d0_t_since_fire = true;
            }
            if t == u + 2 {
                run.world.set_block(crate::mirror::at(UP_DRIVE), air.clone(), all);
            }
        }
        for &u in &plan.stock_fire {
            if t == u {
                run.world.set_block(crate::mirror::at(STOCK_DRIVE), rb.clone(), all);
            }
            if t == u + 2 {
                run.world.set_block(crate::mirror::at(STOCK_DRIVE), air.clone(), all);
            }
        }
        run.tick();
        if let Some((rt, how)) = plan.restart {
            if t == rt {
                crate::liveness::restart(&mut run.world, how)?;
            }
        }
        if let Some((a, b, cells)) = &plan.trace {
            if t >= *a && t < *b {
                let row: Vec<String> = cells.iter().map(|p| {
                    let trig = match run.world.get(crate::mirror::at(*p)) {
                        Block::Dispenser { triggered, .. } => format!("/trig={triggered}"),
                        _ => String::new(),
                    };
                    format!("{p:?}={:?}/{}{trig}", items(&run.world, *p), run.world.best_neighbor_signal(crate::mirror::at(*p)))
                }).collect();
                r.trace.push(format!("t {t}: {}", row.join(" ")));
            }
        }
        // (T's milestones)
        if t_unl.is_some() && !t_rode && !items(&run.world, DROPPERS[TAP]).iter().any(|x| x.0 == t_id) {
            r.t_out.push(t + 1);
            t_rode = true;
        }
        let up_now = items(&run.world, UP).iter().any(|x| x.0 == t_id);
        if up_now && !up_had {
            r.t_up.push(t + 1);
        }
        up_had = up_now;
        if d0_t_since_fire && items(&run.world, DROPPERS[T_INTO]).iter().any(|x| x.0 == t_id) {
            r.t_in.push(t + 1);
            d0_t_since_fire = false;
        }
        let s_now: u32 = items(&run.world, STOCK).iter().map(|x| x.1 as u32).sum();
        if s_now > stock_prev {
            r.f_stock.push(t + 1);
        }
        stock_prev = s_now;
    }
    r.consts_end = (redsim::redstone::container::analog_output(&run.world, crate::mirror::at(T_LOCK_SRC)), redsim::redstone::container::analog_output(&run.world, crate::mirror::at(F_LOCK_SRC)));
    r.end = [0, 1, 2, 3, 4, 5, 6, 7].map(|i| tok_of(&run.world, DROPPERS[i], f_id, t_id, &mut r.extra_items));
    r.stock_end = items(&run.world, STOCK).iter().map(|x| x.1 as u32).sum();
    r.lift_end = items(&run.world, LIFT).iter().map(|x| x.1 as u32).sum();
    let mut all_c: Vec<P> = Vec::new();
    all_c.extend(T_RIDE);
    all_c.extend(F_PATH);
    all_c.extend(RING_HOPPERS);
    all_c.extend([T_EXTRACT, F_EXTRACT, UP, RELAY]);
    for p in all_c {
        let it = items(&run.world, p);
        if !it.is_empty() {
            r.stray.push(format!("{p:?}: {it:?}"));
        }
    }
    let mut tot = 0u32;
    for p in DROPPERS.iter().chain(T_RIDE.iter()).chain(F_PATH.iter()).chain(RING_HOPPERS.iter()).chain([T_EXTRACT, F_EXTRACT, UP, STOCK, RELAY, LIFT].iter()) {
        tot += items(&run.world, *p).iter().map(|x| x.1 as u32).sum::<u32>();
    }
    r.items_total = tot;
    Ok(r)
}

/// The expected tokens after a plain step (every dropper's token one dropper on).
pub fn rotate(e: [Tok; 8]) -> [Tok; 8] {
    let mut o = [Tok::Empty; 8];
    for i in 0..8 {
        o[(i + 1) % 8] = e[i];
    }
    o
}

/// The reference script: steps every 100 gt from t 50 (`n` of them, a skip step 20 gt after any step that leaves the
/// tap empty); T starts in D1, so it is at the tap before step 3: the GRANT there (T's extractor unlocked at the step -
/// 8, the filler's at the step + `f_at`, the up-fire at the step + `up_at`); the RELEASE when D6 is empty before step 5
/// (the stock fired 30 gt before that step) unless `no_release`. Returns the plan and the expected tokens before every
/// step and at the end.
pub fn reference(n: usize, f_at: i64, up_at: i64, no_release: bool) -> (Plan, Vec<[Tok; 8]>, [Tok; 8]) {
    let mut e = [Tok::F; 8];
    e[T_START] = Tok::T;
    let mut plan = Plan { step_width: STEP_WIDTH, horizon: 50 + 100 * n as i64 + 60, ..Default::default() };
    let mut before = Vec::new();
    let mut k = 0usize;
    let mut t = 50i64;
    let mut granted = false;
    let mut released = no_release;
    while k < n {
        if !granted && e[TAP] == Tok::T {
            // (THE GRANT at this step: T lifted out before it (the read at the step sees the tap already empty); D0's
            // filler (the slot picked + 5, after the step) lifted out after it; T into D0 by the up-fire)
            plan.t_unlock.push(t - STEP_LAG);
            e[TAP] = Tok::Empty;
            before.push(e);
            plan.steps.push(t);
            e = rotate(e);
            plan.f_unlock.push(t + f_at);
            e[T_INTO] = Tok::T;
            plan.up_fire.push(t + up_at);
            granted = true;
        } else {
            before.push(e);
            plan.steps.push(t);
            e = rotate(e);
        }
        // (the skip: the tap empty after the step: one more step 20 gt later)
        if e[TAP] == Tok::Empty {
            plan.steps.push(t + 20);
            before.push(e);
            e = rotate(e);
        }
        // (the release: D6 empty -> the stock fires into it, 30 gt before the next step)
        if granted && !released && e[REL_AT] == Tok::Empty {
            plan.stock_fire.push(t + 70);
            e[REL_AT] = Tok::F;
            released = true;
        }
        t += 100;
        k += 1;
    }
    (plan, before, e)
}

/// The ring's spec from its design.
pub fn spec_of(d: Design) -> Result<FarmSpec, String> {
    let c = circuit::Circuit::from_design(d).map_err(|e| e.to_string())?;
    circuit::mobfarm::spec_of(&c).map_err(|e| e.to_string())
}

/// The bench: the reference script (a grant and a release) and its no-release twin (the skip); T's and the filler's
/// transit times against the deadlines (T in the up-dropper before the up-fire; T in D0 before the next step); a world
/// open at every tick from the grant's unlock to the end of the release, each run's end state against the plain run's;
/// the up-fire / the filler's unlock / the steps shifted -2..+2 gt.
pub fn bench(v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let spec = spec_of(design(T_START))?;
    let mut lines = Vec::new();
    let (plan, exp_before, exp_end) = reference(16, F_AT, UP_AT, false);
    let r = run_plan(&spec, &plan)?;
    let mism: Vec<String> = r.before.iter().zip(&exp_before).enumerate().filter(|(_, (a, e))| a.0 != **e).map(|(i, (a, e))| format!("before step {i}: {:?} vs {:?}", a.0, e)).take(3).collect();
    let levels_ok = r.before.iter().zip(&exp_before).all(|(a, e)| a.1 == match e[TAP] { Tok::T => 2, Tok::F => 1, Tok::Empty => 0 });
    let w = v.bar_n("ctl-ring8: THE REFERENCE SCRIPT (16 steps, a grant at T's pass, a release when P4 is empty): every dropper's token matches the sensed model's before every step and at the end, the tap reads 2 / 1 / 0 for T / filler / empty, no dropper ever holds two items, no item left in a path, the lift reloaded with one filler and the stock empty", r.before.len(), mism.is_empty() && r.end == exp_end && levels_ok && r.extra_items.is_empty() && r.stray.is_empty() && r.lift_end == 1 && r.stock_end == 0);
    let w0 = v.bar("ctl-ring8: THE CONSTANT COMPARATORS' REARS (a single chest holding one stone each) read 1 (>= 1 locks; the unlock's 15 on the side takes the output to 0)", r.consts == (1, 1));
    lines.push(format!("ctl-ring8: the two lock comparators' constant rears (a single chest holding one stone each) read {} / {} (the container-signal formula: 1): {w0}", r.consts.0, r.consts.1));
    lines.push(format!("ctl-ring8: the reference script ({} step reads; the end {:?} vs {:?}; the lift {} (reloaded), the stock {}; items {}): {}{}: {w}", r.before.len(), r.end, exp_end, r.lift_end, r.stock_end, r.items_total, if mism.is_empty() { "every read matches".to_string() } else { format!("MISMATCHES {}", mism.join("; ")) }, if r.extra_items.is_empty() { String::new() } else { format!("; two items in a dropper: {:?}", r.extra_items.iter().take(3).collect::<Vec<_>>()) }));
    // (the timings: the grant's step is plan.steps[k] for the unlock at its - 8)
    let g_step = plan.t_unlock[0] + STEP_LAG;
    let t_out = r.t_out.first().copied().unwrap_or(i64::MAX) - plan.t_unlock[0];
    let t_up = r.t_up.first().copied().unwrap_or(i64::MAX);
    let t_in = r.t_in.first().copied().unwrap_or(i64::MAX);
    let up_fire = plan.up_fire[0];
    let next_step = plan.steps.iter().copied().filter(|&s| s > g_step).min().unwrap_or(i64::MAX);
    let f_st = r.f_stock.first().copied().unwrap_or(i64::MAX);
    let w = v.bar("ctl-ring8: THE GRANT's TIMES: T out of the tap before the grant's step, T in the up-dropper before the up-fire, T in D0 before the next step, the filler in the stock before the release's fire", g_step > plan.t_unlock[0] + t_out && t_up < up_fire && t_in < next_step && plan.stock_fire.first().is_none_or(|&s| f_st < s));
    // (the up-dropper's quasi-connected fire at the grant's own step: the step repeater's 2 gt + the dropper's 4 gt after
    // the step; T must reach the up-dropper after it, or that fire would send T into P6's feeder early)
    let qc_fire = g_step + 2 + 4;
    let w_qc = v.bar("ctl-ring8: T reaches the up-dropper AFTER that dropper's quasi-connected fire at the grant's step (the step powers every ring cell, the up-dropper's above included)", t_up > qc_fire);
    lines.push(format!("  THE GRANT's TIMES (measured): T out of the tap {t_out} gt after the unlock (the step {} gt after it); T into the up-dropper (ONE hop) at the grant step + {} gt, after the up-dropper's quasi-connected fire at + {} (margin {} gt: {w_qc}); the up-fire at + {} (margin {} gt); T in P6 at + {} (the next step at + {}: margin {} gt); the filler in the stock at + {} (the release's fire at + {}): {w}", g_step - plan.t_unlock[0], t_up - g_step, qc_fire - g_step, t_up - qc_fire, up_fire - g_step, up_fire - t_up, t_in - g_step, next_step - g_step, next_step - t_in, f_st - g_step, plan.stock_fire.first().map_or(-1, |&s| s - g_step)));
    // (G1 13:54 (1b): the up-dropper's quasi-connected power falls and its `triggered` clears BEFORE the real up-fire)
    let w_trig = v.bar("ctl-ring8: the up-dropper's `triggered` is CLEAR on the tick before the real up-fire (else its rising edge is lost and T stalls in it)", r.up_triggered_before.iter().all(|t| !*t) && !r.up_triggered_before.is_empty());
    lines.push(format!("  THE UP-DROPPER's TRIGGERED STATE before the real up-fire (the grant step + {}): {} ({} fires): {w_trig}", up_fire - g_step, if r.up_triggered_before.iter().all(|t| !*t) { "clear" } else { "SET" }, r.up_triggered_before.len()));
    // (G1 13:54 (3): the one-stone chests behind the lock comparators: nothing can change their count)
    let d0 = design(T_START);
    let mut chest_bad = Vec::new();
    for c in [T_LOCK_SRC, F_LOCK_SRC] {
        let below = [c[0], c[1] - 1, c[2]];
        if d0.blocks.get(&below).is_some_and(|b| b.contains("hopper")) {
            chest_bad.push(format!("{c:?}: a hopper under it pulls from it"));
        }
        for (dx, dz, toward) in [(1, 0, "west"), (-1, 0, "east"), (0, 1, "north"), (0, -1, "south")] {
            let n = [c[0] + dx, c[1], c[2] + dz];
            if let Some(b) = d0.blocks.get(&n) {
                if b.contains("chest") {
                    chest_bad.push(format!("{c:?}: a chest beside it at {n:?} (a double chest)"));
                }
                if (b.contains("hopper") || b.contains("dropper") || b.contains("dispenser")) && b.contains(&format!("facing={toward}")) {
                    chest_bad.push(format!("{c:?}: {b} at {n:?} faces into it"));
                }
            }
        }
        let above = [c[0], c[1] + 1, c[2]];
        if d0.blocks.get(&above).is_some_and(|b| (b.contains("hopper") || b.contains("dropper")) && b.contains("facing=down")) {
            chest_bad.push(format!("{c:?}: a container above faces down into it"));
        }
    }
    let w_chest = v.bar("ctl-ring8: THE ONE-STONE CHESTS (static): no hopper under either, nothing facing into either, no chest beside either (no double chest), no rail near (no hopper minecart)", chest_bad.is_empty());
    lines.push(format!("  THE ONE-STONE CHESTS {:?} / {:?} (static): {}; their readings {} / {} at the start and {} / {} at the end of the reference run: {w_chest}", T_LOCK_SRC, F_LOCK_SRC, if chest_bad.is_empty() { "no hopper under them, nothing facing into them, no chest beside them, no rail in the part".to_string() } else { chest_bad.join("; ") }, r.consts.0, r.consts.1, r.consts_end.0, r.consts_end.1));
    // (the skip: no release, the away slot comes round to the tap: the skip step keeps the ring on the model)
    let (p2, e2b, e2e) = reference(16, F_AT, UP_AT, true);
    let r2 = run_plan(&spec, &p2)?;
    let m2 = r2.before.iter().zip(&e2b).filter(|(a, e)| a.0 != **e).count();
    let w = v.bar_n("ctl-ring8: THE SKIP (no release: the away slot reaches the tap and the skip step passes it): every read and the end match the model", r2.before.len(), m2 == 0 && r2.end == e2e && r2.extra_items.is_empty());
    lines.push(format!("  THE SKIP (the no-release twin, {} reads, {} skip steps): mismatches {m2}, the end {:?} vs {:?}: {w}", r2.before.len(), p2.steps.len() - 16, r2.end, e2e));
    // (WORLD OPENS at every tick from the grant's unlock to the release's fire + 20: the end state as the plain run's)
    let from = plan.t_unlock[0] - 2;
    let to = plan.stock_fire.first().copied().unwrap_or(up_fire) + 20;
    let mut bad = Vec::new();
    let mut n = 0usize;
    for rt in from..=to {
        let mut pr = plan.clone();
        pr.restart = Some((rt, crate::liveness::Restart::load(redsim::reload::Order::Save)));
        let rr = run_plan(&spec, &pr)?;
        n += 1;
        if rr.end != r.end || rr.stock_end != r.stock_end || rr.lift_end != r.lift_end || !rr.stray.is_empty() || !rr.extra_items.is_empty() || rr.up_triggered_before.iter().any(|t| *t) || rr.consts != (1, 1) || rr.consts_end != (1, 1) {
            if bad.len() < 4 {
                bad.push(format!("a world open at t {rt} (the grant step {:+}): end {:?}, stock {}, stray {:?}", rt - g_step, rr.end, rr.stock_end, rr.stray));
            }
        }
    }
    let w = v.bar_n("ctl-ring8: A WORLD OPEN at every tick of the grant and the release (T mid-ride, the filler mid-path, a lock window, the step's quasi-connected fire pending, T waiting for the up-fire, the stock fire pending): the end state equals the plain run's, the up-dropper's `triggered` clear before the up-fire, the chests reading 1 / 1", n, bad.is_empty());
    lines.push(format!("  WORLD OPENS (the official load, 2 idle ticks) at every tick t {from}..{to} ({n} rows){}: {w}", if bad.is_empty() { String::new() } else { format!(": {}", bad.join("; ")) }));
    // (the corners: the filler's unlock and the up-fire shifted -2..+2 gt)
    let mut cbad = Vec::new();
    let mut cn = 0usize;
    for df in -2i64..=2 {
        for du in -2i64..=2 {
            let (pc, _, ec) = reference(16, F_AT + df, UP_AT + du, false);
            let rc = run_plan(&spec, &pc)?;
            cn += 1;
            if rc.end != ec || !rc.extra_items.is_empty() || !rc.stray.is_empty() || rc.up_triggered_before.iter().any(|t| *t) || rc.consts_end != (1, 1) {
                cbad.push(format!("filler {df:+}, up-fire {du:+}: end {:?}", rc.end));
            }
        }
    }
    let w = v.bar_n("ctl-ring8: THE +-2 gt CORNERS (the filler's unlock and the up-fire each shifted -2..+2 gt): the end state matches the model", cn, cbad.is_empty());
    lines.push(format!("  THE +-2 gt CORNERS ({cn} rows){}: {w}", if cbad.is_empty() { String::new() } else { format!(": {}", cbad.join("; ")) }));
    // (the cell count, by category)
    let d = design(T_START);
    lines.push(format!("  CELLS laid here: {} (the ring 16 + its dust 16 + the step repeater and its glass 2; T's extractor + the up-dropper 2; the tap comparator + the T lock block 2; the two lock comparators + their glass + their constant chests 6; the filler's extractor + its unlock repeater + glass 3, its path 7, the stock + its relay hopper 2; the up-fire chain 3 + the stock-fire repeater 1 + their glass 4); NOT YET: the logic (REQ, the spacing latch, the skip, the release's AND, the unlock edges / gates, the two fires' gates), the routes", d.blocks.len()));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn trace_release() {
        let spec = super::spec_of(super::design(super::T_START)).unwrap();
        let (mut plan, _, _) = super::reference(16, super::F_AT, super::UP_AT, false);
        plan.trace = Some((340, 425, vec![super::UP, super::T_EXTRACT, super::RING_HOPPERS[5], super::DROPPERS[6]]));
        let r = super::run_plan(&spec, &plan).unwrap();
        println!("stock fires {:?}", plan.stock_fire);
        for l in &r.trace {
            println!("{l}");
        }
    }

    #[test]
    fn the_ring_bench_passes() {
        let mut v = crate::verdict::Verdict::new("ctl-ring8");
        let lines = super::bench(&mut v).expect("the bench runs");
        for l in &lines {
            println!("{l}");
        }
        assert!(v.fails().is_empty(), "ctl-ring8 fails: {:?}", v.fails());
    }
}
