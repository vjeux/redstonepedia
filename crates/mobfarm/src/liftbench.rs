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
    for y in -1..=h + 1 {
        for (x, z) in [(0, -1), (0, 1)] {
            blocks.push((BlockPos::new(x, y, z), glass.clone()));
        }
        // (the west wall stops under the exit rail's cell)
        if !(exit_rail && y >= h) {
            blocks.push((BlockPos::new(-1, y, 0), glass.clone()));
        }
        if y >= o.entry_h || y < 0 {
            blocks.push((BlockPos::new(1, y, 0), glass.clone()));
        }
    }
    for x in 1..=o.entry_len {
        blocks.push((BlockPos::new(x, 0, 0), flat.clone()));
        blocks.push((BlockPos::new(x, -1, 0), glass.clone()));
    }
    if exit_rail || o.exit == "current" || o.exit == "piston" {
        for x in -8..=-1 {
            blocks.push((BlockPos::new(x, h, 0), flat.clone()));
            blocks.push((BlockPos::new(x, h - 1, 0), glass.clone()));
        }
        // (the exit line's end: a glass stop, the carts queueing against it)
        blocks.push((BlockPos::new(-9, h, 0), glass.clone()));
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
        lo: BlockPos::new(-20, -4, -4),
        hi: BlockPos::new(o.entry_len + 4, h + 8, 4),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
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
    end: Option<(f64, f64, f64, f64, f64)>,
    launch: Option<i64>,
    /// The rider's feet 11.95 above the entry rail's (the summoners' level), live gt from launch.
    reach: Option<i64>,
    /// Landed on the exit rail (x < 0, on the rail's level): live gt from launch, the speed then.
    exit: Option<(i64, f64)>,
    launch_live: i64,
}

/// One bench run's report lines.
pub fn lift_bench(o: &LiftOpts) -> Result<String, String> {
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
    let mut logs: Vec<CartLog> = (0..o.carts).map(|_| CartLog { id: None, rider: None, met_column: None, top_y: f64::NEG_INFINITY, top_at: None, least_air: i32::MAX, dismounted: None, hurt: 0.0, eye_in_block: 0, end: None, launch: None, reach: None, exit: None, launch_live: 0 }).collect();
    let mut health0: Vec<Option<f32>> = vec![None; o.carts];
    let mut column_gap = f64::INFINITY;
    let mut contact_max = 0.0f64;
    let mut contact_at: Option<(i64, usize, usize, f64, f64)> = None;
    let mut piston_fired: Vec<(i64, usize, f64, f64)> = Vec::new();
    let mut piston_until: Option<i64> = None;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let glass_b = Block::parse("minecraft:glass")?;
    let mut frozen_ticks = 0i64;
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
                redsim::reload::unload_some(&mut run.world, [-1, -1, 0, 0]);
            }
            if t == from + len {
                redsim::reload::load_some(&mut run.world, [-1, -1, 0, 0]);
            }
        }
        if frozen_now {
            frozen_ticks += 1;
        }
        let live = t - frozen_ticks;
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
            if lg.id.is_none() && !frozen_now && live >= k as i64 * o.spacing {
                let nbt = format!("{{Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", -o.speed);
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [o.entry_len as f64 - 0.5, 0.0625, 0.5], Some(&nbt))?;
                lg.id = Some(id);
                lg.launch = Some(t);
                lg.launch_live = live;
                lg.rider = redsim::entity::index_of(&run.world, id).and_then(|i| run.world.entities.list[i].passengers.first().copied());
            }
        }
        run.tick();
        // (cart pairs: the closest centres in the shaft's column (a rising cart under a bobbing
        // one) and every contact's relative speed (centres within 1.0, closing))
        let st: Vec<Option<(f64, f64, f64, f64, f64)>> = logs.iter().map(|lg| lg.id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| { let e = &run.world.entities.list[i]; (e.pos.x, e.pos.y, e.delta_movement.x, e.delta_movement.y, e.pos.z) })).collect();
        for a in 0..st.len() {
            for b in a + 1..st.len() {
                if let (Some(pa), Some(pb)) = (st[a], st[b]) {
                    let d = ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt();
                    if pa.0 > -0.1 && pa.0 < 1.0 && pb.0 > -0.1 && pb.0 < 1.0 {
                        column_gap = column_gap.min(d);
                    }
                    let rel = ((pa.2 - pb.2).powi(2) + (pa.3 - pb.3).powi(2)).sqrt();
                    if d < 1.0 && rel > contact_max {
                        contact_max = rel;
                        contact_at = Some((t, a, b, pa.0, pa.1));
                    }
                }
            }
        }
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
            if lg.exit.is_none() && p.x < 0.0 && (p.y - (o.height as f64 + 0.0625)).abs() < 0.01 {
                lg.exit = Some((live - lg.launch_live, (v.x * v.x + v.z * v.z).sqrt()));
            }
            let mut air = None;
            if let Some(r) = lg.rider
                && let Some(j) = redsim::entity::index_of(&run.world, r)
            {
                let re = &run.world.entities.list[j];
                if lg.reach.is_none() && re.pos.y >= 0.0625 + 0.4 + 11.95 {
                    lg.reach = Some(live - lg.launch_live);
                }
                if let EntityKind::Mob(m) = &re.kind {
                    air = Some(m.air);
                    lg.least_air = lg.least_air.min(m.air);
                    let hp = m.health;
                    let h0 = *health0[k].get_or_insert(hp);
                    lg.hurt = lg.hurt.max(h0 - hp);
                }
                let eye = redsim::pos::BlockPos::new(re.pos.x.floor() as i32, (re.pos.y + 1.62).floor() as i32, re.pos.z.floor() as i32);
                // (tick-exact: glass never suffocates; only another block with a collision shape
                // under the eye counts)
                let eb = run.world.get(eye);
                if !redsim::behavior::collision_boxes(&eb).is_empty() && !eb.to_string().contains("glass") {
                    lg.eye_in_block += 1;
                }
            }
            if o.trace == Some(k) {
                out.push(format!("  t {t}: cart {k} at {:.3},{:.3},{:.3} v {:.3},{:.3},{:.3} in {} air {} seated {seated}", p.x, p.y, p.z, v.x, v.y, v.z, here.to_string().trim_start_matches("minecraft:"), air.map_or("-".into(), |a| a.to_string())));
            }
        }
    }
    for (k, lg) in logs.iter().enumerate() {
        out.push(format!(
            "cart {k}: met the column at {}; top y {:.3} at {}; on the exit rail {}; the rider's feet 11.95 over the entry's {}; ends at {}; the rider's least air {}; {}; hurt {:.1}; eye-in-block ticks {}",
            lg.met_column.map_or("never".into(), |t| format!("t {t}")),
            lg.top_y,
            lg.top_at.map_or("-".into(), |t| format!("t {t}")),
            lg.exit.map_or("NEVER".into(), |(d, s)| format!("{d} live gt after launch at {s:.3}")),
            lg.reach.map_or("never".into(), |d| format!("{d} live gt after launch")),
            lg.end.map_or("-".into(), |(x, y, z, s, vy)| format!("{x:.3},{y:.3},{z:.3} (horizontal {s:.3}, vertical {vy:.3})")),
            if lg.least_air == i32::MAX { "-".into() } else { lg.least_air.to_string() },
            lg.dismounted.map_or("seated to the end".into(), |t| format!("DISMOUNTED at t {t}")),
            lg.hurt,
            lg.eye_in_block
        ));
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
    out.push(format!(
        "pairs: the closest centres in the shaft's column {}; the hardest contact (centres within 1.0) {:.3}{}",
        if column_gap.is_finite() { format!("{column_gap:.3}") } else { "-".into() },
        contact_max,
        contact_at.map_or(String::new(), |(t, a, b, x, y)| format!(" (t {t}, carts {a} and {b} at {x:.2},{y:.2})"))
    ));
    if o.exit == "piston" {
        out.push(format!("piston firings (tick, cart, its y, its vertical speed): {}", piston_fired.iter().map(|(t, k, y, vy)| format!("t {t} cart {k} y {y:.3} vy {vy:.3}")).collect::<Vec<_>>().join("; ")));
    }
    Ok(out.join("\n"))
}
