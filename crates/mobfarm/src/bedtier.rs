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

/// The capped flats between the park's slope and the pickup cell (the released cart's speed falls
/// under 0.2 b/t on them, empty-cart drag 0.96 a tick).
pub const FLATS: i32 = 7;
/// The pickup cell's z.
pub const ZP: i32 = 3 + FLATS;
/// The entry rail (the cart comes in moving +x), a booster.
pub const ENTRY: [i32; 3] = [-2, 2, 0];
/// The activator, always on (its redstone block under it).
pub const ACTIVATOR: [i32; 3] = [-1, 2, 0];
/// The park: one braked powered rail on the slope down south; its support is the release's cell
/// (the bench sets a redstone block there).
pub const PARK: [[i32; 3]; 1] = [[1, 1, 1]];
pub const PARK_CTRL: [[i32; 3]; 1] = [[1, 0, 1]];
/// The one cell a woken napper can stand on: the pickup cell.
pub const SWEPT: [[i32; 3]; 1] = [[1, 0, ZP]];
pub const BED_HEAD: [i32; 3] = [0, 0, ZP];
/// The hatch: two trapdoors in the zombie box's one opening (bottom half (-1,0,ZP), top half
/// (-1,1,ZP)); open (their controls powered) their plates cut the eye lines; shut they pass.
pub const HATCH: [i32; 3] = [-1, 0, ZP];
pub const HATCH_HI: [i32; 3] = [-1, 1, ZP];
pub const HATCH_CTRL: [i32; 3] = [-1, 0, ZP - 1];
pub const HATCH_CTRL_HI: [i32; 3] = [-1, 2, ZP];
/// The exit's R detector.
pub const EXIT_DET: [i32; 3] = [1, -1, ZP + 3];

/// The design: the bed tier alone, or (`bench`) with a run-out past the exit and the visiting
/// cart and villager parked far off on a braked rail.
pub fn design(bench: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-min-bench" } else { "bed-tier-min" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    // the line: the entry a booster (always on: a slow arrival still reaches the park), the
    // activator (always on), a plain rail, the curve (top-half trapdoors over all four: a rider
    // passes, nobody stands there), the park (one braked slope rail: every arrival stops on it),
    // a plain slope, FLATS capped flats (the released cart, boosted over the rest of the park's
    // cell only, slows on them under 0.2 b/t: a cart faster than that stops dead against a standing
    // villager instead of seating it), the pickup cell, two capped flats, the exit (R's detector
    // on its slope)
    put(&mut b, ENTRY, "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-2, 1, 0], "redstone_block")?;
    put(&mut b, ACTIVATOR, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, 1, 0], "redstone_block")?;
    put(&mut b, [0, 2, 0], &rail("east_west"))?;
    put(&mut b, [0, 1, 0], "glass")?;
    put(&mut b, [1, 2, 0], &rail("south_west"))?;
    put(&mut b, [1, 1, 0], "glass")?;
    for x in -2..=1 {
        put(&mut b, [x, 3, 0], cap)?;
    }
    put(&mut b, PARK[0], "powered_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
    put(&mut b, PARK_CTRL[0], "stone")?;
    put(&mut b, [1, 0, 2], &rail("ascending_north"))?;
    put(&mut b, [1, -1, 2], "glass")?;
    // (glass two cells over the park, out of the rider space and above the cart: no villager
    // stands on the parked cart's top)
    put(&mut b, [1, 3, 1], "glass")?;
    for z in 3..=ZP + 2 {
        put(&mut b, [1, 0, z], &rail("north_south"))?;
        put(&mut b, [1, -1, z], "glass")?;
        if z != ZP {
            put(&mut b, [1, 1, z], cap)?;
        }
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
    put(&mut b, [1, -2, EXIT_DET[2]], "stone")?;
    // the bed, its head beside the pickup cell, roofed by two top-half trapdoors (0.25 of headroom
    // over the bed's top: nobody stands on it; a standing napper's eye line runs under them)
    put(&mut b, BED_HEAD, "red_bed[facing=north,occupied=false,part=head]")?;
    put(&mut b, [0, 0, ZP + 1], "red_bed[facing=north,occupied=false,part=foot]")?;
    put(&mut b, [0, -1, ZP], "glass")?;
    put(&mut b, [0, -1, ZP + 1], "glass")?;
    put(&mut b, [0, 1, ZP], cap)?;
    put(&mut b, [0, 1, ZP + 1], cap)?;
    // the ledge: L (-1,2,1) south of the activator (its north side glass), south along x -1 to
    // z ZP-1, east onto (0,2,ZP-1), south onto the bed's roof (0,2,ZP); the pit's drop east of
    // that; glass between the ledge and the track's column (the parked cart's top and the caps
    // lead nowhere), west and south
    put(&mut b, [-1, 2, -1], "glass")?;
    put(&mut b, [-1, 3, -1], "glass")?;
    for z in 1..=ZP - 1 {
        put(&mut b, [-1, 1, z], "stone")?;
        for y in 2..=3 {
            put(&mut b, [-2, y, z], "glass")?;
        }
    }
    for z in 1..=ZP - 2 {
        for y in 2..=3 {
            put(&mut b, [0, y, z], "glass")?;
        }
    }
    for z in 3..=ZP - 1 {
        put(&mut b, [1, 2, z], "glass")?;
    }
    put(&mut b, [1, 2, ZP + 1], "glass")?;
    for y in 2..=3 {
        put(&mut b, [0, y, ZP + 1], "glass")?;
    }
    // the pit: glass east of the track (x 2, y 0..1), west of the flats north of the bed, south
    // of its foot
    for z in 1..=ZP + 2 {
        for y in 0..=1 {
            put(&mut b, [2, y, z], "glass")?;
        }
    }
    put(&mut b, [2, 2, 1], "glass")?;
    for z in 1..=ZP - 1 {
        put(&mut b, [0, 0, z], "glass")?;
        if !b.d.blocks.contains_key(&[0, 1, z]) {
            put(&mut b, [0, 1, z], "glass")?;
        }
    }
    put(&mut b, [0, 0, ZP + 2], "glass")?;
    put(&mut b, [0, 1, ZP + 2], "glass")?;
    // the zombie box west of the bed, its one opening two trapdoors at (-1,0,ZP) (bottom half) and
    // (-1,1,ZP) (top half), facing west: open (powered by the redstone blocks (-1,0,ZP-1) and
    // (-1,2,ZP)) their plates stand across the cells' east sides and cut every line; shut (F: both
    // to stone) the sleeper's eye line and a standing napper's (under the bed's roof) pass. The
    // zombie (persistent) in a boat at (-4.5, 0, ZP+0.5), glass all round, leaves over it
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, "redstone_block")?;
    put(&mut b, HATCH_CTRL_HI, "redstone_block")?;
    put(&mut b, [-1, -1, ZP], "glass")?;
    put(&mut b, [-1, 0, ZP + 1], "glass")?;
    put(&mut b, [-1, 1, ZP + 1], "glass")?;
    for y in 2..=3 {
        put(&mut b, [-1, y, ZP + 1], "glass")?;
    }
    for x in -5..=-2 {
        put(&mut b, [x, -1, ZP], "glass")?;
        put(&mut b, [x, 2, ZP], "glass")?;
        for y in 0..=1 {
            put(&mut b, [x, y, ZP - 1], "glass")?;
            put(&mut b, [x, y, ZP + 1], "glass")?;
        }
    }
    for y in 0..=1 {
        put(&mut b, [-6, y, ZP], "glass")?;
    }
    put(&mut b, [-5, 3, ZP], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", [-4.5, 0.0, ZP as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    if bench {
        // the run-out past the exit and a stop
        for z in EXIT_DET[2] + 1..=EXIT_DET[2] + 6 {
            put(&mut b, [1, -1, z], &rail("north_south"))?;
            put(&mut b, [1, -2, z], "glass")?;
        }
        put(&mut b, [1, -1, EXIT_DET[2] + 7], "glass")?;
        // (the visitor is spawned by the bench at the entry at the visit's hour, HOME = the bed:
        // parked from the start it would run REST unable to reach its bed and drop HOME after
        // SetWalkTargetFromBlockMemory's 1200 unreachable ticks)
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    Ok(b.text(&[
        "The one-chunk top tier's bed stop, minimal (paper v2): a booster entry, an always-on activator, a curve, one braked park rail on a slope, a slope, capped flats, the pickup cell beside the bed, two capped flats, the exit.",
        "The rider lands on the ledge, walks off into the pit and sleeps; woken through the hatch it stands only on the pickup cell and keeps seeing the zombie there; the released cart reaches it under 0.2 b/t and seats it.",
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
    /// The cart's horizontal speed in the tick it seated the napper.
    pub pickup_speed: Option<f64>,
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
pub fn bench(spec: &FarmSpec, seed: u64, speed: f64, f_at_in: i64, release_after: i64, day: bool, on_stand: bool, after_sleep: Option<i64>) -> Result<Row, String> {
    // (`after_sleep`: F comes that many gt after the lie-down (the bed's own evidence), or at
    // `f_at_in` at the latest (the give-up))
    let mut f_at = f_at_in;
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
        if let Some(d) = after_sleep
            && let Some(s) = row.slept_at
            && s + d < f_at
            && t <= s + d
        {
            f_at = s + d;
        }
        if t == f_at {
            set(&mut run.world, HATCH_CTRL, "stone");
            set(&mut run.world, HATCH_CTRL_HI, "stone");
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
        if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1") && (t % 5 == 0 || std::env::var("MCTAS_BT_TRACE_FROM").ok().and_then(|s| s.parse::<i64>().ok()).is_some_and(|f| t >= f && t <= f + 15)) {
            use redsim::entity::mob::ai::brain;
            let b = brain::brain(w, vi);
            let act = brain::has_exact(w, vi).then(|| {
                let x = brain::ex(w, vi);
                format!("{:?} cant_reach {:?}", x.active, x.mem.cant_reach_since)
            });
            eprintln!("t {t}: villager {:.2} {:.2} {:.2} riding {riding} asleep {asleep} act {act:?} home {:?} hostile {:?} | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", v.pos.x, v.pos.y, v.pos.z, b.home, b.nearest_hostile, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
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
            row.pickup_speed = before.map(|bp| ((c.pos.x - bp.x).powi(2) + (c.pos.z - bp.z).powi(2)).sqrt());
        }
        if row.release_at.is_none() && row.landed.is_some() && riding && t > 5 {
            row.problems.push(format!("t {t}: the napper back in a cart before the release (at {vcell:?})"));
            break;
        }
        if c.pos.z > EXIT_DET[2] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        if c.pos.z > EXIT_DET[2] as f64 + 2.0 {
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
