//! The shared-release lanes bench (Q2 of the one-bed farm, ironspec Nov 11 21:27): `lanes` parallel
//! lanes along +x, `spacing` blocks apart in z, each a pressed gravity queue on a 45-degree run of
//! `run` plain slopes ending at its front gate (a fence gate over the next slope, the cell the front
//! cart presses against), then two slopes and flats to a sink. One release line opens every front
//! gate together for `width` controller-gt every `period` gt (the controller clock counts ring edges
//! with `--ring-q`, pausing with the carts); the batch is the front cart of each lane, side by side.
//! A cart reaching the sink returns after `ret` gt, placed at rest on its lane's top slope (an ideal
//! intake), so each lane keeps its count. Prints, per opening and lane, the carts that crossed the
//! gate's plane (exactly 1), any gate shut onto a cart, the batch's spread across the lanes at the
//! gates, the arrivals at each lane's back, and the pressed spacing.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct LanesOpts {
    pub lanes: usize,
    pub spacing: i32,
    pub run: i32,
    pub per_lane: usize,
    pub period: i64,
    pub width: i64,
    pub cycles: i64,
    pub ret: i64,
    pub corner: i64,
    pub ring_q: i64,
    pub freeze: Vec<(i64, i64)>,
    pub freeze32: Vec<(i64, i64)>,
    pub riders: bool,
}

fn cart_state(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64, f64)> {
    redsim::entity::index_of(w, id).map(|i| {
        let e = &w.entities.list[i];
        (e.pos.x, e.pos.y, e.pos.z, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
    })
}

/// The bench: returns the text.
pub fn lanes_bench(o: &LanesOpts) -> Result<String, String> {
    let glass = Block::parse("minecraft:glass")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let pdown_on = Block::parse("minecraft:powered_rail[powered=true,shape=ascending_west,waterlogged=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let top = o.run + 1;
    let zs: Vec<i32> = (0..o.lanes as i32).map(|l| l * o.spacing).collect();
    for &z in &zs {
        let mut put = |x: i32, y: i32, b: &Block, sup: &Block| {
            blocks.push((BlockPos::new(x, y, z), b.clone()));
            blocks.push((BlockPos::new(x, y - 1, z), sup.clone()));
        };
        // (the run: cell x at height top - 1 - x, rising toward the west; glass on the high side of the
        // first one)
        for x in 0..o.run {
            put(x, top - 1 - x, &slope, &glass);
        }
        // (the gate's cell: a powered slope one lower, always on (a redstone block under it, as P under
        // G0), the gate block above it; then a slope and flats)
        put(o.run, top - 1 - o.run, &pdown_on, &rblock);
        put(o.run + 1, top - 2 - o.run, &slope, &glass);
        for k in 0..6 {
            put(o.run + 2 + k, top - 2 - o.run, &flat, &glass);
        }
    }
    for &z in &zs {
        blocks.push((BlockPos::new(-1, top, z), glass.clone()));
        blocks.push((BlockPos::new(o.run, top - o.run, z), shut_b.clone()));
    }
    let hi_z = *zs.last().unwrap_or(&0);
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(o.run + 6, 1, hi_z + 3), lo: BlockPos::new(-4, top - o.run - 8, -4), hi: BlockPos::new(o.run + 12, top + 4, hi_z + 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let gx = o.run;
    let g_face = gx as f64 + 0.375;
    let g_plane = gx as f64 + 0.5;
    // (the sink on the first flat after the gate's slope: a released cart is taken before friction on
    // the flats can stop it there)
    let sink_x = (o.run + 2) as f64 + 0.3;
    // (the lanes filled pressed: the front cart at the gate's face, then 0.790 apart up the run)
    let slope_y = |x: f64| {
        let cx = x.floor() as i32;
        (top - 1 - cx) as f64 + (1.0 - (x - cx as f64)) + 0.0625
    };
    let mut carts: Vec<(usize, Option<u32>)> = Vec::new();
    for (l, &z) in zs.iter().enumerate() {
        for k in 0..o.per_lane {
            let x = g_face - 0.49 - 0.79 * k as f64;
            if x < 0.6 {
                break;
            }
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, slope_y(x), z as f64 + 0.5], Some(&nbt))?;
            carts.push((l, Some(id)));
        }
    }
    let n = carts.len();
    let mut pending: Vec<(i64, usize)> = Vec::new();
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    let mut gate_open = false;
    let mut open_until: i64 = -1;
    let mut ring_live: i64 = 0;
    let mut ring_next: i64 = 3;
    let mut ring_alt = false;
    let mut ct: i64 = 0;
    let warm = 100i64;
    // (per opening: carts crossing each lane's gate plane, and the crossing ticks)
    let mut openings: Vec<(i64, Vec<usize>, Vec<Vec<i64>>)> = Vec::new();
    let mut shut_onto: Vec<String> = Vec::new();
    let mut arrivals: Vec<(usize, f64)> = Vec::new();
    let mut spacing: Vec<f64> = Vec::new();
    let col = ((gx as f64) + 0.5, 0.5);
    let mut frozen32: Option<[i32; 4]> = None;
    let ticks = warm + o.cycles * o.period;
    for t in 0..ticks {
        let frz = o.freeze.iter().any(|(a, d)| t >= *a && t < a + d);
        // (the real freeze: the whole bench at level 32 from T for D gt, redstone running)
        for (a, d) in &o.freeze32 {
            if t == *a && frozen32.is_none() {
                frozen32 = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == a + d
                && let Some(pb) = frozen32.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        let real_frozen = !run.world.entity_ticks_at(col.0, col.1);
        if o.ring_q > 0 && !(frz || real_frozen) {
            ring_live += 1;
            if ring_live >= ring_next {
                let step = if o.ring_q == 34 { if ring_alt { 4 } else { 3 } } else { o.ring_q };
                ring_alt = !ring_alt;
                ct = ring_next;
                ring_next += step;
            }
        } else if o.ring_q == 0 && !frz {
            ct = t;
        }
        // (the release line: every period from the warm-up (the corner shifts it), all gates open for
        // `width` controller gt)
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && ph == 0 && !frz {
            open_until = ct + o.width;
            openings.push((t, vec![0; o.lanes], vec![Vec::new(); o.lanes]));
        }
        let want = ct < open_until;
        if want != gate_open {
            gate_open = want;
            for &z in &zs {
                run.world.set_block(BlockPos::new(gx, top - gx, z), if want { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
            }
            if !want {
                // (a gate shutting with a cart's box across its cell: the gate's box x +0.375..0.625)
                for (k, (l, id)) in carts.iter().enumerate() {
                    if let Some(id) = id
                        && let Some((x, _, _, _)) = cart_state(&run.world, *id)
                        && x + 0.49 > g_face + 0.02
                        && x - 0.49 < g_face + 0.25
                    {
                        shut_onto.push(format!("t {t}: lane {l} cart {k} at x {x:.2}"));
                    }
                }
            }
        }
        // (returns: a sunk cart placed at rest on its lane's top slope once that cell is clear)
        let due: Vec<usize> = pending.iter().filter(|p| p.0 <= t).map(|p| p.1).collect();
        for k in due {
            let l = carts[k].0;
            let z = zs[l] as f64 + 0.5;
            let x = 1.5;
            let clear = carts.iter().filter(|(ll, _)| *ll == l).all(|(_, id)| id.and_then(|id| cart_state(&run.world, id)).is_none_or(|(cx, _, _, _)| cx > x + 1.2));
            if clear && !frz && !real_frozen {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, slope_y(x), z], Some(&nbt))?;
                carts[k].1 = Some(id);
                pending.retain(|p| p.1 != k);
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|(_, id)| id.and_then(|id| cart_state(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_LANES_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 3 && t >= p[1] && t < p[2] {
                let xs: Vec<String> = (0..n).filter(|&k| carts[k].0 == p[0] as usize).filter_map(|k| now[k].map(|c| format!("{k}:{:.2},{:.2}/{:.2}", c.0, c.1, c.3))).collect();
                eprintln!("t {t} gate {} : {}", if gate_open { "open" } else { "shut" }, xs.join(" "));
            }
        }
        for k in 0..n {
            let l = carts[k].0;
            if let (Some((x, _, _, v)), Some((px, _, _, pv))) = (now[k], prev[k]) {
                if px < g_plane && x >= g_plane
                    && let Some(op) = openings.last_mut()
                {
                    op.1[l] += 1;
                    op.2[l].push(t);
                }
                // (an arrival at the back: this cart's stored speed dropping by 0.04 or more with a
                // cart of its lane within 1.3 ahead)
                if pv - v >= 0.04 && x < g_face && (0..n).any(|j| j != k && carts[j].0 == l && now[j].is_some_and(|(xj, _, _, _)| xj > x && xj - x < 1.3)) {
                    arrivals.push((l, pv));
                }
                if x >= sink_x
                    && let Some(id) = carts[k].1.take()
                {
                    // (the rider first: a villager left behind on the track would stop the next cart)
                    let riders: Vec<u32> = redsim::entity::index_of(&run.world, id).map(|i| run.world.entities.list[i].passengers.clone()).unwrap_or_default();
                    for r in riders {
                        let _ = redsim::entity::kill(&mut run.world, r);
                    }
                    let _ = redsim::entity::kill(&mut run.world, id);
                    pending.push((t + o.ret, k));
                }
            }
        }
        // (the pressed spacing at each opening's start, per lane)
        if t >= warm && ph == 1 {
            for l in 0..o.lanes {
                let mut xs: Vec<f64> = (0..n).filter(|&k| carts[k].0 == l).filter_map(|k| now[k].map(|c| c.0)).filter(|x| *x < g_face).collect();
                xs.sort_by(|a, b| a.total_cmp(b));
                for w in xs.windows(2) {
                    if w[1] - w[0] < 1.6 {
                        spacing.push(w[1] - w[0]);
                    }
                }
            }
        }
        prev = now;
    }
    let mut lines = vec![format!(
        "lanes bench: {} lanes {} apart in z, each a {}-slope pressed run with its front gate, {} carts a lane ({}), one release line opening every gate for {} {}gt every {} gt (corner {:+}), returns after {} gt, {} cycles{}",
        o.lanes, o.spacing, o.run, o.per_lane, if o.riders { "ridden" } else { "empty" }, o.width, if o.ring_q > 0 { "ring-" } else { "" }, o.period, o.corner, o.ret, o.cycles,
        if o.freeze.is_empty() && o.freeze32.is_empty() { String::new() } else { format!(", freezes {:?} {:?}", o.freeze, o.freeze32) }
    )];
    let mut hist: std::collections::BTreeMap<usize, usize> = Default::default();
    for (_, c, _) in &openings {
        for v in c {
            *hist.entry(*v).or_default() += 1;
        }
    }
    lines.push(format!("carts per lane per opening (exactly 1): {}", hist.iter().map(|(k, v)| format!("{k} carts: {v}")).collect::<Vec<_>>().join(", ")));
    let spreads: Vec<i64> = openings.iter().filter_map(|(_, _, ts)| {
        let firsts: Vec<i64> = ts.iter().filter_map(|v| v.first().copied()).collect();
        if firsts.len() == o.lanes { Some(firsts.iter().max().unwrap() - firsts.iter().min().unwrap()) } else { None }
    }).collect();
    lines.push(format!("the batch's spread across the lanes at the gates (gt between the first and the last lane's front cart crossing): {}", if spreads.is_empty() { "-".to_string() } else { format!("max {}, {}", spreads.iter().max().unwrap(), spreads.iter().take(12).map(|s| s.to_string()).collect::<Vec<_>>().join(",")) }));
    lines.push(format!("gate shuts onto a cart (the bar: 0): {}{}", shut_onto.len(), if shut_onto.is_empty() { String::new() } else { format!(" ({})", shut_onto.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
    let amax = arrivals.iter().map(|a| a.1).fold(0.0f64, f64::max);
    lines.push(format!(
        "arrivals at the lanes' backs (the intakes and the trains closing up; this bench's bar is the hold brief's contact bar 0.2, not the gravity run's 0.46): {}, {} above 0.2, the hardest {amax:.3}",
        arrivals.len(),
        arrivals.iter().filter(|a| a.1 > 0.2).count()
    ));
    if !spacing.is_empty() {
        let mn = spacing.iter().copied().fold(f64::INFINITY, f64::min);
        let mx = spacing.iter().copied().fold(0.0f64, f64::max);
        lines.push(format!("the pressed spacing: {} samples, {mn:.3}..{mx:.3}", spacing.len()));
    }
    Ok(lines.join("\n"))
}
