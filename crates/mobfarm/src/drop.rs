//! The vertical drop, M1 of the compact one-bed farm's design space (vjeux 2026-11-11 21:00; the
//! design space doc v0, artifact 1819304709007587): a batch's three carts wait off-rail on closed
//! trapdoors straight above the three stand pockets and fall into them when the trapdoors open (no
//! descent track, no stand gates, no stagger), family-agnostic: the series layout (today's strip,
//! three V pockets along one row, the batch leaving east down it) or Q2's parallel layout (three
//! side-by-side pockets, each the front of its own lane, each leaving east on its own track).
//!
//! Each pocket's cell is a powered rail ascending west on a switched support: unpowered it brakes a
//! cart to rest (`moveAlongTrack`: < 0.03 stops, else x0.5), powered at REL it lets the cart go down
//! the slope. The waiting batch stands on bottom-half trapdoors in hold cells (glass on the cell's
//! four sides at the trapdoor's level: a fixed x, z); the shaft below is clear down to the pocket,
//! whose rider-height cell holds 9fc3155f's occupancy string (tripwire north-south, hooks on either
//! side, stone one cell out). tick-exact (21:07, 26.3 code): a cart falling onto a rail takes no
//! damage from any height (it snaps when a tick starts with its centre over the rail's column and y
//! below the rail + 2; its vertical speed zeroed), off-rail it falls at 0.04 per tick with x0.95
//! drag (0.76 b/t terminal); a cart resting on a closed trapdoor falls straight when it opens; a
//! cart landing on a collision box (a rider's head) hurts its rider.
//!
//! The bench (T: a world time that is a multiple of 100, the summon tick; every timer on the live
//! clock, paused by a freeze): the departing batch at rest in the pockets leaves at REL (+ each
//! pocket's stagger), the pockets re-brake at REL + `rebrake`; trapdoor k opens at T + `release`
//! (gated, with `occupancy`, on its own pocket's string: not while it has been held on for 20 gt or
//! more) and shuts `pulse` gt after (0: stays open). Printed per run: each waiting cart's position
//! and rider's feet at the release, the trapdoor's tick and why it waited, each cart's fall (the
//! trapdoor to its pocket's rail) and its speed, whether its pocket was braked when it landed, its
//! rest tick against the next summon, contacts (centres under 1.25 with the one hit slow), the
//! closest an arriving cart came to a departing one, the riders' damage, each departing cart's exit,
//! and the layout's footprint and height (static).

use crate::layouts::Builder;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// Where the three pockets are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropLayout {
    /// Today's strip (z 0 here): V pockets along x at (13,-2), (9,-1), (5,0), the batch leaving
    /// east down the row to a straight boosted exit.
    Series,
    /// Q2: three side-by-side pockets at x 0, y 0, z 0 / p / 2p, each leaving east on its own
    /// straight boosted track.
    Parallel(i32),
}

impl DropLayout {
    /// Each pocket's rail cell (x, y, z), front first (the series layout's east end first).
    pub fn pockets(&self) -> [(i32, i32, i32); 3] {
        match self {
            DropLayout::Series => [(13, -2, 0), (9, -1, 0), (5, 0, 0)],
            DropLayout::Parallel(p) => [(0, 0, 0), (0, 0, *p), (0, 0, 2 * p)],
        }
    }
    /// The departing carts' sink: x past which a departing cart has left (removed), mid-exit.
    fn sink_x(&self) -> f64 {
        match self {
            DropLayout::Series => 17.5,
            DropLayout::Parallel(_) => 3.5,
        }
    }
}

/// The strip as the S3 file lays it (x, y, state), z 0 here, the gates' blocks left out.
const STRIP: [(i32, i32, &str); 12] = [
    (4, 1, "rail[shape=ascending_west,waterlogged=false]"),
    (5, 0, "rail[shape=ascending_west,waterlogged=false]"),
    (6, -1, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (7, -1, "rail[shape=ascending_east,waterlogged=false]"),
    (8, 0, "rail[shape=east_west,waterlogged=false]"),
    (9, -1, "rail[shape=ascending_west,waterlogged=false]"),
    (10, -2, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (11, -2, "rail[shape=ascending_east,waterlogged=false]"),
    (12, -1, "rail[shape=east_west,waterlogged=false]"),
    (13, -2, "rail[shape=ascending_west,waterlogged=false]"),
    (14, -3, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (15, -3, "rail[shape=east_west,waterlogged=false]"),
];

/// The series layout's gate slopes (the old G0..G2 powered slopes after each pocket) and their
/// sources: switched with their pockets.
const SERIES_SLOPE_SRC: [(i32, i32, i32); 3] = [(14, -4, 0), (10, -3, 0), (6, -2, 0)];

const TRAPDOOR_SHUT: &str = "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]";
const TRAPDOOR_OPEN: &str = "oak_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]";

/// The drop design: the pockets (braked powered slopes on switched stone supports, each with an
/// anchor rail up-west keeping its slope), the departures, the hold cells at `hold_y` (the trapdoor
/// block's y; in the series layout one plane over all three), the occupancy strings, the departing
/// batch at rest mid-cell in the pockets and the waiting batch on the trapdoors.
pub fn design(layout: DropLayout, hold_y: i32, dep: [bool; 3], arr: [bool; 3]) -> String {
    let mut b = Builder::new("drop-bench");
    let pockets = layout.pockets();
    match layout {
        DropLayout::Series => {
            for (x, y, s) in STRIP {
                b.set(x, y, 0, s);
                b.set(x, y - 1, 0, "glass");
            }
            for (x, y, z) in SERIES_SLOPE_SRC {
                b.set(x, y, z, "stone");
            }
            // (the old G0 source sat beside its slope at z +1; here under it: the slope (14,-3)
            // has glass under it from the strip, replaced by the switched stone)
            for x in 16..=19 {
                b.set(x, -3, 0, "powered_rail[powered=true,shape=east_west,waterlogged=false]");
                b.set(x, -4, 0, "redstone_block");
            }
            b.set(20, -3, 0, "rail[shape=east_west,waterlogged=false]");
            b.set(20, -4, 0, "glass");
            b.set(21, -3, 0, "glass");
        }
        DropLayout::Parallel(_) => {
            for (x, y, z) in pockets {
                // the anchor up-west, then east: a plain flat (no power across), four boosters, a
                // plain flat, a stop
                b.set(x - 1, y + 1, z, "rail[shape=ascending_west,waterlogged=false]");
                b.set(x - 1, y, z, "glass");
                b.set(x + 1, y, z, "rail[shape=east_west,waterlogged=false]");
                b.set(x + 1, y - 1, z, "glass");
                for dx in 2..=5 {
                    b.set(x + dx, y, z, "powered_rail[powered=true,shape=east_west,waterlogged=false]");
                    b.set(x + dx, y - 1, z, "redstone_block");
                }
                b.set(x + 6, y, z, "rail[shape=east_west,waterlogged=false]");
                b.set(x + 6, y - 1, z, "glass");
                b.set(x + 7, y, z, "glass");
            }
        }
    }
    for (x, y, z) in pockets {
        b.set(x, y, z, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]");
        b.set(x, y - 1, z, "stone");
        // the hold cell: the trapdoor, glass on its four sides
        b.set(x, hold_y, z, TRAPDOOR_SHUT);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            b.set(x + dx, hold_y, z + dz, "glass");
        }
        // the occupancy string in the pocket's rider-height cell, hooks on either side, stone out
        // (Q2's lanes need a pitch of 4 for them: closer, a lane's hooks and stone sit in the next
        // lane's cells)
        if matches!(layout, DropLayout::Parallel(p) if p < 4) {
            continue;
        }
        b.set(x, y + 1, z, "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]");
        b.set(x, y + 1, z - 1, "tripwire_hook[attached=true,facing=south,powered=false]");
        b.set(x, y + 1, z + 1, "tripwire_hook[attached=true,facing=north,powered=false]");
        b.set(x, y + 1, z - 2, "stone");
        b.set(x, y + 1, z + 2, "stone");
    }
    let rider = "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}";
    let empty = "{Rotation:[0f,0f]}";
    for (k, (x, y, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, *y as f64 + 0.5625, *z as f64 + 0.5], Some(if dep[k] { rider } else { empty }));
    }
    for (k, (x, _, z)) in pockets.iter().enumerate() {
        b.entity("minecart", [*x as f64 + 0.5, hold_y as f64 + 0.1875, *z as f64 + 0.5], Some(if arr[k] { rider } else { empty }));
    }
    b.set(-8, 0, 0, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, 0]);
    b.text(&["The vertical drop (M1): the waiting batch on trapdoors straight above three braked pockets, the batch in them leaving at REL (supports, trapdoors bench-driven on explicit live times)."])
}

/// The layout's footprint and height (static): the bounding box of its blocks from the pockets'
/// supports up to the hold cells' walls and the waiting riders' heads, the drop per pocket (the
/// waiting rider's feet over the pocket's resting rider's feet).
pub fn footprint(layout: DropLayout, hold_y: i32) -> String {
    let pockets = layout.pockets();
    let (x0, x1) = match layout {
        DropLayout::Series => (4, 21),
        DropLayout::Parallel(_) => (-1, 7),
    };
    let zs: Vec<i32> = pockets.iter().map(|p| p.2).collect();
    let (z0, z1) = (zs.iter().min().unwrap() - 2, zs.iter().max().unwrap() + 2);
    let y0 = pockets.iter().map(|p| p.1 - 1).min().unwrap().min(if layout == DropLayout::Series { -4 } else { -1 });
    let y1 = hold_y + 2;
    let feet = hold_y as f64 + 0.1875;
    format!(
        "footprint {} x {} (x {x0}..{x1}, z {z0}..{z1}), height {} (y {y0}..{y1}: the waiting riders' heads at {:.2}); the drop per pocket (the waiting rider's feet {feet:.2} over the resting rider's): {}; the shafts 1 x 1, {} cells clear above each pocket's string",
        x1 - x0 + 1,
        z1 - z0 + 1,
        y1 - y0 + 1,
        feet + 1.95,
        pockets.iter().map(|(_, y, _)| format!("{:.2}", feet - (*y as f64 + 0.5625))).collect::<Vec<_>>().join(", "),
        pockets.iter().map(|(_, y, _)| (hold_y - y - 2).to_string()).collect::<Vec<_>>().join("/")
    )
}

/// One drop run's settings.
#[derive(Clone, Debug)]
pub struct DropOpts {
    pub layout: DropLayout,
    pub hold_y: i32,
    /// REL, gt after T: pocket k powered at REL + stagger[k].
    pub rel: i64,
    pub stagger: [i64; 3],
    /// The pockets re-brake (unpowered) at REL + this.
    pub rebrake: i64,
    /// The trapdoors open at T + this (each gated on its pocket's string with `occupancy`).
    pub release: i64,
    /// The trapdoors shut this long after opening (0: they stay open).
    pub pulse: i64,
    pub occupancy: bool,
    /// The trapdoors' timer on world ticks (a plain redstone clock running through a freeze), not
    /// the live clock: the positive control for the strings.
    pub release_world: bool,
    pub ticks: i64,
    pub seed: i64,
    pub freeze: Option<(i64, i64)>,
    pub trace: bool,
}

/// One drop run's figures for a grid's summary.
#[derive(Clone, Debug, Default)]
pub struct DropStats {
    /// The farthest a waiting cart sat from its cell's centre at the release (hundredths).
    pub spread: i64,
    /// The longest fall (gt; i64::MAX: a cart never landed).
    pub fall: i64,
    /// The least lead of a landing over its pocket's re-brake (gt; negative: it landed powered).
    pub brake_lead: Option<i64>,
    /// The least rest margin before the next summon (i64::MIN: not all at rest).
    pub rest: i64,
    pub wrong: usize,
    pub contacts: usize,
    pub contact_max: f64,
    /// The closest an arriving cart came to a departing one (hundredths).
    pub closest: Option<i64>,
    pub hurt: usize,
    pub unseated: usize,
    /// Departing carts that never reached the exit.
    pub stuck: usize,
    /// The longest a trapdoor waited past T + release on its pocket's string (gt).
    pub waited: i64,
    /// The waiting riders' lowest feet at the release (hundredths).
    pub feet: i64,
}

fn summon_boundary(frozen: &[bool]) -> i64 {
    (1..10i64).map(|k| 100 * k).find(|b| (*b as usize) >= frozen.len() || !frozen[*b as usize]).unwrap_or(100)
}

/// One drop run: the report lines and the figures.
pub fn bench(spec: &FarmSpec, o: &DropOpts) -> Result<(Vec<String>, DropStats), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 6 {
        return Err(format!("{} carts", carts.len()));
    }
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (rs, stone, td_open, td_shut) = (parse("redstone_block")?, parse("stone")?, parse(TRAPDOOR_OPEN)?, parse(TRAPDOOR_SHUT)?);
    for t in 0..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            break;
        }
    }
    let pockets = o.layout.pockets();
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let feet_of = |w: &redsim::world::World, r: Option<u32>| r.and_then(|r| w.entities.list.iter().find(|e| e.id == r && !e.removed)).map(|e| e.pos.y);
    let string_on = |w: &redsim::world::World, k: usize| {
        let (x, y, z) = pockets[k];
        w.get(BlockPos::new(x, y + 1, z)).to_string().contains("powered=true")
    };
    let pocket_powered = |w: &redsim::world::World, k: usize| {
        let (x, y, z) = pockets[k];
        w.get(BlockPos::new(x, y, z)).to_string().contains("powered=true")
    };
    let mut frozen_ticks = 0i64;
    let mut frozen_hist: Vec<bool> = Vec::new();
    let mut opened: [Option<i64>; 3] = [None; 3];
    let mut opened_live: [i64; 3] = [0; 3];
    let mut shut_again = [false; 3];
    let mut from: [Option<Vec3>; 3] = [None; 3];
    let mut feet: [Option<f64>; 3] = [None; 3];
    let mut waited = [0i64; 3];
    // (a string already on at T has been held since the batch came to rest, long before)
    let mut on_since: [Option<i64>; 3] = [0, 1, 2].map(|k| string_on(&run.world, k).then_some(-1000));
    let mut landed: [Option<(i64, f64, bool)>; 3] = [None; 3];
    let mut fall_v = [0.0f64; 3];
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut rebrake_dt: Option<i64> = None;
    let mut exits: [Option<(i64, f64)>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 6];
    let mut touching = [[false; 6]; 6];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut closest: Option<(f64, i64, usize, usize)> = None;
    let mut hurt = 0usize;
    let mut health: [Option<f32>; 6] = [None; 6];
    let mut unseated = 0usize;
    let mut out: Vec<String> = Vec::new();
    for dt in 0..o.ticks {
        let frozen_now = o.freeze.is_some_and(|(f, l)| dt >= f && dt < f + l);
        let live = dt - frozen_ticks;
        // the pockets' supports (and the series layout's gate slopes): REL + stagger .. REL + rebrake
        for k in 0..3 {
            let on = live >= o.rel + o.stagger[k] && live < o.rel + o.rebrake;
            let want = if on { rs.clone() } else { stone.clone() };
            let (x, y, z) = pockets[k];
            let mut cells = vec![BlockPos::new(x, y - 1, z)];
            if o.layout == DropLayout::Series {
                let (sx, sy, sz) = SERIES_SLOPE_SRC[k];
                cells.push(BlockPos::new(sx, sy, sz));
            }
            for p in cells {
                if run.world.get(p) != want {
                    run.world.set_block(p, want.clone(), redsim::world::flags::ALL);
                }
            }
        }
        if rebrake_dt.is_none() && live >= o.rel + o.rebrake {
            rebrake_dt = Some(dt);
        }
        // the trapdoors: open at T + release unless the pocket's string has been held 20 gt
        for k in 0..3 {
            let (x, _, z) = pockets[k];
            let cell = BlockPos::new(x, o.hold_y, z);
            let occupied = o.occupancy && on_since[k].is_some_and(|s| live - s >= 20);
            let due = if o.release_world { dt >= o.release } else { !frozen_now && live >= o.release };
            if opened[k].is_none() && due {
                if occupied {
                    waited[k] = (if o.release_world { dt } else { live }) - o.release + 1;
                } else {
                    opened[k] = Some(dt);
                    opened_live[k] = live;
                    from[k] = pos(&run.world, carts[3 + k]).map(|(p, _)| p);
                    feet[k] = feet_of(&run.world, riders[3 + k]);
                    run.world.set_block(cell, td_open.clone(), redsim::world::flags::ALL);
                }
            }
            if o.pulse > 0 && opened[k].is_some() && !shut_again[k] && live >= opened_live[k] + o.pulse {
                shut_again[k] = true;
                run.world.set_block(cell, td_shut.clone(), redsim::world::flags::ALL);
            }
        }
        if let Some((f, len)) = o.freeze {
            if frozen_now {
                frozen_ticks += 1;
            }
            if dt == f {
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, [0, 0, 0, 0]);
            }
            if dt == f + len {
                redsim::reload::load_some(&mut run.world, [0, 0, 0, 0]);
            }
        }
        frozen_hist.push(frozen_now);
        run.tick();
        let live_after = dt + 1 - frozen_ticks;
        for k in 0..3 {
            if string_on(&run.world, k) {
                on_since[k].get_or_insert(live_after);
            } else {
                on_since[k] = None;
            }
        }
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        let mut sink_now: Vec<usize> = Vec::new();
        for n in 0..6 {
            let Some((p, v)) = st[n] else { continue };
            if n < 3 {
                if exits[n].is_none() && p.x > o.layout.sink_x() {
                    exits[n] = Some((dt, (v.x * v.x + v.z * v.z).sqrt()));
                    sink_now.push(n);
                }
                continue;
            }
            let k = n - 3;
            if opened[k].is_none() {
                continue;
            }
            let (_, py, pz) = pockets[k];
            if landed[k].is_none() && p.y < py as f64 + 1.0 {
                landed[k] = Some((dt, fall_v[k], pocket_powered(w, k)));
            }
            fall_v[k] = -v.y;
            let s3 = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
            if s3 < 1e-3 && (p.z - (pz as f64 + 0.5)).abs() < 0.6 && p.y < py as f64 + 1.0 {
                rest_since[k].get_or_insert(dt);
            } else {
                rest_since[k] = None;
            }
            for m in 0..3 {
                if let Some((pd, _)) = st[m] {
                    let d = ((p.x - pd.x).powi(2) + (p.y - pd.y).powi(2) + (p.z - pd.z).powi(2)).sqrt();
                    if closest.is_none_or(|c| d < c.0) {
                        closest = Some((d, dt, k, m));
                    }
                }
            }
        }
        let live_cart = |n: usize| n < 3 || opened[n - 3].is_some();
        for a in 0..6 {
            for c in 0..6 {
                if a == c || !live_cart(a) || !live_cart(c) {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                let now = d < 1.25;
                if now && !touching[a][c] && sc < 0.05 && dt > 0 {
                    let sa = prev_v[a].max((va.x * va.x + va.y * va.y + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: {} cart {} into {} cart {} at {sa:.3}", if a < 3 { "departing" } else { "arriving" }, a % 3, if c < 3 { "departing" } else { "arriving" }, c % 3));
                }
                touching[a][c] = now;
            }
        }
        for n in 0..6 {
            prev_v[n] = st[n].map_or(0.0, |(_, v)| (v.x * v.x + v.y * v.y + v.z * v.z).sqrt());
            let Some(r) = riders[n] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if health[n].is_some_and(|h0| hp < h0) {
                hurt += 1;
            }
            health[n] = Some(hp);
            if e.vehicle.is_none() {
                unseated += 1;
            }
        }
        if o.trace {
            let strings: String = (0..3).map(|k| if string_on(w, k) { '1' } else { '0' }).collect();
            out.push(format!("T+{dt} strings {strings}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}/{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt(), v.y))).collect::<Vec<_>>().join(" | ")));
        }
        for n in sink_now {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == carts[n]).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == carts[n] || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
    }
    let boundary = summon_boundary(&frozen_hist);
    let mut stats = DropStats::default();
    let wrong: Vec<String> = (0..3)
        .filter_map(|k| {
            let (x, _, z) = pockets[k];
            match pos(&run.world, carts[3 + k]) {
                Some((p, _)) if (p.x - (x as f64 + 0.5)).abs() < 0.3 && (p.z - (z as f64 + 0.5)).abs() < 0.3 && rest_since[k].is_some() => None,
                Some((p, _)) => Some(format!("arriving cart {k} at {:.2},{:.2},{:.2}", p.x, p.y, p.z)),
                None => Some(format!("arriving cart {k} gone")),
            }
        })
        .collect();
    stats.spread = (0..3)
        .filter_map(|k| from[k].map(|p| (((p.x - (pockets[k].0 as f64 + 0.5)).abs().max((p.z - (pockets[k].2 as f64 + 0.5)).abs())) * 100.0).round() as i64))
        .max()
        .unwrap_or(0);
    stats.fall = (0..3).map(|k| landed[k].zip(opened[k]).map_or(i64::MAX, |(l, d)| l.0 - d)).max().unwrap_or(i64::MAX);
    stats.brake_lead = (0..3).filter_map(|k| landed[k].zip(rebrake_dt).map(|(l, r)| l.0 - r)).min();
    let last_rest = (0..3).map(|k| rest_since[k].unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    stats.rest = if last_rest == i64::MAX { i64::MIN } else { boundary - last_rest };
    stats.wrong = wrong.len();
    stats.contacts = contacts.len();
    stats.contact_max = contact_max;
    stats.closest = closest.map(|c| (c.0 * 100.0).floor() as i64);
    stats.hurt = hurt;
    stats.unseated = unseated;
    stats.stuck = (0..3).filter(|k| exits[*k].is_none()).count();
    stats.waited = *waited.iter().max().unwrap_or(&0);
    stats.feet = (0..3).filter_map(|k| feet[k]).map(|y| (y * 100.0).floor() as i64).min().unwrap_or(0);
    out.push(format!(
        "drop ({:?}, hold y {}; REL T+{} +{:?}, re-brake REL+{}{}; trapdoors at T+{}{}{}): released {}; riders' feet {}; falls {}; at rest {} (the next summon T+{boundary}); the closest arriving-to-departing {}",
        o.layout,
        o.hold_y,
        o.rel,
        o.stagger,
        o.rebrake,
        rebrake_dt.map_or(String::new(), |t| format!(" = T+{t}")),
        o.release,
        if o.occupancy { " gated on the pocket strings" } else { "" },
        if o.pulse > 0 { format!(", shut after {} gt", o.pulse) } else { String::new() },
        (0..3).map(|k| match (opened[k], from[k]) {
            (Some(t), Some(p)) => format!("cart {k} T+{t}{} at {:.3},{:.3},{:.3}", if waited[k] > 0 { format!(" (waited {} gt on its string)", waited[k]) } else { String::new() }, p.x, p.y, p.z),
            _ => format!("cart {k} NEVER (waited {} gt)", waited[k]),
        }).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| feet[k].map_or("-".to_string(), |y| format!("{y:.3}"))).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| match (landed[k], opened[k]) {
            (Some((t, v, pw)), Some(d)) => format!("cart {k} {} gt to T+{t} at {v:.3} b/gt{}", t - d, if pw { " ONTO A POWERED POCKET" } else { "" }),
            _ => format!("cart {k} never landed"),
        }).collect::<Vec<_>>().join(", "),
        (0..3).map(|k| rest_since[k].map_or(format!("cart {k} NO"), |t| format!("cart {k} T+{t}"))).collect::<Vec<_>>().join(", "),
        closest.map_or("-".to_string(), |(d, t, a, m)| format!("{d:.2} (arriving cart {a} to departing cart {m} at T+{t})"))
    ));
    out.push(format!(
        "pockets: {}; departures {}; contacts {} (the fastest {:.3}){}; riders hurt {hurt}, unseated ticks {unseated}",
        if wrong.is_empty() { "every arriving cart at rest mid-cell in its own pocket".to_string() } else { format!("WRONG POCKET: {}", wrong.join("; ")) },
        (0..3).map(|k| exits[k].map_or(format!("cart {k} STUCK"), |(t, v)| format!("cart {k} out T+{t} at {v:.3}"))).collect::<Vec<_>>().join(", "),
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    Ok((out, stats))
}

/// The grid's summary over many drop runs.
pub fn summary(label: &str, runs: &[DropStats]) -> String {
    let show = |v: i64| if v == i64::MIN { "FAIL".to_string() } else { v.to_string() };
    format!(
        "{label}: {} runs; the widest release spread {:.2}; the longest fall {}; the least landing lead over its pocket's re-brake {}; arriving at rest: least {} before the next summon; wrong pockets {}, contacts {} (the fastest {:.3}), the closest arriving-to-departing {}; riders hurt {}, unseated ticks {}; departing carts stuck {}; the longest wait on a string {} gt; the lowest waiting rider's feet {:.2}",
        runs.len(),
        runs.iter().map(|s| s.spread).max().unwrap_or(0) as f64 / 100.0,
        runs.iter().map(|s| s.fall).max().map_or("-".to_string(), |f| if f == i64::MAX { "NEVER LANDED".to_string() } else { format!("{f} gt") }),
        runs.iter().filter_map(|s| s.brake_lead).min().map_or("-".to_string(), |v| format!("{v} gt")),
        show(runs.iter().map(|s| s.rest).min().unwrap_or(i64::MIN)),
        runs.iter().map(|s| s.wrong).sum::<usize>(),
        runs.iter().map(|s| s.contacts).sum::<usize>(),
        runs.iter().map(|s| s.contact_max).fold(0.0, f64::max),
        runs.iter().filter_map(|s| s.closest).min().map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0)),
        runs.iter().map(|s| s.hurt).sum::<usize>(),
        runs.iter().map(|s| s.unseated).sum::<usize>(),
        runs.iter().map(|s| s.stuck).sum::<usize>(),
        runs.iter().map(|s| s.waited).max().unwrap_or(0),
        runs.iter().map(|s| s.feet).min().unwrap_or(0) as f64 / 100.0
    )
}
