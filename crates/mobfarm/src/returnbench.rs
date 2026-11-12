//! The return's layout bench (7b90c3f1, F1 + Q2 at 24, option (c); 9fc3155f's budget f1b95a5451,
//! loop frame): the merge, the shared lift and the split laid on the budget's cells, batches of three
//! ridden carts from the station pockets' +z exits through to the lanes' branch flats.
//! - merge (y 0): lanes x 7 / 3 / -1 head +z from z 1 and turn west at z 8 (lane 7's curve
//!   (7,0,8)); x 3 and x -1 join through passive Ts at (3,0,8) and (-1,0,8); the main line runs
//!   west to the lift's wall entry (-4,0,8), glass over it;
//! - shared lift: shaft (-5,y,8), soul sand (-5,-1,8), water to y 22, ceiling (-5,24,8), the exit's
//!   current from (-5,23,7) (back wall (-5,23,6)) pushing +z onto the exit curve (-5,23,9)
//!   (north_east) heading east;
//! - split (y 23 along z 9): powered junctions J(-1) at (-1,23,9) and J(3) at (3,23,9) (east_west
//!   straight, north_west to the branch), lane 7's end curve (7,23,9); each branch (x,23,8), a slope
//!   down (x,22,7) (the branch detector, two cells off its T), the branch flat (x,22,6) where the
//!   lane's intake takes the cart (the bench's sink; the speed there against ironfarm's 0.30);
//! - the split's latches in the budget's merge order (the batch's first cart to lane 7 at the end,
//!   the second to lane 3, the third to lane -1): J(3) armed by the first cart's rise at the main
//!   line's detector (5,23,9) (between J(3) and lane 7's curve, two cells off each), cleared by the
//!   second cart's rise on lane 3's branch detector; J(-1) armed by the second rise of a batch at
//!   (1,23,9) (a copper bulb toggled by each rise there, J(-1) armed on its fall: only the first two
//!   carts pass it), cleared by the third cart's rise on lane -1's branch detector.
//! Printed: each cart's lane against its own, the branch speed, the least gap from each junction's
//! turn to the next cart at it, contacts in 3-D, the shaft's closest pair, the riders.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct ReturnOpts {
    /// Batches of three, a batch every 100 gt.
    pub batches: usize,
    /// The gt between a batch's releases (lane 7, lane 3, lane -1).
    pub spacing: i64,
    /// The release speed along +z.
    pub speed: f64,
    pub ticks: i64,
    /// FROM,LEN: the bench at level 32 (freeze32 / thaw32).
    pub login: Option<(i64, i64)>,
    /// A powered rail (on) in place of the split's first flat (-4,23,9).
    pub boost: bool,
    /// One cart's line a tick.
    pub trace: Option<usize>,
    /// The reverse merge order: x -1 first, x 3, x 7 last (the batch's first cart to the split's
    /// first exit).
    pub reverse: bool,
    /// The three releases' offsets in the batch (in merge order); default 0, spacing, 2 spacing.
    pub offsets: Vec<i64>,
}

const LANES: [i32; 3] = [7, 3, -1];

fn build(o: &ReturnOpts) -> Result<FarmSpec, String> {
    let glass = Block::parse("minecraft:glass")?;
    let water = Block::parse("minecraft:water[level=0]")?;
    let rail = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let det = |s: &str| Block::parse(&format!("minecraft:detector_rail[powered=false,shape={s},waterlogged=false]"));
    let mut b: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, k: &Block| b.push((BlockPos::new(x, y, z), k.clone()));
    // (the merge)
    for x in LANES {
        for z in 1..=7 {
            put(x, 0, z, &rail("north_south")?);
            put(x, -1, z, &glass);
        }
    }
    put(7, 0, 8, &rail("north_west")?);
    put(7, -1, 8, &glass);
    for x in -4..=6 {
        let r = if x == 3 || x == -1 { rail("north_west")? } else { rail("east_west")? };
        put(x, 0, 8, &r);
        put(x, -1, 8, &glass);
    }
    put(-4, 1, 8, &glass);
    // (the shared lift)
    put(-5, -1, 8, &Block::parse("minecraft:soul_sand")?);
    put(-5, -2, 8, &glass);
    for y in 0..=22 {
        put(-5, y, 8, &water);
        put(-6, y, 8, &glass);
        put(-5, y, 7, &glass);
        put(-5, y, 9, &glass);
        if y >= 1 {
            put(-4, y, 8, &glass);
        }
    }
    put(-6, 23, 8, &glass);
    put(-4, 23, 8, &glass);
    put(-5, 24, 8, &glass);
    put(-5, 23, 7, &water);
    for (x, y, z) in [(-5, 23, 6), (-6, 23, 7), (-4, 23, 7), (-5, 24, 7)] {
        put(x, y, z, &glass);
    }
    put(-5, 23, 9, &rail("north_east")?);
    // (the split along z 9 at y 23)
    let first = if o.boost { Block::parse("minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")? } else { rail("east_west")? };
    put(-4, 23, 9, &first);
    put(-4, 22, 9, &glass);
    put(-4, 24, 9, &glass);
    for x in -3..=6 {
        let r = if x == 1 || x == 5 { det("east_west")? } else { rail("east_west")? };
        put(x, 23, 9, &r);
        put(x, 22, 9, &glass);
    }
    put(7, 23, 9, &rail("north_west")?);
    put(7, 22, 9, &glass);
    for x in LANES {
        put(x, 23, 8, &rail("north_south")?);
        put(x, 22, 8, &glass);
        put(x, 22, 7, &det("ascending_south")?);
        put(x, 21, 7, &glass);
        put(x, 22, 6, &rail("north_south")?);
        put(x, 21, 6, &glass);
    }
    Ok(FarmSpec {
        blocks: b,
        entities: Vec::new(),
        output: BlockPos::new(10, 0, 0),
        lo: BlockPos::new(-20, -3, -2),
        hi: BlockPos::new(11, 27, 11),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

struct Cart {
    id: Option<u32>,
    rider: Option<u32>,
    launch_live: i64,
    lane: Option<i32>,
    branch_v: Option<f64>,
    branch_t: Option<i64>,
    least_air: i32,
    hurt: f32,
    health0: Option<f32>,
    eye_in_wall: u32,
    dismounted: Option<i64>,
    met_shaft: Option<i64>,
    end: Option<(f64, f64, f64)>,
}

/// The game's `isInWall` for a rider: its eye box (0.48 square at the eye, 1.62 over its feet)
/// meeting a suffocating block's collision shape (soul sand 14/16, full cubes; glass never).
fn in_wall(w: &redsim::world::World, x: f64, feet: f64, z: f64) -> bool {
    let ey = feet + 1.62;
    for cx in (x - 0.24).floor() as i32..=(x + 0.24).floor() as i32 {
        for cz in (z - 0.24).floor() as i32..=(z + 0.24).floor() as i32 {
            let cp = BlockPos::new(cx, ey.floor() as i32, cz);
            let blk = w.get(cp);
            let name = blk.to_string();
            let boxes = redsim::behavior::collision_boxes(&blk);
            let suffocating = name.contains("soul_sand") || (boxes.len() == 1 && boxes[0] == [0, 16, 0, 16, 0, 16] && !name.contains("glass"));
            if suffocating
                && boxes.iter().any(|bx| {
                    x + 0.24 > cx as f64 + bx[0] as f64 / 16.0
                        && x - 0.24 < cx as f64 + bx[1] as f64 / 16.0
                        && ey > cp.y as f64 + bx[2] as f64 / 16.0
                        && ey < cp.y as f64 + bx[3] as f64 / 16.0
                        && z + 0.24 > cz as f64 + bx[4] as f64 / 16.0
                        && z - 0.24 < cz as f64 + bx[5] as f64 / 16.0
                })
            {
                return true;
            }
        }
    }
    false
}

/// One run's report.
pub fn return_bench(o: &ReturnOpts) -> Result<String, String> {
    let spec = build(o)?;
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, (o.ticks + 200) as u32, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let column = (0..=22).filter(|y| matches!(run.world.get(BlockPos::new(-5, *y, 8)), Block::BubbleColumn { drag: false })).count();
    let mut out = vec![format!(
        "return bench (budget f1b95a5451 cells): {} batch(es) of 3, releases {} gt apart at {:.2}, a batch every 100 gt; the shared lift's bubble cells after settling {column} of 23{}",
        o.batches,
        o.spacing,
        o.speed,
        if o.boost { "; a powered rail at (-4,23,9)" } else { "" }
    )];
    let n = o.batches * 3;
    let mut carts: Vec<Cart> = (0..n).map(|_| Cart { id: None, rider: None, launch_live: 0, lane: None, branch_v: None, branch_t: None, least_air: i32::MAX, hurt: 0.0, health0: None, eye_in_wall: 0, dismounted: None, met_shaft: None, end: None }).collect();
    let shape = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    let on = |w: &redsim::world::World, x: i32, y: i32, z: i32| matches!(w.get(BlockPos::new(x, y, z)), Block::DetectorRail { powered: true, .. });
    // (the latches: L3 = J(3) to lane 3, Lm = J(-1) to lane -1, the bulb toggled by (1,23,9))
    let order: [i32; 3] = if o.reverse { [-1, 3, 7] } else { LANES };
    let offsets: Vec<i64> = if o.offsets.len() == 3 { o.offsets.clone() } else { vec![0, o.spacing, 2 * o.spacing] };
    // (the reverse order starts with J(-1) turned for the batch's first cart)
    let (mut l3, mut lm, mut bulb) = (false, o.reverse, false);
    if o.reverse {
        run.world.set_block(BlockPos::new(-1, 23, 9), shape("north_west")?, redsim::world::flags::ALL);
    }
    let mut was = [false; 5];
    let mut turns: Vec<(i64, i32, bool)> = Vec::new();
    let mut at_j: Vec<(i64, i32, usize)> = Vec::new();
    let mut frozen_ticks = 0i64;
    let mut touches = crate::liftbench::Touches::default();
    let mut trace: Vec<String> = Vec::new();
    let mut shaft_gap = f64::INFINITY;
    for t in 0..o.ticks {
        let frozen = o.login.is_some_and(|(f, l)| t >= f && t < f + l);
        if let Some((f, l)) = o.login {
            if t == f {
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, [-1, -1, 0, 0]);
            }
            if t == f + l {
                redsim::reload::load_some(&mut run.world, [-1, -1, 0, 0]);
            }
        }
        if frozen {
            frozen_ticks += 1;
        }
        let live = t - frozen_ticks;
        for (k, c) in carts.iter_mut().enumerate() {
            let at = (k as i64 / 3) * 100 + offsets[k % 3];
            if c.id.is_none() && !frozen && live >= at {
                let x = order[k % 3];
                let nbt = format!("{{Motion:[0.0d,0.0d,{:.4}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", o.speed);
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x as f64 + 0.5, 0.0625, 1.5], Some(&nbt))?;
                c.id = Some(id);
                c.launch_live = live;
                c.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        // (the split's latches on the detectors' rises; the junctions set to them; detectors: [0]
        // (5,23,9), [1] (1,23,9), [2] lane 3's branch, [3] lane -1's, [4] lane 7's)
        let now = [on(&run.world, 5, 23, 9), on(&run.world, 1, 23, 9), on(&run.world, 3, 22, 7), on(&run.world, -1, 22, 7), on(&run.world, 7, 22, 7)];
        let (p3, pm) = (l3, lm);
        let rise = |i: usize| now[i] && !was[i];
        if o.reverse {
            // (the reverse merge order: the batch's first cart to lane -1 (J(-1) turned at idle),
            // the second to lane 3, the third to lane 7 at the end: lane -1's branch rise clears
            // J(-1) and turns J(3); lane 3's clears J(3); lane 7's turns J(-1) for the next batch)
            if rise(3) {
                lm = false;
                l3 = true;
            }
            if rise(2) {
                l3 = false;
            }
            if rise(4) {
                lm = true;
            }
        } else {
            if rise(0) {
                l3 = true;
            }
            if rise(1) {
                let before = bulb;
                bulb = !bulb;
                if before && !bulb {
                    lm = true;
                }
            }
            if rise(2) {
                l3 = false;
            }
            if rise(3) {
                lm = false;
            }
        }
        was = now;
        if l3 != p3 {
            run.world.set_block(BlockPos::new(3, 23, 9), shape(if l3 { "north_west" } else { "east_west" })?, redsim::world::flags::ALL);
            turns.push((t, 3, l3));
        }
        if lm != pm {
            run.world.set_block(BlockPos::new(-1, 23, 9), shape(if lm { "north_west" } else { "east_west" })?, redsim::world::flags::ALL);
            turns.push((t, -1, lm));
        }
        run.tick();
        let mut sink: Vec<u32> = Vec::new();
        let st: Vec<Option<crate::liftbench::CartState>> = carts.iter().map(|c| c.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| crate::liftbench::CartState::of(&run.world.entities.list[i]))).collect();
        for a in 0..n {
            for bq in a + 1..n {
                if let (Some(pa), Some(pb)) = (st[a], st[bq]) {
                    if pa.x < -4.0 && pb.x < -4.0 && pa.z > 7.9 && pa.z < 9.0 && pb.z > 7.9 && pb.z < 9.0 {
                        shaft_gap = shaft_gap.min(((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt());
                    }
                    if let Some((push, closing)) = crate::liftbench::touch(&pa, &pb) {
                        touches.note(t, a, bq, &pa, push, closing);
                    }
                }
            }
        }
        for (k, c) in carts.iter_mut().enumerate() {
            let Some(s) = st[k] else { continue };
            let (x, y, z, vx, vz) = (s.x, s.y, s.z, s.vx, s.vz);
            let id = c.id.unwrap_or(0);
            let bp = BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
            c.end = Some((x, y, z));
            if o.trace == Some(k) {
                trace.push(format!("  t {t}: cart {k} at {x:.3},{y:.3},{z:.3} v {:.3},{:.3},{:.3} on {}", s.vx, s.vy, s.vz, run.world.get(BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32)).to_string().trim_start_matches("minecraft:")));
            }
            if c.met_shaft.is_none() && bp.x == -5 && bp.z == 8 {
                c.met_shaft = Some(live - c.launch_live);
            }
            if bp.y == 23 && bp.z == 9 && (bp.x == 3 || bp.x == -1) && at_j.last().is_none_or(|l| l.2 != k || l.1 != bp.x) {
                at_j.push((t, bp.x, k));
            }
            if bp.y == 22 && bp.z == 6 && LANES.contains(&bp.x) && c.lane.is_none() {
                c.lane = Some(bp.x);
                c.branch_v = Some((vx * vx + vz * vz).sqrt());
                c.branch_t = Some(live - c.launch_live);
                sink.push(id);
            }
            let seated = c.rider.is_some_and(|r| redsim::entity::index_of(&run.world, id).is_some_and(|i| run.world.entities.list[i].passengers.contains(&r)));
            if !seated && c.dismounted.is_none() {
                c.dismounted = Some(t);
            }
            if let Some(r) = c.rider
                && let Some(j) = redsim::entity::index_of(&run.world, r)
            {
                let re = &run.world.entities.list[j];
                if in_wall(&run.world, re.pos.x, re.pos.y, re.pos.z) {
                    c.eye_in_wall += 1;
                }
                if let EntityKind::Mob(m) = &re.kind {
                    c.least_air = c.least_air.min(m.air);
                    let h0 = *c.health0.get_or_insert(m.health);
                    c.hurt = c.hurt.max(h0 - m.health);
                }
            }
        }
        for id in sink {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == id || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
    }
    let mut wrong = 0;
    for (k, c) in carts.iter().enumerate() {
        let want = order[k % 3];
        if c.lane != Some(want) {
            wrong += 1;
        }
        out.push(format!(
            "cart {k} (batch {}, lane {want}): into the shaft {}; {}{}; branch speed {}; air {}; {}; hurt {:.1}; eye-in-wall ticks {}",
            k / 3,
            c.met_shaft.map_or("never".into(), |d| format!("{d} live gt after release")),
            match c.lane {
                Some(l) if l == want => format!("lane {l}"),
                Some(l) => format!("lane {l} (WRONG LANE)"),
                None => format!("NEVER at a lane (ends at {})", c.end.map_or("-".into(), |(x, y, z)| format!("{x:.2},{y:.2},{z:.2}"))),
            },
            c.branch_t.map_or(String::new(), |d| format!(", at its branch flat {d} live gt after release")),
            c.branch_v.map_or("-".into(), |v| format!("{v:.3}")),
            if c.least_air == i32::MAX { "-".into() } else { c.least_air.to_string() },
            if c.lane.is_some() && c.dismounted.is_none() { "seated" } else if c.dismounted.is_some() && c.lane.is_none() { "DISMOUNTED" } else { "seated" },
            c.hurt,
            c.eye_in_wall
        ));
    }
    // (each junction's turns against the next cart at it: the least gap, the latches' margin)
    let mut least: Vec<String> = Vec::new();
    for j in [3, -1] {
        let mut g = i64::MAX;
        for &(tt, jj, _) in turns.iter().filter(|x| x.1 == j) {
            if let Some(&(ta, _, _)) = at_j.iter().find(|a| a.1 == jj && a.0 >= tt) {
                g = g.min(ta - tt);
            }
        }
        least.push(format!("J({j}) {}", if g == i64::MAX { "-".into() } else { format!("{g} gt") }));
    }
    let bv = carts.iter().filter_map(|c| c.branch_v).fold(0.0f64, f64::max);
    out.push(format!(
        "split: {wrong} wrong lane(s) of {n}; the fastest branch speed {bv:.3} (ironfarm's bar 0.30, margin {:.3}); the least gap from a junction's turn to the next cart at it: {}; the shaft's closest pair {}",
        0.30 - bv,
        least.join(", "),
        if shaft_gap.is_finite() { format!("{shaft_gap:.3}") } else { "-".into() }
    ));
    out.push(format!("pairs: {}", touches.line()));
    out.extend(trace);
    Ok(out.join("\n"))
}
