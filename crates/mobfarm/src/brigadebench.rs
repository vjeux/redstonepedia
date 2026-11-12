//! The bucket-brigade lane (ironfarm 22:25, against the margin bar: every waiting cell pocketed):
//! `lanes` parallel lanes along +x, `spacing` apart in z, each a 45-degree staircase of `pockets`
//! slope pockets ending at the hold (the off-rail drop cell, a bottom-half trapdoor one block below the
//! rail end, glass on three sides). Pocket k (1 = next to the hold) rests its cart against its own gate
//! G_k, a fence gate one cell ahead at the pocket's level (G_1 above the hold cell); pocket 1's rail is
//! a powered slope, always on (a redstone block beside it, clear of the trapdoor), so its cart reaches
//! the hold fast enough to slide flush on the far wall. Each pocket has a rider string two cells above
//! its gate ((x_hold - k + 1, h + k + 1), the only cell its resting rider meets alone). One release line
//! opens every trapdoor for `drop_w` gt each `period`; G_1 opens (for `wp` gt) once its pocket's string
//! has held >= `hold_n` gt and the hold has been emptied by a release since the last hand-off; G_k
//! (k >= 2) opens when pocket k's string has held and pocket k-1's string is off. A cart through the
//! floor is taken (with its rider) and returns after `ret` gt at rest in the last pocket once that
//! pocket's string is off. Prints the carts per opening and per release, gate and trapdoor shuts onto a
//! cart, each string's trips with its own pocket empty (a neighbour's rider), where the hold's cart
//! rests, and each pocket's refill time after the release.

use crate::dropbench::DropOpts;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

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

/// The bench: returns the text.
pub fn brigade_bench(o: &DropOpts) -> Result<String, String> {
    let kp = o.pockets.max(2);
    let glass = Block::parse("minecraft:glass")?;
    let stone = Block::parse("minecraft:stone")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let shut_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let open_b = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let pdown_on = Block::parse("minecraft:powered_rail[powered=true,shape=ascending_west,waterlogged=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let trap_shut = Block::parse("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = Block::parse("minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string = Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_s = Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_n = Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    // (the hold column x_d; the rail end (pocket 1's low end) at level h; the hold cell one lower)
    let xd = kp as i32 + 2;
    let h = 2i32;
    let floor = h as f64 - 1.0 + 0.1875;
    let rail = |k: usize| (xd - k as i32, h + k as i32 - 1);
    let gate = |k: usize| (xd - k as i32 + 1, h + k as i32 - 1);
    let sdy = o.string_dy;
    let spos = |k: usize| (xd - k as i32 + 1, h + k as i32 - 1 + sdy);
    let rest_x = |k: usize| (xd - k as i32 + 1) as f64 + 0.375 - 0.49;
    let zs: Vec<i32> = (0..o.lanes as i32).map(|l| l * o.spacing).collect();
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for &z in &zs {
        for k in 1..=kp {
            let (x, y) = rail(k);
            if k == 1 {
                blocks.push((BlockPos::new(x, y, z), pdown_on.clone()));
                blocks.push((BlockPos::new(x, y - 1, z), glass.clone()));
                blocks.push((BlockPos::new(x, y, z + 1), rblock.clone()));
            } else {
                blocks.push((BlockPos::new(x, y, z), slope.clone()));
                blocks.push((BlockPos::new(x, y - 1, z), glass.clone()));
            }
            let (gx, gy) = gate(k);
            blocks.push((BlockPos::new(gx, gy, z), shut_b.clone()));
            let (sx, sy) = spos(k);
            blocks.push((BlockPos::new(sx, sy, z), string.clone()));
            blocks.push((BlockPos::new(sx, sy, z - 1), hook_s.clone()));
            blocks.push((BlockPos::new(sx, sy, z + 1), hook_n.clone()));
            blocks.push((BlockPos::new(sx, sy, z - 2), stone.clone()));
            blocks.push((BlockPos::new(sx, sy, z + 2), stone.clone()));
        }
        // (the high side of the last pocket's slope)
        let (x, y) = rail(kp);
        blocks.push((BlockPos::new(x - 1, y + 1, z), glass.clone()));
        // (the hold: the trapdoor floor in the cell one below the rail end, glass far wall two high
        // and both sides at the cart's level)
        blocks.push((BlockPos::new(xd, h - 1, z), trap_shut.clone()));
        blocks.push((BlockPos::new(xd + 1, h - 1, z), glass.clone()));
        blocks.push((BlockPos::new(xd + 1, h, z), glass.clone()));
        blocks.push((BlockPos::new(xd, h - 1, z - 1), glass.clone()));
        blocks.push((BlockPos::new(xd, h - 1, z + 1), glass.clone()));
    }
    let hi_z = *zs.last().unwrap_or(&0);
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(xd + 4, h + kp as i32 + 4, hi_z + 4), lo: BlockPos::new(-4, h - 26, -4), hi: BlockPos::new(xd + 6, h + kp as i32 + 6, hi_z + 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let pocket_y = |k: usize| {
        let (x, y) = rail(k);
        let f = rest_x(k) - x as f64;
        y as f64 + (1.0 - f) + 0.0625
    };
    // (every pocket filled at rest; the hold empty)
    let nl = o.lanes;
    let mut carts: Vec<(usize, Option<u32>)> = Vec::new();
    for l in 0..nl {
        for k in 1..=kp {
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [rest_x(k), pocket_y(k), zs[l] as f64 + 0.5], Some(&nbt))?;
            carts.push((l, Some(id)));
        }
    }
    let n = carts.len();
    let warm = 300i64;
    let start = 20i64;
    let ticks = warm + o.cycles * o.period;
    let hook = |l: usize, k: usize| {
        let (sx, sy) = spos(k);
        BlockPos::new(sx, sy, zs[l] - 1)
    };
    let mut held = vec![vec![0i64; kp + 1]; nl];
    let mut is_open = vec![vec![false; kp + 1]; nl];
    let mut until = vec![vec![-1i64; kp + 1]; nl];
    let mut cond_prev = vec![vec![false; kp + 1]; nl];
    let mut hold_full = vec![false; nl];
    let mut trap_until = -1i64;
    let mut trap_is = false;
    let mut ops: Vec<(usize, usize, i64, usize)> = Vec::new();
    let mut releases: Vec<(i64, Vec<usize>, Vec<Option<i64>>)> = Vec::new();
    let mut shut_onto: Vec<String> = Vec::new();
    let mut cross_trips = vec![0usize; kp + 1];
    let mut cross_note: Vec<String> = Vec::new();
    let mut own_trips = vec![0usize; kp + 1];
    let mut rests: Vec<(f64, f64, f64)> = Vec::new();
    let mut rest_ticks = vec![0i64; n];
    let mut rested = vec![false; n];
    let mut fell = vec![false; n];
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    let mut pending: Vec<(i64, usize)> = Vec::new();
    let mut refill: Vec<Vec<i64>> = vec![Vec::new(); kp + 1];
    let mut last_release = -1i64;
    let mut missing_at_release = 0usize;
    for t in 0..ticks {
        let hk: Vec<Vec<bool>> = (0..nl).map(|l| (0..=kp).map(|k| k >= 1 && matches!(run.world.get(hook(l, k)), Block::TripwireHook { powered: true, .. })).collect()).collect();
        for l in 0..nl {
            for k in 1..=kp {
                let was = held[l][k] >= o.hold_n;
                held[l][k] = if hk[l][k] { held[l][k] + 1 } else { 0 };
                if !was && held[l][k] >= o.hold_n && last_release >= 0 && t - last_release < o.period {
                    refill[k].push(t - last_release);
                }
            }
        }
        // (the release line)
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && ph == 0 {
            trap_until = t + o.drop_w;
            last_release = t;
            missing_at_release += (0..nl).filter(|l| !hold_full[*l]).count();
            releases.push((t, vec![0; nl], vec![None; nl]));
        }
        let want_trap = t < trap_until;
        if want_trap != trap_is {
            trap_is = want_trap;
            for &z in &zs {
                run.world.set_block(BlockPos::new(xd, h - 1, z), if want_trap { trap_open.clone() } else { trap_shut.clone() }, redsim::world::flags::ALL);
            }
            if !want_trap {
                for l in 0..nl {
                    hold_full[l] = false;
                }
                for (k, (l, id)) in carts.iter().enumerate() {
                    if let Some(id) = id
                        && let Some((x, y, _, _)) = cart_state(&run.world, *id)
                        && x + 0.49 > xd as f64
                        && x - 0.49 < (xd + 1) as f64
                        && y < h as f64 - 1.0 + 0.1875 - 0.001
                        && y + 0.7 > h as f64 - 1.0 + 0.001
                    {
                        shut_onto.push(format!("t {t}: the trapdoor of lane {l} onto cart {k} at {x:.2},{y:.2}"));
                    }
                }
            }
        }
        // (the brigade: each gate on its own pocket's evidence)
        for l in 0..nl {
            for k in 1..=kp {
                let mine = held[l][k] >= o.hold_n;
                let ahead_free = if k == 1 { !hold_full[l] && !trap_is } else { !hk[l][k - 1] && !is_open[l][k - 1] };
                let cond = t >= start && mine && ahead_free && !is_open[l][k];
                if cond && !cond_prev[l][k] {
                    until[l][k] = t + o.wp;
                    ops.push((l, k, t, 0));
                    if k == 1 {
                        hold_full[l] = true;
                    }
                }
                cond_prev[l][k] = cond;
                let want = t < until[l][k];
                if want != is_open[l][k] {
                    is_open[l][k] = want;
                    let (gx, gy) = gate(k);
                    run.world.set_block(BlockPos::new(gx, gy, zs[l]), if want { open_b.clone() } else { shut_b.clone() }, redsim::world::flags::ALL);
                    if !want {
                        let face = gx as f64 + 0.375;
                        for (c, (ll, id)) in carts.iter().enumerate() {
                            if *ll == l
                                && let Some(id) = id
                                && let Some((x, y, _, _)) = cart_state(&run.world, *id)
                                && x + 0.49 > face + 0.02
                                && x - 0.49 < face + 0.25
                                && y + 0.7 > gy as f64
                                && y < gy as f64 + 1.5
                            {
                                shut_onto.push(format!("t {t}: G_{k} of lane {l} onto cart {c} at {x:.2},{y:.2}"));
                            }
                        }
                    }
                }
            }
        }
        // (returns: at rest in the last pocket once its string is off)
        let due: Vec<usize> = pending.iter().filter(|p| p.0 <= t).map(|p| p.1).collect();
        for c in due {
            let l = carts[c].0;
            let occupied = carts.iter().any(|(ll, id)| *ll == l && id.and_then(|id| cart_state(&run.world, id)).is_some_and(|(x, _, _, _)| (x - rest_x(kp)).abs() < 1.0));
            if !hk[l][kp] && !occupied {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [rest_x(kp), pocket_y(kp), zs[l] as f64 + 0.5], Some(&nbt))?;
                carts[c].1 = Some(id);
                pending.retain(|p| p.1 != c);
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|(_, id)| id.and_then(|id| cart_state(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_BRIGADE_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 3 && t >= p[1] && t < p[2] {
                let l = p[0] as usize;
                let gs: String = (1..=kp).map(|k| if is_open[l][k] { 'o' } else { '.' }).collect();
                let ss: String = (1..=kp).map(|k| if hk[l][k] { 'S' } else { '_' }).collect();
                let xs: Vec<String> = (0..n).filter(|&c| carts[c].0 == l).filter_map(|c| now[c].map(|s| format!("{c}:{:.2},{:.2}/{:.2}", s.0, s.1, s.3))).collect();
                eprintln!("t {t}: gates {gs} strings {ss} trap {} hold {} : {}", trap_is as u8, hold_full[l] as u8, xs.join(" "));
            }
        }
        // (string truth: a string on while no cart of its lane is in its pocket (the pocket's cell or
        // moving through its gate) is a neighbour's rider)
        for l in 0..nl {
            for k in 1..=kp {
                if !hk[l][k] {
                    continue;
                }
                let (rx, _) = rail(k);
                let in_pocket = (0..n).any(|c| carts[c].0 == l && now[c].is_some_and(|(x, _, _, _)| x >= rx as f64 - 0.2 && x < rx as f64 + 1.6));
                if in_pocket {
                    own_trips[k] += 1;
                } else {
                    cross_trips[k] += 1;
                    if cross_note.len() < 6 {
                        let who: Vec<String> = (0..n).filter(|&c| carts[c].0 == l).filter_map(|c| now[c].map(|s| format!("{:.2},{:.2}", s.0, s.1))).collect();
                        cross_note.push(format!("t {t}: lane {l} string {k} with carts at {}", who.join(" ")));
                    }
                }
            }
        }
        for c in 0..n {
            let l = carts[c].0;
            let (Some((x, y, z, v)), Some((px, _, _, _))) = (now[c], prev[c]) else { continue };
            for k in 1..=kp {
                let (gx, _) = gate(k);
                if px < gx as f64 + 0.5 && x >= gx as f64 + 0.5
                    && let Some(op) = ops.iter_mut().rev().find(|op| op.0 == l && op.1 == k)
                {
                    op.3 += 1;
                }
            }
            if x > xd as f64 && v < 0.002 && (y - floor).abs() < 0.05 {
                rest_ticks[c] += 1;
                if rest_ticks[c] == 3 && !rested[c] {
                    rested[c] = true;
                    rests.push((x - xd as f64, z - (zs[l] as f64 + 0.5), y));
                }
            } else {
                rest_ticks[c] = 0;
            }
            if y < floor - 1.0 && x > xd as f64 - 0.5 {
                if !fell[c] {
                    fell[c] = true;
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
                    && let Some(id) = carts[c].1.take()
                {
                    kill_with_rider(&mut run.world, id);
                    pending.push((t + o.ret, c));
                    fell[c] = false;
                    rested[c] = false;
                    rest_ticks[c] = 0;
                }
            }
        }
        prev = now;
    }
    let mut lines = vec![format!(
        "brigade bench: {} lanes {} apart, {} pockets a lane ({}), each gate {} gt, pocket 1 a powered slope, strings {} above each pocket's rail; the hold (bottom-half trapdoor one below the rail end); the release opens every trapdoor for {} gt every {} gt (corner {:+}); strings held >= {} gt; returns after {} gt into the last pocket; {} cycles",
        nl, o.spacing, kp, if o.riders { "ridden" } else { "empty" }, o.wp, sdy, o.drop_w, o.period, o.corner, o.hold_n, o.ret, o.cycles
    )];
    let per_gate: Vec<String> = (1..=kp)
        .map(|k| {
            let mut m: std::collections::BTreeMap<usize, usize> = Default::default();
            for op in ops.iter().filter(|op| op.1 == k && op.2 >= warm) {
                *m.entry(op.3).or_default() += 1;
            }
            format!("G_{k} [{}]", m.iter().map(|(a, b)| format!("{a}: {b}")).collect::<Vec<_>>().join(", "))
        })
        .collect();
    lines.push(format!("carts per gate opening (exactly 1, after the warm-up): {}", per_gate.join(" ")));
    let mut rh: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &releases {
        for c in &r.1 {
            *rh.entry(*c).or_default() += 1;
        }
    }
    let spreads: Vec<i64> = releases.iter().filter_map(|r| {
        let ts: Vec<i64> = r.2.iter().filter_map(|x| *x).collect();
        if ts.len() == nl { Some(ts.iter().max().unwrap() - ts.iter().min().unwrap()) } else { None }
    }).collect();
    lines.push(format!(
        "carts per hold per release (exactly 1): {}; holds not filled at a release: {}; the batch's spread through the floors: {}",
        rh.iter().map(|(a, b)| format!("{a}: {b}")).collect::<Vec<_>>().join(", "),
        missing_at_release,
        spreads.iter().max().map_or("-".to_string(), |s| format!("max {s} gt"))
    ));
    lines.push(format!("gate and trapdoor shuts onto a cart (the bar: 0): {}{}", shut_onto.len(), if shut_onto.is_empty() { String::new() } else { format!(" ({})", shut_onto.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }));
    lines.push(format!(
        "the strings: on-ticks with a cart in their own pocket {}; with their pocket empty (a neighbour's rider; the bar: 0) {}{}",
        (1..=kp).map(|k| format!("S_{k} {}", own_trips[k])).collect::<Vec<_>>().join(" "),
        (1..=kp).map(|k| format!("S_{k} {}", cross_trips[k])).collect::<Vec<_>>().join(" "),
        if cross_note.is_empty() { String::new() } else { format!(" ({})", cross_note.join("; ")) }
    ));
    if !rests.is_empty() {
        let mm = |f: &dyn Fn(&(f64, f64, f64)) -> f64| (rests.iter().map(f).fold(f64::INFINITY, f64::min), rests.iter().map(f).fold(f64::NEG_INFINITY, f64::max));
        let (x0, x1) = mm(&|r| r.0);
        let (z0, z1) = mm(&|r| r.1);
        lines.push(format!("the hold: {} carts at rest, the centre {x0:.3}..{x1:.3} into the cell (flush on the far wall = 0.51), z off-centre {z0:+.3}..{z1:+.3}", rests.len()));
    } else {
        lines.push("the hold: no cart came to rest on the floor".to_string());
    }
    let refills: Vec<String> = (1..=kp).filter(|k| !refill[*k].is_empty()).map(|k| format!("P{k} {}..{}", refill[k].iter().min().unwrap(), refill[k].iter().max().unwrap())).collect();
    lines.push(format!("each pocket's string held again after the release (gt): {}", refills.join(" ")));
    Ok(lines.join("\n"))
}
