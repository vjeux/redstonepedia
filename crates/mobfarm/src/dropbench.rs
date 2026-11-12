//! The per-lane escapement into the drop cell (ironspec Nov 11 22:02; Q2 of the one-bed farm, F1 + Q2
//! at 24 villagers): `lanes` parallel lanes along +x, `spacing` apart in z. Each lane: a pressed run of
//! `run` plain slopes; G0 over P (a powered slope, always on); the staging slope S with a rider string
//! over it; G_P over R (a plain slope); `entry` plain slopes more; a level rail end E; then the drop
//! cell: an off-rail cell (glass on three sides, open toward E) floored by a closed trapdoor, with its
//! own rider string. One release line opens every lane's trapdoor together for `drop_w` gt every
//! `period` gt; each lane restages on its own evidence: G_P opens (for `wp` gt) when the drop cell's
//! string is off, the trapdoor is shut and the staging string has held >= `hold_n` gt; G0 opens (for
//! `w0` gt) when the staging string is off and both gates are shut. A cart that falls through the
//! trapdoor is taken (with its rider) and returns after `ret` gt at rest near its lane's top (an ideal
//! intake). Game-time widths (with the chunk loader there are no mid-session freezes).

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The bench's options.
#[derive(Clone, Debug)]
pub struct DropOpts {
    pub lanes: usize,
    pub spacing: i32,
    pub run: i32,
    pub per_lane: usize,
    pub period: i64,
    pub cycles: i64,
    pub ret: i64,
    pub corner: i64,
    pub w0: i64,
    pub wp: i64,
    pub drop_w: i64,
    pub entry: i32,
    pub hold_n: i64,
    pub front_boost: bool,
    /// The bucket-brigade lane instead (every waiting cell a pocket with its own gate): the pockets.
    pub pockets: usize,
    /// The brigade's strings: this many cells above each pocket's rail block (2 = (x,18+k,k-1)).
    pub string_dy: i32,
    /// The switched-R lane instead (ironspec 22:32): the R line's pulse (0: off).
    pub r_pulse: i64,
    /// The switched-R lane: retry the R line this long after a pulse if a D hasn't settled.
    pub retry: i64,
    /// Logins: T,D (the entities frozen for D gt while the redstone runs).
    pub login: Vec<(i64, i64)>,
    /// The switched-R lane: a flat intake and the split's flat branch above the run; returns arrive
    /// on the branch at `v_in`.
    pub intake_flat: bool,
    pub v_in: f64,
    /// R (under G_P) a powered slope, always on: the staged cart leaves faster.
    pub r_boost: bool,
    pub floor_bottom: bool,
    pub riders: bool,
}

fn cart_state(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64, f64)> {
    redsim::entity::index_of(w, id).map(|i| {
        let e = &w.entities.list[i];
        (e.pos.x, e.pos.y, e.pos.z, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt())
    })
}

fn kill_with_rider(w: &mut redsim::world::World, id: u32) {
    let riders: Vec<u32> = redsim::entity::index_of(w, id).map(|i| w.entities.list[i].passengers.clone()).unwrap_or_default();
    for r in riders {
        let _ = redsim::entity::kill(w, r);
    }
    let _ = redsim::entity::kill(w, id);
}

/// The lane's fixed coordinates.
struct Lane {
    z: i32,
    hook_s: BlockPos,
    hook_d: BlockPos,
}

/// One cart's visit to the drop cell.
struct Visit {
    lane: usize,
    entry_t: i64,
    entry_v: f64,
    rest: Option<(i64, f64, f64)>,
    on_t: Option<i64>,
}

/// The bench: returns the text.
pub fn drop_bench(o: &DropOpts) -> Result<String, String> {
    if o.r_pulse > 0 {
        return crate::switchbench::switch_bench(o).map(|r| r.text);
    }
    if o.pockets > 0 {
        return crate::brigadebench::brigade_bench(o);
    }
    let glass = Block::parse("minecraft:glass")?;
    let stone = Block::parse("minecraft:stone")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let pdown_on = Block::parse("minecraft:powered_rail[powered=true,shape=ascending_west,waterlogged=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let half = if o.floor_bottom { "bottom" } else { "top" };
    let trap_shut = Block::parse(&format!("minecraft:oak_trapdoor[facing=north,half={half},open=false,powered=false,waterlogged=false]"))?;
    let trap_open = Block::parse(&format!("minecraft:oak_trapdoor[facing=north,half={half},open=true,powered=false,waterlogged=false]"))?;
    let string = Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_s = Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_n = Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let top = o.run + 1;
    let (xp, yp) = (o.run, top - 1 - o.run);
    let (xs, ys) = (o.run + 1, top - 2 - o.run);
    let (xr, yr) = (o.run + 2, top - 3 - o.run);
    let xe = o.run + 3 + o.entry;
    let h = top - 3 - o.run - o.entry;
    let xd = xe + 1;
    // (the drop cell's floor: a top-half trapdoor in the block under the cell (its top flush with the
    // rail end's level), or a bottom-half one in the cell one lower; the cart rests on it at `floor`)
    let (trap_pos_y, cell_y, floor) = if o.floor_bottom { (h - 1, h - 1, h as f64 - 1.0 + 0.1875) } else { (h - 1, h, h as f64) };
    let string_y = cell_y + 1;
    let zs: Vec<i32> = (0..o.lanes as i32).map(|l| l * o.spacing).collect();
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut lanes: Vec<Lane> = Vec::new();
    for &z in &zs {
        let mut put = |x: i32, y: i32, b: &Block, sup: &Block| {
            blocks.push((BlockPos::new(x, y, z), b.clone()));
            blocks.push((BlockPos::new(x, y - 1, z), sup.clone()));
        };
        for x in 0..o.run {
            if o.front_boost && x == o.run - 1 {
                put(x, top - 1 - x, &pdown_on, &rblock);
            } else {
                put(x, top - 1 - x, &slope, &glass);
            }
        }
        put(xp, yp, &pdown_on, &rblock);
        put(xs, ys, &slope, &glass);
        if o.r_boost {
            put(xr, yr, &pdown_on, &rblock);
        } else {
            put(xr, yr, &slope, &glass);
        }
        for k in 0..o.entry {
            put(o.run + 3 + k, top - 4 - o.run - k, &slope, &glass);
        }
        put(xe, h, &flat, &glass);
        blocks.push((BlockPos::new(-1, top, z), glass.clone()));
        blocks.push((BlockPos::new(xp, yp + 1, z), shut_b.clone()));
        blocks.push((BlockPos::new(xr, yr + 1, z), shut_b.clone()));
        // (the staging string over S: hooks across the track facing it, stone one further out)
        blocks.push((BlockPos::new(xs, ys + 1, z), string.clone()));
        blocks.push((BlockPos::new(xs, ys + 1, z - 1), hook_s.clone()));
        blocks.push((BlockPos::new(xs, ys + 1, z + 1), hook_n.clone()));
        blocks.push((BlockPos::new(xs, ys + 1, z - 2), stone.clone()));
        blocks.push((BlockPos::new(xs, ys + 1, z + 2), stone.clone()));
        // (the drop cell: the trapdoor floor, glass on the far side (two high) and both sides at the
        // cart's level, its string above with hooks and stone)
        blocks.push((BlockPos::new(xd, trap_pos_y, z), trap_shut.clone()));
        blocks.push((BlockPos::new(xd + 1, cell_y, z), glass.clone()));
        blocks.push((BlockPos::new(xd + 1, cell_y + 1, z), glass.clone()));
        blocks.push((BlockPos::new(xd, cell_y, z - 1), glass.clone()));
        blocks.push((BlockPos::new(xd, cell_y, z + 1), glass.clone()));
        blocks.push((BlockPos::new(xd, string_y, z), string.clone()));
        blocks.push((BlockPos::new(xd, string_y, z - 1), hook_s.clone()));
        blocks.push((BlockPos::new(xd, string_y, z + 1), hook_n.clone()));
        blocks.push((BlockPos::new(xd, string_y, z - 2), stone.clone()));
        blocks.push((BlockPos::new(xd, string_y, z + 2), stone.clone()));
        lanes.push(Lane { z, hook_s: BlockPos::new(xs, ys + 1, z - 1), hook_d: BlockPos::new(xd, string_y, z - 1) });
    }
    let hi_z = *zs.last().unwrap_or(&0);
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(xd + 4, top + 2, hi_z + 4), lo: BlockPos::new(-4, h - 26, -4), hi: BlockPos::new(xd + 6, top + 4, hi_z + 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let slope_y = |x: f64| {
        let cx = x.floor() as i32;
        (top - 1 - cx) as f64 + (1.0 - (x - cx as f64)) + 0.0625
    };
    let g0_face = xp as f64 + 0.375;
    let gp_face = xr as f64 + 0.375;
    let mut carts: Vec<(usize, Option<u32>)> = Vec::new();
    for (l, ln) in lanes.iter().enumerate() {
        for k in 0..o.per_lane {
            let x = g0_face - 0.49 - 0.79 * k as f64;
            if x < 0.6 {
                break;
            }
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, slope_y(x), ln.z as f64 + 0.5], Some(&nbt))?;
            carts.push((l, Some(id)));
        }
    }
    let n = carts.len();
    let nl = o.lanes;
    let warm = 300i64;
    let start = 20i64;
    let ticks = warm + o.cycles * o.period;
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    let mut pending: Vec<(i64, usize)> = Vec::new();
    let (mut s_run, mut d_run) = (vec![0i64; nl], vec![0i64; nl]);
    let (mut g0_until, mut gp_until) = (vec![-1i64; nl], vec![-1i64; nl]);
    let (mut g0_is, mut gp_is) = (vec![false; nl], vec![false; nl]);
    let (mut c0_prev, mut cp_prev) = (vec![false; nl], vec![false; nl]);
    let mut trap_until: i64 = -1;
    let mut trap_is = false;
    // (per opening: the carts through the gate's plane; per release: the carts through each floor)
    let mut g0_ops: Vec<(usize, i64, usize)> = Vec::new();
    let mut gp_ops: Vec<(usize, i64, usize)> = Vec::new();
    let mut releases: Vec<(i64, Vec<usize>, Vec<Option<i64>>, Vec<bool>)> = Vec::new();
    let mut shut_onto: Vec<String> = Vec::new();
    let mut arrivals: Vec<(usize, f64)> = Vec::new();
    let mut visits: Vec<Visit> = Vec::new();
    let mut open_visit: Vec<Option<usize>> = vec![None; n];
    let mut rest_ticks: Vec<i64> = vec![0; n];
    let mut fell: Vec<bool> = vec![false; n];
    let mut d_off_after_release: Vec<i64> = Vec::new();
    let mut s_on_after_g0: Vec<i64> = Vec::new();
    let mut last_g0_t: Vec<i64> = vec![-1; nl];
    let mut d_was = vec![false; nl];
    let mut s_was = vec![false; nl];
    let mut late_cells: Vec<String> = Vec::new();
    let set_gate = |w: &mut redsim::world::World, p: BlockPos, open: bool| {
        w.set_block(p, if open { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
    };
    for t in 0..ticks {
        // (the strings: the hook's power, and how long it has held)
        let mut hs = vec![false; nl];
        let mut hd = vec![false; nl];
        for (l, ln) in lanes.iter().enumerate() {
            hs[l] = matches!(run.world.get(ln.hook_s), Block::TripwireHook { powered: true, .. });
            hd[l] = matches!(run.world.get(ln.hook_d), Block::TripwireHook { powered: true, .. });
            s_run[l] = if hs[l] { s_run[l] + 1 } else { 0 };
            d_run[l] = if hd[l] { d_run[l] + 1 } else { 0 };
        }
        // (the release line: all trapdoors together)
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && ph == 0 {
            trap_until = t + o.drop_w;
            let full: Vec<bool> = (0..nl).map(|l| d_run[l] >= o.hold_n).collect();
            for l in 0..nl {
                if !full[l] {
                    late_cells.push(format!("t {t}: lane {l}"));
                }
            }
            releases.push((t, vec![0; nl], vec![None; nl], full));
        }
        let want_trap = t < trap_until;
        if want_trap != trap_is {
            trap_is = want_trap;
            for ln in &lanes {
                run.world.set_block(BlockPos::new(xd, trap_pos_y, ln.z), if want_trap { trap_open.clone() } else { trap_shut.clone() }, redsim::world::flags::ALL);
            }
            if !want_trap {
                // (a trapdoor shutting with a cart's box through its closed volume)
                let (lo_y, hi_y) = if o.floor_bottom { (trap_pos_y as f64, trap_pos_y as f64 + 0.1875) } else { (trap_pos_y as f64 + 0.8125, trap_pos_y as f64 + 1.0) };
                for (k, (l, id)) in carts.iter().enumerate() {
                    if let Some(id) = id
                        && let Some((x, y, _, _)) = cart_state(&run.world, *id)
                        && x + 0.49 > xd as f64
                        && x - 0.49 < (xd + 1) as f64
                        && y < hi_y - 0.001
                        && y + 0.7 > lo_y + 0.001
                    {
                        shut_onto.push(format!("t {t}: the trapdoor of lane {l} onto cart {k} at {x:.2},{y:.2}"));
                    }
                }
            }
        }
        // (each lane's evidence)
        for l in 0..nl {
            let staged = s_run[l] >= o.hold_n;
            let cond_p = t >= start && staged && !hd[l] && !trap_is && !gp_is[l] && !g0_is[l];
            if cond_p && !cp_prev[l] {
                gp_until[l] = t + o.wp;
                gp_ops.push((l, t, 0));
            }
            cp_prev[l] = cond_p;
            let cond_0 = t >= start && !hs[l] && !gp_is[l] && !g0_is[l] && t >= gp_until[l];
            if cond_0 && !c0_prev[l] {
                g0_until[l] = t + o.w0;
                g0_ops.push((l, t, 0));
                last_g0_t[l] = t;
            }
            c0_prev[l] = cond_0;
            for (until, is, x_gate, yg, face, name) in [(g0_until[l], &mut g0_is[l], xp, yp + 1, g0_face, "G0"), (gp_until[l], &mut gp_is[l], xr, yr + 1, gp_face, "G_P")] {
                let want = t < until;
                if want != *is {
                    *is = want;
                    set_gate(&mut run.world, BlockPos::new(x_gate, yg, lanes[l].z), want);
                    if !want {
                        for (k, (ll, id)) in carts.iter().enumerate() {
                            if *ll == l
                                && let Some(id) = id
                                && let Some((x, _, _, _)) = cart_state(&run.world, *id)
                                && x + 0.49 > face + 0.02
                                && x - 0.49 < face + 0.25
                            {
                                shut_onto.push(format!("t {t}: {name} of lane {l} onto cart {k} at x {x:.2}"));
                            }
                        }
                    }
                }
            }
            // (string timing: the staging string's rise after G0's opening; the drop string's fall
            // after the release)
            if hs[l] && !s_was[l] && last_g0_t[l] >= 0 {
                s_on_after_g0.push(t - last_g0_t[l]);
            }
            if !hd[l] && d_was[l]
                && let Some(r) = releases.last()
                && t - r.0 < o.period
            {
                d_off_after_release.push(t - r.0);
            }
            s_was[l] = hs[l];
            d_was[l] = hd[l];
        }
        // (returns: near the lane's top, at rest, once that cell is clear)
        let due: Vec<usize> = pending.iter().filter(|p| p.0 <= t).map(|p| p.1).collect();
        for k in due {
            let l = carts[k].0;
            let x = 1.5;
            let clear = carts.iter().filter(|(ll, _)| *ll == l).all(|(_, id)| id.and_then(|id| cart_state(&run.world, id)).is_none_or(|(cx, _, _, _)| cx > x + 1.2));
            if clear {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, slope_y(x), lanes[l].z as f64 + 0.5], Some(&nbt))?;
                carts[k].1 = Some(id);
                pending.retain(|p| p.1 != k);
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|(_, id)| id.and_then(|id| cart_state(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_DROP_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 3 && t >= p[1] && t < p[2] {
                let l = p[0] as usize;
                let xs: Vec<String> = (0..n).filter(|&k| carts[k].0 == l).filter_map(|k| now[k].map(|c| format!("{k}:{:.2},{:.2}/{:.2}", c.0, c.1, c.3))).collect();
                eprintln!("t {t}: G0 {} G_P {} trap {} s {} d {} : {}", g0_is[l] as u8, gp_is[l] as u8, trap_is as u8, hs[l] as u8, hd[l] as u8, xs.join(" "));
            }
        }
        for k in 0..n {
            let l = carts[k].0;
            let (Some((x, y, z, v)), Some((px, _, _, pv))) = (now[k], prev[k]) else { continue };
            if px < xp as f64 + 0.5 && x >= xp as f64 + 0.5
                && let Some(op) = g0_ops.iter_mut().rev().find(|op| op.0 == l)
            {
                op.2 += 1;
            }
            if px < xr as f64 + 0.5 && x >= xr as f64 + 0.5
                && let Some(op) = gp_ops.iter_mut().rev().find(|op| op.0 == l)
            {
                op.2 += 1;
            }
            // (the visit to the drop cell: the entry (the centre leaving the rail end) and the rest)
            if px < xd as f64 && x >= xd as f64 && open_visit[k].is_none() {
                visits.push(Visit { lane: l, entry_t: t, entry_v: pv, rest: None, on_t: None });
                open_visit[k] = Some(visits.len() - 1);
            }
            if let Some(vi) = open_visit[k] {
                if visits[vi].on_t.is_none() && matches!(run.world.get(lanes[l].hook_d), Block::TripwireHook { powered: true, .. }) {
                    visits[vi].on_t = Some(t);
                }
                if v < 0.002 && (y - floor).abs() < 0.05 {
                    rest_ticks[k] += 1;
                    if rest_ticks[k] == 3 && visits[vi].rest.is_none() {
                        visits[vi].rest = Some((t - 2, x - xd as f64, z - (lanes[l].z as f64 + 0.5)));
                    }
                } else {
                    rest_ticks[k] = 0;
                }
            }
            // (falling through the floor: counted once for the release, then taken)
            if y < floor - 1.0 && x > xd as f64 - 0.5 {
                if !fell[k] {
                    fell[k] = true;
                    if let Some(r) = releases.last_mut()
                        && t - r.0 < o.period
                    {
                        r.1[l] += 1;
                        if r.2[l].is_none() {
                            r.2[l] = Some(t);
                        }
                    }
                }
                if y < floor - 3.0
                    && let Some(id) = carts[k].1.take()
                {
                    kill_with_rider(&mut run.world, id);
                    pending.push((t + o.ret, k));
                    open_visit[k] = None;
                    rest_ticks[k] = 0;
                    fell[k] = false;
                }
            }
            // (an arrival at the lane's back: the stored speed dropping by 0.04 or more with a cart of
            // the lane within 1.3 ahead, behind G0)
            if pv - v >= 0.04 && x < g0_face && (0..n).any(|j| j != k && carts[j].0 == l && now[j].is_some_and(|(xj, _, _, _)| xj > x && xj - x < 1.3)) {
                arrivals.push((l, pv));
            }
        }
        prev = now;
    }
    let mut lines = vec![format!(
        "drop bench: {} lanes {} apart, each a {}-slope pressed run ({} carts, {}{}), G0 {} gt over P, the staging slope S (string), G_P {} gt over R, {} entry slope(s), the rail end, the drop cell ({}-half trapdoor floor, its string); the release opens every trapdoor for {} gt every {} gt (corner {:+}); strings held >= {} gt; returns after {} gt; {} cycles",
        nl, o.spacing, o.run, o.per_lane, if o.riders { "ridden" } else { "empty" }, if o.front_boost { ", the run's last slope powered" } else { "" }, o.w0, o.wp, o.entry, half, o.drop_w, o.period, o.corner, o.hold_n, o.ret, o.cycles
    )];
    let hist = |ops: &[(usize, i64, usize)]| {
        let mut m: std::collections::BTreeMap<usize, usize> = Default::default();
        for op in ops.iter().filter(|op| op.1 >= warm) {
            *m.entry(op.2).or_default() += 1;
        }
        m.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")
    };
    lines.push(format!("carts per G0 opening (exactly 1, after the warm-up): {}; per G_P opening: {}", hist(&g0_ops), hist(&gp_ops)));
    let mut rel_hist: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &releases {
        for c in &r.1 {
            *rel_hist.entry(*c).or_default() += 1;
        }
    }
    let spreads: Vec<i64> = releases.iter().filter_map(|r| {
        let ts: Vec<i64> = r.2.iter().filter_map(|x| *x).collect();
        if ts.len() == nl { Some(ts.iter().max().unwrap() - ts.iter().min().unwrap()) } else { None }
    }).collect();
    lines.push(format!(
        "carts per cell per release (exactly 1): {}; cells not settled (string held) at a release: {}{}; the batch's spread through the floors: {}",
        rel_hist.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", "),
        late_cells.len(),
        if late_cells.is_empty() { String::new() } else { format!(" ({})", late_cells.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        if spreads.is_empty() { "-".to_string() } else { format!("max {} gt", spreads.iter().max().unwrap()) }
    ));
    lines.push(format!("gate and trapdoor shuts onto a cart (the bar: 0): {}{}", shut_onto.len(), if shut_onto.is_empty() { String::new() } else { format!(" ({})", shut_onto.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
    let rested: Vec<&Visit> = visits.iter().filter(|v| v.rest.is_some()).collect();
    if !rested.is_empty() {
        let xs: Vec<f64> = rested.iter().map(|v| v.rest.unwrap().1).collect();
        let zs2: Vec<f64> = rested.iter().map(|v| v.rest.unwrap().2).collect();
        let vs: Vec<f64> = rested.iter().map(|v| v.entry_v).collect();
        let tt: Vec<i64> = rested.iter().map(|v| v.rest.unwrap().0 - v.entry_t).collect();
        let mm = |a: &[f64]| (a.iter().copied().fold(f64::INFINITY, f64::min), a.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let (x0, x1) = mm(&xs);
        let (z0, z1) = mm(&zs2);
        let (v0, v1) = mm(&vs);
        lines.push(format!(
            "the drop cell: {} arrivals at rest of {} entries; entry speed {v0:.3}..{v1:.3} b/t; the cart's centre at rest {x0:.3}..{x1:.3} into the cell (0.5 = centred over the pocket; flush on the far wall = 0.51), z off-centre {z0:+.3}..{z1:+.3}; entry to rest {}..{} gt",
            rested.len(),
            visits.len(),
            tt.iter().min().unwrap(),
            tt.iter().max().unwrap()
        ));
        let ons: Vec<i64> = visits.iter().filter_map(|v| v.on_t.map(|o2| o2 - v.entry_t)).collect();
        if !ons.is_empty() {
            lines.push(format!("the drop string: on {}..{} gt after the cart's centre leaves the rail end; off {}..{} gt after the release opens the trapdoors", ons.iter().min().unwrap(), ons.iter().max().unwrap(), d_off_after_release.iter().min().map_or("-".to_string(), |v| v.to_string()), d_off_after_release.iter().max().map_or("-".to_string(), |v| v.to_string())));
        }
    } else {
        lines.push(format!("the drop cell: {} entries, none came to rest on the floor", visits.len()));
    }
    if !s_on_after_g0.is_empty() {
        lines.push(format!("the staging string: on {}..{} gt after G0 opens", s_on_after_g0.iter().min().unwrap(), s_on_after_g0.iter().max().unwrap()));
    }
    let amax = arrivals.iter().map(|a| a.1).fold(0.0f64, f64::max);
    lines.push(format!("arrivals at the lanes' backs (the ideal returns and the trains closing up): {}, {} above 0.46, the hardest {amax:.3}", arrivals.len(), arrivals.iter().filter(|a| a.1 > 0.46).count()));
    Ok(lines.join("\n"))
}
