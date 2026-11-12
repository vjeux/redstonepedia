//! The napper's lane-exact siding (ironspec Nov 11 23:01; 9fc3155f's cells, budget f1b95a5451), one
//! lane in the budget's own coordinates (lane x = `lx`, descending toward -z): the hold (x,16,0) (a
//! bottom-half trapdoor floor, glass on three sides, its string at (x,17,0)); R_1 (x,17,1), the switched
//! powered slope (powered from the side, (x+1,17,1)); four plain pressed slopes (x,18..21,2..5); the
//! merge (x,22,6), the split's flat branch at level 22 travelling -z ((x,22,7..9)), with a rider string
//! over it at (x,23,6); the siding beside it: the feed flat (x-1,23,8), the siding slope (x-1,22,7) (a
//! braked switched powered slope descending north, powered from the side (x-2,22,7)), the curve
//! (x-1,22,6) turning east onto the merge. One release line opens the hold's trapdoor every `period`
//! gt; the R line restages once it shuts. A dropped cart returns on the branch after `ret` gt at
//! `v_in` b/t (the split, in drop order); every `nap_every`-th dropped cart is a napper instead: it
//! comes back onto the siding after `nap_ret` gt, braked, and is released (the siding slope powered for
//! `nap_pulse` gt) `k` gt after its predecessor (the cart dropped one cycle before it) passes the
//! merge's string. Prints the napper's way through the merge (it must turn north into the lane), the
//! curve rule at the curve and the merge (the follower's entry minus the leader's clear, >= 0, no
//! touching), its contact with its predecessor's back and the successor's with it (<= 0.46), the
//! lane's batch counts, and the siding's creep.

use crate::dropbench::DropOpts;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

fn st(w: &redsim::world::World, id: u32) -> Option<(f64, f64, f64, f64)> {
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

/// The siding's own options.
#[derive(Clone, Debug)]
pub struct SidingOpts {
    pub k: i64,
    pub nap_every: usize,
    pub nap_ret: i64,
    pub nap_pulse: i64,
    pub merge_curve: bool,
}

/// The bench: returns the text.
pub fn siding_bench(o: &DropOpts, so: &SidingOpts) -> Result<String, String> {
    let lx = 3i32;
    let b = |s: &str| Block::parse(s);
    let glass = b("minecraft:glass")?;
    let stone = b("minecraft:stone")?;
    let air = b("minecraft:air")?;
    let slope_s = b("minecraft:rail[shape=ascending_south,waterlogged=false]")?;
    let flat_ns = b("minecraft:rail[shape=north_south,waterlogged=false]")?;
    let curve_se = b("minecraft:rail[shape=south_east,waterlogged=false]")?;
    let curve_nw = b("minecraft:rail[shape=north_west,waterlogged=false]")?;
    let pslope_s = b("minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]")?;
    let rblock = b("minecraft:redstone_block")?;
    let trap_shut = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string_ew = b("minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,south=false,west=true,powered=false]")?;
    let hook_e = b("minecraft:tripwire_hook[attached=true,facing=east,powered=false]")?;
    let hook_w = b("minecraft:tripwire_hook[attached=true,facing=west,powered=false]")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, blk: &Block| blocks.push((BlockPos::new(x, y, z), blk.clone()));
    // (the hold: trapdoor floor, glass far side (z -1, two high) and both sides, its string over it)
    put(lx, 16, 0, &trap_shut);
    put(lx, 16, -1, &glass);
    put(lx, 17, -1, &glass);
    put(lx - 1, 16, 0, &glass);
    put(lx + 1, 16, 0, &glass);
    put(lx, 17, 0, &string_ew);
    put(lx - 1, 17, 0, &hook_e);
    put(lx + 1, 17, 0, &hook_w);
    put(lx - 2, 17, 0, &stone);
    put(lx + 2, 17, 0, &stone);
    // (R_1 and the four slopes, each on glass)
    put(lx, 17, 1, &pslope_s);
    put(lx, 16, 1, &glass);
    for k in 0..4 {
        put(lx, 18 + k, 2 + k, &slope_s);
        put(lx, 17 + k, 2 + k, &glass);
    }
    // (the merge and the branch: flats at level 22 from z 9 to z 6; the merge's string at (x,23,6))
    for z in 6..=9 {
        // (the merge cell a north_west curve with --merge-curve (the siding from the west and the branch
        // from the south both leave north into the intake), else a straight north_south flat)
        put(lx, 22, z, if z == 6 && so.merge_curve { &curve_nw } else { &flat_ns });
        put(lx, 21, z, &glass);
    }
    put(lx, 22, 10, &glass);
    put(lx, 23, 6, &string_ew);
    put(lx - 1, 23, 6, &hook_e);
    put(lx + 1, 23, 6, &hook_w);
    put(lx - 2, 23, 6, &stone);
    put(lx + 2, 23, 6, &stone);
    // (the siding: the feed flat (x-1,23,8) on glass with glass behind it, the braked slope (x-1,22,7),
    // the curve (x-1,22,6))
    put(lx - 1, 23, 8, &flat_ns);
    put(lx - 1, 22, 8, &glass);
    put(lx - 1, 23, 9, &glass);
    put(lx - 1, 22, 7, &pslope_s);
    put(lx - 1, 21, 7, &glass);
    put(lx - 1, 22, 6, &curve_se);
    put(lx - 1, 21, 6, &glass);
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(lx + 6, 30, 12), lo: BlockPos::new(lx - 6, -12, -6), hi: BlockPos::new(lx + 6, 30, 14), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    let rider = if o.riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt0 = format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}");
    let nbt_in = format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -o.v_in);
    // (the lane's carts: the front braked mid-R_1, the rest up the four slopes 0.79 apart (z), the
    // overflow arriving on the branch as returns)
    let zc = lx as f64 + 0.5;
    let slope_y = |z: f64| {
        let cz = z.floor() as i32;
        // (ascending south: block (x, 16 + cz, cz) rises toward +z)
        (16 + cz) as f64 + (z - cz as f64) + 0.0625
    };
    let mut carts: Vec<Option<u32>> = Vec::new();
    let mut pending: Vec<(i64, usize, bool)> = Vec::new();
    for k in 0..o.per_lane {
        let z = 1.5 + 0.79 * k as f64;
        if z > 5.6 {
            carts.push(None);
            pending.push((20 + 25 * k as i64, carts.len() - 1, false));
            continue;
        }
        carts.push(Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, slope_y(z), z], Some(&nbt0))?));
    }
    let n = carts.len();
    let warm = 300i64;
    let rel_end = warm + o.cycles * o.period;
    let ticks = rel_end + 400;
    let hold_hook = BlockPos::new(lx - 1, 17, 0);
    let merge_hook = BlockPos::new(lx - 1, 23, 6);
    let (mut trap_until, mut trap_is) = (-1i64, false);
    let (mut r_until, mut r_is) = (10 + o.r_pulse, false);
    let (mut s_until, mut s_is) = (-1i64, false);
    let mut held = 0i64;
    let mut drops: Vec<(i64, usize)> = Vec::new();
    let mut per_release: Vec<usize> = Vec::new();
    let mut fell = vec![false; n];
    let mut prev: Vec<Option<(f64, f64, f64, f64)>> = vec![None; n];
    // (the napper bookkeeping: the cart index of the napper waiting, its predecessor's index, the
    // predecessor's merge pass tick, the release tick)
    let mut nap: Option<(usize, usize)> = None;
    let mut pred_pass: Option<i64> = None;
    let mut released_at: Option<i64> = None;
    let mut merge_was = false;
    let mut nap_rows: Vec<String> = Vec::new();
    let mut contacts: Vec<(usize, f64, f64, f64)> = Vec::new();
    let mut creep = 0.0f64;
    let mut nap_rest: Option<(f64, f64)> = None;
    let mut wrong_way: Vec<String> = Vec::new();
    // (the curve rule: who is on the curve (x-1,22,6) and the merge (x,22,6), entry and clear ticks)
    let in_cell = |p: (f64, f64, f64), cx: i32, cy: i32, cz: i32| p.0 + 0.49 > cx as f64 && p.0 - 0.49 < (cx + 1) as f64 && p.2 + 0.49 > cz as f64 && p.2 - 0.49 < (cz + 1) as f64 && p.1 > cy as f64 - 0.5 && p.1 < cy as f64 + 1.0;
    let mut merge_occ: Vec<(usize, i64, Option<i64>)> = Vec::new();
    let mut touching = 0usize;
    let mut touch_note: Vec<String> = Vec::new();
    let mut drop_count = 0usize;
    for t in 0..ticks {
        let hh = matches!(run.world.get(hold_hook), Block::TripwireHook { powered: true, .. });
        held = if hh { held + 1 } else { 0 };
        let mh = matches!(run.world.get(merge_hook), Block::TripwireHook { powered: true, .. });
        // (the predecessor's pass at the merge string: the rising edge while a napper waits and its
        // predecessor is the cart over the merge)
        if mh && !merge_was
            && let Some((_, pred)) = nap
            && pred_pass.is_none()
            && let Some(id) = carts[pred]
            && let Some(p) = st(&run.world, id)
            && in_cell((p.0, p.1, p.2), lx, 22, 6)
        {
            pred_pass = Some(t);
        }
        merge_was = mh;
        // (the release: the predecessor's pass seen, K gt after it, and the napper resting on the siding
        // (the napper window's evidence that it is queued there))
        let nap_ready = nap.is_some_and(|(nc, _)| carts[nc].and_then(|id| st(&run.world, id)).is_some_and(|p| p.3 < 0.002 && p.1 > 21.9 && (p.0 - (lx as f64 - 0.5)).abs() < 0.3));
        if let (Some(pp), None) = (pred_pass, released_at)
            && t >= pp + so.k
            && nap_ready
        {
            s_until = t + so.nap_pulse;
            released_at = Some(t);
        }
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && t < rel_end && ph == 0 {
            trap_until = t + o.drop_w;
            per_release.push(0);
        }
        let want_trap = t < trap_until;
        if want_trap != trap_is {
            trap_is = want_trap;
            run.world.set_block(BlockPos::new(lx, 16, 0), if want_trap { trap_open.clone() } else { trap_shut.clone() }, redsim::world::flags::ALL);
            if !want_trap {
                r_until = t + o.r_pulse;
            }
        }
        let want_r = t < r_until;
        if want_r != r_is {
            r_is = want_r;
            run.world.set_block(BlockPos::new(lx + 1, 17, 1), if want_r { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
        }
        let want_s = t < s_until;
        if want_s != s_is {
            s_is = want_s;
            run.world.set_block(BlockPos::new(lx - 2, 22, 7), if want_s { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
        }
        // (returns: on the branch at z 8.5 moving -z, or a napper onto the siding slope at rest)
        let due: Vec<(usize, bool)> = pending.iter().filter(|p| p.0 <= t).map(|p| (p.1, p.2)).collect();
        for (c, is_nap) in due {
            if is_nap {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [lx as f64 - 0.5, 22.0 + 0.5 + 0.0625, 7.5], Some(&nbt0))?;
                carts[c] = Some(id);
                pending.retain(|p| p.1 != c);
                continue;
            }
            let clear = carts.iter().all(|id| id.and_then(|id| st(&run.world, id)).is_none_or(|(x, y, z, _)| !(y > 21.5 && (z - 8.5).abs() < 1.3 && (x - zc).abs() < 0.9)));
            if clear {
                carts[c] = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, 22.0625, 8.5], Some(&nbt_in))?);
                pending.retain(|p| p.1 != c);
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|id| id.and_then(|id| st(&run.world, id))).collect();
        if let Ok(tr) = std::env::var("MCTAS_SIDING_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 2 && t >= p[0] && t < p[1] {
                let xs: Vec<String> = (0..n).filter_map(|c| now[c].map(|s| format!("{c}:{:.2},{:.2},{:.2}/{:.2}", s.0, s.1, s.2, s.3))).collect();
                eprintln!("t {t}: trap {} R {} S {} hold {} merge {} : {}", trap_is as u8, r_is as u8, s_is as u8, hh as u8, mh as u8, xs.join(" "));
            }
        }
        // (the merge's occupants: entry and clear)
        for c in 0..n {
            let on = now[c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, 22, 6));
            let open = merge_occ.iter().position(|m| m.0 == c && m.2.is_none());
            match (on, open) {
                (true, None) => merge_occ.push((c, t, None)),
                (false, Some(i)) => merge_occ[i].2 = Some(t),
                _ => {}
            }
        }
        let on_merge = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, 22, 6))).count();
        if on_merge > 1 {
            touching += 1;
            if touch_note.len() < 4 {
                let who: Vec<String> = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, 22, 6))).map(|c| { let p = now[c].unwrap(); format!("cart {c} at {:.2},{:.2},{:.2}/{:.2}", p.0, p.1, p.2, p.3) }).collect();
                touch_note.push(format!("t {t}: {}", who.join(", ")));
            }
        }
        for c in 0..n {
            let (Some((x, y, z, v)), Some((_, _, _, pv))) = (now[c], prev[c]) else { continue };
            // (the napper waiting: its creep while braked; the wrong way: south past z 7 on the branch)
            if let Some((nc, _)) = nap
                && nc == c
            {
                if released_at.is_none() {
                    // (the creep, measured only once the napper rests on the siding)
                    match nap_rest {
                        None if v < 0.002 && y > 21.9 && (x - (lx as f64 - 0.5)).abs() < 0.3 => nap_rest = Some((x, z)),
                        Some((x0, z0)) => creep = creep.max(((x - x0).powi(2) + (z - z0).powi(2)).sqrt()),
                        _ => {}
                    }
                } else if (x - zc).abs() < 0.6 && z > 7.2 && y > 21.5 {
                    if wrong_way.len() < 4 {
                        wrong_way.push(format!("t {t}: the napper (cart {c}) at {x:.2},{y:.2},{z:.2} heading up the branch"));
                    }
                }
            }
            // (contacts in the lane: a stored speed dropping by 0.04 or more with a cart within 1.3 ahead
            // (toward -z) in the lane's column)
            if pv - v >= 0.04
                && (x - zc).abs() < 0.6
                && let Some(j) = (0..n).filter(|&j| j != c && now[j].is_some_and(|(xj, _, zj, _)| (xj - zc).abs() < 0.6 && zj < z && z - zj < 1.3)).min_by(|a, b| now[*b].unwrap().2.total_cmp(&now[*a].unwrap().2))
            {
                contacts.push((c, pv, z, now[j].map_or(f64::NAN, |p| p.2)));
                let _ = j;
            }
            // (through the floor)
            if y < 16.0 - 1.5 && z < 1.0 {
                if !fell[c] {
                    fell[c] = true;
                    if let Some(r) = per_release.last_mut() {
                        *r += 1;
                    }
                    drop_count += 1;
                    drops.push((t, c));
                    // (the napper: every nap_every-th drop, if none waits; its predecessor = the cart
                    // dropped one cycle before)
                    let is_nap = so.nap_every > 0 && drop_count % so.nap_every == 0 && nap.is_none() && drops.len() >= 2;
                    if is_nap {
                        let pred = drops[drops.len() - 2].1;
                        nap = Some((c, pred));
                        pred_pass = None;
                        released_at = None;
                        nap_rest = None;
                        creep = 0.0;
                    }
                    pending.push((t + if is_nap { so.nap_ret } else { o.ret }, c, is_nap));
                }
                if y < 16.0 - 4.0
                    && let Some(id) = carts[c].take()
                {
                    kill_with_rider(&mut run.world, id);
                    fell[c] = false;
                }
            }
            // (the napper at the lane's back: past the merge, at rest on the intake side; its row closes)
            if let (Some((nc, pred)), Some(ra)) = (nap, released_at)
                && nc == c
                && z < 6.0
                && v < 0.002
                && t > ra + 5
                && (x - zc).abs() < 0.6
            {
                let m_pred = merge_occ.iter().rev().find(|m| m.0 == pred && m.1 <= ra + 400).and_then(|m| m.2);
                let m_nap = merge_occ.iter().rev().find(|m| m.0 == c).map(|m| m.1);
                let curve_rule = match (m_pred, m_nap) { (Some(cl), Some(en)) => format!("{}", en - cl), _ => "-".to_string() };
                let c_nap = contacts.iter().filter(|k| k.0 == c).map(|k| k.1).fold(0.0f64, f64::max);
                nap_rows.push(format!(
                    "napper cart {c} (predecessor {pred}): the predecessor over the merge string t {}, released t {ra}, at the lane's back t {t} (z {z:.2}); the merge's curve rule (its entry minus the predecessor's clear) {curve_rule} gt; its contact {c_nap:.3}; braked creep {creep:.4}",
                    pred_pass.map_or("-".to_string(), |p| p.to_string())
                ));
                nap = None;
            }
        }
        prev = now;
    }
    let mut ph: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &per_release {
        *ph.entry(*r).or_default() += 1;
    }
    let cmax = contacts.iter().map(|k| k.1).fold(0.0f64, f64::max);
    let mut lines = vec![format!(
        "siding bench: lane x {lx} in the budget's cells (the hold (x,16,0), R_1 (x,17,1), four slopes, the merge (x,22,6) with its string, the siding (x-1,22,7) and its curve), {} carts ({}), the R line {} gt, the trapdoor {} gt every {} gt (corner {:+}), returns on the branch after {} gt at {:.2} b/t, a napper every {} drops back on the siding after {} gt, released {} gt after its predecessor's merge pass for {} gt; {} cycles",
        o.per_lane, if o.riders { "ridden" } else { "empty" }, o.r_pulse, o.drop_w, o.period, o.corner, o.ret, o.v_in, so.nap_every, so.nap_ret, so.k, so.nap_pulse, o.cycles
    )];
    lines.push(format!("carts per release (exactly 1): {}", ph.iter().map(|(a, b2)| format!("{a}: {b2}")).collect::<Vec<_>>().join(", ")));
    lines.push(format!("nappers through the siding: {}", if nap_rows.is_empty() { "none".to_string() } else { nap_rows.join(" | ") }));
    lines.push(format!("the napper heading the wrong way at the merge (the bar: 0): {}{}", wrong_way.len(), if wrong_way.is_empty() { String::new() } else { format!(" ({})", wrong_way.join("; ")) }));
    lines.push(format!("ticks with two carts over the merge (the curve rule's touching; the bar: 0): {touching}{}", if touch_note.is_empty() { String::new() } else { format!(" ({})", touch_note.join("; ")) }));
    lines.push(format!("contacts in the lane: {}, {} above 0.46, the hardest {cmax:.3}", contacts.len(), contacts.iter().filter(|k| k.1 > 0.46).count()));
    if let Some((c, _)) = nap {
        lines.push(format!("a napper still waiting at the end: cart {c} (predecessor pass {:?}, released {:?})", pred_pass, released_at));
    }
    Ok(lines.join("\n"))
}
