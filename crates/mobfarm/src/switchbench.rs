//! The switched-R lane into the drop cell (ironspec Nov 11 22:32; dfb89499's switched R_1): `lanes`
//! parallel lanes along +x, `spacing` apart in z. Each lane: a pressed run of `run` plain slopes; R, a
//! powered slope as the lane's last rail cell, switched by one line for every lane (a redstone block
//! beside it, clear of the trapdoor; unpowered it brakes); R's low end is the rail end; then D, the
//! off-rail drop cell (a bottom-half trapdoor one block below the rail end, glass on three sides, its
//! rider string above). At rest cart 1 sits in D and cart 2 rests on R, braked, against cart 1's rider.
//! One release line opens every trapdoor for `drop_w` gt each `period` (cart 1 falls, cart 2 stays
//! braked); once the trapdoors shut, the R line powers every R for `r_pulse` gt: cart 2 rolls off the
//! rail end into D, cart 3 follows onto R and is stopped by cart 2's rider, then halts braked when R
//! unpowers. A D whose string hasn't held `hold_n` gt by `retry` gt after the pulse gets the pulse
//! again (every lane together, as one line). A cart through the floor is taken with its rider and
//! returns after `ret` gt at rest near its lane's top (an ideal intake). `--login T,D` lands a login at
//! T: the entities frozen for D gt while the redstone runs (the level-32 freeze).

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

fn rider_eye(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64)> {
    let i = redsim::entity::index_of(w, id)?;
    let r = *w.entities.list[i].passengers.first()?;
    let j = redsim::entity::index_of(w, r)?;
    let e = &w.entities.list[j];
    Some((e.pos.x, e.pos.y + 1.62, e.pos.z))
}

fn kill_with_rider(w: &mut redsim::world::World, id: u32) {
    let riders: Vec<u32> = redsim::entity::index_of(w, id).map(|i| w.entities.list[i].passengers.clone()).unwrap_or_default();
    for r in riders {
        let _ = redsim::entity::kill(w, r);
    }
    let _ = redsim::entity::kill(w, id);
}

/// One run's summary for the matrices.
pub struct SwitchRow {
    pub per_release: std::collections::BTreeMap<usize, usize>,
    pub shuts: usize,
    pub unsettled: usize,
    pub retries: usize,
    pub hanging: usize,
    /// Arrivals at the lanes' backs above the brief's 0.46 gravity-run bar.
    pub arrivals_over: usize,
    pub text: String,
}

/// The bench: returns the summary and the text.
pub fn switch_bench(o: &DropOpts) -> Result<SwitchRow, String> {
    let glass = Block::parse("minecraft:glass")?;
    let stone = Block::parse("minecraft:stone")?;
    let air = Block::parse("minecraft:air")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let pslope = Block::parse("minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    let rblock = Block::parse("minecraft:redstone_block")?;
    let trap_shut = Block::parse("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = Block::parse("minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string = Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_s = Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_n = Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let top = o.run + 2;
    let xr = o.run;
    let h = top - 1 - o.run;
    let xd = xr + 1;
    let floor = h as f64 - 1.0 + 0.1875;
    let zs: Vec<i32> = (0..o.lanes as i32).map(|l| l * o.spacing).collect();
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for &z in &zs {
        for x in 0..o.run {
            blocks.push((BlockPos::new(x, top - 1 - x, z), slope.clone()));
            blocks.push((BlockPos::new(x, top - 2 - x, z), glass.clone()));
        }
        if o.intake_flat {
            // (the flat intake and the split's flat branch behind it, level with the top slope's high end)
            blocks.push((BlockPos::new(-1, top, z), flat.clone()));
            blocks.push((BlockPos::new(-1, top - 1, z), glass.clone()));
            blocks.push((BlockPos::new(-2, top, z), flat.clone()));
            blocks.push((BlockPos::new(-2, top - 1, z), glass.clone()));
            blocks.push((BlockPos::new(-3, top, z), glass.clone()));
        } else {
            blocks.push((BlockPos::new(-1, top, z), glass.clone()));
        }
        blocks.push((BlockPos::new(xr, h, z), pslope.clone()));
        blocks.push((BlockPos::new(xr, h - 1, z), glass.clone()));
        // (D: the trapdoor floor, the far wall two high, the sides at the cart's level, the string)
        blocks.push((BlockPos::new(xd, h - 1, z), trap_shut.clone()));
        blocks.push((BlockPos::new(xd + 1, h - 1, z), glass.clone()));
        blocks.push((BlockPos::new(xd + 1, h, z), glass.clone()));
        blocks.push((BlockPos::new(xd, h - 1, z - 1), glass.clone()));
        blocks.push((BlockPos::new(xd, h - 1, z + 1), glass.clone()));
        blocks.push((BlockPos::new(xd, h, z), string.clone()));
        blocks.push((BlockPos::new(xd, h, z - 1), hook_s.clone()));
        blocks.push((BlockPos::new(xd, h, z + 1), hook_n.clone()));
        blocks.push((BlockPos::new(xd, h, z - 2), stone.clone()));
        blocks.push((BlockPos::new(xd, h, z + 2), stone.clone()));
    }
    let hi_z = *zs.last().unwrap_or(&0);
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(xd + 4, top + 2, hi_z + 4), lo: BlockPos::new(-4, h - 26, -4), hi: BlockPos::new(xd + 6, top + 4, hi_z + 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let nbt_in = format!("{{Motion:[{:.4}d,0.0d,0.0d]{rider}}}", o.v_in);
    let slope_y = |x: f64| {
        let cx = x.floor() as i32;
        (top - 1 - cx) as f64 + (1.0 - (x - cx as f64)) + 0.0625
    };
    // (the lanes: the front cart braked mid-R, the rest 0.79 apart up the run; D empty, so the first
    // restage fills it)
    let nl = o.lanes;
    let mut carts: Vec<(usize, Option<u32>)> = Vec::new();
    let mut overflow: Vec<(i64, usize)> = Vec::new();
    for l in 0..nl {
        for k in 0..o.per_lane {
            let x = xr as f64 + 0.5 - 0.79 * k as f64;
            if x < 0.6 {
                // (a lane's carts beyond the run start as returns, arriving through the intake as room
                // allows: the fill level)
                carts.push((l, None));
                overflow.push((40 + 25 * k as i64, carts.len() - 1));
                continue;
            }
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, slope_y(x), zs[l] as f64 + 0.5], Some(&nbt))?;
            carts.push((l, Some(id)));
        }
    }
    let n = carts.len();
    let warm = 300i64;
    // (the last release gets 60 gt to finish: releases stop at the cycles' end, the run goes on)
    let rel_end = warm + o.cycles * o.period;
    let ticks = rel_end + 60;
    let hook = |l: usize| BlockPos::new(xd, h, zs[l] - 1);
    let mut held = vec![0i64; nl];
    let mut trap_until = -1i64;
    let mut trap_is = false;
    let mut r_until = 10i64 + o.r_pulse;
    let mut r_is = false;
    let mut retry_at = 10 + o.r_pulse + o.retry;
    let mut retries = 0usize;
    let mut releases: Vec<(i64, Vec<usize>)> = Vec::new();
    let mut unsettled = 0usize;
    let mut shut_onto: Vec<String> = Vec::new();
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    let mut pending: Vec<(i64, usize)> = overflow;
    let mut fell = vec![false; n];
    let mut rest_d: Vec<f64> = Vec::new();
    let mut rested_d = vec![false; n];
    let mut rest_ticks = vec![0i64; n];
    // (the cart braked on R: its stop place against R's cell, the speed it had before the stop, and
    // its creep while R is unpowered)
    let mut r_stops: Vec<(f64, f64)> = Vec::new();
    let mut moving_peak = vec![0.0f64; n];
    let mut r_rest_x: Vec<Option<f64>> = vec![None; n];
    let mut creep_max = 0.0f64;
    let mut hanging: Vec<String> = Vec::new();
    let mut arrivals: Vec<f64> = Vec::new();
    let mut hang_ticks = vec![0i64; n];
    let mut eye_d: std::collections::BTreeMap<String, usize> = Default::default();
    let mut eye_r: std::collections::BTreeMap<String, usize> = Default::default();
    let mut frozen32: Option<[i32; 4]> = None;
    let mut first_held: Option<i64> = None;
    for t in 0..ticks {
        // (the logins and freezes as one schedule: frozen while any interval covers t, so overlapping
        // ones neither drop a start nor thaw early)
        let want_frozen = o.login.iter().chain(o.freeze32.iter()).any(|(a, d)| t >= *a && t < a + d);
        if want_frozen && frozen32.is_none() {
            frozen32 = Some(crate::liveness::freeze32(&mut run.world));
        }
        if !want_frozen
            && let Some(pb) = frozen32.take()
        {
            crate::liveness::thaw32(&mut run.world, pb);
        }
        let hk: Vec<bool> = (0..nl).map(|l| matches!(run.world.get(hook(l)), Block::TripwireHook { powered: true, .. })).collect();
        for l in 0..nl {
            held[l] = if hk[l] { held[l] + 1 } else { 0 };
        }
        let full: Vec<bool> = (0..nl).map(|l| held[l] >= o.hold_n).collect();
        if first_held.is_none() && full.iter().all(|f| *f) {
            first_held = Some(t);
        }
        // (the release line)
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && t < rel_end && ph == 0 {
            trap_until = t + o.drop_w;
            unsettled += full.iter().filter(|f| !**f).count();
            releases.push((t, vec![0; nl]));
        }
        let want_trap = t < trap_until;
        if want_trap != trap_is {
            trap_is = want_trap;
            for &z in &zs {
                run.world.set_block(BlockPos::new(xd, h - 1, z), if want_trap { trap_open.clone() } else { trap_shut.clone() }, redsim::world::flags::ALL);
            }
            if !want_trap {
                for (c, (l, id)) in carts.iter().enumerate() {
                    if let Some(id) = id
                        && let Some((x, y, _, _)) = cart_state(&run.world, *id)
                        && x + 0.49 > xd as f64
                        && x - 0.49 < (xd + 1) as f64
                        && y < h as f64 - 1.0 + 0.1875 - 0.001
                        && y + 0.7 > h as f64 - 1.0 + 0.001
                    {
                        shut_onto.push(format!("t {t}: the trapdoor of lane {l} onto cart {c} at {x:.2},{y:.2}"));
                    }
                }
                // (the restage: the R line once the trapdoors are shut)
                r_until = t + o.r_pulse;
                retry_at = t + o.r_pulse + o.retry;
            }
        }
        // (a retry: a D still not settled once the pulse has had its time)
        if t == retry_at && !trap_is && full.iter().any(|f| !*f) {
            r_until = t + o.r_pulse;
            retry_at = t + o.r_pulse + o.retry;
            retries += 1;
        }
        let want_r = t < r_until;
        if want_r != r_is {
            r_is = want_r;
            for &z in &zs {
                run.world.set_block(BlockPos::new(xr, h, z + 1), if want_r { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
            }
        }
        // (returns near the lane's top once clear; with --intake-flat, onto the split's flat branch two
        // cells above the run's top slope, moving toward the lane at --v-in)
        let due: Vec<usize> = pending.iter().filter(|p| p.0 <= t).map(|p| p.1).collect();
        for c in due {
            let l = carts[c].0;
            let x = if o.intake_flat { -1.5 } else { 1.5 };
            let clear = carts.iter().filter(|(ll, _)| *ll == l).all(|(_, id)| id.and_then(|id| cart_state(&run.world, id)).is_none_or(|(cx, _, _, _)| cx > x + 1.2));
            if clear && frozen32.is_none() {
                let (y, nb) = if o.intake_flat { (top as f64 + 0.0625, nbt_in.clone()) } else { (slope_y(x), nbt.clone()) };
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, y, zs[carts[c].0] as f64 + 0.5], Some(&nb))?;
                carts[c].1 = Some(id);
                pending.retain(|p| p.1 != c);
            }
        }
        // (a restart between ticks t - 1 and t (cc767b78's helper, 58f6f0084): a save/stop/start in a
        // load order, every entity back under a new id, followed; the controller runs on)
        for (rt, how) in &o.restart {
            if *rt == t {
                crate::liveness::restart(&mut run.world, *how)?;
                for (_, id) in carts.iter_mut() {
                    if let Some(i) = id {
                        *i = crate::liveness::follow(&run.world, *i);
                    }
                }
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|(_, id)| id.and_then(|id| cart_state(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_SWITCH_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 3 && t >= p[1] && t < p[2] {
                let l = p[0] as usize;
                let xs: Vec<String> = (0..n).filter(|&c| carts[c].0 == l).filter_map(|c| now[c].map(|s| format!("{c}:{:.2},{:.2}/{:.2}", s.0, s.1, s.3))).take(4).collect();
                eprintln!("t {t}: R {} trap {} string {} : {}", r_is as u8, trap_is as u8, hk[l] as u8, xs.join(" "));
            }
        }
        for c in 0..n {
            let l = carts[c].0;
            let (Some((x, y, z, v)), Some((_, _, _, pv))) = (now[c], prev[c]) else { continue };
            let id = carts[c].1.unwrap_or(0);
            // (at rest in D)
            if x > xd as f64 && v < 0.002 && (y - floor).abs() < 0.05 {
                rest_ticks[c] += 1;
                if rest_ticks[c] == 3 && !rested_d[c] {
                    rested_d[c] = true;
                    rest_d.push(x - xd as f64);
                    if let Some(e) = rider_eye(&run.world, id) {
                        let b = run.world.get(BlockPos::new(e.0.floor() as i32, e.1.floor() as i32, e.2.floor() as i32));
                        *eye_d.entry(format!("{b:?}").chars().take(40).collect()).or_default() += 1;
                    }
                }
            } else {
                rest_ticks[c] = 0;
            }
            let _ = z;
            // (on R: the stop after a restage, and the creep while braked)
            let on_r = x >= xr as f64 && x < xd as f64 && y > h as f64 - 0.1;
            if on_r {
                moving_peak[c] = moving_peak[c].max(pv);
                if v < 0.002 && pv < 0.002 {
                    if r_rest_x[c].is_none() {
                        r_rest_x[c] = Some(x);
                        if moving_peak[c] > 0.01 {
                            r_stops.push((x - xr as f64, moving_peak[c]));
                        }
                        if let Some(e) = rider_eye(&run.world, id) {
                            let b = run.world.get(BlockPos::new(e.0.floor() as i32, e.1.floor() as i32, e.2.floor() as i32));
                            *eye_r.entry(format!("{b:?}").chars().take(40).collect()).or_default() += 1;
                        }
                    } else if !r_is
                        && let Some(x0) = r_rest_x[c]
                    {
                        creep_max = creep_max.max((x - x0).abs());
                    }
                } else if r_is {
                    r_rest_x[c] = None;
                }
                if v > 0.01 {
                    moving_peak[c] = moving_peak[c].max(v);
                }
            } else {
                moving_peak[c] = 0.0;
                r_rest_x[c] = None;
            }
            // (hanging: at rest with the centre past the rail end (off R) but not on D's floor: the box
            // caught on the rail end's support column)
            if v < 0.002 && x >= xd as f64 && x < xd as f64 + 0.49 && (y - floor).abs() > 0.05 {
                hang_ticks[c] += 1;
                if hang_ticks[c] == 40 && hanging.len() < 6 {
                    hanging.push(format!("t {t}: lane {l} cart {c} at {x:.2},{y:.2}"));
                }
            } else {
                hang_ticks[c] = 0;
            }
            // (through the floor)
            // (an arrival at the lane's back: the stored speed dropping by 0.04 or more with a cart of
            // the lane within 1.3 ahead, behind R)
            if pv - v >= 0.04 && x < xr as f64 && (0..n).any(|j| j != c && carts[j].0 == l && now[j].is_some_and(|(xj, _, _, _)| xj > x && xj - x < 1.3)) {
                arrivals.push(pv);
            }
            if y < floor - 1.0 && x > xd as f64 - 0.5 {
                if !fell[c] {
                    fell[c] = true;
                    if let Some(r) = releases.last_mut()
                        && t - r.0 < o.period
                    {
                        r.1[l] += 1;
                    }
                }
                if y < floor - 3.0
                    && let Some(id) = carts[c].1.take()
                {
                    kill_with_rider(&mut run.world, id);
                    pending.push((t + o.ret, c));
                    fell[c] = false;
                    rested_d[c] = false;
                    rest_ticks[c] = 0;
                }
            }
        }
        prev = now;
    }
    let mut per_release: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &releases {
        for c in &r.1 {
            *per_release.entry(*c).or_default() += 1;
        }
    }
    let mut lines = vec![format!(
        "switched-R bench: {} lanes {} apart, a {}-slope pressed run ({} carts, {}), R a switched powered slope (the R line {} gt), D (bottom-half trapdoor one below the rail end, string); the release opens every trapdoor for {} gt every {} gt (corner {:+}); retry {} gt after a pulse; strings held >= {} gt; returns after {} gt; {} cycles{}",
        nl, o.spacing, o.run, o.per_lane, if o.riders { "ridden" } else { "empty" }, o.r_pulse, o.drop_w, o.period, o.corner, o.retry, o.hold_n, o.ret, o.cycles,
        if o.login.is_empty() { String::new() } else { format!(", logins {:?}", o.login) }
    )];
    lines.push(format!(
        "carts per D per release (exactly 1): {}; cells not settled (string held) at a release: {}; R-line retries: {}",
        per_release.iter().map(|(a, b)| format!("{a}: {b}")).collect::<Vec<_>>().join(", "),
        unsettled,
        retries
    ));
    lines.push(format!("trapdoor shuts onto a cart (the bar: 0): {}{}", shut_onto.len(), if shut_onto.is_empty() { String::new() } else { format!(" ({})", shut_onto.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    if !rest_d.is_empty() {
        let (a, b) = (rest_d.iter().copied().fold(f64::INFINITY, f64::min), rest_d.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        lines.push(format!("D: {} carts at rest, the centre {a:.3}..{b:.3} into the cell (flush on the far wall = 0.51)", rest_d.len()));
    } else {
        lines.push("D: no cart came to rest on the floor".to_string());
    }
    if !r_stops.is_empty() {
        let (a, b) = (r_stops.iter().map(|s| s.0).fold(f64::INFINITY, f64::min), r_stops.iter().map(|s| s.0).fold(f64::NEG_INFINITY, f64::max));
        let vmax = r_stops.iter().map(|s| s.1).fold(0.0f64, f64::max);
        lines.push(format!(
            "the next cart on R after a restage: {} stops, the centre {a:.3}..{b:.3} into R's cell (its low end, the rail end, at 1.0), the fastest before the stop {vmax:.3} b/t; creep while braked (max) {creep_max:.4}",
            r_stops.len()
        ));
    }
    lines.push(format!("carts hanging over the rail end's support column (the bar: 0): {}{}", hanging.len(), if hanging.is_empty() { String::new() } else { format!(" ({})", hanging.join("; ")) }));
    let amax = arrivals.iter().copied().fold(0.0f64, f64::max);
    lines.push(format!("arrivals at the lanes' backs ({}): {}, {} above 0.46, the hardest {amax:.3}", if o.intake_flat { format!("returns on the split's flat at {:.2} b/t", o.v_in) } else { "ideal returns near the top".to_string() }, arrivals.len(), arrivals.iter().filter(|a| **a > 0.46).count()));
    lines.push(format!(
        "the riders' eye blocks: in D {}; on R {}",
        eye_d.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(", "),
        eye_r.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(", ")
    ));
    // (the cold start: the bench starts built, D empty and the front cart braked on R; the first R pulse
    // restages; the first release must already drop 1 cart per D)
    let first = releases.first().map(|r| r.1.clone()).unwrap_or_default();
    lines.push(format!(
        "the cold start: the first release (t {}) dropped {:?} (1 per D expected), the first D string held {} gt after the start",
        releases.first().map_or(-1, |r| r.0),
        first,
        first_held.map_or("-".to_string(), |v| v.to_string())
    ));
    Ok(SwitchRow { per_release, shuts: shut_onto.len(), unsettled, retries, hanging: hanging.len(), arrivals_over: arrivals.iter().filter(|a| **a > 0.46).count(), text: lines.join("\n") })
}
