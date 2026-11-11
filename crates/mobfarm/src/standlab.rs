//! The stop-based stand's lab (the parent 2026-11-10 09:48: the single lane's batch of 3 caught back
//! to back with no brake timing; ironfarm's catch slots work only at +25/+26 after the detector).
//! A straight track along +x at z 0: a plain run at y 1 (x < 0), then the stand's cells from x 0
//! as a spec string; three ridden carts set on the run moving +x at a given speed, a given
//! number of cells apart (the release train); the run ticked: where each cart comes to rest, how
//! long the catch takes, whether the batch stays put; then the departure (the spec's controlled
//! cells powered) and how the three leave (their order, ticks apart, speeds).
//!
//! The spec: one token per cell from x 0 east, a type letter and a slope mark:
//! - `r` plain rail, `d` detector rail, `b` powered rail never powered (a brake), `B` powered rail
//!   always powered (a booster), `c` powered rail powered at the departure (braked before), `s` a
//!   stop (stone at track level: the track ends);
//! - `.` flat, `v` down east (an ascending_west rail one block lower), `^` up east.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// One cell of the lab's stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabCell {
    pub kind: char,
    pub slope: char,
    /// The rail's cell (x, y) on z 0.
    pub x: i32,
    pub y: i32,
}

/// The stand's cells from the spec, the track level starting at 1 (the run's).
pub fn parse(spec: &str) -> Result<Vec<LabCell>> {
    let mut out = Vec::new();
    let mut level = 1;
    for (x, tok) in spec.split_whitespace().enumerate() {
        let mut ch = tok.chars();
        let kind = ch.next().unwrap_or('?');
        // (a gate cell is always a slope down: the gate stands over its high end)
        let slope = if kind == 'G' || kind == 'S' || kind == 'K' { 'v' } else { ch.next().unwrap_or('.') };
        if !"rdbBcsGkSK".contains(kind) || !".v^".contains(slope) {
            bail!("{tok:?}: a type of r d b B c s G and a slope of . v ^");
        }
        let y = match slope {
            'v' => {
                level -= 1;
                level
            }
            '^' => {
                level += 1;
                level - 1
            }
            _ => level,
        };
        out.push(LabCell { kind, slope, x: x as i32, y });
        if kind == 's' {
            break;
        }
    }
    Ok(out)
}

/// The lab's design: the run (x -24..-1 at y 1), the stand, a plain run-out after the last rail
/// cell (unless a stop ends it), three ridden carts parked far off (the bench places them).
pub fn design(spec: &str) -> Result<String> {
    let cells = parse(spec)?;
    let mut b = Builder::new("stand-lab");
    for x in -24..0 {
        b.set(x, 1, 0, "rail[shape=east_west,waterlogged=false]");
        b.set(x, 0, 0, "glass");
    }
    for c in &cells {
        let shape = match c.slope {
            'v' => "ascending_west",
            '^' => "ascending_east",
            _ => "east_west",
        };
        match c.kind {
            's' => {
                b.set(c.x, c.y, 0, "stone");
                continue;
            }
            'G' | 'S' | 'K' => {
                // a plain slope down with a closed fence gate over its high end (the stop; the
                // departure opens it)
                b.set(c.x, c.y, 0, "rail[shape=ascending_west,waterlogged=false]");
                b.set(c.x, c.y - 1, 0, "glass");
                b.set(c.x, c.y + 1, 0, &format!("oak_fence_gate[facing=east,in_wall=false,open={},powered=false]", c.kind == 'K'));
                continue;
            }
            'r' => b.set(c.x, c.y, 0, &format!("rail[shape={shape},waterlogged=false]")),
            'd' => b.set(c.x, c.y, 0, &format!("detector_rail[powered=false,shape={shape},waterlogged=false]")),
            'b' | 'c' => b.set(c.x, c.y, 0, &format!("powered_rail[powered=false,shape={shape},waterlogged=false]")),
            _ => b.set(c.x, c.y, 0, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]")),
        }
        b.set(c.x, c.y - 1, 0, if c.kind == 'B' || c.kind == 'k' { "redstone_block" } else { "glass" });
    }
    // the run-out: plain, at the last cell's exit level
    if let Some(last) = cells.last()
        && last.kind != 's'
    {
        let level = if last.slope == '^' { last.y + 1 } else { last.y };
        for x in last.x + 1..last.x + 30 {
            b.set(x, level, 0, "rail[shape=east_west,waterlogged=false]");
            b.set(x, level - 1, 0, "glass");
        }
    }
    for k in 0..3 {
        let z = 20 + 3 * k;
        b.set(0, 0, z, "powered_rail[powered=false,shape=east_west,waterlogged=false]");
        b.set(0, -1, z, "glass");
        b.entity("minecart", [0.5, 0.0625, z as f64 + 0.5], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"));
    }
    // a zombie in a boat on glass 4 north of the stand, leaves over it (the riders panic at night,
    // as on the loop near its posts)
    b.set(3, 2, 5, "glass");
    b.set(3, 6, 5, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
    b.entity("oak_boat", [3.5, 3.0, 5.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-5, 1, 10]);
    Ok(b.text(&["The stop-based stand's lab: a run, the stand's cells, three ridden carts."]))
}

/// One lab run's options.
#[derive(Clone, Debug)]
pub struct LabOpts {
    /// The carts' stored speed on the run (b/t; the movement is capped at 0.4).
    pub speed: f64,
    /// Added to the 2nd and 3rd carts' speeds (the loop's trains arrive back cart fastest).
    pub skew: [f64; 2],
    /// The mob AI on (the loop bench runs it on).
    pub ai: bool,
    /// The back cart first in the entity list (the loop's order).
    pub reverse: bool,
    /// The train's spacing (cells, front to back), each gap its own (2 gaps).
    pub gaps: [f64; 2],
    /// The front cart's start on the run (its centre's x).
    pub start_x: f64,
    /// The departure (the `c` cells powered) at this tick; None: no departure.
    pub depart_at: Option<i64>,
    /// The `k` cells (powered for the catch) switched off (braked) at this tick, and on again at
    /// `release_at`.
    pub hold_at: Option<i64>,
    pub release_at: Option<i64>,
    /// Metering: 3 departure pulses `meter` gt apart, each closing again once a cart is 1.5 past
    /// the gate (one cart a pulse).
    pub meter: Option<i64>,
    /// Metering: the gate shuts once a leaving cart's centre is this far past the gate cell's
    /// west edge (1.29: a detector rail as the gate's slope, its falling edge).
    pub close_at: f64,
    /// Pockets: every gate its own stop, shut by the falling-edge logic, opened by the releases.
    pub pockets: bool,
    /// Pockets: the stand's gate `S` opened (and its `c` cells powered) at this tick, shut again at
    /// `stand_close_at`.
    pub stand_open_at: Option<i64>,
    pub stand_close_at: Option<i64>,
    /// Pockets: gate k opened (with every gate east of it) at `depart_at` + `releases[k]`.
    pub releases: Vec<i64>,
    pub ticks: i64,
    pub trace: bool,
}

/// What a lab run saw.
#[derive(Clone, Debug, Default)]
pub struct LabReport {
    /// Each cart's position at the departure (or the end): (x, y), its speed.
    pub rest: Vec<(f64, f64, f64)>,
    /// The tick all 3 came to rest for good before the departure (speeds under 1e-3 from then on).
    pub settled_at: Option<i64>,
    /// The tick from which no cart moved (more than 1e-4 a tick) until the departure.
    pub frozen_at: Option<i64>,
    /// The largest x excursion of any cart in the last 40 ticks before the departure.
    pub wobble: f64,
    /// The carts in order front to back at rest (x decreasing).
    pub ordered: bool,
    /// Each cart's exit tick (crossing the run-out's x) and speed then.
    pub exits: Vec<Option<(i64, f64)>>,
}

pub fn bench(spec: &FarmSpec, cells: &[LabCell], o: &LabOpts) -> Result<LabReport, String> {
    let ro = RunOpts { ai: o.ai, ..RunOpts::steady(1, (o.ticks + 400) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 3 {
        return Err(format!("{} carts", carts.len()));
    }
    let place = |w: &mut redsim::world::World, id: u32, x: f64, v: f64| {
        let Some(i) = w.entities.list.iter().position(|e| e.id == id) else { return };
        w.entities.set_pos(i, Vec3::new(x, 1.0625, 0.5));
        w.entities.list[i].delta_movement = Vec3::new(v, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for p in ps {
            if let Some(j) = w.entities.list.iter().position(|e| e.id == p) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    };
    let xs = [o.start_x, o.start_x - o.gaps[0], o.start_x - o.gaps[0] - o.gaps[1]];
    // (`reverse`: the back cart first in the entity list, as in the loop, where the release
    // group's front cart was spawned last)
    let mut carts = carts;
    if o.reverse {
        carts.reverse();
    }
    for (k, id) in carts.iter().enumerate() {
        place(&mut run.world, *id, xs[k], o.speed + if k == 0 { 0.0 } else { o.skew[k - 1] });
    }
    let exit_x = cells.last().map(|c| c.x as f64 + 6.0).unwrap_or(10.0);
    let on = redsim::block::Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let glass = redsim::block::Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let gate_open = redsim::block::Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=true]").map_err(|e| e.to_string())?;
    let gate_shut = redsim::block::Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]").map_err(|e| e.to_string())?;
    let set_open = |w: &mut redsim::world::World, open: bool| -> Result<(), String> {
        for c in cells.iter().filter(|c| c.kind == 'c') {
            w.set_block(BlockPos::new(c.x, c.y - 1, 0), if open { on.clone() } else { glass.clone() }, redsim::world::flags::ALL);
        }
        for c in cells.iter().filter(|c| c.kind == 'G' || c.kind == 'S') {
            w.set_block(BlockPos::new(c.x, c.y + 1, 0), if open { gate_open.clone() } else { gate_shut.clone() }, redsim::world::flags::ALL);
        }
        Ok(())
    };
    let mut armed = false;
    let mut pocket_open: Vec<bool> = Vec::new();
    let mut pocket_seen: Vec<Vec<bool>> = Vec::new();
    let mut releasing = false;
    let mut back_seen = 0usize;
    let mut back_shut = false;
    let mut rep = LabReport { exits: vec![None; 3], ..Default::default() };
    let mut still_since: Option<i64> = None;
    let mut hist: Vec<[f64; 3]> = Vec::new();
    let end_catch = o.depart_at.unwrap_or(o.ticks);
    for t in 0..o.ticks {
        for (when, state) in [(o.hold_at, "glass"), (o.release_at, "redstone_block")] {
            if Some(t) == when {
                let b = redsim::block::Block::parse(&format!("minecraft:{state}")).map_err(|e| e.to_string())?;
                for c in cells.iter().filter(|c| c.kind == 'k') {
                    run.world.set_block(BlockPos::new(c.x, c.y - 1, 0), b.clone(), redsim::world::flags::ALL);
                }
            }
        }
        // the departure: the gates opened and the `c` cells powered at `depart_at` (and with
        // `meter`, again every `meter` gt, 3 pulses); with `meter` each pulse closes again (gates
        // shut, `c` cells braked) as soon as a cart's centre is 1.5 past a gate's cell: one cart
        // a pulse (the metering gate's latch, reset by a detector there)
        let pulse = !o.pockets && o.depart_at.is_some_and(|d| t == d || o.meter.is_some_and(|m| t > d && (t - d) % m == 0 && (t - d) / m < 3));
        if pulse {
            set_open(&mut run.world, true)?;
            armed = true;
        }
        if armed && o.meter.is_some() {
            // (a detector rail as the gate's slope: its signal falls as the leaving cart's box
            // leaves the detector's box, x + 0.8, i.e. its centre at x + 1.29, its tail then past
            // the gate's collision box, x + 0.625: the latch resets on that falling edge)
            let gx = cells.iter().find(|c| c.kind == 'G').map(|c| c.x as f64 + o.close_at).unwrap_or(f64::MAX);
            let crossed = carts.iter().any(|id| run.world.entities.list.iter().find(|e| e.id == *id).is_some_and(|e| e.pos.x >= gx && e.pos.x < gx + 1.0));
            if crossed {
                set_open(&mut run.world, false)?;
                armed = false;
            }
        }
        // pockets (`pockets`): every gate its own pocket's stop, the gates counted from the east
        // (gate 0 the last); at the start gate 0 shut, the others open; a gate shuts on its
        // detector's falling edge (a leaving cart's centre past x + `close_at`) once the gate east
        // of it is shut: the front cart passes every gate but the last, the second stops at the
        // one before, the third at the one before that; the releases open gate 0, then 1, then 2
        // (`depart_at` + k `meter`), each cart rolling out through the open pockets east of it
        // the back gate `K`: open, it shuts once the third cart has passed it (its centre 1.29 past
        // the gate slope's start): a cart that bounces back off the batch stops against it
        for c in cells.iter().filter(|c| c.kind == 'K') {
            let n = carts.iter().filter(|id| run.world.entities.list.iter().find(|e| e.id == **id).is_some_and(|e| e.pos.x >= c.x as f64 + o.close_at)).count();
            back_seen = back_seen.max(n);
            if back_seen >= 3 && !back_shut {
                run.world.set_block(BlockPos::new(c.x, c.y + 1, 0), gate_shut.clone(), redsim::world::flags::ALL);
                back_shut = true;
            }
        }
        if o.pockets {
            // (the stand's gate `S` and its `c` cells: opened and powered at `stand_open_at`,
            // shut and braked at `stand_close_at`)
            for (when, open) in [(o.stand_open_at, true), (o.stand_close_at, false)] {
                if Some(t) == when {
                    for c in cells.iter().filter(|c| c.kind == 'c') {
                        run.world.set_block(BlockPos::new(c.x, c.y - 1, 0), if open { on.clone() } else { glass.clone() }, redsim::world::flags::ALL);
                    }
                    for c in cells.iter().filter(|c| c.kind == 'S') {
                        run.world.set_block(BlockPos::new(c.x, c.y + 1, 0), if open { gate_open.clone() } else { gate_shut.clone() }, redsim::world::flags::ALL);
                    }
                }
            }
            let gates: Vec<i32> = { let mut g: Vec<i32> = cells.iter().filter(|c| c.kind == 'G').map(|c| c.x).collect(); g.sort(); g.reverse(); g };
            if pocket_open.is_empty() {
                pocket_open = (0..gates.len()).map(|k| k > 0).collect();
                pocket_seen = vec![vec![false; carts.len()]; gates.len()];
            }
            if let Some(d) = o.depart_at {
                for (k, off) in o.releases.iter().enumerate() {
                    if t == d + off && k < gates.len() {
                        for j in 0..=k {
                            pocket_open[j] = true;
                        }
                        releasing = true;
                    }
                }
            }
            if false
                && let (Some(d), Some(m)) = (o.depart_at, o.meter)
                && t >= d
                && (t - d) % m == 0
                && ((t - d) / m) < gates.len() as i64
            {
                let k = ((t - d) / m) as usize;
                for j in 0..=k {
                    pocket_open[j] = true;
                }
                releasing = true;
            }
            if !releasing {
                // (gate k lets k carts by: it shuts on its detector's k-th falling edge)
                for k in 1..gates.len() {
                    let edge = gates[k] as f64 + o.close_at;
                    for (n, id) in carts.iter().enumerate() {
                        let x = run.world.entities.list.iter().find(|e| e.id == *id).map(|e| e.pos.x).unwrap_or(f64::NAN);
                        if x >= edge && !pocket_seen[k][n] {
                            pocket_seen[k][n] = true;
                            if pocket_seen[k].iter().filter(|b| **b).count() >= k {
                                pocket_open[k] = false;
                            }
                        }
                    }
                }
            }
            for (k, gx) in gates.iter().enumerate() {
                let y = cells.iter().find(|c| c.kind == 'G' && c.x == *gx).map(|c| c.y).unwrap_or(0);
                let want = if pocket_open[k] { gate_open.clone() } else { gate_shut.clone() };
                if run.world.get(BlockPos::new(*gx, y + 1, 0)) != want {
                    run.world.set_block(BlockPos::new(*gx, y + 1, 0), want, redsim::world::flags::ALL);
                }
            }
        }
        run.tick();
        let w = &run.world;
        let st: Vec<(f64, f64, f64)> = carts.iter().map(|id| w.entities.list.iter().find(|e| e.id == *id).map(|e| (e.pos.x, e.pos.y, e.delta_movement.horizontal_distance())).unwrap_or((f64::NAN, f64::NAN, f64::NAN))).collect();
        if o.trace && (t < 60 || (t >= 198 && t < 240)) {
            println!("t {t}: {}", st.iter().map(|(x, y, v)| format!("{x:.3},{y:.2} v{v:.3}")).collect::<Vec<_>>().join(" | "));
        }
        if t < end_catch {
            hist.push([st[0].0, st[1].0, st[2].0]);
            if st.iter().all(|s| s.2 < 1e-3) {
                still_since.get_or_insert(t);
            } else {
                still_since = None;
            }
            if t == end_catch - 1 {
                rep.rest = st.clone();
                rep.settled_at = still_since;
                // (frozen: the first tick from which no cart moves more than 1e-4 a tick)
                let mut frozen = hist.len();
                while frozen > 1 && (0..3).all(|k| (hist[frozen - 1][k] - hist[frozen - 2][k]).abs() < 1e-4) {
                    frozen -= 1;
                }
                rep.frozen_at = (frozen < hist.len()).then_some(frozen as i64 - 1);
                let tail = &hist[hist.len().saturating_sub(40)..];
                rep.wobble = (0..3).map(|k| tail.iter().map(|h| h[k]).fold(f64::NEG_INFINITY, f64::max) - tail.iter().map(|h| h[k]).fold(f64::INFINITY, f64::min)).fold(0.0, f64::max);
                // (in order and apart: two carts in one spot, one slipped into the other, is no catch)
                rep.ordered = st[0].0 > st[1].0 + 0.6 && st[1].0 > st[2].0 + 0.6;
            }
        }
        for k in 0..3 {
            if rep.exits[k].is_none() && st[k].0 >= exit_x {
                rep.exits[k] = Some((t, st[k].2));
            }
        }
    }
    if o.depart_at.is_none() && rep.rest.is_empty() {
        rep.rest = vec![(f64::NAN, f64::NAN, f64::NAN); 3];
    }
    Ok(rep)
}
