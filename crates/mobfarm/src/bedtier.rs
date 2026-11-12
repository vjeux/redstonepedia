//! The bed stop (the napper's stop under the station: F1 + Q2, one bed, 24 villagers). A booster
//! entry and an always-on activator that drops every arrival on a ledge, two braked powered rails
//! down a slope (the park: every arrival stops on the first), seven glass-covered flats to the pickup cell
//! beside the bed's head, the curve east, a capped booster, the slope down, R. The rider walks the
//! ledge onto the bed's roof and lies down; woken through the hatch at F (the zombie seen), it can
//! stand only on the pickup cell, which the released cart sweeps.
//!
//! Normal operation only (vjeux 2026-11-12; ironspec 01:16): under L1‴ a restart cannot put a woken
//! napper back to bed before the cart reaches it (wake to pickup 55-75 gt, under SleepInBed's 100-gt
//! LAST_WOKEN guard, a saved memory), so the stop handles no strand. The spurs-and-kicks strand fix
//! (explore/farmopt-station 47bbde753..f8147b303) is the pearl alternative's, for chunks unloaded
//! while game time runs.
//!
//! The unit benches drive the controller's signals (F, the release, the re-arm, the queue's
//! release) and check the sensors and the napper: `bench` one visit, `two_visit_bench` a visit and
//! the next napper's from the queue.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::Vec3;
use redsim::pos::BlockPos;
use redsim::world::World;

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("bed stop: {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The stop's local frame: loop = local + (0,-18,-1) (9fc3155f's budget, explore/farmopt-c3-budget).
/// The entry (a booster, always on), heading west along z -4.
pub const ENTRY: [i32; 3] = [3, 2, -4];
/// The activator, always on; its south side is the landing.
pub const ACTIVATOR: [i32; 3] = [2, 2, -4];
/// The park: two braked powered rails down the slope west; their supports are the release's cells
/// (the bench sets redstone blocks there).
pub const PARK: [[i32; 3]; 2] = [[1, 1, -4], [0, 0, -4]];
pub const PARK_CTRL: [[i32; 3]; 2] = [[1, 0, -4], [0, -1, -4]];
/// The one cell a woken napper can stand on: the pickup cell.
pub const SWEPT: [[i32; 3]; 1] = [[-2, 0, 4]];
pub const BED_HEAD: [i32; 3] = [-3, 0, 4];
/// The hatch: two trapdoors in the zombie box's one opening west of the bed's head (bottom half
/// (-4,0,4), top half (-4,1,4)); open (their controls powered) their plates cut the eye lines;
/// shut (F) they pass.
pub const HATCH: [i32; 3] = [-4, 0, 4];
/// The zombie's post: one glass-cased cell west of the hatch; its boat at the cell's centre.
pub const ZOMBIE_POST: [i32; 3] = [-5, 0, 4];
pub const HATCH_HI: [i32; 3] = [-4, 1, 4];
pub const HATCH_CTRL: [i32; 3] = [-4, 0, 3];
pub const HATCH_CTRL_HI: [i32; 3] = [-4, 2, 4];
/// R's detector (loop (4,-19,4)), heading east.
pub const EXIT_DET: [i32; 3] = [4, -1, 5];
/// The queue cell (loop (9,-8,-6), 9fc3155f's budget 28b1bbd37e): the next napper waits there awake
/// in its cart, 17.7 from the bed's head (the two-visit bench's; the bench design carries its rail).
pub const QUEUE: [i32; 3] = [9, 10, -5];

/// The design (`bench`: with a run-out past R and the queue cell's rail; `trapdoor_caps`: the
/// approach flats covered by top-half trapdoors, the contact mode's positive control).
pub fn design(bench: bool, trapdoor_caps: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-fold-bench" } else { "bed-tier-fold" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    let wall = |b: &mut Builder, c: [i32; 3]| -> Result<()> {
        if !b.d.blocks.contains_key(&c) {
            put(b, c, "glass")?;
        }
        Ok(())
    };
    // the arrival, west along z -4: the entry a booster (always on), the activator (always on),
    // both capped (top-half trapdoors: a rider passes, nobody stands there)
    put(&mut b, ENTRY, "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2]], "redstone_block")?;
    put(&mut b, ACTIVATOR, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
    // (the activator's redstone block on its north side, not under it: under it, it would sit
    // beside the park rail and power it; on the north it is also the dismount's first choice,
    // blocked)
    put(&mut b, [ACTIVATOR[0], 1, ACTIVATOR[2]], "stone")?;
    put(&mut b, [ACTIVATOR[0], 2, ACTIVATOR[2] - 1], "redstone_block")?;
    put(&mut b, [ENTRY[0], 3, ENTRY[2]], cap)?;
    put(&mut b, [ACTIVATOR[0], 3, ACTIVATOR[2]], cap)?;
    // (golem-proofing, ironspec 04:33: a closed top-half trapdoor and a redstone block are golem
    // floors (legacy-solid tops), so the spawn cell over each is filled: glass over the entry's and
    // the activator's caps (no rider reaches y 4) and over the activator's redstone block (it powers
    // through it))
    put(&mut b, [ENTRY[0], 4, ENTRY[2]], "glass")?;
    put(&mut b, [ACTIVATOR[0], 4, ACTIVATOR[2]], "glass")?;
    put(&mut b, [ACTIVATOR[0], 3, ACTIVATOR[2] - 1], "glass")?;
    // the park, two braked powered rails down the slope (every arrival stops on the first; the
    // release powers both: the full second cell's boost narrows the spread of release speeds that
    // the first cell's stop position leaves), a capped flat, the curve south
    for (p, c) in PARK.iter().zip(PARK_CTRL.iter()) {
        put(&mut b, *p, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?;
        put(&mut b, *c, "stone")?;
    }
    let za = ENTRY[2];
    // (the flat after the park keeps its top-half trapdoor: the cart coming off the park's slope
    // still rides high (y 0.55) as its box enters the flat's cell, and a full block at y 1 there
    // stops it; a floor stone button over the trapdoor fills the golem's spawn cell instead. The
    // curve's cover is glass: the cart is down at y 0.06 by then, no rider past the activator)
    put(&mut b, [-1, 0, za], &rail("east_west"))?;
    put(&mut b, [-1, -1, za], "glass")?;
    put(&mut b, [-1, 1, za], cap)?;
    put(&mut b, [-1, 2, za], "stone_button[face=floor,facing=north,powered=false]")?;
    put(&mut b, [-2, 0, za], &rail("south_east"))?;
    put(&mut b, [-2, -1, za], "glass")?;
    put(&mut b, [-2, 1, za], "glass")?;
    // (glass two cells over the park: no villager stands on the parked cart's top)
    put(&mut b, [1, 3, za], "glass")?;
    // seven glass-covered flats down x -2 to the pickup cell (the bed's head west of it): the released
    // cart slows on its 0.96 empty drag to under 0.2 b/t there (a cart faster than that stops dead
    // against a standing villager)
    for z in za + 1..=SWEPT[0][2] {
        put(&mut b, [-2, 0, z], &rail("north_south"))?;
        put(&mut b, [-2, -1, z], "glass")?;
        if z != SWEPT[0][2] {
            put(&mut b, [-2, 1, z], if trapdoor_caps { cap } else { "glass" })?;
        }
    }
    // (the flats' covers are glass, full blocks at y 1, over the empty cart (its box tops out at y
    // 0.76; it carries no rider past the activator), not top-half trapdoors: the pathfinder takes
    // a trapdoor cell for passable, so a panicking napper on the pickup cell (on a rail, it may path
    // along rails) could take a flee path north up the approach, toward the arriving cart, and press
    // into it: after a restart its rebuilt brain draws a new flee target, and the cart stopped dead
    // against it (G1/ironspec's contact mode). With glass there is no path north; its flee path
    // runs south, away from the cart. (-2,1,3) is also the ledge's floor under (-2,2,3). `trapdoor_caps`:
    // the old covers, the positive control)
    // the curve east (capped), a capped booster (always on: a timing part, the exit's speed), a
    // capped flat, the slope down, two flats, R
    put(&mut b, [-2, 0, 5], &rail("north_east"))?;
    put(&mut b, [-2, -1, 5], "glass")?;
    put(&mut b, [-2, 1, 5], cap)?;
    put(&mut b, [-1, 0, 5], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, -1, 5], "redstone_block")?;
    put(&mut b, [-1, 1, 5], cap)?;
    put(&mut b, [0, 0, 5], &rail("east_west"))?;
    put(&mut b, [0, -1, 5], "glass")?;
    put(&mut b, [0, 1, 5], cap)?;
    // (golem-proofing: the rider's box reaches the cells over these caps, so a floor stone button on
    // each fills the spawn cell (the floor's cell above must be air or liquid; the button is no golem
    // floor, has no collision box and, unpressed, powers nothing; ironspec's carpet, which redsim
    // doesn't model, does the same))
    for x in -2..=0 {
        put(&mut b, [x, 2, 5], "stone_button[face=floor,facing=north,powered=false]")?;
    }
    put(&mut b, [1, -1, 5], &rail("ascending_west"))?;
    put(&mut b, [1, -2, 5], "stone")?;
    for x in 2..=3 {
        put(&mut b, [x, -1, 5], &rail("east_west"))?;
        put(&mut b, [x, -2, 5], "glass")?;
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "stone")?;
    // the bed, its head beside the pickup cell, its foot north, roofed by two top-half trapdoors
    // (0.25 of headroom over the bed's top: nobody stands on it; a standing napper's eye line runs
    // under them)
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    put(&mut b, [-3, -1, 4], "glass")?;
    put(&mut b, [-3, 1, 3], cap)?;
    put(&mut b, [-3, 1, 4], cap)?;
    // (golem-proofing: the napper's walk ends on the roof, so a floor stone button on each roof
    // trapdoor fills the spawn cell over it; the napper walks through it (no collision box) and
    // stands on the trapdoor's top as before, 1.80 and 1.50 from the bed head's centre, inside
    // SleepInBed's 2)
    put(&mut b, [-3, 2, 3], "stone_button[face=floor,facing=north,powered=false]")?;
    put(&mut b, [-3, 2, 4], "stone_button[face=floor,facing=north,powered=false]")?;
    // the hatch west of the bed's head and the zombie box behind it (one cell, x -5; the west wall
    // on x -6): open (powered by the redstone blocks (-4,0,3) and (-4,2,4)) the plates stand across
    // the cells' east sides and cut every line; shut (F) the sleeper's eye line and a standing
    // napper's (under the bed's roof) pass. The zombie (persistent) in a boat at (-5.0, 0, 4.5),
    // glass all round, leaves over it
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, "redstone_block")?;
    put(&mut b, HATCH_CTRL_HI, "redstone_block")?;
    put(&mut b, [-4, -1, 4], "glass")?;
    put(&mut b, [-5, -1, 4], "glass")?;
    put(&mut b, [-5, 2, 4], "glass")?;
    for y in 0..=1 {
        put(&mut b, [-5, y, 3], "glass")?;
        put(&mut b, [-5, y, 5], "glass")?;
        put(&mut b, [-6, y, 4], "glass")?;
        put(&mut b, [-4, y, 5], "glass")?;
    }
    put(&mut b, [-4, 1, 3], "glass")?;
    // (the zombie's cover: persistent leaves at distance 7, as the build leaves them with no log near)
    put(&mut b, [-5, 3, 4], "oak_leaves[distance=7,persistent=true,waterlogged=false]")?;
    // (the boat at the post cell's centre (-4.5, 0, 4.5): cbcac083a's (-5.0, 0, 4.5) was the cell's
    // west edge, x written without its + 0.5)
    b.entity("oak_boat", [ZOMBIE_POST[0] as f64 + 0.5, ZOMBIE_POST[1] as f64, ZOMBIE_POST[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    // the pit: glass north of the bed's foot, south of its head, east of the flats and the pickup
    // cell (x -1, z 0..4: the ledge's floor at y 1; z 5 is the exit's booster and its cap)
    for y in 0..=1 {
        put(&mut b, [-3, y, 2], "glass")?;
        wall(&mut b, [-3, y, 5])?;
        for z in 0..=5 {
            wall(&mut b, [-1, y, z])?;
        }
    }
    // the landing L (2,2,-3) south of the activator (its north side the activator's redstone
    // block: the dismount's first choice, blocked), the ledge: south to (2,2,0), west along z 0 to
    // (-1,2,0), south down x -1 to (-1,2,3), west over the flat's cover (-2,2,3) onto the bed's roof
    // (-3,2,3), (-3,2,4): there the bed's head is within SleepInBed's 2 (a villager off a rail
    // cannot path onto one, so the walk ends there; the open column over the pickup cell west of
    // (-2,2,3) is a way in too). Walls two high round the ledge (the parked cart's top and the
    // caps' tops lead nowhere)
    // (the ledge's floor is glowstone, not stone: the same full block for the napper's walk and its
    // path types, and no golem floor (`SpawnUtil`'s legacy iron golem test excludes it))
    put(&mut b, [2, 1, -3], "glowstone")?;
    put(&mut b, [2, 1, -2], "glowstone")?;
    put(&mut b, [2, 1, -1], "glowstone")?;
    put(&mut b, [2, 1, 0], "glowstone")?;
    put(&mut b, [1, 1, 0], "glowstone")?;
    put(&mut b, [0, 1, 0], "glowstone")?;
    // (the pickup cell's column (-2,1..2,4) stays open: a napper falling in stands on a rail and so
    // can path to the bed)
    let ledge: [[i32; 3]; 12] = [[2, 2, -3], [2, 2, -2], [2, 2, -1], [2, 2, 0], [1, 2, 0], [0, 2, 0], [-1, 2, 0], [-1, 2, 1], [-1, 2, 2], [-1, 2, 3], [-2, 2, 3], [-3, 2, 3]];
    let ledge_end = [-3, 2, 4];
    for c in ledge.iter().chain(std::iter::once(&ledge_end)) {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [c[0] + dx, 2, c[2] + dz];
            if ledge.contains(&n) || n == ledge_end || n == [-2, 2, 4] || n == [ACTIVATOR[0], 2, ACTIVATOR[2]] {
                continue;
            }
            wall(&mut b, n)?;
            wall(&mut b, [n[0], 3, n[2]])?;
        }
    }
    if bench {
        // the run-out past R and a stop; the queue cell's rail (the two-visit bench's)
        for x in EXIT_DET[0] + 1..=EXIT_DET[0] + 6 {
            put(&mut b, [x, -1, EXIT_DET[2]], &rail("east_west"))?;
            put(&mut b, [x, -2, EXIT_DET[2]], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, EXIT_DET[2]], "glass")?;
        put(&mut b, QUEUE, &rail("east_west"))?;
        put(&mut b, [QUEUE[0], QUEUE[1] - 1, QUEUE[2]], "stone")?;
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    Ok(b.text(&[
        "The bed stop (local frame; under the station, loop = local + (0,-18,-1); the bed's head at (-3,0,4)): a booster entry and an always-on activator heading west, two braked park rails down a slope, a capped flat, a curve south, seven flats under glass, the pickup cell beside the bed, the curve east, a capped booster, a capped flat, the slope down, two flats, R.",
        "The rider lands on the ledge south of the activator, walks round onto the bed's roof and sleeps; woken through the hatch it stands only on the pickup cell and keeps seeing the zombie there; the released cart reaches it under 0.2 b/t and seats it; the cart runs on to R. Normal operation only: under L1 a restart cannot put a woken napper back to bed before the cart reaches it (SleepInBed's 100-gt LAST_WOKEN guard), so no strand handling.",
    ]))
}

/// One visit's unit-bench row (ticks counted from the cart's spawn at the entry).
#[derive(Debug, Default, Clone)]
pub struct Row {
    pub seed: u64,
    pub speed: f64,
    /// At each lie-down, the napper's distance (feet, the tick before) to the bed head's centre
    /// (SleepInBed's bar: under 2).
    pub lie_dists: Vec<f64>,
    /// The rider's first standing cell after the eject, and when.
    pub landed: Option<([i32; 3], i64)>,
    /// Where the arriving cart stopped (its centre) and when.
    pub parked: Option<([f64; 3], i64)>,
    pub slept_at: Option<i64>,
    pub woke_at: Option<i64>,
    /// Every lie-down and every wake, in order.
    pub sleeps: Vec<i64>,
    pub wakes: Vec<i64>,
    /// The first tick the napper had no HOME memory (it arrived with one).
    pub home_lost_at: Option<i64>,
    /// After the restart: the directions of the woken napper's paths off the pickup cell (north is
    /// toward the arriving cart).
    pub paths_after_restart: Vec<&'static str>,
    /// The napper's cell on its first awake tick after the nap.
    pub stood: Option<[i32; 3]>,
    /// Every cell the awake napper stood on between the wake and the pickup.
    pub cells_after_wake: Vec<[i32; 3]>,
    /// The release tick, and the cart's horizontal speed as it crossed each swept cell's centre.
    pub release_at: Option<i64>,
    pub swept_speed: Vec<([i32; 3], f64)>,
    /// The pickup: the cell the napper stood on and the tick.
    pub pickup: Option<([i32; 3], i64)>,
    /// The cart's horizontal speed in the tick it seated the napper.
    pub pickup_speed: Option<f64>,
    /// The cart over R's detector (the stop clear).
    pub r_at: Option<i64>,
    /// The cart a block past R (the visit's end), and whether the napper rode it.
    pub out_at: Option<i64>,
    pub seated_out: bool,
    pub problems: Vec<String>,
}

/// The bench's fault and control rows.
#[derive(Debug, Default, Clone)]
pub struct Faults {
    /// The cart sent empty (no villager): a control.
    pub empty: bool,
    /// The villager arrives with no HOME memory: a control (it never walks to the bed).
    pub no_home: bool,
    /// The waker hidden again once the napper is awake (the hatch back open-plated): the refuge's
    /// control (the napper should go back to bed).
    pub hide_after_wake: bool,
    /// A save, stop and start between ticks T - 1 and T (T gt after the entry) in this load order
    /// (`liveness::restart`: no game time passes; the lead's rows run all 10 orders).
    pub restart: Option<(i64, crate::liveness::Restart)>,
    /// The visit's day time exactly (in place of the seeded night hour).
    pub hour: Option<i64>,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// The direction of a villager's current path from the pickup cell: its first node off the cell
/// (north is up the approach, toward the arriving cart).
fn path_dir(w: &World, i: usize) -> Option<&'static str> {
    use redsim::entity::mob::ai::brain;
    if !brain::has_exact(w, i) {
        return None;
    }
    let p = brain::ex(w, i).mem.path.as_ref()?;
    let n = p.nodes.iter().skip(p.next_node_index).find(|n| (n.x, n.z) != (SWEPT[0][0], SWEPT[0][2]))?;
    Some(if n.z < SWEPT[0][2] {
        "north"
    } else if n.z > SWEPT[0][2] {
        "south"
    } else if n.x > SWEPT[0][0] {
        "east"
    } else {
        "west"
    })
}

/// One visit: the visitor's cart onto the entry at `speed` (-x) at night (or by day with `day`), F
/// (the hatch's controls off: the zombie seen) `after_sleep` gt after the napper's lie-down, the
/// release (the park's supports to redstone blocks) `release_after` gt after F, until the cart is a
/// block past R or 2400 gt; the controller on game ticks.
pub fn bench(spec: &FarmSpec, seed: u64, speed: f64, release_after: i64, day: bool, after_sleep: i64, f: &Faults) -> Result<Row, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut row = Row { seed, speed, ..Default::default() };
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    // (wait for the visit's hour: night 13,800 + seed spread, or day 2,000, or `f.hour`)
    let want = f.hour.unwrap_or(if day { 2000 } else { 13_800 } + (seed as i64 * 37) % 400);
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    // the visitor at the entry, HOME = the bed (as the game loads the memory)
    let h = BED_HEAD;
    let nbt = if f.empty {
        "{Rotation:[0f,0f]}".to_string()
    } else if f.no_home {
        "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}".to_string()
    } else {
        format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2])
    };
    let start = [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5];
    let mut cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
    let mut vill = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied());
    if vill.is_none() && !f.empty {
        return Err("no rider".into());
    }
    run.own.push(cart_id);
    run.own.extend(vill);
    {
        let w = &mut run.world;
        let i = idx(w, cart_id).ok_or("cart gone")?;
        w.entities.list[i].pos = Vec3::new(start[0], start[1], start[2]);
        w.entities.list[i].delta_movement = Vec3::new(-speed, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = idx(w, pid) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(st), redsim::world::flags::ALL);
    let mut was_asleep = false;
    let mut last_awake: Option<Vec3> = None;
    let mut last_cart = [0f64; 3];
    let mut still = 0;
    let mut crossed: Vec<[i32; 3]> = Vec::new();
    let mut hidden = false;
    let mut live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut had_home = !f.empty && !f.no_home;
    for t in 1..=2400i64 {
        // (a save, stop and start between ticks t - 1 and t: no game time passes; every entity back
        // as a new object under a new id, in the row's load order)
        if let Some((at, how)) = f.restart
            && t == at
        {
            crate::liveness::restart(&mut run.world, how)?;
            cart_id = crate::liveness::follow(&run.world, cart_id);
            vill = vill.map(|v| crate::liveness::follow(&run.world, v));
        }
        live += 1;
        // (F: `after_sleep` gt after the lie-down (the bed observer's edge); the release
        // `release_after` gt after F)
        if !f_fired && f_live.is_some_and(|x| live >= x) {
            f_fired = true;
            set(&mut run.world, HATCH_CTRL, "stone");
            set(&mut run.world, HATCH_CTRL_HI, "stone");
        }
        if f.hide_after_wake && !hidden && row.woke_at.is_some() {
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            hidden = true;
        }
        if row.release_at.is_none() && f_fired && f_live.is_some_and(|x| live >= x + release_after) {
            for c in PARK_CTRL {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        run.tick();
        let w = &run.world;
        let Some(ci) = idx(w, cart_id) else {
            row.problems.push(format!("t {t}: the cart is gone"));
            break;
        };
        let c = &w.entities.list[ci];
        let vi = vill.and_then(|id| idx(w, id));
        if vill.is_some() && vi.is_none() {
            row.problems.push(format!("t {t}: the villager is gone"));
            break;
        }
        let mut riding = false;
        if let Some(vi) = vi {
            let v = &w.entities.list[vi];
            let vcell = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32];
            riding = v.vehicle.is_some();
            // (the napper's HOME every tick; its loss is a problem)
            let has_home = redsim::entity::mob::ai::brain::brain(w, vi).home.is_some();
            if had_home && !has_home && row.home_lost_at.is_none() {
                row.home_lost_at = Some(t);
            }
            had_home = has_home;
            let asleep = redsim::entity::mob::mob(w, vi).sleeping_pos.is_some();
            if !riding && row.landed.is_none() && v.on_ground {
                row.landed = Some((vcell, t));
            }
            if asleep && !was_asleep {
                row.sleeps.push(t);
                // (SleepInBed's reach: the napper's feet on the tick before, to the bed head's centre)
                if let Some(p) = last_awake {
                    let c = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5);
                    row.lie_dists.push(p.distance_to_sqr(c).sqrt());
                }
            }
            if !asleep && !riding {
                last_awake = Some(v.pos);
            }
            if asleep && row.slept_at.is_none() {
                row.slept_at = Some(t);
                f_live = Some(live + after_sleep);
            }
            if was_asleep && !asleep {
                row.woke_at = Some(t);
                row.wakes.push(t);
                row.stood = Some(vcell);
            }
            if row.woke_at.is_some() && !riding && !asleep && row.pickup.is_none() && !row.cells_after_wake.contains(&vcell) {
                row.cells_after_wake.push(vcell);
            }
            // (after a restart: the woken napper's path off the pickup cell, by direction)
            if f.restart.is_some_and(|(at, _)| t >= at)
                && !riding
                && !asleep
                && vcell == SWEPT[0]
                && let Some(d) = path_dir(w, vi)
                && !row.paths_after_restart.contains(&d)
            {
                row.paths_after_restart.push(d);
            }
            was_asleep = asleep;
            if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && t % 5 == 0 {
                let b = redsim::entity::mob::ai::brain::brain(w, vi);
                eprintln!("t {t}: villager {:.2} {:.2} {:.2} riding {riding} asleep {asleep} home {:?} hostile {:?} | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", v.pos.x, v.pos.y, v.pos.z, b.home, b.nearest_hostile, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
            }
            if riding && row.release_at.is_some() && row.pickup.is_none() {
                row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
                row.pickup_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
            }
            if row.release_at.is_none() && row.landed.is_some() && riding && t > 5 {
                row.problems.push(format!("t {t}: the napper back in a cart before the release (at {vcell:?})"));
                break;
            }
        }
        // the cart: parked (still for 10 gt before the release), its speed over the swept cells
        let cp = [c.pos.x, c.pos.y, c.pos.z];
        let moved = (cp[0] - last_cart[0]).abs() + (cp[2] - last_cart[2]).abs() + (cp[1] - last_cart[1]).abs();
        still = if moved < 1e-4 { still + 1 } else { 0 };
        if still == 10 && row.parked.is_none() && row.release_at.is_none() {
            row.parked = Some((cp, t));
        }
        last_cart = cp;
        if let Some(bp) = before
            && row.release_at.is_some()
        {
            for s in SWEPT {
                let zc = s[2] as f64 + 0.5;
                if (bp.z < zc) != (c.pos.z < zc) && (c.pos.x - (s[0] as f64 + 0.5)).abs() < 0.3 && !crossed.contains(&s) {
                    crossed.push(s);
                    row.swept_speed.push((s, c.delta_movement.x.hypot(c.delta_movement.z)));
                }
            }
        }
        // (R and out: the exit runs east along R's row)
        let on_exit_row = (c.pos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3;
        if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 1.5 {
            row.out_at = Some(t);
            row.seated_out = riding;
            break;
        }
    }
    if row.out_at.is_none() {
        row.problems.push("the cart not out in 2400 gt".into());
    }
    if row.out_at.is_some() && !row.seated_out {
        row.problems.push("a riderless cart left the stop".into());
    }
    if let Some(t) = row.home_lost_at {
        row.problems.push(format!("the napper's HOME lost at t {t}"));
    }
    if row.paths_after_restart.contains(&"north") {
        row.problems.push("a northward path off the pickup cell after the restart".into());
    }
    let expect_seated = !f.empty && !f.no_home && !f.hide_after_wake;
    if expect_seated && !row.seated_out {
        row.problems.push("the napper not seated".into());
    }
    if !f.hide_after_wake {
        for c in &row.cells_after_wake {
            if !SWEPT.contains(c) {
                row.problems.push(format!("the woken napper stood on {c:?}, outside the swept cells"));
            }
        }
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}

/// G1's two-visit row on the lead's geometry (ironspec 00:52). Visit 1's napper A arrives at the
/// entry; the next napper C waits awake in its cart on the queue cell. R's pulse (A's cart past
/// R's detector) re-arms the stop (the park braked, the hatch open) and releases the queue:
/// C's cart reaches the entry `transit` gt later heading west at the visit's speed (emulated: the
/// trapdoor's 8-block drop and the run west, 9fc3155f's ~37 gt; the bed is free all that time).
/// A rides from R into its lift (emulated: its cart straight up from where it is `lift_delay` gt
/// after R, at `lift_speed` b/t, to local y 20, loop 2; farmopt's pick (c), past 17 about 30 gt
/// after R). The controller as in `bench`: F 10 gt after each visit's lie-down edge (re-armed per
/// arrival), the release 30 gt after F. A restart (`liveness::restart`) between two ticks.
/// Measured: A's distance to the bed head's centre at C's lie-down (the bar: over 17), both HOMEs
/// every tick and at the end, each visit's R.
///
/// The daylight row (`day_hold`, ironspec 02:33): A, picked before a night skip, reaches the queue
/// cell at day time `.0`; the queue's NOT DAY gate holds it there until day time `.1` (DAY off),
/// then it drops (its cart at the entry `transit` gt later); C, the night's first scheduled
/// napper, reaches the queue cell `.2` gt after A dropped and waits there for A's R.
#[derive(Debug, Clone, Copy)]
pub struct TwoOpts {
    pub restart: Option<(i64, crate::liveness::Restart)>,
    pub transit: i64,
    pub lift_delay: i64,
    pub lift_speed: f64,
    pub day_hold: Option<(i64, i64, i64)>,
    /// (the daylight row) C reaches the queue cell at this day time (its real first-napper time)
    /// instead of `day_hold.2` gt after A's drop.
    pub c_at: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct TwoRow {
    /// The world's day time at tick 0 (A's arrival).
    pub start_day: i64,
    /// (the daylight row) A's drop from the queue, C's arrival at the queue cell.
    pub a_drop: Option<i64>,
    pub c_queued: Option<i64>,
    pub a_entry: Option<i64>,
    pub a_slept: Option<i64>,
    pub a_r: Option<i64>,
    pub c_entry: Option<i64>,
    pub c_slept: Option<i64>,
    pub a_dist_at_c_sleep: Option<f64>,
    pub c_r: Option<i64>,
    pub a_seated: bool,
    pub c_seated: bool,
    pub home_lost: Vec<(&'static str, i64)>,
    pub homes_end: (bool, bool),
    /// After the restart, a napper on the pickup cell had a path north (toward the arriving cart).
    pub north_path: bool,
    /// At each lie-down (A's and C's), the napper's feet the tick before to the bed head's centre.
    pub lie_dists: Vec<f64>,
    pub problems: Vec<String>,
}

pub fn two_visit_bench(spec: &FarmSpec, seed: u64, speed: f64, o: &TwoOpts) -> Result<TwoRow, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let want = o.day_hold.map_or(13_800 + (seed as i64 * 37) % 400, |d| d.0);
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(st), redsim::world::flags::ALL);
    let h = BED_HEAD;
    let nbt = format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2]);
    // (move a cart to `at` with velocity `v`, its rider with it)
    let place = |w: &mut World, cid: u32, at: [f64; 3], v: Vec3| {
        if let Some(i) = w.entities.list.iter().position(|e| e.id == cid && !e.removed) {
            w.entities.list[i].pos = Vec3::new(at[0], at[1], at[2]);
            w.entities.list[i].delta_movement = v;
            let ps = w.entities.list[i].passengers.clone();
            for pid in ps {
                if let Some(j) = w.entities.list.iter().position(|e| e.id == pid) {
                    redsim::entity::position_rider(w, i, j);
                }
            }
        }
    };
    let spawn = |run: &mut redsim::entity::mob::farm::Run, at: [f64; 3], v: Vec3| -> Result<(u32, u32), String> {
        let cid = redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, Some(&nbt))?;
        let vid = run.world.entities.list.iter().find(|e| e.id == cid).and_then(|e| e.passengers.first().copied()).ok_or("no rider")?;
        run.own.push(cid);
        run.own.push(vid);
        place(&mut run.world, cid, at, v);
        Ok((cid, vid))
    };
    let entry = [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5];
    let queue = [QUEUE[0] as f64 + 0.5, QUEUE[1] as f64 + 0.0625, QUEUE[2] as f64 + 0.5];
    if !matches!(run.world.get(BlockPos::new(QUEUE[0], QUEUE[1], QUEUE[2])), Block::PlainRail { .. }) {
        return Err("no queue rail: the two-visit row needs a `bedtier --bench` design".into());
    }
    // (the normal row: A at the entry, C waiting at the queue; the daylight row: A waiting at the
    // queue, C to come)
    let (mut cart1, mut a) = if o.day_hold.is_some() { spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))? } else { spawn(&mut run, entry, Vec3::new(-speed, 0.0, 0.0))? };
    let mut c2: Option<(u32, u32)> = if o.day_hold.is_some() { None } else { Some(spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))?) };
    let head = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5);
    let mut row = TwoRow { start_day: want, a_entry: o.day_hold.is_none().then_some(0), ..Default::default() };
    let mut visit: u8 = 1;
    let mut live = 0i64;
    let mut arrival_live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut released = false;
    let mut was_occ = false;
    let mut homes = [true, true];
    let mut last_awake2: [Option<Vec3>; 2] = [None, None];
    let mut was_asleep2 = [false, false];
    let mut lift_x: Option<f64> = None;
    let past_r = |w: &World, cid: u32| -> bool {
        w.entities.list.iter().find(|e| e.id == cid && !e.removed).is_some_and(|e| (e.pos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3 && e.pos.x > EXIT_DET[0] as f64 + 0.5)
    };
    let limit = 4000 + o.day_hold.map_or(0, |d| (d.1 - d.0).rem_euclid(24000));
    for t in 1..=limit {
        if let Some((at, how)) = o.restart
            && t == at
        {
            crate::liveness::restart(&mut run.world, how)?;
            cart1 = crate::liveness::follow(&run.world, cart1);
            a = crate::liveness::follow(&run.world, a);
            c2 = c2.map(|(cc, cv)| (crate::liveness::follow(&run.world, cc), crate::liveness::follow(&run.world, cv)));
        }
        live += 1;
        // (the daylight row: the NOT DAY gate drops A at DAY off (its cart at the entry `transit`
        // gt later); C reaches the queue cell `.2` gt after the drop)
        if let Some((_, day_off, gap)) = o.day_hold {
            if row.a_drop.is_none() && run.world.day_time().rem_euclid(24000) == day_off {
                row.a_drop = Some(t);
            }
            if row.a_entry.is_none() && row.a_drop.is_some_and(|d| t >= d + o.transit) {
                place(&mut run.world, cart1, entry, Vec3::new(-speed, 0.0, 0.0));
                row.a_entry = Some(t);
                arrival_live = live;
            }
            if c2.is_none() && row.a_drop.is_some_and(|d| match o.c_at { Some(at) => t > d && run.world.day_time().rem_euclid(24000) == at, None => t >= d + gap }) {
                c2 = Some(spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))?);
                row.c_queued = Some(t);
            }
        }
        // (the queue's release: C's cart at the entry `transit` gt after A's R, or after C reaches
        // the queue if that is later: the gate is open by then)
        if visit == 1
            && let Some((cc, _)) = c2
            && row.a_r.is_some_and(|r| t >= r.max(row.c_queued.unwrap_or(0)) + o.transit)
        {
            place(&mut run.world, cc, entry, Vec3::new(-speed, 0.0, 0.0));
            row.c_entry = Some(t);
            visit = 2;
            arrival_live = live;
        }
        // (F: a lie-down edge after the visit's arrival, +10; the release 30 after F)
        let occ = matches!(run.world.get(BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])), Block::Bed { occupied: true, .. });
        if row.a_entry.is_some() {
            if occ && !was_occ && f_live.is_none() && live > arrival_live {
                f_live = Some(live + 10);
            }
            was_occ = occ;
            if !f_fired && f_live.is_some_and(|x| live >= x) {
                f_fired = true;
                set(&mut run.world, HATCH_CTRL, "stone");
                set(&mut run.world, HATCH_CTRL_HI, "stone");
            }
            if f_fired && !released && f_live.is_some_and(|x| live >= x + 30) {
                released = true;
                for pc in PARK_CTRL {
                    set(&mut run.world, pc, "redstone_block");
                }
            }
        }
        run.tick();
        // (A's lift: its cart straight up from `lift_delay` gt after R, to local y 20 (loop 2:
        // past the shaft's top at loop -3), then held there)
        if let Some(r) = row.a_r
            && t >= r + o.lift_delay
        {
            let x = *lift_x.get_or_insert_with(|| run.world.entities.list.iter().find(|e| e.id == cart1).map_or(EXIT_DET[0] as f64 + 0.5, |e| e.pos.x));
            let y = (EXIT_DET[1] as f64 + 0.0625 + o.lift_speed * (t - r - o.lift_delay) as f64).min(20.0625);
            place(&mut run.world, cart1, [x, y, EXIT_DET[2] as f64 + 0.5], Vec3::new(0.0, 0.0, 0.0));
        }
        let w = &run.world;
        if std::env::var_os("MCTAS_BT2_TRACE").is_some() && (t % 10 == 0 || std::env::var("MCTAS_BT2_TRACE").is_ok_and(|s| s.split(',').filter_map(|x| x.parse::<i64>().ok()).collect::<Vec<_>>().windows(2).any(|r| t >= r[0] && t <= r[1]))) {
            let p = |id: u32| w.entities.list.iter().find(|e| e.id == id).map(|e| format!("({:.2},{:.2},{:.2}){}{}", e.pos.x, e.pos.y, e.pos.z, if e.removed { " removed" } else { "" }, e.vehicle.map_or(String::new(), |v| format!(" in {v}"))));
            // (A's brain: HOME, CANT_REACH_WALK_TARGET_SINCE, WALK_TARGET, the activity)
            let br = idx(w, a).filter(|&i| redsim::entity::mob::ai::brain::has_exact(w, i)).map(|i| {
                let x = redsim::entity::mob::ai::brain::ex(w, i);
                format!("home {:?} cant_reach {:?} walk_target {} act {:?}", redsim::entity::mob::ai::brain::brain(w, i).home.map(|h| [h.x, h.y, h.z]), x.mem.cant_reach_since, x.mem.walk_target.is_some(), x.active)
            });
            eprintln!("t {t} day {}: A {:?} cart1 {:?} {:?} | C {:?}", w.day_time().rem_euclid(24000), p(a), p(cart1), br, c2.map(|(cc, cv)| (p(cv), p(cc))));
        }
        let mut who = vec![(0usize, a, cart1)];
        if let Some((cc, cv)) = c2 {
            who.push((1, cv, cc));
        }
        for (k, id, cart) in who {
            let Some(i) = idx(w, id) else {
                row.problems.push(format!("t {t}: villager {} gone", if k == 0 { "A" } else { "C" }));
                return Ok(row);
            };
            let has = redsim::entity::mob::ai::brain::brain(w, i).home.is_some();
            if homes[k] && !has {
                row.home_lost.push((if k == 0 { "A" } else { "C" }, t));
            }
            homes[k] = has;
            let asleep = redsim::entity::mob::mob(w, i).sleeping_pos.is_some();
            let seated = w.entities.list[i].vehicle == Some(cart) && released;
            // (after a restart: a napper on the pickup cell with a path north, toward the cart)
            let v = &w.entities.list[i];
            if o.restart.is_some_and(|(at, _)| t >= at)
                && v.vehicle.is_none()
                && !asleep
                && [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32] == SWEPT[0]
                && path_dir(w, i) == Some("north")
            {
                row.north_path = true;
            }
            // (SleepInBed's reach at each lie-down: the feet the tick before, to the bed head's centre)
            if asleep && !was_asleep2[k] {
                if let Some(p) = last_awake2[k] {
                    row.lie_dists.push(p.distance_to_sqr(head).sqrt());
                }
            }
            if !asleep && v.vehicle.is_none() {
                last_awake2[k] = Some(v.pos);
            }
            was_asleep2[k] = asleep;
            if k == 0 {
                if asleep && row.a_slept.is_none() {
                    row.a_slept = Some(t);
                }
                if visit == 1 && seated {
                    row.a_seated = true;
                }
            } else {
                if asleep && row.c_slept.is_none() && row.c_entry.is_some() {
                    row.c_slept = Some(t);
                    if let Some(ai) = idx(w, a) {
                        row.a_dist_at_c_sleep = Some(w.entities.list[ai].pos.distance_to_sqr(head).sqrt());
                    }
                }
                if visit == 2 && seated {
                    row.c_seated = true;
                }
            }
        }
        // (A's cart past R: R's pulse re-arms the stop and releases the queue)
        if row.a_r.is_none() && past_r(w, cart1) {
            row.a_r = Some(t);
            for pc in PARK_CTRL {
                set(&mut run.world, pc, "stone");
            }
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            f_live = None;
            f_fired = false;
            released = false;
        }
        if visit == 2
            && row.c_r.is_none()
            && let Some((cc, _)) = c2
            && past_r(&run.world, cc)
        {
            row.c_r = Some(t);
            break;
        }
    }
    row.homes_end = (homes[0], homes[1]);
    if !row.a_seated || row.a_r.is_none() {
        row.problems.push("A not seated out".into());
    }
    if row.c_slept.is_none() {
        row.problems.push("C never lay down".into());
    }
    if let Some(d) = row.a_dist_at_c_sleep
        && d <= 17.0
    {
        row.problems.push(format!("A within 17 of the bed's head at C's lie-down ({d:.2})"));
    }
    if !row.c_seated || row.c_r.is_none() {
        row.problems.push("C not seated out".into());
    }
    if !row.home_lost.is_empty() {
        row.problems.push(format!("HOME lost {:?}", row.home_lost));
    }
    if row.north_path {
        row.problems.push("a northward path off the pickup cell after the restart".into());
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}

/// `--orders`: `all` (the helper's 10 rows) or a list of save, reverse, seed:K.
pub fn orders(s: &str) -> Result<Vec<crate::liveness::Restart>> {
    if s.trim() == "all" {
        return Ok(crate::liveness::Restart::rows());
    }
    s.split(',').map(|x| crate::liveness::Restart::parse(x).ok_or_else(|| anyhow::anyhow!("--orders: {x} is not save, reverse or seed:K"))).collect()
}

/// ironspec's placement-order rule (farmopt 03:38): every cell the guide's build order leaves unlike
/// the design (a rail whose natural shape under the order is not its designed one: a curve ending
/// at no rail, a slope with no rail above its high end), as `crate::placing::diff_from_file` finds
/// it after a build in order (20 gt between placements). Each needs a listed order or a temporary
/// helper rail.
pub fn placement_order_cells(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Built { gap: Some(20) }, &crate::placing::BuildPlan::default())?;
    crate::placing::diff_from_file(&run.world, spec, &ro)
}

/// ironspec 04:46: the zombie's boat and its rider at bench start (the benches' start: the judge's
/// placement, the design's entities spawned) and 20 gt later, each against the post cell.
pub fn zombie_post_lines(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut out = Vec::new();
    for t in [0, 20] {
        for _ in 0..t {
            run.tick();
        }
        for e in run.world.entities.list.iter().filter(|e| !e.removed && e.id >= first) {
            let inside = e.pos.x >= ZOMBIE_POST[0] as f64 && e.pos.x <= ZOMBIE_POST[0] as f64 + 1.0 && e.pos.z >= ZOMBIE_POST[2] as f64 && e.pos.z <= ZOMBIE_POST[2] as f64 + 1.0;
            let name = match &e.kind {
                redsim::entity::EntityKind::Boat(_) => "the boat".to_string(),
                redsim::entity::EntityKind::Mob(m) => format!("the {:?}", m.ty),
                _ => "an entity".to_string(),
            };
            // (a mob's eye: the sky light there (ironspec's zombie-post check: under 15, so it never
            // sees the sky and never burns))
            let sky = matches!(e.kind, redsim::entity::EntityKind::Mob(_)).then(|| {
                let eye = BlockPos::new(e.pos.x.floor() as i32, (e.pos.y + e.kind.eye_height() as f64).floor() as i32, e.pos.z.floor() as i32);
                format!(", eye cell ({},{},{}) sky light {}", eye.x, eye.y, eye.z, redsim::daylight::sky_light(&run.world, eye).map_or_else(|x| x, |l| l.to_string()))
            });
            out.push(format!("t {t}: {name} at ({:.3}, {:.3}, {:.3}): {}{}", e.pos.x, e.pos.y, e.pos.z, if inside { "inside the post cell" } else { "OUTSIDE the post cell" }, sky.unwrap_or_default()));
        }
    }
    Ok(out)
}

/// A rail block's shape (plain, powered, activator or detector).
fn rail_shape(b: &Block) -> Option<redsim::block::RailShape> {
    match b {
        Block::Rail { shape, .. } | Block::PlainRail { shape } | Block::DetectorRail { shape, .. } => Some(*shape),
        _ => None,
    }
}

/// The cells a rail's shape points to (its raw connections; an ascending end one level up).
fn raw_connections(p: BlockPos, s: redsim::block::RailShape) -> [BlockPos; 2] {
    use redsim::block::RailShape::*;
    let at = |dx: i32, dy: i32, dz: i32| BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
    match s {
        NorthSouth => [at(0, 0, -1), at(0, 0, 1)],
        EastWest => [at(-1, 0, 0), at(1, 0, 0)],
        AscendingEast => [at(-1, 0, 0), at(1, 1, 0)],
        AscendingWest => [at(-1, 1, 0), at(1, 0, 0)],
        AscendingNorth => [at(0, 1, -1), at(0, 0, 1)],
        AscendingSouth => [at(0, 0, -1), at(0, 1, 1)],
        SouthEast => [at(0, 0, 1), at(1, 0, 0)],
        SouthWest => [at(0, 0, 1), at(-1, 0, 0)],
        NorthWest => [at(0, 0, -1), at(-1, 0, 0)],
        NorthEast => [at(0, 0, -1), at(1, 0, 0)],
    }
}

/// farmopt's rail rules (ironspec's detector rule, 03:54; the sloped switched rail's neighbours,
/// 03:55): the stop's Ts (a rail with rails on 3 or more sides, each side's rail at its level, one
/// up or one down: vanilla's neighbour search), any T among a detector's raw connections, and any T
/// face-adjacent to the cell above a sloped switched rail (powered, activator or detector:
/// `BaseRailBlock.affectNeighborsAfterRemoval` updates it).
pub fn rail_rule_lines(spec: &FarmSpec) -> Vec<String> {
    use std::collections::HashMap;
    let rails: HashMap<BlockPos, (redsim::block::RailShape, &Block)> = spec.blocks.iter().filter_map(|(p, b)| rail_shape(b).map(|s| (*p, (s, b)))).collect();
    let rail_near = |c: BlockPos| [c, BlockPos::new(c.x, c.y + 1, c.z), BlockPos::new(c.x, c.y - 1, c.z)].into_iter().find(|q| rails.contains_key(q));
    let sides = |p: BlockPos| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| rail_near(BlockPos::new(p.x + dx, p.y, p.z + dz)).is_some()).count();
    let mut out = Vec::new();
    let mut ts: Vec<BlockPos> = rails.keys().copied().filter(|&p| sides(p) >= 3).collect();
    ts.sort_by_key(|p| (p.y, p.x, p.z));
    out.push(format!("Ts (a rail with rails on 3+ sides): {}", if ts.is_empty() { "none".to_string() } else { ts.iter().map(|p| format!("({},{},{})", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ") }));
    let mut dets: Vec<(BlockPos, redsim::block::RailShape)> = rails.iter().filter(|(_, (_, b))| matches!(b, Block::DetectorRail { .. })).map(|(p, (s, _))| (*p, *s)).collect();
    dets.sort_by_key(|(p, _)| (p.y, p.x, p.z));
    let at: std::collections::HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    for (p, s) in dets {
        let conns: Vec<String> = raw_connections(p, s).into_iter().map(|c| match rail_near(c) {
            Some(q) => format!("({},{},{}) {} sides", q.x, q.y, q.z, sides(q)),
            None => format!("({},{},{}) no rail", c.x, c.y, c.z),
        }).collect();
        let bad = raw_connections(p, s).into_iter().filter_map(rail_near).any(|q| sides(q) >= 3);
        out.push(format!("detector ({},{},{}): raw connections {}{}", p.x, p.y, p.z, conns.join(", "), if bad { " — A T: needs an official probe" } else { "; none with 3" }));
        // (G1's full reach set: the 6 face neighbours of the detector and of the cell below it, each
        // cell's block and, for a rail, its sides; no plain rail at 3)
        let below = BlockPos::new(p.x, p.y - 1, p.z);
        for base in [p, below] {
            let cells: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
                .iter()
                .map(|(dx, dy, dz)| BlockPos::new(base.x + dx, base.y + dy, base.z + dz))
                .map(|c| {
                    let name = at.get(&c).map_or("air".to_string(), |b| format!("{b:?}"));
                    let n = if rails.contains_key(&c) { format!(", {} sides{}", sides(c), if sides(c) >= 3 && matches!(at.get(&c), Some(Block::PlainRail { .. })) { " — A PLAIN RAIL AT 3" } else { "" }) } else { String::new() };
                    format!("({},{},{}) {name}{n}", c.x, c.y, c.z)
                })
                .collect();
            out.push(format!("  reach set around ({},{},{}): {}", base.x, base.y, base.z, cells.join("; ")));
        }
    }
    let mut sloped: Vec<BlockPos> = rails.iter().filter(|(_, (s, b))| matches!(b, Block::Rail { .. } | Block::DetectorRail { .. }) && matches!(s, redsim::block::RailShape::AscendingEast | redsim::block::RailShape::AscendingWest | redsim::block::RailShape::AscendingNorth | redsim::block::RailShape::AscendingSouth)).map(|(p, _)| *p).collect();
    sloped.sort_by_key(|p| (p.y, p.x, p.z));
    for p in sloped {
        let q = BlockPos::new(p.x, p.y + 1, p.z);
        let near: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().map(|(dx, dy, dz)| BlockPos::new(q.x + dx, q.y + dy, q.z + dz)).filter(|n| rails.contains_key(n) && sides(*n) >= 3).map(|n| format!("({},{},{})", n.x, n.y, n.z)).collect();
        out.push(format!("sloped switched rail ({},{},{}): Ts face-adjacent to the cell above it: {}", p.x, p.y, p.z, if near.is_empty() { "none".to_string() } else { near.join(" ") }));
    }
    out
}

fn list<T: std::str::FromStr>(s: Option<&str>) -> Result<Vec<T>>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    Ok(match s {
        Some(s) => s.split(',').map(|x| x.trim().parse()).collect::<std::result::Result<_, _>>()?,
        None => Vec::new(),
    })
}

/// `mctas mobfarm bedtier-bench`: one row (each visit printed) or a sweep (each row's clean count)
/// with the totals. `sweeps`: the restart phases (each at every order of `orders`), the visit day
/// times.
#[allow(clippy::too_many_arguments)]
pub fn bench_cmd(spec: &FarmSpec, from: u64, to: u64, speeds: &str, after_sleep: i64, release_after: i64, day: bool, base: &Faults, sweeps: [Option<&str>; 2], orders: &[crate::liveness::Restart]) -> Result<()> {
    let sp: Vec<f64> = list(Some(speeds))?;
    let (rp, hs): (Vec<i64>, Vec<i64>) = (list(sweeps[0])?, list(sweeps[1])?);
    let mut rows: Vec<(String, Faults)> = Vec::new();
    for &p in &rp {
        for &how in orders {
            rows.push((format!("restart +{p} {}", how.label()), Faults { restart: Some((p, how)), ..base.clone() }));
        }
    }
    if rows.is_empty() {
        rows.push(("night".to_string(), base.clone()));
    }
    if !hs.is_empty() {
        rows = rows.into_iter().flat_map(|(n, f)| hs.iter().map(move |&h| (format!("{n}, hour {h}"), Faults { hour: Some(h), ..f.clone() }))).collect();
    }
    let one = rows.len() == 1;
    // (each seed and speed's night exit, for each row's exit delay)
    let mut night_r = std::collections::HashMap::new();
    if !one {
        for seed in from..=to {
            for &v in &sp {
                if let Ok(r) = bench(spec, seed, v, release_after, day, after_sleep, &Faults::default())
                    && let Some(rr) = r.r_at
                {
                    night_r.insert((seed, v.to_bits()), rr);
                }
            }
        }
    }
    let (mut tot, mut clean, mut seated, mut not_out, mut lost) = (0, 0, 0, 0, 0);
    // (after a restart, the woken napper's paths off the pickup cell: visits with each direction)
    let mut dirs: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    // (the lie-downs' reach to the bed head's centre: the most, against SleepInBed's 2)
    let mut lie_max = 0f64;
    let mut lie_min = f64::MAX;
    let (mut min_pick, mut max_pick) = (f64::MAX, 0f64);
    let mut delays: Vec<i64> = Vec::new();
    for (name, f) in &rows {
        let (mut ok, mut bad) = (0, Vec::new());
        let (mut napped, mut seated_here, mut lie_downs) = (0, 0, Vec::<i64>::new());
        for seed in from..=to {
            for &v in &sp {
                tot += 1;
                match bench(spec, seed, v, release_after, day, after_sleep, f) {
                    Ok(r) => {
                        seated += r.seated_out as usize;
                        seated_here += r.seated_out as usize;
                        not_out += r.out_at.is_none() as usize;
                        lost += r.home_lost_at.is_some() as usize;
                        for d in &r.paths_after_restart {
                            *dirs.entry(d).or_default() += 1;
                        }
                        for &d in &r.lie_dists {
                            lie_max = lie_max.max(d);
                            lie_min = lie_min.min(d);
                        }
                        if let Some(s) = r.pickup_speed {
                            min_pick = min_pick.min(s);
                            max_pick = max_pick.max(s);
                        }
                        if let Some(s) = r.slept_at {
                            napped += 1;
                            if let Some(h) = f.hour {
                                lie_downs.push(h + s);
                            }
                        }
                        if let (Some(rr), Some(nr)) = (r.r_at, night_r.get(&(seed, v.to_bits()))) {
                            delays.push(rr - nr);
                        }
                        if one {
                            println!("seed {seed} speed {v}: landed {:?}, parked {:?}, slept {:?}, woke {:?}, release {:?}, pickup {:?} at {:?} b/t, R {:?}, out {:?}, seated {}{}", r.landed, r.parked.map(|(p, t)| (format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2]), t)), r.slept_at, r.woke_at, r.release_at, r.pickup, r.pickup_speed.map(|s| format!("{s:.3}")), r.r_at, r.out_at, r.seated_out, if r.problems.is_empty() { String::new() } else { format!(" PROBLEMS {:?}", r.problems) });
                        }
                        if r.problems.is_empty() {
                            ok += 1;
                        } else {
                            bad.push(format!("seed {seed} speed {v}: {:?}", r.problems));
                        }
                    }
                    Err(e) => bad.push(format!("seed {seed} speed {v}: ERROR {e}")),
                }
            }
        }
        clean += ok;
        if !one && !bad.is_empty() {
            println!("{name}: {ok} of {} clean; {}", ok + bad.len(), bad.iter().take(3).cloned().collect::<Vec<_>>().join("; "));
        }
        if f.hour.is_some() {
            lie_downs.sort();
            println!("{name}: napped {napped}, seated out {seated_here}; lie-downs at day time {:?}..{:?} (REST ends at 24010)", lie_downs.first(), lie_downs.last());
        }
    }
    delays.sort();
    let pct = |p: f64| delays.get(((delays.len() as f64 - 1.0) * p).round() as usize).copied();
    println!("bed stop bench: {tot} visits ({} rows), {clean} clean, seated {seated}; not out {not_out}; HOME lost {lost}; pickup speeds {:.3}..{:.3}; exit later than the night's (gt): min {:?}, median {:?}, p99 {:?}, max {:?}; after a restart, visits with a path off the pickup cell by direction {dirs:?}; lie-downs' reach to the bed head's centre {:.3}..{:.3} (SleepInBed's bar 2)", rows.len(), if min_pick == f64::MAX { 0.0 } else { min_pick }, max_pick, delays.first(), pct(0.5), pct(0.99), delays.last(), if lie_min == f64::MAX { 0.0 } else { lie_min }, lie_max);
    Ok(())
}

/// `mctas mobfarm bedtier-two`: the two-visit rows (a row without a restart, one per restart phase
/// and order; or the daylight rows, one per held napper's queue arrival x DAY-off time x C's
/// queue arrival) with the totals.
#[allow(clippy::too_many_arguments)]
pub fn two_cmd(spec: &FarmSpec, from: u64, to: u64, speeds: &str, restart_phases: Option<&str>, orders: &[crate::liveness::Restart], transit: i64, lift_delay: i64, lift_speed: f64, day_hold: Option<&str>, day_offs: &str, gap_c: i64, c_at: Option<i64>) -> Result<()> {
    let sp: Vec<f64> = list(Some(speeds))?;
    let base = TwoOpts { restart: None, transit, lift_delay, lift_speed, day_hold: None, c_at: None };
    let mut rows: Vec<(String, TwoOpts)> = Vec::new();
    if let Some(dh) = day_hold {
        for off in list::<i64>(Some(day_offs))? {
            for h in list::<i64>(Some(dh))? {
                rows.push((format!("held from day time {h}, DAY off {off}, C {gap_c} gt after the drop"), TwoOpts { day_hold: Some((h, off, gap_c)), ..base }));
                if let Some(at) = c_at {
                    rows.push((format!("held from day time {h}, DAY off {off}, C at day time {at}"), TwoOpts { day_hold: Some((h, off, gap_c)), c_at: Some(at), ..base }));
                }
            }
        }
    } else {
        rows.push(("no restart".to_string(), base));
        for p in list::<i64>(restart_phases)? {
            for &how in orders {
                rows.push((format!("restart +{p} {}", how.label()), TwoOpts { restart: Some((p, how)), ..base }));
            }
        }
    }
    let (mut n, mut clean, mut lost, mut close) = (0, 0, 0, 0);
    let mut least: Option<f64> = None;
    let mut gap: Vec<i64> = Vec::new();
    let mut c_waits: Vec<i64> = Vec::new();
    let (mut lie_min, mut lie_max) = (f64::MAX, 0f64);
    for (name, o) in &rows {
        let (mut ok, mut bad) = (0, Vec::new());
        // (the daylight rows: per row, A's lie-down and R as day times, C's wait at the queue, C's
        // arrival after A's drop (never two at the queue))
        let (mut a_sleeps, mut a_rs, mut waits, mut after_drop) = (Vec::<i64>::new(), Vec::<i64>::new(), Vec::<i64>::new(), Vec::<i64>::new());
        for seed in from..=to {
            for &v in &sp {
                n += 1;
                match two_visit_bench(spec, seed, v, o) {
                    Ok(r) => {
                        if let Some(d) = r.a_dist_at_c_sleep {
                            least = Some(least.map_or(d, |x: f64| x.min(d)));
                            close += (d <= 17.0) as usize;
                        }
                        if let (Some(ar), Some(cs)) = (r.a_r, r.c_slept) {
                            gap.push(cs - ar);
                        }
                        if let (Some(q), Some(e)) = (r.c_queued, r.c_entry) {
                            c_waits.push(e - q - o.transit);
                            waits.push(e - q - o.transit);
                        }
                        if let (Some(q), Some(d)) = (r.c_queued, r.a_drop) {
                            after_drop.push(q - d);
                        }
                        if let Some(s) = r.a_slept {
                            a_sleeps.push((r.start_day + s).rem_euclid(24000));
                        }
                        if let Some(x) = r.a_r {
                            a_rs.push((r.start_day + x).rem_euclid(24000));
                        }
                        lost += (!r.home_lost.is_empty() || !r.homes_end.0 || !r.homes_end.1) as usize;
                        for &d in &r.lie_dists {
                            lie_min = lie_min.min(d);
                            lie_max = lie_max.max(d);
                        }
                        if r.problems.is_empty() {
                            ok += 1;
                        } else {
                            bad.push(format!("seed {seed} speed {v}: {:?} (A's drop {:?}, A at the entry {:?}, A's R {:?}, C queued {:?}, C at the entry {:?}, C lay down {:?}, A at {:?})", r.problems, r.a_drop, r.a_entry, r.a_r, r.c_queued, r.c_entry, r.c_slept, r.a_dist_at_c_sleep.map(|d| format!("{d:.2}"))));
                        }
                    }
                    Err(e) => bad.push(format!("seed {seed} speed {v}: ERROR {e}")),
                }
            }
        }
        clean += ok;
        if !bad.is_empty() {
            println!("{name}: {ok} of {} clean; {}", ok + bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
        }
        if o.day_hold.is_some() {
            let span = |v: &mut Vec<i64>| {
                v.sort();
                format!("{:?}..{:?}", v.first(), v.last())
            };
            println!("{name}: {ok} clean; A lay down at day time {}, A's R at day time {}, C's wait at the queue {} gt, C queued {} gt after A's drop", span(&mut a_sleeps), span(&mut a_rs), span(&mut waits), span(&mut after_drop));
        }
    }
    gap.sort();
    c_waits.sort();
    println!("two visits (queue released on R, C at the entry {transit} gt later, A's lift from R + {lift_delay} at {lift_speed} b/t): {n} rows, {clean} clean; A within 17 at C's lie-down {close}; HOME lost or missing at the end {lost}; A's least distance at C's lie-down {:?}; A's R to C's lie-down min {:?} median {:?} max {:?} gt; lie-downs' reach to the bed head's centre {:.3}..{:.3} (SleepInBed's bar 2)", least.map(|d| format!("{d:.2}")), gap.first(), gap.get(gap.len() / 2), gap.last(), if lie_min == f64::MAX { 0.0 } else { lie_min }, lie_max);
    if day_hold.is_some() {
        println!("daylight rows (the NOT DAY gate opens at day times {day_offs}): C's wait at the queue for A's R min {:?} median {:?} max {:?} gt", c_waits.first(), c_waits.get(c_waits.len() / 2), c_waits.last());
    }
    Ok(())
}
