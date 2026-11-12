//! (ironspec 17:00) The one-chunk top tier's bed stop, the minimal through-line (paper v2,
//! https://artifacts.internalmeta.com/a/2132081997402168): an entry from the switch at the top of
//! the departure climb, an always-on activator that drops every arrival, a curve, two braked
//! powered rails sloping down south (the arrival stops on them), the pickup cell beside the bed's
//! head, a flat beside its foot, a flat under a top-half trapdoor, the exit (R's detector on its
//! slope). The rider lands on a ledge, walks off it into the pit and lies down; woken through the
//! hatch at F, it can stand only on the track cells the released cart sweeps.
//!
//! The unit benches, before any wiring (the bench drives F, the hatch and the release): where the
//! eject lands, the napper's path into the bed, the stand-up cell after the wake, the braked stop,
//! the release's speed over the swept cells and the pickup.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("bed tier: {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The entry rail (the cart comes in moving +x).
pub const ENTRY: [i32; 3] = [-2, 2, 0];
/// The activator, always on (its redstone block under it).
pub const ACTIVATOR: [i32; 3] = [-1, 2, 0];
/// The park: two braked powered rails on the slope down south; their supports are the release's
/// cells (the bench sets redstone blocks there).
pub const PARK: [[i32; 3]; 2] = [[1, 1, 1], [1, 0, 2]];
pub const PARK_CTRL: [[i32; 3]; 2] = [[1, 0, 1], [1, -1, 2]];
/// The cells a woken napper can stand on: the lower slope, the pickup cell, the flat.
pub const SWEPT: [[i32; 3]; 4] = [[1, 0, 2], [1, 1, 2], [1, 0, 3], [1, 0, 4]];
/// The ledge the eject lands on, and its continuation over the bed's roof.
pub const LEDGE: [[i32; 3]; 5] = [[-1, 2, 1], [-1, 2, 2], [-1, 2, 3], [0, 2, 3], [0, 2, 4]];
pub const BED_HEAD: [i32; 3] = [0, 0, 3];
/// The hatch (a top-half iron trapdoor in the zombie box's one open cell: closed, the sleeper's
/// eye line passes under it; open (its control powered), its plate cuts the line) and its control.
pub const HATCH: [i32; 3] = [-1, 0, 3];
pub const HATCH_CTRL: [i32; 3] = [-1, 0, 2];
/// The exit's R detector.
pub const EXIT_DET: [i32; 3] = [1, -1, 6];

/// The design: the bed tier alone, or (`bench`) with a run-out past the exit and the visiting
/// cart and villager parked far off on a braked rail.
pub fn design(bench: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-min-bench" } else { "bed-tier-min" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    // the line: entry, the activator (always on), a plain rail, the curve (top-half trapdoors over
    // all four: a rider passes, nobody stands there), the park, the swept cells, the cap, the exit
    put(&mut b, ENTRY, &rail("east_west"))?;
    put(&mut b, [-2, 1, 0], "glass")?;
    put(&mut b, ACTIVATOR, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, 1, 0], "redstone_block")?;
    put(&mut b, [0, 2, 0], &rail("east_west"))?;
    put(&mut b, [0, 1, 0], "glass")?;
    put(&mut b, [1, 2, 0], &rail("south_west"))?;
    put(&mut b, [1, 1, 0], "glass")?;
    for x in -2..=1 {
        put(&mut b, [x, 3, 0], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
    }
    for (p, c) in PARK.iter().zip(PARK_CTRL.iter()) {
        put(&mut b, *p, "powered_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
        put(&mut b, *c, "stone")?;
    }
    // (glass two cells over the upper park slope, out of the rider space and above the cart: no
    // villager stands on the parked cart's top)
    put(&mut b, [1, 3, 1], "glass")?;
    for z in 3..=5 {
        put(&mut b, [1, 0, z], &rail("north_south"))?;
        put(&mut b, [1, -1, z], "glass")?;
    }
    put(&mut b, [1, 1, 5], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
    put(&mut b, [1, -2, 6], "stone")?;
    // the bed, its head beside the pickup cell, roofed (glass at y 1: 0.44 of headroom)
    put(&mut b, BED_HEAD, "red_bed[facing=north,occupied=false,part=head]")?;
    put(&mut b, [0, 0, 4], "red_bed[facing=north,occupied=false,part=foot]")?;
    put(&mut b, [0, -1, 3], "glass")?;
    put(&mut b, [0, -1, 4], "glass")?;
    put(&mut b, [0, 1, 3], "glass")?;
    put(&mut b, [0, 1, 4], "glass")?;
    // the ledge: L (-1,2,1) south of the activator (its north side glass), on to (-1,2,3), east
    // over the bed's roof (0,2,3), (0,2,4); the pit's drop east of those; glass between the ledge
    // and the park's column at z 1, 2 (the parked cart's top leads nowhere), west and south
    put(&mut b, [-1, 2, -1], "glass")?;
    put(&mut b, [-1, 3, -1], "glass")?;
    for z in 1..=3 {
        put(&mut b, [-1, 1, z], "stone")?;
        for y in 2..=3 {
            put(&mut b, [-2, y, z], "glass")?;
        }
    }
    for y in 2..=3 {
        put(&mut b, [0, y, 1], "glass")?;
        put(&mut b, [0, y, 2], "glass")?;
        put(&mut b, [-1, y, 4], "glass")?;
        put(&mut b, [0, y, 5], "glass")?;
    }
    // the pit: glass east of the track (x 2, y 0..1, z 1..5), north of the bed's head, south of
    // its foot
    for z in 1..=5 {
        for y in 0..=1 {
            put(&mut b, [2, y, z], "glass")?;
        }
    }
    put(&mut b, [2, 2, 1], "glass")?;
    put(&mut b, [0, 0, 2], "glass")?;
    put(&mut b, [0, 1, 2], "glass")?;
    put(&mut b, [0, 0, 5], "glass")?;
    put(&mut b, [0, 1, 5], "glass")?;
    // the zombie box west of the bed, one level down so the sleeper's eye line (y about 0.7) runs
    // level through one cell: the hatch (-1,0,3), a bottom-half iron trapdoor facing west; open
    // (its control (-1,0,2) a redstone block) its plate stands across the cell's east side and
    // cuts the line, shut (F: the control to stone) the line passes over it. The zombie in a boat
    // at (-4.5, -1, 3.5); glass all round; leaves over it
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, "redstone_block")?;
    for c in [[-1, -1, 3], [-1, 1, 3], [-1, 0, 4], [-1, 1, 4], [-1, 1, 2]] {
        if !b.d.blocks.contains_key(&c) {
            put(&mut b, c, "glass")?;
        }
    }
    for x in -5..=-2 {
        put(&mut b, [x, -2, 3], "glass")?;
        put(&mut b, [x, 1, 3], "glass")?;
        for y in -1..=0 {
            put(&mut b, [x, y, 2], "glass")?;
            put(&mut b, [x, y, 4], "glass")?;
        }
    }
    for y in -1..=0 {
        put(&mut b, [-6, y, 3], "glass")?;
    }
    put(&mut b, [-5, 2, 3], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", [-4.5, -1.0, 3.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    if bench {
        // the run-out past the exit and a stop; the visitor parked far off on a braked rail
        for z in 7..=12 {
            put(&mut b, [1, -1, z], &rail("north_south"))?;
            put(&mut b, [1, -2, z], "glass")?;
        }
        put(&mut b, [1, -1, 13], "glass")?;
        put(&mut b, [20, 0, 20], "powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
        put(&mut b, [20, -1, 20], "glass")?;
        // (the visitor is spawned by the bench at the entry at the visit's hour, HOME = the bed:
        // parked from the start it would run REST unable to reach its bed and drop HOME after
        // SetWalkTargetFromBlockMemory's 1200 unreachable ticks)
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    Ok(b.text(&[
        "The one-chunk top tier's bed stop, minimal (paper v2): entry, an always-on activator, a curve, two braked powered rails down a slope, the pickup cell beside the bed, a flat, a capped flat, the exit.",
        "The rider lands on the ledge, walks off into the pit and sleeps; woken through the hatch it stands only on cells the released cart sweeps.",
    ]))
}

/// One visit's unit-bench row.
#[derive(Debug, Default, Clone)]
pub struct Row {
    pub seed: u64,
    pub speed: f64,
    /// The rider's first standing cell after the eject (gt after the entry).
    pub landed: Option<([i32; 3], i64)>,
    /// Where the arriving cart stopped (its centre) and when.
    pub parked: Option<([f64; 3], i64)>,
    pub slept_at: Option<i64>,
    pub woke_at: Option<i64>,
    /// The napper's cell on its first awake tick after the nap.
    pub stood: Option<[i32; 3]>,
    /// Every cell the awake napper stood on between the wake and the pickup.
    pub cells_after_wake: Vec<[i32; 3]>,
    /// The release tick, and the cart's horizontal speed as it crossed each swept cell's centre.
    pub release_at: Option<i64>,
    pub swept_speed: Vec<([i32; 3], f64)>,
    /// The pickup: the cell the napper stood on and the tick.
    pub pickup: Option<([i32; 3], i64)>,
    pub out_at: Option<i64>,
    /// The cart over R's detector (the tier clear), gt after the entry.
    pub r_at: Option<i64>,
    pub problems: Vec<String>,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// One visit: the visitor's cart onto the entry at `speed` (+x) at night (or by day with `day`),
/// F at `f_at` gt after the entry (the hatch's control off: the zombie seen), the release
/// `release_after` gt after F (the park's supports to redstone blocks), until the cart passes
/// the run-out's z 8 or 1200 gt.
pub fn bench(spec: &FarmSpec, seed: u64, speed: f64, f_at: i64, release_after: i64, day: bool, on_stand: bool) -> Result<Row, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut row = Row { seed, speed, ..Default::default() };
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    // (wait for the visit's hour: night 13,800 + seed spread, or day 2,000)
    let want = if day { 2000 } else { 13_800 } + (seed as i64 * 37) % 400;
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
    let nbt = format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2]);
    let cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5], Some(&nbt))?;
    let vill_id = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied()).ok_or("no rider")?;
    run.own.push(cart_id);
    run.own.push(vill_id);
    // the entry
    {
        let w = &mut run.world;
        let i = idx(w, cart_id).ok_or("cart gone")?;
        w.entities.list[i].pos = Vec3::new(ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5);
        w.entities.list[i].delta_movement = Vec3::new(speed, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = idx(w, pid) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(st), redsim::world::flags::ALL);
    let mut was_asleep = false;
    let mut last_cart = [0f64; 3];
    let mut still = 0;
    let mut crossed: Vec<[i32; 3]> = Vec::new();
    for t in 1..=1200i64 {
        if t == f_at {
            set(&mut run.world, HATCH_CTRL, "stone");
        }
        let stand_now = on_stand && t > f_at && row.release_at.is_none() && idx(&run.world, vill_id).is_some_and(|vi| { let v = &run.world.entities.list[vi]; let c = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32]; v.vehicle.is_none() && redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_none() && SWEPT.contains(&c) });
        if row.release_at.is_none() && (stand_now || t == f_at + release_after) {
            for c in PARK_CTRL {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        run.tick();
        let w = &run.world;
        let (Some(ci), Some(vi)) = (idx(w, cart_id), idx(w, vill_id)) else {
            row.problems.push(format!("t {t}: the cart or the villager is gone"));
            break;
        };
        let c = &w.entities.list[ci];
        let v = &w.entities.list[vi];
        let vcell = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32];
        let riding = v.vehicle.is_some();
        let asleep = redsim::entity::mob::mob(w, vi).sleeping_pos.is_some();
        if !riding && row.landed.is_none() && v.on_ground {
            row.landed = Some((vcell, t));
        }
        if asleep && row.slept_at.is_none() {
            row.slept_at = Some(t);
        }
        if was_asleep && !asleep {
            row.woke_at = Some(t);
            row.stood = Some(vcell);
        }
        if row.woke_at.is_some() && !riding && row.pickup.is_none() && !row.cells_after_wake.contains(&vcell) {
            row.cells_after_wake.push(vcell);
        }
        was_asleep = asleep;
        if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && t % 5 == 0 {
            use redsim::entity::mob::ai::brain;
            let b = brain::brain(w, vi);
            let act = brain::has_exact(w, vi).then(|| {
                let x = brain::ex(w, vi);
                format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since)
            });
            eprintln!("t {t}: villager {:.2} {:.2} {:.2} riding {riding} asleep {asleep} act {act:?} home {:?} hostile {:?} | cart {:.2} {:.2} {:.2}", v.pos.x, v.pos.y, v.pos.z, b.home, b.nearest_hostile, c.pos.x, c.pos.y, c.pos.z);
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
                    let hv = c.delta_movement.x.hypot(c.delta_movement.z);
                    row.swept_speed.push((s, hv));
                }
            }
        }
        if riding && row.slept_at.is_some() && row.pickup.is_none() {
            row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
        }
        if row.release_at.is_none() && row.landed.is_some() && riding && t > 5 {
            row.problems.push(format!("t {t}: the napper back in a cart before the release (at {vcell:?})"));
            break;
        }
        if c.pos.z > EXIT_DET[2] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        if c.pos.z > 8.0 {
            row.out_at = Some(t);
            break;
        }
    }
    if row.out_at.is_none() {
        row.problems.push("the cart not out in 1200 gt".into());
    }
    if row.pickup.is_none() && row.slept_at.is_some() {
        row.problems.push("the napper not picked up".into());
    }
    for c in &row.cells_after_wake {
        if !SWEPT.contains(c) {
            row.problems.push(format!("the woken napper stood on {c:?}, outside the swept cells"));
        }
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}
