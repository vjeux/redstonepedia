//! One-chunk (b): the climb bench (ironspec and farmopt 2026-11-11, paper v0 "Iron carousel in one
//! chunk"): one batch of 3 ridden carts from rest at the foot hold H, up a straight powered climb of
//! `cells` boosters (no curves) to today's approach top T, down today's two slopes and the snap
//! turn into today's 3 pockets, every cell as the S3 file lays it (s3-wired-pair-c-a: the design
//! frame = the loop's + (6,0,6), the approach along x 3 heading -z, the stand's strip along z 6
//! heading +x). The pockets' gates bench-driven by position triggers as the queue-loop bench's
//! stand-stop (gate k from the east lets k carts by, shutting on its k-th leaving cart's centre past
//! the gate cell + `close_at`), H a fence gate the bench opens at T + `h_open` (T: a world time that
//! is a multiple of 100, the summon tick). Printed: each cart's tick at rest (from T) and its margin
//! before T + 100, each arrival into a waiting cart's speed, the riders' damage, dismounts and eye
//! point inside a collision shape, each rider's least box gap to the golem box while a golem
//! exists (T..T+2), and with `freeze`, the bench's chunks at level 32 (redstone running, entities
//! frozen) for a span and what each cart did.

use crate::layouts::Builder;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// The x of the approach column and the strip's z (the design frame).
const AX: i32 = 3;
const SZ: i32 = 6;

/// The stand's strip as the S3 file lays it: (x, y, state) on z 6, with each gate's block over its
/// slope's high end.
const STRIP: [(i32, i32, &str); 13] = [
    (3, 2, "rail[shape=ascending_west,waterlogged=false]"),
    (4, 1, "rail[shape=ascending_west,waterlogged=false]"),
    (5, 0, "rail[shape=ascending_west,waterlogged=false]"),
    (6, -1, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (7, -1, "rail[shape=ascending_east,waterlogged=false]"),
    (8, 0, "rail[shape=east_west,waterlogged=false]"),
    (9, -1, "rail[shape=ascending_west,waterlogged=false]"),
    (10, -2, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (11, -2, "rail[shape=ascending_east,waterlogged=false]"),
    (12, -1, "detector_rail[powered=false,shape=east_west,waterlogged=false]"),
    (13, -2, "rail[shape=ascending_west,waterlogged=false]"),
    (14, -3, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (15, -3, "rail[shape=east_west,waterlogged=false]"),
];
/// The pockets' gates (x, y of the gate block) on z 6, west to east: G2, G1, G0.
const GATES: [(i32, i32); 3] = [(6, 0), (10, -1), (14, -2)];
const GATE_SHUT: &str = "oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]";
const GATE_OPEN: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=true]";
const H_SHUT: &str = "oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]";
const H_OPEN: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=true]";

/// T's cell z for a crest at height `crest` (today's 5: T at z 9, two slopes down to the snap turn;
/// 3, the lowest with today's snap turn (its high end at 3): T right before it, at z 7).
fn crest_z(crest: i32) -> i32 {
    7 + (crest - 3)
}

/// The climb's foot slope (its lowest booster) for `cells` boosters ending at T.
fn foot(cells: i32, crest: i32) -> (i32, i32) {
    (crest - cells, crest_z(crest) + cells)
}

/// The bench's design: H's queue (3 slopes down north), H's slope and gate, the climb, T, the
/// approach's two slopes down, the snap turn, the strip and its gates; a marker block in the chunk
/// west of the bench (the forced box's spare chunk, so a freeze leaves the bench at level 32).
pub fn design(cells: i32, with_riders: [bool; 3]) -> String {
    design_with(cells, 5, "", with_riders, false)
}

/// The bench's design with a climb pattern (one letter per booster cell from the foot: `B` a
/// booster on a redstone block, `P` a plain slope on glass; empty: all boosters) and, with
/// `spaced`, the 2nd and 3rd carts parked off the track (x -6..-4, the spare chunk) for the bench
/// to set at H one at a time (the per-gate hold's release emulated: a cart at rest at Ga every S gt).
pub fn design_with(cells: i32, crest: i32, pattern: &str, with_riders: [bool; 3], spaced: bool) -> String {
    let mut b = Builder::new("climb-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    // the gates: G2 and G1 open (their slopes powered from their sources, as the meter powers an
    // open gate's slope 8 gt after it opens), G0 shut (its source, beside its slope, stone)
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // T and the approach's slopes down to the snap turn (today's crest 5: two)
    let zt = crest_z(crest);
    b.set(AX, crest, zt, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, crest - 1, zt, "glass");
    for k in 1..=(crest - 3) {
        b.set(AX, crest - k, zt - k, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, crest - k - 1, zt - k, "glass");
    }
    // the climb: `cells` boosters ascending north on redstone blocks, the top one rising to T
    let (fy, fz) = foot(cells, crest);
    for k in 0..cells {
        let plain = pattern.chars().nth(k as usize) == Some('P');
        if plain {
            b.set(AX, fy + k, fz - k, "rail[shape=ascending_north,waterlogged=false]");
            b.set(AX, fy + k - 1, fz - k, "glass");
        } else {
            b.set(AX, fy + k, fz - k, "powered_rail[powered=true,shape=ascending_north,waterlogged=false]");
            b.set(AX, fy + k - 1, fz - k, "redstone_block");
        }
    }
    // H: a booster slope down north into the climb's foot (a cart let go from rest gets going at
    // once), its gate over the high end; the queue: three slopes down north and a flat, then a stop
    b.set(AX, fy, fz + 1, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
    b.set(AX, fy - 1, fz + 1, "redstone_block");
    b.set(AX, fy + 1, fz + 1, H_SHUT);
    for k in 0..3 {
        b.set(AX, fy + 1 + k, fz + 2 + k, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, fy + k, fz + 2 + k, "glass");
    }
    b.set(AX, fy + 4, fz + 5, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, fy + 3, fz + 5, "glass");
    b.set(AX, fy + 4, fz + 6, "stone");
    // the 3 carts on the queue's slopes, front first
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        if spaced && k > 0 {
            // (parked: a flat rail on glass in the spare chunk, far from the bench)
            b.set(-6, 0, SZ + 3 * ki, "rail[shape=north_south,waterlogged=false]");
            b.set(-6, -1, SZ + 3 * ki, "glass");
            b.entity("minecart", [-5.5, 0.0625, (SZ + 3 * ki) as f64 + 0.5], Some(nbt));
        } else {
            b.entity("minecart", [AX as f64 + 0.5, (fy + 1 + ki) as f64 + 0.6, (fz + 2 + ki) as f64 + 0.5], Some(nbt));
        }
    }
    // the spare chunk's marker (x -8: chunk -1)
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["One-chunk (b): the climb bench: H's queue, a straight powered climb to today's approach top, the snap turn and today's 3 pockets (gates bench-driven)."])
}

/// One climb run's options.
#[derive(Clone, Debug)]
pub struct ClimbOpts {
    pub cells: i32,
    /// T's height (today's 5; 3 the lowest with today's snap turn).
    pub crest: i32,
    /// H opens at T + this (gt).
    pub h_open: i64,
    /// H shuts again this many gt after it opened (0: never).
    pub h_for: i64,
    /// The per-gate hold's release emulated (with a `spaced` design): cart k set at rest at Ga
    /// (cart 0's rest spot against H at T) at T + h_open + k * h_every, H open from T + h_open.
    pub h_every: i64,
    /// A gate shuts once a leaving cart's centre is this far past its cell's west edge.
    pub close_at: f64,
    /// The run's length after T (gt).
    pub ticks: i64,
    pub seed: i64,
    /// The golem's box (design frame, x0,y0,z0,x1,y1,z1) while a golem exists (T..T+2).
    pub golem_box: Option<[f64; 6]>,
    /// The freeze: the bench's chunks at level 32 from T + FROM for LEN gt.
    pub freeze: Option<(i64, i64)>,
    pub trace: bool,
}

/// One climb run's report lines.
pub fn bench(spec: &FarmSpec, o: &ClimbOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
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
    // (front first: the spawn order is the queue's, front first)
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_shut, g_open, h_shut, h_open) = (parse(GATE_SHUT)?, parse(GATE_OPEN)?, parse(H_SHUT)?, parse(H_OPEN)?);
    let stone = parse("stone")?;
    let (fy, fz) = foot(o.cells, o.crest);
    let zt = crest_z(o.crest);
    let h_cell = BlockPos::new(AX, fy + 1, fz + 1);
    // the warm-up: the carts settle against H; T: the first world time that is a multiple of 100,
    // at least 60 gt on
    let mut t0 = 0i64;
    for t in 0..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            t0 = t;
            break;
        }
    }
    let mut out: Vec<String> = Vec::new();
    // (the per-gate hold emulated: cart 0's rest spot against H at T, where carts 1 and 2 are set
    // at rest later)
    let ga_spot: Option<Vec3> = run.world.entities.list.iter().find(|e| e.id == carts[0]).map(|e| e.pos);
    let place_at = |w: &mut redsim::world::World, id: u32, p: Vec3| {
        let Some(i) = w.entities.list.iter().position(|e| e.id == id) else { return };
        w.entities.set_pos(i, p);
        w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for r in ps {
            if let Some(j) = w.entities.list.iter().position(|e| e.id == r) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    };
    let mut open = [true, true, false];
    let mut seen = [[false; 3]; 3];
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut touching = [[false; 3]; 3];
    let mut prev_v = [0.0f64; 3];
    let mut eye_ticks = [0u32; 3];
    let mut hurt: Vec<String> = Vec::new();
    let mut health: [Option<f32>; 3] = [None; 3];
    let mut dismount: Vec<String> = Vec::new();
    let mut golem_gap = [f64::INFINITY; 3];
    let mut top_v = [0.0f64; 3];
    let mut crest_at: [Option<i64>; 3] = [None; 3];
    // (the least distance between each cart and the one it follows, front cart k and back cart
    // k+1, while the back one is still on the approach (off the strip): a catch is under 1.0)
    let mut least_gap = [f64::INFINITY; 2];
    let h_feet: Option<f64> = riders[0].and_then(|r| run.world.entities.list.iter().find(|e| e.id == r).map(|e| e.pos.y));
    let mut froze: Vec<String> = Vec::new();
    let mut freeze_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    let chunks = {
        let xs: Vec<i32> = [2i32, 16].iter().map(|x| x >> 4).collect();
        let zs: Vec<i32> = [SZ - 1, fz + 6].iter().map(|z| z >> 4).collect();
        [xs[0], zs[0], xs[1].max(xs[0]), zs[1].max(zs[0])]
    };
    for dt in 0..o.ticks {
        // H: opened at T + h_open, shut again after h_for
        if dt == o.h_open {
            run.world.set_block(h_cell, h_open.clone(), redsim::world::flags::ALL);
        }
        if o.h_for > 0 && dt == o.h_open + o.h_for {
            run.world.set_block(h_cell, h_shut.clone(), redsim::world::flags::ALL);
        }
        // the per-gate hold's next cart at rest at Ga every h_every gt
        if o.h_every > 0
            && let Some(p) = ga_spot
        {
            for k in 1..3i64 {
                if dt == o.h_open + k * o.h_every {
                    place_at(&mut run.world, carts[k as usize], p);
                }
            }
        }
        // the freeze: the bench's chunks unforced (level 32: blocks tick, entities don't)
        if let Some((from, len)) = o.freeze {
            if dt == from {
                // (a bench world has no forced chunk box: the chunks its box meets, the marker's
                // spare chunk west of the bench among them)
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
        }
        // the pockets' gates (bench position triggers)
        for (k, (gx, _)) in GATES.iter().rev().enumerate() {
            if k == 0 || !open[2 - k] {
                continue;
            }
            let edge = *gx as f64 + o.close_at;
            for (n, id) in carts.iter().enumerate() {
                if let Some((p, _)) = pos(&run.world, *id)
                    && (p.z - (SZ as f64 + 0.5)).abs() < 0.6
                    && p.x >= edge
                    && !seen[k][n]
                {
                    seen[k][n] = true;
                    if seen[k].iter().filter(|b| **b).count() >= k {
                        open[2 - k] = false;
                    }
                }
            }
        }
        for (i, (gx, gy)) in GATES.iter().enumerate() {
            let want = if open[i] { g_open.clone() } else { g_shut.clone() };
            let p = BlockPos::new(*gx, *gy, SZ);
            if run.world.get(p) != want {
                run.world.set_block(p, want, redsim::world::flags::ALL);
                // (a shut gate's slope unpowered: its source back to stone)
                if !open[i] && i < 2 {
                    let src = if i == 0 { BlockPos::new(6, -2, SZ) } else { BlockPos::new(10, -3, SZ) };
                    run.world.set_block(src, stone.clone(), redsim::world::flags::ALL);
                }
            }
        }
        run.tick();
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        for k in 0..3 {
            let Some((p, v)) = st[k] else { continue };
            let s = (v.x * v.x + v.z * v.z).sqrt();
            top_v[k] = top_v[k].max(s);
            if crest_at[k].is_none() && (p.x - (AX as f64 + 0.5)).abs() < 0.6 && p.z < zt as f64 + 0.5 && p.y > o.crest as f64 - 0.1 {
                crest_at[k] = Some(dt);
            }
            if s < 1e-3 && (p.z - (SZ as f64 + 0.5)).abs() < 0.6 {
                rest_since[k].get_or_insert(dt);
            } else {
                rest_since[k] = None;
            }
        }
        for k in 0..2 {
            if let (Some((pa, _)), Some((pb, _))) = (st[k], st[k + 1])
                && (pb.z - (SZ as f64 + 0.5)).abs() > 0.6
                && pb.x < AX as f64 + 1.0
                && pb.z < fz as f64 + 1.0
            {
                let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt();
                least_gap[k] = least_gap[k].min(d);
            }
        }
        // arrivals into a waiting cart: two carts' centres within 1.0 (their boxes touching), the
        // slower at under 0.05: the other's stored speed the tick before
        for a in 0..3 {
            for c in 0..3 {
                if a == c {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                let now = d < 1.0;
                if now && !touching[a][c] && sc < 0.05 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: cart {a} into waiting cart {c} at {sa:.3}"));
                }
                touching[a][c] = now;
            }
        }
        for k in 0..3 {
            prev_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
        }
        // the riders: damage, dismounts, the eye point inside a collision shape, the golem box gap
        for k in 0..3 {
            let Some(r) = riders[k] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                dismount.push(format!("T+{dt}: cart {k}'s rider gone"));
                continue;
            };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if let Some(h0) = health[k]
                && hp < h0
            {
                hurt.push(format!("T+{dt}: cart {k}'s rider {h0} -> {hp}"));
            }
            health[k] = Some(hp);
            if e.vehicle.is_none() && !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                dismount.push(format!("T+{dt}: cart {k}'s rider not seated"));
            }
            let (ex, ey, ez) = (e.pos.x, e.pos.y + 1.62, e.pos.z);
            let mut blind = false;
            for y in ey.floor() as i32 - 1..=ey.floor() as i32 + 1 {
                for z in ez.floor() as i32 - 1..=ez.floor() as i32 + 1 {
                    for x in ex.floor() as i32 - 1..=ex.floor() as i32 + 1 {
                        let bp = BlockPos::new(x, y, z);
                        let s = w.get(bp);
                        if s.is_air() {
                            continue;
                        }
                        for bx in redsim::entity::collide::boxes_at(w, &s, bp) {
                            if bx.min_x < ex && ex < bx.max_x && bx.min_y < ey && ey < bx.max_y && bx.min_z < ez && ez < bx.max_z {
                                blind = true;
                            }
                        }
                    }
                }
            }
            if blind {
                eye_ticks[k] += 1;
            }
            if let Some(g) = o.golem_box
                && dt <= 2
            {
                // (the villager's box: 0.6 wide, 1.95 tall; the gap along the axis that separates
                // the boxes most)
                let vb = [e.pos.x - 0.3, e.pos.y, e.pos.z - 0.3, e.pos.x + 0.3, e.pos.y + 1.95, e.pos.z + 0.3];
                let gap = (0..3).map(|i| (g[i] - vb[i + 3]).max(vb[i] - g[i + 3]).max(0.0)).fold(0.0, f64::max);
                golem_gap[k] = golem_gap[k].min(gap);
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
    }
    // the freeze's effect: each cart's move over the frozen span
    if freeze_pos.len() == 2 {
        for k in 0..3 {
            if let (Some(a), Some(b)) = (freeze_pos[0][k], freeze_pos[1][k]) {
                froze.push(format!("cart {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2}", a.x, a.y, a.z, b.x, b.y, b.z));
            }
        }
    }
    let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(&run.world, *id)).collect();
    let rest: Vec<String> = (0..3)
        .map(|k| {
            let p = st[k].map_or("gone".to_string(), |(p, _)| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z));
            match rest_since[k] {
                Some(r) => format!("cart {k}{} at rest from T+{r} (margin {} gt before T+100) at {p}, crest T+{}, top speed {:.3}", if riders[k].is_some() { "" } else { " (EMPTY)" }, 100 - r, crest_at[k].map_or("-".into(), |c| c.to_string()), top_v[k]),
                None => format!("cart {k}{} NOT at rest by T+{} ({p}), crest T+{}", if riders[k].is_some() { "" } else { " (EMPTY)" }, o.ticks, crest_at[k].map_or("-".into(), |c| c.to_string())),
            }
        })
        .collect();
    out.push(format!(
        "climb bench (H opens at T+{}{}; {} cells, crest {}; H's rider feet at y {}): {}",
        o.h_open,
        if o.h_every > 0 { format!(", a cart every {} gt", o.h_every) } else { String::new() },
        o.cells,
        o.crest,
        h_feet.map_or("-".into(), |y| format!("{y:.2}")),
        rest.join("; ")
    ));
    out.push(format!(
        "the least distance to the cart ahead before the strip: cart 1 {}, cart 2 {} (a catch under 1.0); crest gaps {}",
        if least_gap[0].is_finite() { format!("{:.2}", least_gap[0]) } else { "-".into() },
        if least_gap[1].is_finite() { format!("{:.2}", least_gap[1]) } else { "-".into() },
        match (crest_at[0], crest_at[1], crest_at[2]) { (Some(a), Some(b), Some(c)) => format!("{}, {} gt", b - a, c - b), _ => "-".into() }
    ));
    let last = rest_since.iter().map(|r| r.unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    out.push(format!(
        "batch at rest: {}; arrivals into a waiting cart: {} (the fastest {:.3}; bar 0.2){}",
        if last == i64::MAX { "NO".to_string() } else { format!("T+{last}, margin {} gt before T+100", 100 - last) },
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    out.push(format!(
        "riders: hurt {}{}; dismounts {}{}; eye point inside a collision shape (ticks per cart) {:?}{}",
        hurt.len(),
        if hurt.is_empty() { String::new() } else { format!(" ({})", hurt.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        dismount.len(),
        if dismount.is_empty() { String::new() } else { format!(" ({})", dismount.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        eye_ticks,
        if o.golem_box.is_some() { format!("; the least box gap to the golem box while it exists (T..T+2), per rider: {}", golem_gap.iter().map(|g| if g.is_finite() { format!("{g:.2}") } else { "-".into() }).collect::<Vec<_>>().join(", ")) } else { String::new() }
    ));
    if let Some((from, len)) = o.freeze {
        out.push(format!("freeze T+{from}..T+{} (the bench's chunks {:?} at level 32; the world's errors: {}): {}", from + len, chunks, if run.world.errors.is_empty() { "none".to_string() } else { run.world.errors.join("; ") }, froze.join("; ")));
    }
    let _ = t0;
    Ok(out)
}

/// The descent bench's design (ironspec 2026-11-11, one-chunk (b) 3): today's 3 pockets with the
/// batch at rest in them (the S3 file's rest positions), G0's corner south, a straight descent of
/// `pattern.len()` slopes down south along x 15 (each `B` a booster on a redstone block, `P` a
/// plain slope on glass), then R1's entry: a brake (an unpowered powered rail, flat), `flats` plain
/// flats and a shut fence gate (the hold's stop).
pub fn descent_design(pattern: &str, flats: i32, with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("descent-bench");
    for (x, y, s) in STRIP.iter().take(12) {
        b.set(*x, *y, SZ, s);
        b.set(*x, *y - 1, SZ, "glass");
    }
    for (x, y) in GATES {
        b.set(x, y, SZ, GATE_SHUT);
    }
    // (the gate slopes' power cells, stone at rest (the meter's sources): G0's beside its slope,
    // G1's and G2's under theirs)
    for (x, y, z) in [(14, -3, 7), (10, -3, 6), (6, -2, 6)] {
        b.set(x, y, z, "stone");
    }
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // G0's corner, then the descent south along x 15
    b.set(15, -3, SZ, "rail[shape=south_west,waterlogged=false]");
    b.set(15, -4, SZ, "glass");
    let n = pattern.len() as i32;
    for (k, ch) in pattern.chars().enumerate() {
        let k = k as i32;
        let (y, z) = (-4 - k, SZ + 1 + k);
        if ch == 'B' {
            b.set(15, y, z, "powered_rail[powered=true,shape=ascending_north,waterlogged=false]");
            b.set(15, y - 1, z, "redstone_block");
        } else {
            b.set(15, y, z, "rail[shape=ascending_north,waterlogged=false]");
            b.set(15, y - 1, z, "glass");
        }
    }
    // R1's entry: the brake, the flats, the stop
    let (ey, ez) = (-3 - n, SZ + 1 + n);
    b.set(15, ey, ez, "powered_rail[powered=false,shape=north_south,waterlogged=false]");
    b.set(15, ey - 1, ez, "glass");
    for f in 0..flats {
        b.set(15, ey, ez + 1 + f, "rail[shape=north_south,waterlogged=false]");
        b.set(15, ey - 1, ez + 1 + f, "glass");
    }
    b.set(15, ey, ez + 1 + flats, "oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]");
    b.set(15, ey - 1, ez + 1 + flats, "glass");
    // the batch at rest in its pockets (P1, P2, P3: the S3 file's rest positions), front first
    for (k, (x, y)) in [(13.885, -1.8225), (9.885, -0.8225), (5.885, 0.1775)].iter().enumerate() {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        b.entity("minecart", [*x, *y, SZ as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["One-chunk (b): the descent bench: today's 3 pockets with a batch at rest, G0's corner, a straight descent and R1's entry (a brake, flats, a stop)."])
}

/// One descent run's options.
#[derive(Clone, Debug)]
pub struct DescentOpts {
    pub n: i32,
    pub flats: i32,
    /// REL at T + this: G0 and G1 open (G0's slope powered at once, G1's 8 gt after), G2 `g2_delay`
    /// gt later (its slope 8 gt after).
    pub rel: i64,
    pub g2_delay: i64,
    pub ticks: i64,
    pub seed: i64,
    pub freeze: Option<(i64, i64)>,
    pub trace: bool,
}

/// One descent run's report lines: each cart's speed at the brake's first cell (its stored speed
/// the tick it enters the cell), each contact into a waiting cart, off-rail carts, the riders.
pub fn descent_bench(spec: &FarmSpec, o: &DescentOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
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
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_shut, g_open, rs, stone) = (parse(GATE_SHUT)?, parse(GATE_OPEN)?, parse("redstone_block")?, parse("stone")?);
    for _ in 0..400 {
        run.tick();
        if run.world.time % 100 == 0 {
            break;
        }
    }
    let (ey, ez) = (-3 - o.n, SZ + 1 + o.n);
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let src = [BlockPos::new(14, -3, 7), BlockPos::new(10, -3, 6), BlockPos::new(6, -2, 6)];
    // (GATES west to east is G2, G1, G0; the departure order G0, G1, G2)
    let gate_at = [BlockPos::new(GATES[2].0, GATES[2].1, SZ), BlockPos::new(GATES[1].0, GATES[1].1, SZ), BlockPos::new(GATES[0].0, GATES[0].1, SZ)];
    let opens = [o.rel, o.rel, o.rel + o.g2_delay];
    let mut at_brake: [Option<(i64, f64)>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 3];
    let mut touching = [[false; 3]; 3];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut off_rail: Vec<String> = Vec::new();
    let mut hurt: Vec<String> = Vec::new();
    let mut health: [Option<f32>; 3] = [None; 3];
    let mut dismount: Vec<String> = Vec::new();
    let mut corner_at: [Option<i64>; 3] = [None; 3];
    let mut out: Vec<String> = Vec::new();
    let mut freeze_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    let chunks = [0, 0, 0, (ez + 4) >> 4];
    for dt in 0..o.ticks {
        for k in 0..3 {
            if dt == opens[k] {
                run.world.set_block(gate_at[k], g_open.clone(), redsim::world::flags::ALL);
            }
            if dt == opens[k] + if k == 0 { 0 } else { 8 } {
                run.world.set_block(src[k], rs.clone(), redsim::world::flags::ALL);
            }
        }
        let _ = (&g_shut, &stone);
        if let Some((from, len)) = o.freeze {
            if dt == from {
                // (a bench world has no forced chunk box: the chunks its box meets, the marker's
                // spare chunk west of the bench among them)
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
        }
        run.tick();
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        for k in 0..3 {
            let Some((p, v)) = st[k] else {
                if !off_rail.iter().any(|s| s.contains(&format!("cart {k} "))) {
                    off_rail.push(format!("cart {k} gone at T+{dt}"));
                }
                continue;
            };
            let s = (v.x * v.x + v.z * v.z).sqrt();
            let cell = BlockPos::new(p.x.floor() as i32, (p.y + 0.01).floor() as i32, p.z.floor() as i32);
            if corner_at[k].is_none() && cell.x == 15 && cell.z == SZ {
                corner_at[k] = Some(dt);
            }
            if at_brake[k].is_none() && cell.x == 15 && cell.z == ez && (p.y - ey as f64).abs() < 0.6 {
                at_brake[k] = Some((dt, prev_v[k].max(s)));
            }
            // (off the rails: no rail at the cart's cell or the one below)
            let rail_here = |c: BlockPos| w.get(c).to_string().contains("rail");
            if !rail_here(cell) && !rail_here(BlockPos::new(cell.x, cell.y - 1, cell.z)) && !off_rail.iter().any(|s| s.contains(&format!("cart {k} "))) {
                off_rail.push(format!("cart {k} off the rails at T+{dt} ({:.2},{:.2},{:.2})", p.x, p.y, p.z));
            }
        }
        for a in 0..3 {
            for c in 0..3 {
                if a == c {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                let now = d < 1.0;
                if now && !touching[a][c] && sc < 0.05 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: cart {a} into waiting cart {c} at {sa:.3} ({:.2},{:.2},{:.2})", pa.x, pa.y, pa.z));
                }
                touching[a][c] = now;
            }
        }
        for k in 0..3 {
            prev_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
        }
        for k in 0..3 {
            let Some(r) = riders[k] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                if !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                    dismount.push(format!("T+{dt}: cart {k}'s rider gone"));
                }
                continue;
            };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if let Some(h0) = health[k]
                && hp < h0
            {
                hurt.push(format!("T+{dt}: cart {k}'s rider {h0} -> {hp}"));
            }
            health[k] = Some(hp);
            if e.vehicle.is_none() && !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                dismount.push(format!("T+{dt}: cart {k}'s rider not seated"));
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
    }
    let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(&run.world, *id)).collect();
    out.push(format!(
        "descent bench ({} slopes; REL T+{}, G2 +{}): at the brake's first cell {}",
        o.n,
        o.rel,
        o.g2_delay,
        (0..3).map(|k| format!("cart {k}{} {}", if riders[k].is_some() { "" } else { " (EMPTY)" }, at_brake[k].map_or("never".to_string(), |(t, v)| format!("T+{t} at {v:.3}")))).collect::<Vec<_>>().join(", ")
    ));
    let mut gaps: Vec<String> = Vec::new();
    let mut ab: Vec<(i64, usize)> = (0..3).filter_map(|k| at_brake[k].map(|(t, _)| (t, k))).collect();
    ab.sort();
    for w2 in ab.windows(2) {
        gaps.push(format!("{}", w2[1].0 - w2[0].0));
    }
    out.push(format!(
        "corner passes {:?}; brake arrivals gt apart {}; contacts into a waiting cart: {} (the fastest {:.3}; bar 0.2){}; off the rails: {}; riders hurt {}, dismounts {}; at the end: {}",
        corner_at,
        if gaps.is_empty() { "-".into() } else { gaps.join(", ") },
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
        if off_rail.is_empty() { "none".to_string() } else { off_rail.join("; ") },
        hurt.len(),
        dismount.len(),
        st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")
    ));
    if freeze_pos.len() == 2 {
        let f = o.freeze.unwrap_or((0, 0));
        out.push(format!("freeze T+{}..T+{} (chunks {:?} at level 32): {}", f.0, f.0 + f.1, chunks, (0..3).map(|k| match (freeze_pos[0][k], freeze_pos[1][k]) { (Some(a), Some(b)) => format!("cart {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2}", a.x, a.y, a.z, b.x, b.y, b.z), _ => format!("cart {k} -") }).collect::<Vec<_>>().join("; ")));
    }
    Ok(out)
}
