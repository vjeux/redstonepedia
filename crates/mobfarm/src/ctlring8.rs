//! The picks' per-lane ITEM RING as blocks (day-1, Nov 14 2026; ironspec 11:56a / 12:02p / 13:07p, the parent 13:01p):
//! the sensed pointer of ctl-pick (`ctlpick::Cfg::sensed` with `positional_release`) laid as one 8-dropper ring per
//! lane, on the counter-ring generator's 23-block ring (`mctas circuit memory ring --n 8 --unchecked`'s
//! counter8-ring8-0, its own tap comparator dropped).
//!
//! THE TOKENS: every PRESENT slot holds a stackable filler F (stone: a lone item of a 64-stack reads 1 on a comparator),
//! the TARGET slot holds the unstackable T (a wooden shovel: a lone unstackable reads 2) instead of its filler (in normal
//! operation the target's slot is never an away slot: the away slots sit at picked - 5 / - 10 = + 3 / + 6), and an
//! AWAY slot is an EMPTY dropper (reads 0). Between steps every token rests in a dropper; the ring's own hoppers have
//! no container above them.
//!
//! THE CELLS (ring-local; D0..D7 in flow order): the TAP is D4 (read by a comparator on its west face); a STEP fires
//! all eight droppers (the input cell (0,0,0) and its two relays); a tap EMPTY 20 gt after a step takes one more step
//! (the skip, the logic's); GRANT: the T EXTRACTOR, a hopper under D4 (locked except a 4-gt pulse at the grant's
//! capture, REL + 92, before the pop), lifts T out (the picked slot now empty = away) into T's RIDE (8 hoppers) to the
//! UP-DROPPER under the INSERTER; after the pop's step the F EXTRACTOR, a hopper under D0 (= the tap + 4, where the
//! slot picked + 5 sits once the pop has stepped), unlocked for 4 gt at the lane's pop + 40 (REL + 40 / 57 / 67),
//! lifts D0's filler into the FILLER PATH (5 hoppers) to the STOCK dropper under D6; the up-dropper fires T up at
//! REL + 80 into the inserter, which pushes it into D0 before the next pop; RELEASE (the positional rule): at a circuit
//! entry with D6 (2 downstream of the tap) EMPTY and the siding's presence, the stock fires one filler up into D6 and
//! the R hold is released (the logic's). Every hopper path avoids the cells under ring containers (a hopper there would
//! drain them).
//!
//! This file lays the MECHANICS (the ring, the extractors, the paths, the up-dropper, the inserter, the stock, the two
//! comparators); the bench drives the locks, the step input and the two timed fires as emulated stand-ins (redstone
//! blocks placed / removed), and judges the item movements, their transit times against the deadlines, and a world
//! open (`liveness::restart`) at every tick of a grant and a release.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::redstone::container::CRef;

type P = [i32; 3];

/// The ring's droppers in flow order (D0..D7), ring-local.
pub const DROPPERS: [P; 8] = [[5, 0, 0], [4, 1, 0], [2, 1, 0], [0, 1, 0], [-1, 0, 0], [0, 0, -1], [2, 0, -1], [4, 0, -1]];
/// The ring's hoppers in flow order (the one after D0 .. the one after D7).
pub const RING_HOPPERS: [P; 8] = [[5, 1, 0], [3, 1, 0], [1, 1, 0], [-1, 1, 0], [-1, 0, -1], [1, 0, -1], [3, 0, -1], [5, 0, -1]];
/// The tap (D4), the dropper T joins after the pop's step (D0 = the tap + 4), the release's dropper (D6 = the tap + 2).
pub const TAP: usize = 4;
pub const T_INTO: usize = 0;
pub const REL_AT: usize = 6;
/// The step input cell (the bench's drive: a redstone block placed there for the pulse).
pub const INPUT: P = [0, 0, 0];
/// T's extractor (under the tap), its lock stand-in (a redstone block under it: locked while present), its ride, the
/// up-dropper, the up-dropper's fire stand-in (under it) and the inserter (on D0's south face).
pub const T_EXTRACT: P = [-1, -1, 0];
pub const T_LOCK: P = [-1, -2, 0];
pub const T_RIDE: [P; 8] = [[-1, -1, 1], [-1, -1, 2], [0, -1, 2], [1, -1, 2], [2, -1, 2], [3, -1, 2], [4, -1, 2], [5, -1, 2]];
pub const UP: P = [5, -1, 1];
pub const UP_DRIVE: P = [5, -2, 1];
pub const INSERTER: P = [5, 0, 1];
/// The filler's extractor (under D0), its lock stand-in, its path, the stock (under D6) and the stock's fire stand-in.
pub const F_EXTRACT: P = [5, -1, 0];
pub const F_LOCK: P = [5, -2, 0];
pub const F_PATH: [P; 5] = [[4, -1, 0], [4, -1, 1], [3, -1, 1], [2, -1, 1], [2, -1, 0]];
pub const STOCK: P = [2, -1, -1];
pub const STOCK_DRIVE: P = [2, -2, -1];
/// The tap's comparator (west of D4, reading it: output west) and the release's (north of D6: output north).
pub const TAP_CMP: P = [-2, 0, 0];
pub const TAP_OUT: P = [-3, 0, 0];
pub const REL_CMP: P = [2, 0, -2];
pub const REL_OUT: P = [2, 0, -3];

/// The token items.
pub const FILLER: &str = "minecraft:stone";
pub const TARGET: &str = "minecraft:wooden_shovel";
/// The extractors' unlock windows (gt): a locked hopper neither pulls nor pushes, so each window covers the pull (within
/// 1 gt of the unlock, 26.3: the cooldown counts down while locked) and the push 8 gt later, and closes before the
/// next item reaches the dropper above (the tap's at its step + 4 (the dropper's delay) + 7 (the chained hop), from an
/// unlock at the step - 8: 12 gt; D0's: the next item comes only with T's insertion, so 10 gt).
pub const T_WINDOW: i64 = 12;
pub const F_WINDOW: i64 = 10;

/// The ring's design with T starting in dropper `t_start` and fillers in the other seven.
pub fn design(t_start: usize) -> Design {
    let mut d = Design { name: "picks-ring8".into(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    // (the ring: counter8-ring8-0's cells, its comparator dropped)
    let ring: [(P, &str); 22] = [
        ([-1, 0, -1], "minecraft:hopper[enabled=true,facing=east]"),
        ([0, 0, -1], "minecraft:dropper[facing=east,triggered=false]"),
        ([1, 0, -1], "minecraft:hopper[enabled=true,facing=east]"),
        ([2, 0, -1], "minecraft:dropper[facing=east,triggered=false]"),
        ([3, 0, -1], "minecraft:hopper[enabled=true,facing=east]"),
        ([4, 0, -1], "minecraft:dropper[facing=east,triggered=false]"),
        ([5, 0, -1], "minecraft:hopper[enabled=true,facing=south]"),
        ([-1, 0, 0], "minecraft:dropper[facing=north,triggered=false]"),
        ([1, 0, 0], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]"),
        ([2, 0, 0], "minecraft:stone"),
        ([3, 0, 0], "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]"),
        ([4, 0, 0], "minecraft:stone"),
        ([5, 0, 0], "minecraft:dropper[facing=up,triggered=false]"),
        ([-1, 1, 0], "minecraft:hopper[enabled=true,facing=down]"),
        ([0, 1, 0], "minecraft:dropper[facing=west,triggered=false]"),
        ([1, 1, 0], "minecraft:hopper[enabled=true,facing=west]"),
        ([2, 1, 0], "minecraft:dropper[facing=west,triggered=false]"),
        ([3, 1, 0], "minecraft:hopper[enabled=true,facing=west]"),
        ([4, 1, 0], "minecraft:dropper[facing=west,triggered=false]"),
        ([5, 1, 0], "minecraft:hopper[enabled=true,facing=west]"),
        // (the floors under the repeater and the dust: GLASS, so neither powers the hoppers around them)
        ([1, -1, 0], "minecraft:glass"),
        ([3, -1, 0], "minecraft:glass"),
    ];
    for (p, s) in ring {
        put(p, s);
    }
    // (T's extractor and ride, the up-dropper, the inserter)
    put(T_EXTRACT, "minecraft:hopper[enabled=true,facing=south]");
    let ride_facing = ["south", "east", "east", "east", "east", "east", "east", "north"];
    for (p, f) in T_RIDE.iter().zip(ride_facing) {
        put(*p, &format!("minecraft:hopper[enabled=true,facing={f}]"));
    }
    put(UP, "minecraft:dropper[facing=up,triggered=false]");
    put(INSERTER, "minecraft:hopper[enabled=true,facing=north]");
    // (the filler's extractor and path, the stock)
    put(F_EXTRACT, "minecraft:hopper[enabled=true,facing=west]");
    let path_facing = ["south", "west", "west", "north", "north"];
    for (p, f) in F_PATH.iter().zip(path_facing) {
        put(*p, &format!("minecraft:hopper[enabled=true,facing={f}]"));
    }
    put(STOCK, "minecraft:dropper[facing=up,triggered=false]");
    // (the comparators and their glass supports)
    put(TAP_CMP, "minecraft:comparator[facing=east,mode=compare,powered=false]");
    put([TAP_CMP[0], -1, TAP_CMP[2]], "minecraft:glass");
    put(REL_CMP, "minecraft:comparator[facing=south,mode=compare,powered=false]");
    put([REL_CMP[0], -1, REL_CMP[2]], "minecraft:glass");
    // (the lock stand-ins, present: both extractors locked)
    put(T_LOCK, "minecraft:redstone_block");
    put(F_LOCK, "minecraft:redstone_block");
    // (the tokens)
    for (i, p) in DROPPERS.iter().enumerate() {
        let id = if i == t_start { TARGET } else { FILLER };
        d.block_entities.insert(*p, format!("{{Items:[{{Slot:0b,id:\"{id}\",count:1}}]}}"));
    }
    // (the mob-farm kind's port: the tap comparator's output cell, as ctlring's)
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
    pub stray: Vec<String>,
    pub items_total: u32,
    pub extra_items: Vec<String>,
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
                run.world.set_block(crate::mirror::at(INPUT), rb.clone(), all);
            }
            if t == s + plan.step_width {
                run.world.set_block(crate::mirror::at(INPUT), air.clone(), all);
            }
        }
        for &u in &plan.t_unlock {
            if t == u {
                run.world.set_block(crate::mirror::at(T_LOCK), air.clone(), all);
                t_unl = Some(t);
                t_rode = false;
            }
            if t == u + T_WINDOW {
                run.world.set_block(crate::mirror::at(T_LOCK), rb.clone(), all);
            }
        }
        for &u in &plan.f_unlock {
            if t == u {
                run.world.set_block(crate::mirror::at(F_LOCK), air.clone(), all);
            }
            if t == u + F_WINDOW {
                run.world.set_block(crate::mirror::at(F_LOCK), rb.clone(), all);
            }
        }
        for &u in &plan.up_fire {
            if t == u {
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
    r.end = [0, 1, 2, 3, 4, 5, 6, 7].map(|i| tok_of(&run.world, DROPPERS[i], f_id, t_id, &mut r.extra_items));
    r.stock_end = items(&run.world, STOCK).iter().map(|x| x.1 as u32).sum();
    let mut all_c: Vec<P> = Vec::new();
    all_c.extend(T_RIDE);
    all_c.extend(F_PATH);
    all_c.extend(RING_HOPPERS);
    all_c.extend([T_EXTRACT, F_EXTRACT, UP, INSERTER]);
    for p in all_c {
        let it = items(&run.world, p);
        if !it.is_empty() {
            r.stray.push(format!("{p:?}: {it:?}"));
        }
    }
    let mut tot = 0u32;
    for p in DROPPERS.iter().chain(T_RIDE.iter()).chain(F_PATH.iter()).chain(RING_HOPPERS.iter()).chain([T_EXTRACT, F_EXTRACT, UP, INSERTER, STOCK].iter()) {
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
    e[1] = Tok::T;
    let mut plan = Plan { step_width: 4, horizon: 50 + 100 * n as i64 + 60, ..Default::default() };
    let mut before = Vec::new();
    let mut k = 0usize;
    let mut t = 50i64;
    let mut granted = false;
    let mut released = no_release;
    while k < n {
        if !granted && e[TAP] == Tok::T {
            // (THE GRANT at this step: T lifted out before it (the read at the step sees the tap already empty); D0's
            // filler (the slot picked + 5, after the step) lifted out after it; T into D0 by the up-fire)
            plan.t_unlock.push(t - 8);
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
    let spec = spec_of(design(1))?;
    let mut lines = Vec::new();
    let (plan, exp_before, exp_end) = reference(16, 40, 80, false);
    let r = run_plan(&spec, &plan)?;
    let mism: Vec<String> = r.before.iter().zip(&exp_before).enumerate().filter(|(_, (a, e))| a.0 != **e).map(|(i, (a, e))| format!("before step {i}: {:?} vs {:?}", a.0, e)).take(3).collect();
    let levels_ok = r.before.iter().zip(&exp_before).all(|(a, e)| a.1 == match e[TAP] { Tok::T => 2, Tok::F => 1, Tok::Empty => 0 });
    let w = v.bar_n("ctl-ring8: THE REFERENCE SCRIPT (16 steps, a grant at T's pass, a release when D6 is empty): every dropper's token matches the sensed model's before every step and at the end, the tap reads 2 / 1 / 0 for T / filler / empty, no dropper ever holds two items, no item left in a path", r.before.len(), mism.is_empty() && r.end == exp_end && levels_ok && r.extra_items.is_empty() && r.stray.is_empty());
    lines.push(format!("ctl-ring8: the reference script ({} step reads; the end {:?} vs {:?}; the stock {}; items {}): {}{}: {w}", r.before.len(), r.end, exp_end, r.stock_end, r.items_total, if mism.is_empty() { "every read matches".to_string() } else { format!("MISMATCHES {}", mism.join("; ")) }, if r.extra_items.is_empty() { String::new() } else { format!("; two items in a dropper: {:?}", r.extra_items.iter().take(3).collect::<Vec<_>>()) }));
    // (the timings: the grant's step is plan.steps[k] for the unlock at its - 8)
    let g_step = plan.t_unlock[0] + 8;
    let t_out = r.t_out.first().copied().unwrap_or(i64::MAX) - plan.t_unlock[0];
    let t_up = r.t_up.first().copied().unwrap_or(i64::MAX);
    let t_in = r.t_in.first().copied().unwrap_or(i64::MAX);
    let up_fire = plan.up_fire[0];
    let next_step = plan.steps.iter().copied().filter(|&s| s > g_step).min().unwrap_or(i64::MAX);
    let f_st = r.f_stock.first().copied().unwrap_or(i64::MAX);
    let w = v.bar("ctl-ring8: THE GRANT's TIMES: T out of the tap before the grant's step, T in the up-dropper before the up-fire, T in D0 before the next step, the filler in the stock before the release's fire", g_step > plan.t_unlock[0] + t_out && t_up < up_fire && t_in < next_step && plan.stock_fire.first().is_none_or(|&s| f_st < s));
    lines.push(format!("  THE GRANT's TIMES (measured): T out of the tap {t_out} gt after the unlock (the step {} gt after it); T's ride: in the up-dropper at the grant step + {} gt (the up-fire at + {}: margin {} gt); T in D0 at + {} (the next step at + {}: margin {} gt); the filler in the stock at + {} (the release's fire at + {}): {w}", g_step - plan.t_unlock[0], t_up - g_step, up_fire - g_step, up_fire - t_up, t_in - g_step, next_step - g_step, next_step - t_in, f_st - g_step, plan.stock_fire.first().map_or(-1, |&s| s - g_step)));
    // (the skip: no release, the away slot comes round to the tap: the skip step keeps the ring on the model)
    let (p2, e2b, e2e) = reference(16, 40, 80, true);
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
        if rr.end != r.end || rr.stock_end != r.stock_end || !rr.stray.is_empty() || !rr.extra_items.is_empty() {
            if bad.len() < 4 {
                bad.push(format!("a world open at t {rt} (the grant step {:+}): end {:?}, stock {}, stray {:?}", rt - g_step, rr.end, rr.stock_end, rr.stray));
            }
        }
    }
    let w = v.bar_n("ctl-ring8: A WORLD OPEN at every tick of the grant and the release (T mid-ride, the filler mid-path, a lock pulse, the up-fire and the stock fire pending): the end state equals the plain run's", n, bad.is_empty());
    lines.push(format!("  WORLD OPENS (the official load, 2 idle ticks) at every tick t {from}..{to} ({n} rows){}: {w}", if bad.is_empty() { String::new() } else { format!(": {}", bad.join("; ")) }));
    // (the corners: the filler's unlock and the up-fire shifted -2..+2 gt)
    let mut cbad = Vec::new();
    let mut cn = 0usize;
    for df in -2i64..=2 {
        for du in -2i64..=2 {
            let (pc, _, ec) = reference(16, 40 + df, 80 + du, false);
            let rc = run_plan(&spec, &pc)?;
            cn += 1;
            if rc.end != ec || !rc.extra_items.is_empty() || !rc.stray.is_empty() {
                cbad.push(format!("filler {df:+}, up-fire {du:+}: end {:?}", rc.end));
            }
        }
    }
    let w = v.bar_n("ctl-ring8: THE +-2 gt CORNERS (the filler's unlock and the up-fire each shifted -2..+2 gt): the end state matches the model", cn, cbad.is_empty());
    lines.push(format!("  THE +-2 gt CORNERS ({cn} rows){}: {w}", if cbad.is_empty() { String::new() } else { format!(": {}", cbad.join("; ")) }));
    // (the cell count, by category)
    let d = design(1);
    let n_ring = 22;
    let n_t = 1 + T_RIDE.len() + 2;
    let n_f = 1 + F_PATH.len() + 1;
    let n_cmp = 2 + 2;
    let n_locks = 2;
    lines.push(format!("  CELLS laid here: {} (the ring {n_ring}; T's extractor + ride + up-dropper + inserter {n_t}; the filler's extractor + path + stock {n_f}; the comparators + their glass {n_cmp}; the two lock stand-ins {n_locks} (the real locks are the logic's)); NOT YET: the logic (REQ, the spacing latch, the skip's sample, the release's AND, the lock drives, the two timed fires' gates), the caps and the routes", d.blocks.len()));
    Ok(lines)
}

#[cfg(test)]
mod tests {
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
