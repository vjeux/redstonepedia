//! `mctas mobfarm stack-bench` (cc767b78; farmopt 2026-11-11 21:06, vjeux's primitive (f), search
//! option (f); tick-exact's physics read 21:10): the vertical cart stack queue. Ridden carts stacked
//! in a 1x1 glass shaft, each resting on the head of the rider below (a moving cart treats every
//! `isPushable` entity as a solid box: the pitch 1.95, the cart above at rail y + 2.0125, 0.0125
//! over the rail window), the bottom one on the foot: a powered rail with a solid block behind it
//! (the from-rest kick 0.02 away from it, then +0.06 a tick). The exit east: tokens (`r` a plain
//! rail, `D` a detector rail: the departures' evidence, `B` a booster: a powered rail on a redstone
//! block, `b` an unpowered powered rail: a brake), then a flat run to the sink (the bench takes each
//! cart and its rider out there). A release powers the foot for exactly K departures; the
//! controller is the bench (it sets the foot's power block): `detector:X:D` cuts it D gt after the
//! K-th rising edge of the exit's detector at x X; `edges:N` keeps it N ring edges (day-1's fast
//! ring: rising edges 3 or 4 gt apart, pausing with the carts); `ticks:N` N game ticks (the
//! control). The refill drops K carts onto the stack from a height (the bubble lift's delivery
//! point, emulated). Bench, not wired.

use crate::layouts::Builder;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::BlockPos;

/// The pitch of a ridden stack (tick-exact 21:10): a villager sits with its feet at its cart's y and
/// stands 1.95 tall; the cart above rests on its head.
pub const PITCH: f64 = 1.95;
/// A cart's y on a flat rail (the rail's y + 0.0625).
pub const ON_RAIL: f64 = 0.0625;
/// A zombie's eye height (its feet given).
const ZOMBIE_EYE: f64 = 1.74;

/// The stack's design.
#[derive(Clone, Debug)]
pub struct StackDesign {
    /// The carts stacked at the start (the bottom one on the foot).
    pub carts: usize,
    /// The exit's tokens east of the foot (`r` rail, `D` detector rail, `B` booster, `b` brake).
    pub exit: String,
    /// The flat run after the exit's tokens (cells).
    pub run: i32,
    /// Every cart ridden (normal operation: tick-exact, an empty cart at the bottom merges the next).
    pub riders: bool,
    /// Shaft cells above the settled stack's top rider head (the refill's drop room).
    pub headroom: i32,
}

impl StackDesign {
    /// The exit's tokens.
    pub fn tokens(&self) -> Vec<char> {
        self.exit.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// The shaft's top cell (y): the settled stack's top rider head, plus the headroom.
    pub fn top(&self) -> i32 {
        (ON_RAIL + PITCH * self.carts as f64).ceil() as i32 + self.headroom
    }

    /// The sink's x (a cart's centre past it is taken out).
    pub fn sink_x(&self) -> f64 {
        (self.tokens().len() as i32 + self.run) as f64 + 0.5
    }
}

/// The design text.
pub fn design(d: &StackDesign) -> Result<String, String> {
    let toks = d.tokens();
    match toks.first() {
        Some('r') => {}
        Some('B') => return Err("--exit: a booster next to the foot powers it (powered rails chain power along their line): start the exit with r".into()),
        Some('D') => return Err("--exit: a detector rail next to the foot powers it while a cart is on it: start the exit with r".into()),
        _ => return Err("--exit: start the exit with r (a plain rail between the foot and anything powered)".into()),
    }
    let mut b = Builder::new("stack-bench");
    // the foot: a powered rail on its power block (stone: the bench sets a redstone block there to
    // power it), the solid block behind it (the kick's direction: away from it)
    b.set(0, -1, 0, "stone");
    b.set(0, 0, 0, "powered_rail[powered=false,shape=east_west,waterlogged=false]");
    b.set(-1, 0, 0, "stone");
    let top = d.top();
    for y in 0..=top {
        b.set(0, y, -1, "glass");
        b.set(0, y, 1, "glass");
        if y >= 1 {
            b.set(-1, y, 0, "glass");
            b.set(1, y, 0, "glass");
        }
    }
    let mut x = 1;
    for c in &toks {
        let (rail, under) = match c {
            'r' => ("rail[shape=east_west,waterlogged=false]", "glass"),
            'D' => ("detector_rail[powered=false,shape=east_west,waterlogged=false]", "glass"),
            'B' => ("powered_rail[powered=true,shape=east_west,waterlogged=false]", "redstone_block"),
            'b' => ("powered_rail[powered=false,shape=east_west,waterlogged=false]", "glass"),
            _ => return Err(format!("--exit: token {c:?} (r rail, D detector rail, B booster, b brake)")),
        };
        b.set(x, 0, 0, rail);
        b.set(x, -1, 0, under);
        x += 1;
    }
    for _ in 0..d.run.max(2) {
        b.set(x, 0, 0, "rail[shape=east_west,waterlogged=false]");
        b.set(x, -1, 0, "glass");
        x += 1;
    }
    // (the stack: each cart a little above its resting place, settling onto the rider below)
    for k in 0..d.carts {
        let y = ON_RAIL + k as f64 * (PITCH + 0.05);
        let nbt = if d.riders { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        b.entity("minecart", [0.5, y, 0.5], Some(nbt));
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [x - 1, 1, 0]);
    let caption = format!(
        "The vertical cart stack queue bench (cc767b78, vjeux's primitive (f)): {} ridden carts stacked in a 1x1 glass shaft on the foot (a powered rail, a solid block behind it), the exit east `{}`, a run of {} to the sink.",
        d.carts, d.exit, d.run
    );
    Ok(b.text(&[caption.as_str()]))
}

/// How a release's power ends.
#[derive(Clone, Copy, Debug)]
pub enum Cut {
    /// `delay` gt after the K-th rising edge of the exit's detector at x (`detector:X:D`).
    Detector { x: i32, delay: i64 },
    /// After `n` ring edges (`edges:N`: day-1's fast ring, pausing with the carts).
    Edges { n: i64 },
    /// After `n` game ticks (`ticks:N`: the control).
    Ticks { n: i64 },
}

impl Cut {
    pub fn parse(s: &str) -> Result<Cut, String> {
        let p: Vec<&str> = s.split(':').collect();
        let num = |t: &str| t.trim().parse::<i64>().map_err(|e| format!("--cut {s}: {e}"));
        match p.as_slice() {
            ["detector", x, d] => Ok(Cut::Detector { x: num(x)? as i32, delay: num(d)?.max(1) }),
            ["edges", n] => Ok(Cut::Edges { n: num(n)? }),
            ["ticks", n] => Ok(Cut::Ticks { n: num(n)? }),
            _ => Err(format!("--cut {s}: detector:X:DELAY, edges:N or ticks:N")),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Cut::Detector { x, delay } => format!("{delay} gt after the K-th rising edge of the detector at x {x}"),
            Cut::Edges { n } => format!("after {n} ring edges (day-1's fast ring, pausing with the carts)"),
            Cut::Ticks { n } => format!("after {n} game ticks (the control)"),
        }
    }
}

/// One run's options.
#[derive(Clone, Debug)]
pub struct StackOpts {
    /// Departures per release (K).
    pub release: usize,
    pub cut: Cut,
    /// The first release, gt after the stack settled.
    pub first: i64,
    /// Releases this many gt apart.
    pub every: i64,
    pub releases: usize,
    /// The refill: K carts dropped onto the stack from this y (the lift's delivery point), the
    /// first `refill_after` gt after each release's power ends, `refill_every` gt apart.
    pub refill_y: Option<f64>,
    pub refill_after: i64,
    pub refill_every: i64,
    /// A real level-32 freeze (the whole bench, redstone running) from tick T (the bench's clock)
    /// for D gt.
    pub freeze: Option<(i64, i64)>,
    /// A save, unload and load of the bench's chunks between two ticks, before tick T.
    pub reload_at: Option<i64>,
    pub ticks: i64,
    pub seed: i64,
    /// The plane (x) the departure spacing is measured at.
    pub plane: f64,
    /// Rules: zombies (feet), a golem spot, a bed.
    pub zombies: Vec<[f64; 3]>,
    pub golem: Option<[f64; 3]>,
    pub bed: Option<[f64; 3]>,
    pub trace: bool,
}

/// One release's record.
#[derive(Clone, Debug, Default)]
struct Rel {
    start: i64,
    off: Option<i64>,
    cut_due: Option<i64>,
    det_rises: usize,
    edges: i64,
    kicks: Vec<(u32, i64)>,
    left: Vec<(u32, i64)>,
    plane: Vec<(u32, i64)>,
    snaps: Vec<(u32, i64)>,
    stack_after: Option<usize>,
}

/// The stack's design metrics (blocks, the box, redstone components, height) of a farm spec.
pub fn metrics(spec: &FarmSpec) -> String {
    metrics_in(spec, None)
}

/// [`metrics`] of the cells inside a box (inclusive) only.
pub fn metrics_in(spec: &FarmSpec, within: Option<([i32; 3], [i32; 3])>) -> String {
    let inside = |p: &BlockPos| within.is_none_or(|(a, z)| p.x >= a[0] && p.x <= z[0] && p.y >= a[1] && p.y <= z[1] && p.z >= a[2] && p.z <= z[2]);
    let blocks: Vec<&(BlockPos, Block)> = spec.blocks.iter().filter(|(p, b)| inside(p) && !b.to_string().contains(":air")).collect();
    if blocks.is_empty() {
        return "no blocks".into();
    }
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, _) in &blocks {
        for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[i] = lo[i].min(v);
            hi[i] = hi[i].max(v);
        }
    }
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    const RED: [&str; 14] = ["powered_rail", "detector_rail", "activator_rail", "redstone", "repeater", "comparator", "observer", "piston", "lever", "button", "pressure_plate", "target", "daylight_detector", "hopper"];
    let red = blocks.iter().filter(|(_, b)| {
        let s = b.to_string();
        RED.iter().any(|r| s.contains(r))
    });
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, b) in red {
        let s = b.to_string();
        let name = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        *by.entry(name).or_default() += 1;
    }
    let n_red: usize = by.values().sum();
    format!(
        "{} blocks; box {}x{}x{} ({} cells, {} high); redstone components {} ({})",
        blocks.len(),
        size[0],
        size[1],
        size[2],
        size[0] as i64 * size[1] as i64 * size[2] as i64,
        size[1],
        n_red,
        by.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
    )
}

/// One run's report lines.
pub fn bench(spec: &FarmSpec, d: &StackDesign, o: &StackOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 800) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (power_on, power_off) = (parse("redstone_block")?, parse("stone")?);
    let foot_power = BlockPos::new(0, -1, 0);
    let ridden = |w: &redsim::world::World, id: u32| -> Option<u32> { w.entities.list.iter().find(|e| e.id == id && !e.removed).and_then(|e| e.passengers.first().copied()) };
    let pos_of = |w: &redsim::world::World, id: u32| -> Option<(Vec3, Vec3)> { w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement)) };
    let health_of = |w: &redsim::world::World, id: u32| -> Option<f32> {
        w.entities.list.iter().find(|e| e.id == id && !e.removed).and_then(|e| match &e.kind {
            EntityKind::Mob(m) => Some(m.health),
            _ => None,
        })
    };
    let mut carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "stack bench (cc767b78, bench: the controller and the refill are the bench's): {} carts{} in a 1x1 glass shaft (top cell y {}), the foot a powered rail with stone behind it, the exit `{}` and a run of {} to the sink at x {:.1}; {} releases of {} every {} gt from T+{}, the cut {}",
        d.carts,
        if d.riders { " (all ridden)" } else { " (EMPTY)" },
        d.top(),
        d.exit,
        d.run,
        d.sink_x(),
        o.releases,
        o.release,
        o.every,
        o.first,
        o.cut.label()
    ));
    out.push(format!("the design: {}", metrics(spec)));
    // (the stack settles: every cart at rest for 10 gt, at most 200 gt)
    let mut rest_for = 0;
    let mut settle = 0;
    for t in 0..200 {
        run.tick();
        let moving = carts.iter().filter_map(|id| pos_of(&run.world, *id)).any(|(_, v)| v.x.abs() + v.y.abs() + v.z.abs() > 1e-4);
        rest_for = if moving { 0 } else { rest_for + 1 };
        settle = t;
        if rest_for >= 10 {
            break;
        }
    }
    let mut ys: Vec<(u32, f64)> = carts.iter().filter_map(|id| pos_of(&run.world, *id).map(|(p, _)| (*id, p.y))).collect();
    ys.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let pitches: Vec<String> = ys.windows(2).map(|w| format!("{:.4}", w[1].1 - w[0].1)).collect();
    out.push(format!(
        "the stack at rest after {} gt: carts at y {} (the pitch {}; tick-exact: 1.95, the bottom at rail y + 0.0625)",
        settle + 1,
        ys.iter().map(|(_, y)| format!("{y:.4}")).collect::<Vec<_>>().join(", "),
        if pitches.is_empty() { "-".to_string() } else { pitches.join(", ") }
    ));
    // (the rules at the settled stack: each rider's eye)
    let mut rules: Vec<String> = Vec::new();
    for (k, (id, _)) in ys.iter().enumerate() {
        let Some(r) = ridden(&run.world, *id) else {
            rules.push(format!("cart {k}: EMPTY"));
            continue;
        };
        let Some(j) = run.world.entities.list.iter().position(|e| e.id == r && !e.removed) else { continue };
        let eye = redsim::entity::mob::ai::eye_pos(&run.world, j);
        let cell = BlockPos::new(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
        let inside = redsim::entity::collide::collides_with_blocks(&run.world, Aabb::new(eye.x - 5e-7, eye.y - 5e-7, eye.z - 5e-7, eye.x + 5e-7, eye.y + 5e-7, eye.z + 5e-7));
        let mut s = format!("cart {k}'s rider: eye y {:.3} in cell y {} ({}{})", eye.y, cell.y, run.world.get(cell).to_string().trim_start_matches("minecraft:"), if inside { ": INSIDE A COLLISION SHAPE (blind)" } else { "" });
        if !o.zombies.is_empty() {
            let zs: Vec<String> = o
                .zombies
                .iter()
                .map(|z| {
                    let ze = Vec3::new(z[0], z[1] + ZOMBIE_EYE, z[2]);
                    let dist = ((ze.x - eye.x).powi(2) + (ze.y - eye.y).powi(2) + (ze.z - eye.z).powi(2)).sqrt();
                    let seen = redsim::entity::mob::los::has_line_of_sight(&run.world, eye, ze);
                    format!("{dist:.2}{}{}", if seen { " seen" } else { " NOT SEEN" }, if dist < crate::fleecheck::FLEE_BAR { " (inside the flee bar 6.05)" } else if dist <= crate::fleecheck::PANIC_BAR { " (within the panic bar 7.95)" } else { "" })
                })
                .collect();
            s.push_str(&format!("; zombies {}", zs.join(", ")));
        }
        if let Some(g) = o.golem {
            let side = ((eye.x - g[0]).powi(2) + (eye.z - g[2]).powi(2)).sqrt();
            let above = eye.y - g[1];
            s.push_str(&format!("; the golem spot: {side:.2} sideways (bar 18.84), {above:.2} above (bar 20.54): {}", if side >= 18.84 || above >= 20.54 { "clear" } else { "INSIDE THE SENSOR BOX" }));
        }
        if let Some(bd) = o.bed {
            let dist = ((eye.x - bd[0]).powi(2) + (eye.y - bd[1]).powi(2) + (eye.z - bd[2]).powi(2)).sqrt();
            s.push_str(&format!("; the bed {dist:.2} away"));
        }
        rules.push(s);
    }
    out.push(format!("rules at the settled stack: {}", rules.join(" | ")));
    // (the ring for `edges:N`)
    let mut gate = crate::liveness::Liveness::new(crate::liveness::Ring::fast(0.5, 0.5));
    gate.stretch = crate::liveness::FAST_STRETCH;
    let det_cell = match o.cut {
        Cut::Detector { x, .. } => Some(BlockPos::new(x, 0, 0)),
        _ => None,
    };
    let mut det_was = false;
    let mut rels: Vec<Rel> = Vec::new();
    let mut powered = false;
    let mut prev_y: std::collections::HashMap<u32, f64> = Default::default();
    let mut prev_hp: std::collections::HashMap<u32, f32> = Default::default();
    let mut left_once: std::collections::HashSet<u32> = Default::default();
    let mut kicked_once: std::collections::HashSet<u32> = Default::default();
    let mut planed: std::collections::HashSet<u32> = Default::default();
    let mut hurt: Vec<String> = Vec::new();
    let mut merges: Vec<String> = Vec::new();
    let mut merged: std::collections::HashSet<(u32, u32)> = Default::default();
    let mut sunk = 0usize;
    let mut stray: Vec<String> = Vec::new();
    let mut refills: Vec<(i64, f64)> = Vec::new();
    let mut refill_log: Vec<String> = Vec::new();
    let mut refill_pending: Vec<(u32, i64, f64)> = Vec::new();
    let mut frozen32: Option<[i32; 4]> = None;
    let mut freeze_note: Vec<String> = Vec::new();
    for (id, _) in &ys {
        if let Some(r) = ridden(&run.world, *id)
            && let Some(h) = health_of(&run.world, r)
        {
            prev_hp.insert(r, h);
        }
    }
    let world_chunks = {
        let (mn, sz) = (run.world.min, run.world.size);
        [mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]
    };
    for t in 0..o.ticks {
        // (a freeze, a reload)
        if let Some((f, l)) = o.freeze {
            if t == f && frozen32.is_none() {
                frozen32 = Some(crate::liveness::freeze32(&mut run.world));
                freeze_note.push(format!("froze T+{t} (the foot {})", if powered { "POWERED" } else { "unpowered" }));
            }
            if t == f + l
                && let Some(pb) = frozen32.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
                freeze_note.push(format!("thawed T+{t} (the foot {})", if powered { "POWERED" } else { "unpowered" }));
            }
        }
        if o.reload_at == Some(t) {
            let chunks = redsim::reload::box_chunks(world_chunks);
            redsim::reload::reload_chunks(&mut run.world, &chunks);
            freeze_note.push(format!("reloaded the bench's chunks before T+{t} (the foot {})", if powered { "POWERED" } else { "unpowered" }));
        }
        // (the controller: a release's start, its cut)
        let c = rels.len();
        if c < o.releases && t == o.first + c as i64 * o.every {
            let mut r = Rel { start: t, ..Default::default() };
            if let Cut::Ticks { n } = o.cut {
                r.cut_due = Some(t + n);
            }
            rels.push(r);
            run.world.set_block(foot_power, power_on.clone(), redsim::world::flags::ALL);
            powered = true;
        }
        if powered
            && let Some(r) = rels.last_mut()
            && r.cut_due == Some(t)
        {
            run.world.set_block(foot_power, power_off.clone(), redsim::world::flags::ALL);
            powered = false;
            r.off = Some(t);
        }
        // (the refill: a cart and its rider dropped onto the stack)
        if let Some(ry) = o.refill_y {
            for r in &rels {
                if let Some(off) = r.off {
                    for k in 0..o.release as i64 {
                        if t == off + o.refill_after + k * o.refill_every {
                            let top = carts
                                .iter()
                                .filter_map(|id| pos_of(&run.world, *id).map(|(p, _)| (*id, p)))
                                .filter(|(_, p)| p.x < 1.0)
                                .map(|(id, p)| p.y + if ridden(&run.world, id).is_some() { PITCH } else { 0.7 })
                                .fold(ON_RAIL, f64::max);
                            let nid = run.world.entities.next_id;
                            let cid = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [0.5, ry, 0.5], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?;
                            for e in nid..run.world.entities.next_id {
                                run.own.push(e);
                            }
                            carts.push(cid);
                            if let Some(rr) = ridden(&run.world, cid)
                                && let Some(h) = health_of(&run.world, rr)
                            {
                                prev_hp.insert(rr, h);
                            }
                            refills.push((t, ry - top));
                            refill_pending.push((cid, t, ry - top));
                        }
                    }
                }
            }
        }
        run.tick();
        let w = &run.world;
        // (the ring's edges, pausing with the carts)
        let col_frozen = !w.entity_ticks_at(0.5, 0.5);
        gate.column_frozen = Some(col_frozen);
        let _ = gate.alive(w, t);
        let edge = gate.last_edge == Some(t) && !col_frozen;
        if powered
            && let Some(r) = rels.last_mut()
        {
            if edge {
                r.edges += 1;
                if let Cut::Edges { n } = o.cut
                    && r.edges >= n
                    && r.cut_due.is_none()
                {
                    r.cut_due = Some(t + 1);
                }
            }
        }
        // (the exit detector's rises)
        if let Some(dc) = det_cell {
            let on = matches!(w.get(dc), Block::DetectorRail { powered: true, .. });
            if on && !det_was
                && let Some(r) = rels.last_mut()
                && powered
            {
                r.det_rises += 1;
                if let Cut::Detector { delay, .. } = o.cut
                    && r.det_rises == o.release
                    && r.cut_due.is_none()
                {
                    r.cut_due = Some(t + delay);
                }
            }
            det_was = on;
        }
        // (each cart: its kick at the foot, its leaving the foot, the plane, a snap onto the foot)
        let live: Vec<(u32, Vec3, Vec3)> = carts.iter().filter_map(|id| pos_of(w, *id).map(|(p, v)| (*id, p, v))).collect();
        for (id, p, v) in &live {
            let at_foot = p.x < 1.0 && p.y < 1.0;
            if at_foot && (v.x * v.x + v.z * v.z).sqrt() > 0.005 && !kicked_once.contains(id) {
                kicked_once.insert(*id);
                match rels.last_mut() {
                    Some(r) => r.kicks.push((*id, t)),
                    None => stray.push(format!("cart {id} kicked at T+{t} before any release")),
                }
            }
            if p.x >= 1.0 && !left_once.contains(id) {
                left_once.insert(*id);
                match rels.last_mut() {
                    Some(r) => r.left.push((*id, t)),
                    None => stray.push(format!("cart {id} left the foot at T+{t} before any release")),
                }
            }
            if p.x >= o.plane && !planed.contains(id) {
                planed.insert(*id);
                if let Some(r) = rels.last_mut() {
                    r.plane.push((*id, t));
                }
            }
            if let Some(py) = prev_y.get(id)
                && py - p.y > 1.0
                && p.y < 0.5
                && let Some(r) = rels.last_mut()
            {
                r.snaps.push((*id, t));
            }
        }
        for (id, p, _) in &live {
            prev_y.insert(*id, p.y);
        }
        // (merges: two carts' centres within 0.5)
        for a in 0..live.len() {
            for b2 in a + 1..live.len() {
                let (pa, pb) = (live[a].1, live[b2].1);
                let dd = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt();
                let key = (live[a].0.min(live[b2].0), live[a].0.max(live[b2].0));
                if dd < 0.5 && !merged.contains(&key) {
                    merged.insert(key);
                    merges.push(format!("T+{t}: carts {} and {} {dd:.2} apart at {:.2},{:.2}", key.0, key.1, pa.x, pa.y));
                }
            }
        }
        // (the riders' health)
        for (id, _, _) in &live {
            if let Some(r) = ridden(w, *id)
                && let Some(h) = health_of(w, r)
            {
                if let Some(ph) = prev_hp.get(&r)
                    && h < *ph
                {
                    hurt.push(format!("T+{t}: cart {id}'s rider {:.1} -> {:.1}", ph, h));
                }
                prev_hp.insert(r, h);
            }
        }
        // (a refill cart's landing: at rest on the stack)
        refill_pending.retain(|(cid, t0, drop)| {
            if let Some((p, v)) = pos_of(w, *cid)
                && t > *t0 + 2
                && v.y.abs() < 1e-4
            {
                refill_log.push(format!("dropped T+{t0} from {drop:.2} above the stack's top, at rest T+{t} at y {:.3}", p.y));
                return false;
            }
            true
        });
        // (the sink)
        let gone: Vec<u32> = live.iter().filter(|(_, p, _)| p.x >= d.sink_x()).map(|(id, _, _)| *id).collect();
        for id in gone {
            let rider = ridden(&run.world, id);
            if let Some(r) = rider {
                let _ = redsim::entity::kill(&mut run.world, r);
            }
            let _ = redsim::entity::kill(&mut run.world, id);
            sunk += 1;
            carts.retain(|c| *c != id);
        }
        // (the stack after each release: carts still in the shaft once the next cart is at rest)
        if let Some(r) = rels.last_mut()
            && r.off.is_some_and(|off| t == off + 30)
        {
            r.stack_after = Some(carts.iter().filter_map(|id| pos_of(&run.world, *id)).filter(|(p, _)| p.x < 1.0).count());
        }
        if o.trace && t % 2 == 0 && t < o.first + o.every * o.releases as i64 {
            let st: Vec<String> = carts.iter().filter_map(|id| pos_of(&run.world, *id).map(|(p, v)| format!("{id}:{:.2},{:.2} v{:.3}", p.x, p.y, v.x))).collect();
            out.push(format!("T+{t}{}: {}", if powered { " P" } else { "" }, st.join(" ")));
        }
    }
    // (the report)
    let mut exact = 0usize;
    for (i, r) in rels.iter().enumerate() {
        let n_left = r.left.len();
        if n_left == o.release {
            exact += 1;
        }
        let gaps: Vec<i64> = r.plane.windows(2).map(|w| w[1].1 - w[0].1).collect();
        // (the cut window: from cart K leaving the foot to cart K+1's snap onto it)
        let window = match (r.left.get(o.release - 1), r.snaps.get(o.release - 1)) {
            (Some((_, tl)), Some((_, ts))) => format!("cart {} left the foot at T+{tl}, cart {} snapped onto it at T+{ts}: the cut must fall in [T+{tl}, T+{ts}) ({} gt)", o.release, o.release + 1, ts - tl),
            _ => "-".to_string(),
        };
        out.push(format!(
            "release {i}: the foot powered T+{}..{} ({} gt; detector rises {}, ring edges {}); kicks {}; left the foot {}; DEPARTURES {}{}; at the plane x {:.1}: {} (gaps {}); snaps onto the foot {}; {}; carts in the shaft after {}",
            r.start,
            r.off.map_or("never cut".to_string(), |x| format!("T+{x}")),
            r.off.map_or("-".to_string(), |x| (x - r.start).to_string()),
            r.det_rises,
            r.edges,
            r.kicks.iter().map(|(_, t)| format!("T+{t}")).collect::<Vec<_>>().join(", "),
            r.left.iter().map(|(_, t)| format!("T+{t}")).collect::<Vec<_>>().join(", "),
            n_left,
            if n_left == o.release { "" } else { " (NOT the release size)" },
            o.plane,
            r.plane.iter().map(|(_, t)| format!("T+{t}")).collect::<Vec<_>>().join(", "),
            if gaps.is_empty() { "-".to_string() } else { gaps.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(", ") },
            r.snaps.iter().map(|(_, t)| format!("T+{t}")).collect::<Vec<_>>().join(", "),
            window,
            r.stack_after.map_or("-".to_string(), |n| n.to_string())
        ));
    }
    if o.refill_y.is_some() {
        out.push(format!(
            "refill drops (the lift's delivery emulated: dropped from y {:.2}; the safe drop 3.0): {}{}",
            o.refill_y.unwrap_or(0.0),
            if refills.is_empty() { "none".to_string() } else { refills.iter().map(|(t, h)| format!("T+{t} {h:.2}{}", if *h > 3.0 { " (OVER 3)" } else { "" })).collect::<Vec<_>>().join(", ") },
            if refill_log.is_empty() { String::new() } else { format!("; {}", refill_log.join("; ")) }
        ));
    }
    if !freeze_note.is_empty() {
        out.push(format!("freeze / reload: {}", freeze_note.join("; ")));
    }
    out.push(format!(
        "over the run: releases with exactly {} departures {} of {}; rider damage {}{}; merges {}{}; carts through the sink {}{}",
        o.release,
        exact,
        rels.len(),
        hurt.len(),
        if hurt.is_empty() { String::new() } else { format!(" ({})", hurt.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
        merges.len(),
        if merges.is_empty() { String::new() } else { format!(" ({})", merges.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        sunk,
        if stray.is_empty() { String::new() } else { format!("; {}", stray.join("; ")) }
    ));
    if !run.world.errors.is_empty() {
        out.push(format!("the world's errors: {}", run.world.errors.join("; ")));
    }
    Ok(out)
}
