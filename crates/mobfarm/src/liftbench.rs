//! The vertical lift bench (7b90c3f1, farmopt Nov 11 20:58: vjeux's idea of a bubble-column lift in
//! place of the carousel's diagonal climb back), built alone: a soul-sand bubble column `height`
//! water cells tall at x 0 (z 0), glass-walled; an entry flat of rails from the east at its foot (the
//! shaft's east wall open `entry_h` cells from the rail up); at its top the exit per `exit`:
//! - `none`: the shaft's top water under air (the column's launch), nothing beside it;
//! - `rail`: a flat rail west of the shaft's top cell, one up (x -1, y height), on glass, with the
//!   shaft's west wall stopping under it.
//! Carts with villager riders are launched west along the entry at `speed`, `spacing` gt apart.
//! Printed per cart: the tick it first meets the column, the top reached, where it ends, the rider's
//! least air, seated to the end or not, the rider's damage, eye-in-block ticks; with `trace` a line
//! a tick for one cart.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct LiftOpts {
    pub height: i32,
    pub entry_len: i32,
    pub entry_h: i32,
    pub speed: f64,
    pub carts: usize,
    pub spacing: i64,
    pub ticks: i64,
    pub exit: String,
    pub trace: Option<usize>,
    /// A glass block this many cells over the shaft's top water (0: none).
    pub ceiling: i32,
    /// Cells descending west right after the exit's landing cell (0: a flat exit line).
    pub exit_slope: i32,
    /// Q2's merge: three entry lanes (A the entry line, B and C joining it through passive T
    /// junctions), carts launched on A, B, C in turn.
    pub merge3: bool,
    /// Q2's split: J1/J2 on the exit line after the slope, armed and disarmed by rise-keyed
    /// detector latches, routing carts A/B/C back to lanes z 0 / 2 / -2 in merge order.
    pub split3: bool,
    /// The shaft's four wall cells as unconnected fence posts (sight-passing walls).
    pub pane_walls: bool,
    /// The rider-height mark: the rider's feet this far over the entry's (default 11.95).
    pub reach_dy: f64,
    /// A detector rail on the entry line at this x (the napper-only leg's evidence detector).
    pub detector_at: Option<i32>,
    /// (c)'s napper: one more cart launched on the entry line (lane A) at this live tick, after
    /// the batches (the y 0 hold's release into a batch's gap).
    pub napper_at: Option<i64>,
    /// (a‴), rejected (the rider's eye crosses the soul sand on the way up; its head sits in the
    /// stopper's cell while held): a soul sand block in the shaft at this height (the napper held
    /// under it, the column above standing on it).
    pub stopper: Option<i32>,
    /// L1: launches on game ticks through a freeze (default L2: on live time).
    pub game_ticks: bool,
    /// The bench frozen (its chunks at level 32: blocks tick, entities don't) from tick FROM for LEN
    /// gt; the carts' launches wait it out.
    pub freeze: Option<(i64, i64)>,
}

fn build(o: &LiftOpts) -> Result<FarmSpec, String> {
    let glass = Block::parse("minecraft:glass")?;
    let water = Block::parse("minecraft:water[level=0]")?;
    let soul = Block::parse("minecraft:soul_sand")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let h = o.height;
    let mut blocks: Vec<(BlockPos, Block)> = vec![(BlockPos::new(0, -1, 0), soul)];
    for y in 0..h {
        blocks.push((BlockPos::new(0, y, 0), water.clone()));
    }
    let exit_rail = o.exit == "rail" || o.exit == "current" || o.exit == "piston";
    // (`pane_walls`: the shaft's four wall cells unconnected oak fence posts (redsim models no glass pane; a post: sight passes
    // either side of it; waterloggable, so flowing water can't enter), the corners left open)
    let wall = if o.pane_walls { Block::parse("minecraft:oak_fence[east=false,north=false,south=false,west=false,waterlogged=false]")? } else { glass.clone() };
    for y in -1..=h + 1 {
        for (x, z) in [(0, -1), (0, 1)] {
            blocks.push((BlockPos::new(x, y, z), if y >= 0 && y < h { wall.clone() } else { glass.clone() }));
        }
        // (the west wall stops under the exit rail's cell)
        if !(exit_rail && y >= h) {
            // (the far wall stays solid at the entry's two cells: an entering cart stops against
            // its face, centred in the shaft (against a thin post it would roll past the cell's
            // edge and catch the glass at the top))
            blocks.push((BlockPos::new(-1, y, 0), if y >= 2 && y < h - 1 { wall.clone() } else { glass.clone() }));
        }
        if y >= o.entry_h || y < 0 {
            blocks.push((BlockPos::new(1, y, 0), if y >= 0 && y < h - 1 { wall.clone() } else { glass.clone() }));
        }
    }
    for x in 1..=o.entry_len {
        // (`detector_at`: the napper-only leg's evidence detector, its rise the queue's release)
        let r = if o.detector_at == Some(x) { Block::parse("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]")? } else { flat.clone() };
        blocks.push((BlockPos::new(x, 0, 0), r));
        blocks.push((BlockPos::new(x, -1, 0), glass.clone()));
    }
    if o.merge3 {
        // (Q2's merge, farmopt 21:43: lane A the entry line itself; lane B along z 2 joining it at
        // x jB = entry_len - 6 through a passive T (the junction a south_west curve: the branch from
        // the south into the downstream west), lane C along z -2 joining at jC = entry_len - 2
        // through a north_west T; A, B and C launched at each lane's east end; C passes jB's T
        // from its unconnected (east) side)
        let ns = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]")?;
        let (jb, jc) = (o.entry_len - 6, o.entry_len - 2);
        for (j, side, junction, corner) in [(jb, 1, "south_west", "north_east"), (jc, -1, "north_west", "south_east")] {
            blocks.push((BlockPos::new(j, 0, 0), Block::parse(&format!("minecraft:rail[shape={junction},waterlogged=false]"))?));
            blocks.push((BlockPos::new(j, 0, side), ns.clone()));
            blocks.push((BlockPos::new(j, -1, side), glass.clone()));
            blocks.push((BlockPos::new(j, 0, 2 * side), Block::parse(&format!("minecraft:rail[shape={corner},waterlogged=false]"))?));
            blocks.push((BlockPos::new(j, -1, 2 * side), glass.clone()));
            for x in j + 1..=o.entry_len {
                blocks.push((BlockPos::new(x, 0, 2 * side), flat.clone()));
                blocks.push((BlockPos::new(x, -1, 2 * side), glass.clone()));
            }
        }
    }
    if exit_rail || o.exit == "current" || o.exit == "piston" {
        // (the exit line west of the landing cell: flat, or `exit_slope` cells descending west
        // right after it (each cart pulling away from the next to land), then flats to a stop)
        let up_east = Block::parse("minecraft:rail[shape=ascending_east,waterlogged=false]")?;
        let mut y = h;
        let mut x = -1;
        blocks.push((BlockPos::new(x, y, 0), flat.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
        for _ in 0..o.exit_slope {
            x -= 1;
            y -= 1;
            blocks.push((BlockPos::new(x, y, 0), up_east.clone()));
            blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
            blocks.push((BlockPos::new(x + 1, y, 0), glass.clone()));
        }
        for _ in 0..(14 - o.exit_slope).max(7) {
            x -= 1;
            blocks.push((BlockPos::new(x, y, 0), flat.clone()));
            blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
        }
        // (the exit line's end: a glass stop, the carts queueing against it)
        blocks.push((BlockPos::new(x - 1, y, 0), glass.clone()));
        if o.split3 {
            // (Q2's split, ironspec 22:03: two powered junctions on the exit line after the slope,
            // J1 (straight or south_east: to lane B along z 2) and J2 (straight or north_east: to
            // lane C along z -2), lane A straight on z 0; the detectors two cells off every
            // junction (tick-exact: no signal source beside a T): D1a on the main line between J1
            // and J2, Db on lane B, Dc on lane C)
            let ye = h - o.exit_slope;
            let x0 = -1 - o.exit_slope;
            let (j1, j2) = (x0 - 1, x0 - 5);
            let ns = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]")?;
            let det = Block::parse("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
            blocks.retain(|(p, _)| !(p.y == ye && p.z == 0 && (p.x == x0 - 3 || p.x == j2 - 2)));
            blocks.push((BlockPos::new(x0 - 3, ye, 0), det.clone()));
            // (Da2: lane A two cells past J2, crossed only by the batch's first cart)
            blocks.push((BlockPos::new(j2 - 2, ye, 0), det.clone()));
            for (j, side, corner) in [(j1, 1, "north_west"), (j2, -1, "south_west")] {
                blocks.push((BlockPos::new(j, ye, side), ns.clone()));
                blocks.push((BlockPos::new(j, ye - 1, side), glass.clone()));
                blocks.push((BlockPos::new(j, ye, 2 * side), Block::parse(&format!("minecraft:rail[shape={corner},waterlogged=false]"))?));
                blocks.push((BlockPos::new(j, ye - 1, 2 * side), glass.clone()));
                blocks.push((BlockPos::new(j - 1, ye, 2 * side), det.clone()));
                blocks.push((BlockPos::new(j - 1, ye - 1, 2 * side), glass.clone()));
                for xx in (x0 - 14)..(j - 1) {
                    blocks.push((BlockPos::new(xx, ye, 2 * side), flat.clone()));
                    blocks.push((BlockPos::new(xx, ye - 1, 2 * side), glass.clone()));
                }
                blocks.push((BlockPos::new(x0 - 15, ye, 2 * side), glass.clone()));
            }
        }
    }
    if o.exit == "current" {
        // (a water source east of the shaft's top air cell, glass-boxed: its flow into (0,h,0)
        // points west, away from it; the exit rail at (-1,h,0) is waterloggable, so the flow stops
        // there)
        blocks.retain(|(p, _)| !(p.x == 1 && p.y == h && p.z == 0));
        blocks.push((BlockPos::new(1, h, 0), water.clone()));
        for (x, y, z) in [(2, h, 0), (1, h + 1, 0), (1, h, -1), (1, h, 1), (1, h - 1, 0)] {
            blocks.push((BlockPos::new(x, y, z), glass.clone()));
        }
    }
    // (`stopper`: (a‴)'s soul sand block in the shaft; the water under it rises from the bottom
    // soul sand, the water over it from the stopper)
    if let Some(sy) = o.stopper {
        blocks.retain(|(p, _)| !(p.x == 0 && p.y == sy && p.z == 0));
        blocks.push((BlockPos::new(0, sy, 0), soul.clone()));
    }
    // (`ceiling`: a glass block K cells over the shaft's top water, capping the column's launch)
    if o.ceiling > 0 {
        blocks.push((BlockPos::new(0, h - 1 + o.ceiling, 0), glass.clone()));
    }
    if o.exit == "piston" {
        // (a piston east of the shaft's top air cell facing west, the bench powering it (a
        // redstone block behind it) when the cart's bottom is at the exit rail's level: its head
        // pushes the cart one block west over the exit rail)
        blocks.retain(|(p, _)| !(p.x == 1 && p.y == h && p.z == 0));
        blocks.push((BlockPos::new(1, h, 0), Block::parse("minecraft:piston[extended=false,facing=west]")?));
        blocks.push((BlockPos::new(2, h, 0), glass.clone()));
    }
    Ok(FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(o.entry_len + 3, 0, 3),
        lo: BlockPos::new(-40, -4, -4),
        hi: BlockPos::new(o.entry_len + 4, h + 8, 4),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

/// One cart's state for the pair tests.
#[derive(Clone, Copy, Debug)]
pub struct CartState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    /// The cart's yaw (degrees; the minecart's heading vector is (cos, sin) of it).
    pub yaw: f32,
}

impl CartState {
    pub fn of(e: &redsim::entity::Entity) -> CartState {
        CartState { x: e.pos.x, y: e.pos.y, z: e.pos.z, vx: e.delta_movement.x, vy: e.delta_movement.y, vz: e.delta_movement.z, yaw: e.y_rot }
    }
}

/// The game's cart-cart contact (`AbstractMinecart`: the carts meeting its box inflated 0.2
/// sideways; 0.98 x 0.7 x 0.98 boxes, so |dx|, |dz| < 1.18 and |dy| < 0.7): `None` apart, else
/// whether the push acts (`push`: the centre line within |cos| >= 0.8 of either cart's yaw vector
/// (cos yaw, sin yaw)) and the closing speed (the relative velocity's component along the centre
/// line, positive closing).
pub fn touch(a: &CartState, b: &CartState) -> Option<(bool, f64)> {
    let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
    if dx.abs() >= 1.18 || dz.abs() >= 1.18 || dy.abs() >= 0.7 {
        return None;
    }
    let h = (dx * dx + dz * dz).sqrt();
    let push = h >= 1.0e-2
        && [a.yaw, b.yaw].iter().any(|y| {
            let r = (*y as f64).to_radians();
            ((dx * r.cos() + dz * r.sin()) / h).abs() >= 0.8
        });
    let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);
    let closing = -((b.vx - a.vx) * dx + (b.vy - a.vy) * dy + (b.vz - a.vz) * dz) / d;
    Some((push, closing))
}

/// The pair tests' tally: every touch (pair-ticks), the hardest closing, and the subset where the
/// push acts.
#[derive(Default)]
pub struct Touches {
    pub n: usize,
    pub hardest: Option<(f64, i64, usize, usize, f64, f64, f64)>,
    pub pushes: usize,
    pub hardest_push: Option<(f64, i64, usize, usize, f64, f64, f64)>,
}

impl Touches {
    pub fn note(&mut self, t: i64, a: usize, b: usize, s: &CartState, push: bool, closing: f64) {
        let row = (closing, t, a, b, s.x, s.y, s.z);
        self.n += 1;
        if self.hardest.is_none_or(|h| closing > h.0) {
            self.hardest = Some(row);
        }
        if push {
            self.pushes += 1;
            if self.hardest_push.is_none_or(|h| closing > h.0) {
                self.hardest_push = Some(row);
            }
        }
    }
    pub fn line(&self) -> String {
        let f = |h: Option<(f64, i64, usize, usize, f64, f64, f64)>| h.map_or(String::new(), |(c, t, a, b, x, y, z)| format!(", the hardest closing {c:.3} (t {t}, carts {a} and {b} at {x:.2},{y:.2},{z:.2})"));
        format!("box touches (the game's push test: inflated 0.2 sideways, |dy| < 0.7) {} pair-ticks{}; where a push acts (|cos| >= 0.8 against either yaw) {}{}", self.n, f(self.hardest), self.pushes, f(self.hardest_push))
    }
}

struct CartLog {
    id: Option<u32>,
    rider: Option<u32>,
    met_column: Option<i64>,
    top_y: f64,
    top_at: Option<i64>,
    least_air: i32,
    dismounted: Option<i64>,
    hurt: f32,
    eye_in_block: u32,
    /// `--stopper`: the cart's top and the rider's eye at the last tick.
    held: Option<(f64, f64)>,
    /// `--split3`: the speed entering its lane's first branch cell (ironfarm's intake: <= 0.30).
    branch_v: Option<f64>,
    /// Taken by the sink at the exit lines' far end (live gt), as a lane's intake takes it.
    sunk: Option<i64>,
    end: Option<(f64, f64, f64, f64, f64)>,
    launch: Option<i64>,
    /// The rider's feet 11.95 above the entry rail's (the summoners' level), live gt from launch.
    reach: Option<i64>,
    /// The entry detector's rise under this cart, live gt from launch.
    det_rise: Option<i64>,
    /// Landed on the exit rail (x < 0, on the rail's level): live gt from launch, the speed then.
    exit: Option<(i64, f64)>,
    launch_live: i64,
}

/// One bench run's report lines.
pub fn lift_bench(o: &LiftOpts) -> Result<String, String> {
    // (`--napper-at`: the napper follows the batches; G1: enforce it)
    if let Some(na) = o.napper_at {
        let last = (0..o.carts as i64).map(|k| if o.merge3 { (k / 3) * 100 + (k % 3) * o.spacing } else { k * o.spacing }).max().unwrap_or(0);
        if na <= last {
            return Err(format!("--napper-at {na} must come after the batches' last release (t {last})"));
        }
    }
    let spec = build(o)?;
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, (o.ticks + 200) as u32, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let mut out: Vec<String> = Vec::new();
    // (the shaft and the entry as settled: water spilled out of the entry opening or not)
    let spill: Vec<String> = (1..=o.entry_len.min(3))
        .flat_map(|x| (0..o.entry_h.max(1) + 1).map(move |y| (x, y)))
        .filter(|(x, y)| matches!(run.world.get(BlockPos::new(*x, *y, 0)), Block::Water { .. }))
        .map(|(x, y)| format!("({x},{y})"))
        .collect();
    let column: usize = (0..o.height).filter(|y| matches!(run.world.get(BlockPos::new(0, *y, 0)), Block::BubbleColumn { drag: false })).count();
    out.push(format!(
        "lift bench: a {}-cell column (bubble column cells after settling: {column}), the entry {} rails long open {} cells, exit {}, {} cart(s) {} gt apart at {:.2}; water outside the shaft at the entry: {}",
        o.height,
        o.entry_len,
        o.entry_h,
        o.exit,
        o.carts,
        o.spacing,
        o.speed,
        if spill.is_empty() { "none".to_string() } else { spill.join(" ") }
    ));
    let n_carts = o.carts + usize::from(o.napper_at.is_some());
    let mut logs: Vec<CartLog> = (0..n_carts).map(|_| CartLog { id: None, rider: None, met_column: None, top_y: f64::NEG_INFINITY, top_at: None, least_air: i32::MAX, dismounted: None, hurt: 0.0, eye_in_block: 0, held: None, branch_v: None, sunk: None, end: None, launch: None, reach: None, det_rise: None, exit: None, launch_live: 0 }).collect();
    let mut det_was = false;
    let mut health0: Vec<Option<f32>> = vec![None; n_carts];
    let mut column_gap = f64::INFINITY;
    let mut touches = Touches::default();
    let mut piston_fired: Vec<(i64, usize, f64, f64)> = Vec::new();
    let mut piston_until: Option<i64> = None;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let glass_b = Block::parse("minecraft:glass")?;
    let mut frozen_ticks = 0i64;
    // (`split3`: the latches (L1: J1 to lane B; L2: J2 to lane C), the detectors' last states, the
    // junction cells; each junction set to its latch's shape, the redstone the bench stands in for)
    let ye = o.height - o.exit_slope;
    let x0 = -1 - o.exit_slope;
    let (j1, j2) = (x0 - 1, x0 - 5);
    let (d1a, db, dc) = (BlockPos::new(x0 - 3, ye, 0), BlockPos::new(j1 - 1, ye, 2), BlockPos::new(j2 - 1, ye, -2));
    let da2 = BlockPos::new(j2 - 2, ye, 0);
    let (mut l1, mut l2) = (false, false);
    let mut was = [false; 4];
    let mut split_log: Vec<String> = Vec::new();
    let shape = |s: &str| Block::parse(&format!("minecraft:rail[shape={s},waterlogged=false]"));
    if o.split3 {
        run.world.set_block(BlockPos::new(j1, ye, 0), shape("east_west")?, redsim::world::flags::ALL);
        run.world.set_block(BlockPos::new(j2, ye, 0), shape("east_west")?, redsim::world::flags::ALL);
    }
    for t in 0..o.ticks {
        // (the freeze: the bench's chunks (x -1..0, z -1..0) at level 32, the spare chunk west of
        // them still forced)
        let frozen_now = o.freeze.is_some_and(|(f, l)| t >= f && t < f + l);
        if let Some((from, len)) = o.freeze {
            if t == from {
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, [-2, -1, 0, 0]);
            }
            if t == from + len {
                redsim::reload::load_some(&mut run.world, [-2, -1, 0, 0]);
            }
        }
        if frozen_now {
            frozen_ticks += 1;
        }
        let live = t - frozen_ticks;
        // (`split3`: rises arm and disarm the latches: D1a (crossed by the batch's first and third
        // carts) arms L1 only with both latches off; Db (lane B's cart) clears L1; Da2 (lane A past
        // J2: the first cart only, so J2 turns only once it is clear) arms L2; Dc clears L2)
        if o.split3 {
            let on = |p: BlockPos| matches!(run.world.get(p), Block::DetectorRail { powered: true, .. });
            let now = [on(d1a), on(db), on(dc), on(da2)];
            let (p1, p2) = (l1, l2);
            if now[0] && !was[0] && !l1 && !l2 {
                l1 = true;
            }
            if now[1] && !was[1] {
                l1 = false;
            }
            if now[3] && !was[3] {
                l2 = true;
            }
            if now[2] && !was[2] {
                l2 = false;
            }
            was = now;
            if l1 != p1 {
                run.world.set_block(BlockPos::new(j1, ye, 0), shape(if l1 { "south_east" } else { "east_west" })?, redsim::world::flags::ALL);
                if split_log.len() < 30 {
                    split_log.push(format!("t {t} J1 {}", if l1 { "to B" } else { "straight" }));
                }
            }
            if l2 != p2 {
                run.world.set_block(BlockPos::new(j2, ye, 0), shape(if l2 { "north_east" } else { "east_west" })?, redsim::world::flags::ALL);
                if split_log.len() < 30 {
                    split_log.push(format!("t {t} J2 {}", if l2 { "to C" } else { "straight" }));
                }
            }
        }
        // (`piston`: the bench powers the piston while a cart's bottom is at the exit rail's level
        // in the shaft's column, then unpowers it 4 gt later)
        if o.exit == "piston" {
            if let Some(u) = piston_until
                && t >= u
            {
                run.world.set_block(BlockPos::new(2, o.height, 0), glass_b.clone(), redsim::world::flags::ALL);
                piston_until = None;
            }
            if piston_until.is_none() && piston_fired.last().is_none_or(|f| t - f.0 > 10) {
                for (k, lg) in logs.iter().enumerate() {
                    let Some(id) = lg.id else { continue };
                    let Some(i) = redsim::entity::index_of(&run.world, id) else { continue };
                    let e = &run.world.entities.list[i];
                    let h = o.height as f64;
                    if e.pos.x > -0.1 && e.pos.x < 1.0 && e.pos.y >= h && e.pos.y < h + 0.6 {
                        piston_fired.push((t, k, e.pos.y, e.delta_movement.y));
                        run.world.set_block(BlockPos::new(2, o.height, 0), rblock.clone(), redsim::world::flags::ALL);
                        piston_until = Some(t + 4);
                        break;
                    }
                }
            }
        }
        for (k, lg) in logs.iter_mut().enumerate() {
            // (with --merge3 the carts come in batches of three, one per lane, a batch every 100 gt:
            // cart k at (k / 3) x 100 + (k % 3) x spacing; --napper-at: the last cart, on lane A)
            let napper = o.napper_at.is_some() && k == o.carts;
            let at = if napper { o.napper_at.unwrap_or(0) } else if o.merge3 { (k as i64 / 3) * 100 + (k as i64 % 3) * o.spacing } else { k as i64 * o.spacing };
            // (L2: launches wait out a freeze (live time); `game_ticks` (L1, the lead's rule): the
            // upstream releases run on game ticks through it, a cart released into frozen chunks
            // waiting there frozen)
            let when = if o.game_ticks { t } else { live };
            if lg.id.is_none() && (o.game_ticks || !frozen_now) && when >= at {
                let nbt = format!("{{Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", -o.speed);
                // (--merge3: cart k on lane A, B, C in turn: z 0, 2, -2)
                let lane_z = if o.merge3 && !napper { [0.5, 2.5, -1.5][k % 3] } else { 0.5 };
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [o.entry_len as f64 - 0.5, 0.0625, lane_z], Some(&nbt))?;
                lg.id = Some(id);
                lg.launch = Some(t);
                lg.launch_live = live;
                lg.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        run.tick();
        // (`detector_at`: a rise belongs to the cart nearest the detector's cell)
        if let Some(dx) = o.detector_at {
            let on = matches!(run.world.get(BlockPos::new(dx, 0, 0)), Block::DetectorRail { powered: true, .. });
            if on && !det_was {
                let mut best: Option<(f64, usize)> = None;
                for (k, lg) in logs.iter().enumerate() {
                    let Some(id) = lg.id else { continue };
                    let Some(i) = redsim::entity::index_of(&run.world, id) else { continue };
                    let d = (run.world.entities.list[i].pos.x - (dx as f64 + 0.5)).abs();
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, k));
                    }
                }
                if let Some((_, k)) = best
                    && logs[k].det_rise.is_none()
                {
                    logs[k].det_rise = Some(live - logs[k].launch_live);
                }
            }
            det_was = on;
        }
        // (cart pairs: the closest centres in the shaft's column (a rising cart under a bobbing
        // one), and the game's cart-cart contact (G1, lift3 review): `AbstractMinecart` takes the
        // carts meeting its box inflated 0.2 sideways (0.98 x 0.7 x 0.98 boxes: |dx|, |dz| < 1.18,
        // |dy| < 0.7); the push acts only where the centre line is within |cos| >= 0.8 of either
        // cart's yaw)
        let st: Vec<Option<CartState>> = logs.iter().map(|lg| lg.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| CartState::of(&run.world.entities.list[i]))).collect();
        for a in 0..st.len() {
            for b in a + 1..st.len() {
                if let (Some(pa), Some(pb)) = (st[a], st[b]) {
                    if pa.x > -0.1 && pa.x < 1.0 && pb.x > -0.1 && pb.x < 1.0 {
                        let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt();
                        column_gap = column_gap.min(d);
                    }
                    if let Some((push, closing)) = touch(&pa, &pb) {
                        touches.note(t, a, b, &pa, push, closing);
                    }
                }
            }
        }
        let mut sink_now: Vec<u32> = Vec::new();
        for (k, lg) in logs.iter_mut().enumerate() {
            let Some(id) = lg.id else { continue };
            let Some(i) = redsim::entity::index_of(&run.world, id) else { continue };
            let e = &run.world.entities.list[i];
            let (p, v) = (e.pos, e.delta_movement);
            let here = run.world.get(e.block_position());
            if lg.met_column.is_none() && matches!(here, Block::BubbleColumn { .. } | Block::Water { .. }) {
                lg.met_column = Some(t);
            }
            if p.y > lg.top_y {
                lg.top_y = p.y;
                lg.top_at = Some(t);
            }
            let seated = lg.rider.is_some_and(|r| e.passengers.contains(&r));
            if !seated && lg.dismounted.is_none() && lg.rider.is_some() {
                lg.dismounted = Some(t);
            }
            lg.end = Some((p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt(), v.y));
            // (`split3`: the speed entering the lane's first branch cell: lane B's (j1, ye, 1), lane
            // C's (j2, ye, -1), lane A's the main line's cell past J2)
            if o.split3 && lg.branch_v.is_none() {
                let bp = e.block_position();
                if (bp.y == ye) && ((bp.x == j1 && bp.z == 1) || (bp.x == j2 && bp.z == -1) || (bp.x == j2 - 1 && bp.z == 0)) {
                    lg.branch_v = Some((v.x * v.x + v.z * v.z).sqrt());
                }
            }
            // (the sink: a cart past the exit lines' tenth cell west of the slope leaves the world
            // with its rider, as a lane's intake takes it (no end-stop queue contacts))
            if lg.sunk.is_none() && p.x < (-1 - o.exit_slope - 10) as f64 && p.y > (o.height - o.exit_slope) as f64 - 0.5 {
                lg.sunk = Some(live);
                sink_now.push(id);
            }
            if lg.exit.is_none() && p.x < 0.0 && (p.y - (o.height as f64 + 0.0625)).abs() < 0.01 {
                lg.exit = Some((live - lg.launch_live, (v.x * v.x + v.z * v.z).sqrt()));
            }
            let mut air = None;
            if let Some(r) = lg.rider
                && let Some(j) = redsim::entity::index_of(&run.world, r)
            {
                let re = &run.world.entities.list[j];
                if lg.reach.is_none() && re.pos.y >= 0.0625 + 0.4 + o.reach_dy {
                    lg.reach = Some(live - lg.launch_live);
                }
                if let EntityKind::Mob(m) = &re.kind {
                    air = Some(m.air);
                    lg.least_air = lg.least_air.min(m.air);
                    let hp = m.health;
                    let h0 = *health0[k].get_or_insert(hp);
                    lg.hurt = lg.hurt.max(h0 - hp);
                }
                // (the game's `isInWall`: the rider's eye box (its width x 0.8 square, 1e-6 tall at
                // the eye, 1.62 over its feet) meeting a suffocating block's collision shape;
                // tick-exact: glass never suffocates; soul sand does, its shape 14/16 tall)
                let (ex, ey, ez) = (re.pos.x, re.pos.y + 1.62, re.pos.z);
                let mut in_wall = false;
                for cx in (ex - 0.24).floor() as i32..=(ex + 0.24).floor() as i32 {
                    for cz in (ez - 0.24).floor() as i32..=(ez + 0.24).floor() as i32 {
                        let cp = redsim::pos::BlockPos::new(cx, ey.floor() as i32, cz);
                        let eb = run.world.get(cp);
                        let name = eb.to_string();
                        let boxes = redsim::behavior::collision_boxes(&eb);
                        let suffocating = name.contains("soul_sand") || (boxes.len() == 1 && boxes[0] == [0, 16, 0, 16, 0, 16] && !name.contains("glass"));
                        if !suffocating {
                            continue;
                        }
                        for b in &boxes {
                            let (x0, x1) = (cx as f64 + b[0] as f64 / 16.0, cx as f64 + b[1] as f64 / 16.0);
                            let (y0, y1) = (cp.y as f64 + b[2] as f64 / 16.0, cp.y as f64 + b[3] as f64 / 16.0);
                            let (z0, z1) = (cz as f64 + b[4] as f64 / 16.0, cz as f64 + b[5] as f64 / 16.0);
                            if ex + 0.24 > x0 && ex - 0.24 < x1 && ey > y0 && ey < y1 && ez + 0.24 > z0 && ez - 0.24 < z1 {
                                in_wall = true;
                            }
                        }
                    }
                }
                if in_wall {
                    lg.eye_in_block += 1;
                }
                if o.stopper.is_some() {
                    lg.held = Some((p.y + 0.7, ey));
                }
            }
            if o.trace == Some(k) {
                out.push(format!("  t {t}: cart {k} at {:.3},{:.3},{:.3} v {:.3},{:.3},{:.3} in {} air {} seated {seated}", p.x, p.y, p.z, v.x, v.y, v.z, here.to_string().trim_start_matches("minecraft:"), air.map_or("-".into(), |a| a.to_string())));
            }
        }
        // (the sink takes its carts and their riders)
        for id in sink_now.drain(..) {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == id || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
    }
    for (k, lg) in logs.iter().enumerate() {
        out.push(format!(
            "cart {k}: met the column at {}; top y {:.3} at {}; on the exit rail {}; the rider's feet {:.2} over the entry's {}{}; ends at {}; the rider's least air {}; {}; hurt {:.1}; eye-in-block ticks {}",
            lg.met_column.map_or("never".into(), |t| format!("t {t}")),
            lg.top_y,
            lg.top_at.map_or("-".into(), |t| format!("t {t}")),
            lg.exit.map_or("NEVER".into(), |(d, s)| format!("{d} live gt after launch at {s:.3}")),
            o.reach_dy,
            lg.reach.map_or("never".into(), |d| format!("{d} live gt after launch")),
            match (o.detector_at, lg.det_rise, lg.reach) {
                (Some(_), Some(d), Some(r)) => format!(" (the entry detector's rise {d} live gt after launch: {} gt from the rise to the mark)", r - d),
                (Some(_), None, _) => " (the entry detector never rose under it)".to_string(),
                _ => String::new(),
            },
            lg.end.map_or("-".into(), |(x, y, z, s, vy)| format!("{x:.3},{y:.3},{z:.3} (horizontal {s:.3}, vertical {vy:.3})")),
            if lg.least_air == i32::MAX { "-".into() } else { lg.least_air.to_string() },
            lg.dismounted.map_or("seated to the end".into(), |t| format!("DISMOUNTED at t {t}")),
            lg.hurt,
            lg.eye_in_block
        ));
        if let Some((top, eye)) = lg.held
            && let Some(sy) = o.stopper
        {
            out.push(format!("  cart {k} under the stopper ((a‴), rejected) at y {sy}: its top at {top:.3} (the stopper's bottom {sy}), the rider's eye at {eye:.3} (the soul sand's shape {sy}..{:.3})", sy as f64 + 0.875));
        }
    }
    // (the primitive's cost: the shaft and its walls 3 x 3 from the soul sand's support to the
    // ceiling; the entry's rail cell in the east wall; the exit's water source and its glass box
    // one cell east at the top; the exit rail in the west wall's line)
    let top = o.height - 1 + o.ceiling.max(1);
    out.push(format!(
        "the primitive: footprint 3 x 3 (x -1..1, z -1..1) over y -1..{top} ({} cells tall: soul sand, {} water, {} over the top), plus the exit's source box at x 2 on the top layer ({}); entry and exit rails in the walls' lines; rise {} blocks from the entry rail to the exit rail",
        top + 2,
        o.height,
        o.ceiling.max(1),
        if o.exit == "current" { "1 cell" } else { "none" },
        o.height
    ));
    if o.split3 {
        // (each cart's lane at the end against its merge lane (cart k from lane A, B, C in turn))
        let lanes: Vec<String> = logs
            .iter()
            .enumerate()
            .map(|(k, lg)| {
                let got = lg.end.map(|(_, _, z, _, _)| if z > 1.5 { "B" } else if z < -0.5 { "C" } else { "A" }).unwrap_or("-");
                let want = ["A", "B", "C"][k % 3];
                format!("cart {k} {got}{}", if got == want { "" } else { " (WRONG LANE)" })
            })
            .collect();
        out.push(format!("split: {}; the switches: {}", lanes.join(", "), split_log.join(", ")));
        let bv: Vec<f64> = logs.iter().filter_map(|lg| lg.branch_v).collect();
        out.push(format!(
            "branch speeds (entering each lane's first branch cell; ironfarm's intake bar <= 0.30): {}; the fastest {}",
            logs.iter().enumerate().map(|(k, lg)| format!("cart {k} {}", lg.branch_v.map_or("-".into(), |v| format!("{v:.3}")))).collect::<Vec<_>>().join(", "),
            bv.iter().cloned().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))).map_or("-".into(), |v| format!("{v:.3}"))
        ));
    }
    out.push(format!("pairs: the closest centres in the shaft's column {}; {}", if column_gap.is_finite() { format!("{column_gap:.3}") } else { "-".into() }, touches.line()));
    if o.exit == "piston" {
        out.push(format!("piston firings (tick, cart, its y, its vertical speed): {}", piston_fired.iter().map(|(t, k, y, vy)| format!("t {t} cart {k} y {y:.3} vy {vy:.3}")).collect::<Vec<_>>().join("; ")));
    }
    Ok(out.join("\n"))
}
