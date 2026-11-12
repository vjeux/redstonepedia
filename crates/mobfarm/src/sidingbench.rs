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
    /// The lane's plain slopes behind R_1 (the merge at level 18 + N).
    pub slopes: i32,
    /// A guard string over the branch cell before the merge: release only while it is off.
    pub guard: bool,
    /// Release a napper only K after a predecessor's pass seen while it waits (else a lane-lap).
    pub early_only: bool,
    /// The siding's distance west of the lane (1: adjacent to the branch, as laid; 2: one cell clear).
    pub siding_dx: i32,
    /// The siding's rest raised this many levels (9fc3155f: 1, out of the branch carts' push range).
    pub siding_dy: i32,
    /// The merge string north-south on glass supports (nothing over the siding's path at x-1/x-2).
    pub ns_merge: bool,
    /// The guard as a detector rail in the branch cell (x,m,mz+2) (no hooks near the siding).
    pub guard_det: bool,
    /// The passive drop entry (ironspec 00:41): the merge one level up, its north end a rail end over
    /// the drop onto the lane's top slope, a backstop wall two cells north of the merge.
    pub drop_entry: bool,
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
    let flat_ew = b("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let pslope_s = b("minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]")?;
    let rblock = b("minecraft:redstone_block")?;
    let trap_shut = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]")?;
    let trap_open = b("minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]")?;
    let string_ew = b("minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,south=false,west=true,powered=false]")?;
    let hook_e = b("minecraft:tripwire_hook[attached=true,facing=east,powered=false]")?;
    let hook_w = b("minecraft:tripwire_hook[attached=true,facing=west,powered=false]")?;
    let string_ns = b("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_sf = b("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_nf = b("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let det_ns = b("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]")?;
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
    // (R_1 and the N slopes, each on glass; the merge level m = 18 + N at z mz = N + 2; with the drop
    // entry the merge one level up (m = 19 + N): its glass support (x,18+N,mz) beside the top slope's
    // high end (no rail joins them), the drop column (x,m..m-1,mz-1) empty, the backstop wall (x,m,mz-2))
    let ns = so.slopes;
    let m = 18 + ns + so.drop_entry as i32;
    let mz = ns + 2;
    if so.drop_entry {
        put(lx, m, mz - 2, &stone);
    }
    put(lx, 17, 1, &pslope_s);
    put(lx, 16, 1, &glass);
    for k in 0..ns {
        put(lx, 18 + k, 2 + k, &slope_s);
        put(lx, 17 + k, 2 + k, &glass);
    }
    // (the merge and the branch: flats at level m from z mz + 3 to mz; the merge's string over it; with
    // --guard-det the branch cell (x,m,mz+2) a detector rail (straight: it can't be reshaped))
    for z in mz..=mz + 3 {
        // (the merge cell a north_west curve with --merge-curve (the siding from the west and the branch
        // from the south both leave north into the intake), else a straight north_south flat)
        let blk = if z == mz && so.merge_curve { &curve_nw } else if z == mz + 2 && so.guard_det { &det_ns } else { &flat_ns };
        put(lx, m, z, blk);
        put(lx, m - 1, z, &glass);
    }
    put(lx, m, mz + 4, &glass);
    if so.drop_entry {
        // (the pass string across the drop column at the merge level, (x,m,mz-1), its hooks (x+-1,m,mz-1)
        // on glass (x+-2,m,mz-1): every arrival flies through it; nothing over the branch or the siding)
        put(lx, m, mz - 1, &string_ew);
        put(lx - 1, m, mz - 1, &hook_e);
        put(lx + 1, m, mz - 1, &hook_w);
        put(lx - 2, m, mz - 1, &glass);
        put(lx + 2, m, mz - 1, &glass);
    } else if so.ns_merge {
        // (the merge string north-south: hooks (x,m+1,mz-1) facing south and (x,m+1,mz+1) facing north,
        // on glass at (x,m+1,mz-2) and (x,m+1,mz+2): nothing at x-1 or x-2 over the siding's path)
        put(lx, m + 1, mz, &string_ns);
        put(lx, m + 1, mz - 1, &hook_sf);
        put(lx, m + 1, mz + 1, &hook_nf);
        put(lx, m + 1, mz - 2, &glass);
        put(lx, m + 1, mz + 2, &glass);
    } else {
        put(lx, m + 1, mz, &string_ew);
        put(lx - 1, m + 1, mz, &hook_e);
        put(lx + 1, m + 1, mz, &hook_w);
        put(lx - 2, m + 1, mz, &stone);
        put(lx + 2, m + 1, mz, &stone);
    }
    // (the siding, `siding_dx` west of the lane (sx = x - dx): the feed flat (sx,m+1,mz+2) on glass with
    // glass behind it, the braked slope (sx,m,mz+1), the curve (sx,m,mz) turning east, then (dx 2) one
    // east-west flat (x-1,m,mz) onto the merge)
    let sx = lx - so.siding_dx.max(1);
    if so.siding_dy > 0 {
        // (9fc3155f's raised siding: the rest a braked switched powered slope one level up (x-1,m+1,mz+2),
        // then a plain slope (x-1,m,mz+1), the curve (x-1,m,mz) east onto the merge; fed by a flat at
        // (x-1,m+2,mz+3); the rest's box clears the branch carts' heights (no push range))
        put(sx, m + 2, mz + 3, &flat_ns);
        put(sx, m + 1, mz + 3, &glass);
        put(sx, m + 2, mz + 4, &glass);
        put(sx, m + 1, mz + 2, &pslope_s);
        put(sx, m, mz + 2, &glass);
        put(sx, m, mz + 1, &slope_s);
        put(sx, m - 1, mz + 1, &glass);
        put(sx, m, mz, &curve_se);
        put(sx, m - 1, mz, &glass);
    } else {
        put(sx, m + 1, mz + 2, &flat_ns);
        put(sx, m, mz + 2, &glass);
        put(sx, m + 1, mz + 3, &glass);
        put(sx, m, mz + 1, &pslope_s);
        put(sx, m - 1, mz + 1, &glass);
        put(sx, m, mz, &curve_se);
        put(sx, m - 1, mz, &glass);
    }
    for xx in sx + 1..lx {
        put(xx, m, mz, &flat_ew);
        put(xx, m - 1, mz, &glass);
    }
    // (the siding's rest rail and where its side power goes)
    let (rest_y, rest_z) = if so.siding_dy > 0 { (m + 1, mz + 2) } else { (m, mz + 1) };
    // (--nap-guard: a string over the branch cell before the merge (x,m+1,mz+1), hooks at x+-1, stone
    // at x+-2: the siding releases only while no cart is there)
    if so.guard {
        put(lx, m + 1, mz + 1, &string_ew);
        put(lx - 1, m + 1, mz + 1, &hook_e);
        put(lx + 1, m + 1, mz + 1, &hook_w);
        put(lx - 2, m + 1, mz + 1, &stone);
        put(lx + 2, m + 1, mz + 1, &stone);
    }
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(lx + 6, 30, 12), lo: BlockPos::new(lx - 6, -12, -6), hi: BlockPos::new(lx + 6, 30, 14), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &ro)?;
    while run.settle_tick() {}
    // (every rail back to its laid shape without placement logic: in the bench all blocks go down at
    // once, so a rail placed beside another can be reshaped by the connection rule (the branch flat
    // beside the raised siding's lower slope turned toward it); the build places them in order)
    let mut reshaped = 0usize;
    for (p, blk) in &spec.blocks {
        if matches!(blk, Block::PlainRail { .. } | Block::Rail { .. })
            && run.world.get(*p) != *blk
        {
            run.world.set_block(*p, blk.clone(), redsim::world::flags::RAW);
            reshaped += 1;
        }
    }
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
        if z > mz as f64 - 0.4 {
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
    let merge_hook = if so.drop_entry {
        BlockPos::new(lx - 1, m, mz - 1)
    } else if so.ns_merge {
        BlockPos::new(lx, m + 1, mz - 1)
    } else {
        BlockPos::new(lx - 1, m + 1, mz)
    };
    // (the cell a pass is credited in: the drop column with the drop entry, else the merge)
    let pass_z = if so.drop_entry { mz - 1 } else { mz };
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
    let mut contacts: Vec<(usize, f64, f64, f64, bool)> = Vec::new();
    let mut had_contact = vec![true; n];
    let mut creep = 0.0f64;
    let mut nap_rest: Option<(f64, f64)> = None;
    let mut wrong_way: Vec<String> = Vec::new();
    // (the curve rule: who is on the curve (x-1,22,6) and the merge (x,22,6), entry and clear ticks)
    let in_cell = |p: (f64, f64, f64), cx: i32, cy: i32, cz: i32| p.0 + 0.49 > cx as f64 && p.0 - 0.49 < (cx + 1) as f64 && p.2 + 0.49 > cz as f64 && p.2 - 0.49 < (cz + 1) as f64 && p.1 > cy as f64 - 0.5 && p.1 < cy as f64 + 1.0;
    let mut merge_occ: Vec<(usize, i64, Option<i64>)> = Vec::new();
    let mut touching = 0usize;
    let mut last_pass: Vec<Option<i64>> = vec![None; n];
    let mut lap_wait = false;
    let mut nap_since = 0i64;
    let mut late_naps = 0usize;
    let mut nap_rested_at: Option<i64> = None;
    let mut knocked = 0usize;
    let mut knocked_this = false;
    let mut frozen32: Option<[i32; 4]> = None;
    let mut touch_note: Vec<String> = Vec::new();
    let mut drop_count = 0usize;
    let mut entered: Vec<usize> = Vec::new();
    let mut order_note: Vec<String> = Vec::new();
    let mut order_broken = 0usize;
    let mut off_rest = vec![0i64; n];
    let mut off_rail = 0usize;
    let mut off_note: Vec<String> = Vec::new();
    let mut eye_bad: std::collections::BTreeMap<String, usize> = Default::default();
    let mut min_health = f32::MAX;
    let mut restage: Vec<i64> = Vec::new();
    let mut restage_from: Option<i64> = None;
    let mut hh_was = false;
    for t in 0..ticks {
        // (the logins and freezes as one schedule: frozen while any interval covers t, returns waiting
        // while frozen)
        let want_frozen = o.login.iter().chain(o.freeze32.iter()).any(|(a, d)| t >= *a && t < a + d);
        if want_frozen && frozen32.is_none() {
            frozen32 = Some(crate::liveness::freeze32(&mut run.world));
        }
        if !want_frozen
            && let Some(pb) = frozen32.take()
        {
            crate::liveness::thaw32(&mut run.world, pb);
        }
        let hh = matches!(run.world.get(hold_hook), Block::TripwireHook { powered: true, .. });
        held = if hh { held + 1 } else { 0 };
        // (the restage: from a release to the hold's string rising again once the trapdoor shut)
        if hh && !hh_was
            && t > trap_until
            && let Some(t0) = restage_from.take()
        {
            restage.push(t - t0);
        }
        hh_was = hh;
        let mh = matches!(run.world.get(merge_hook), Block::TripwireHook { powered: true, .. });
        // (every cart's last pass at the merge string: the rising edge, given to the cart over the merge)
        if mh && !merge_was
            && let Some(c) = (0..n).find(|c| carts[*c].and_then(|id| st(&run.world, id)).is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, pass_z)))
        {
            last_pass[c] = Some(t);
        }
        merge_was = mh;
        // (the release (day-1's rule): the napper resting on the siding, its predecessor's last merge
        // pass seen and not yet followed by its successor's (else it waits for the predecessor's next
        // pass, a lane-lap later), K gt after that pass)
        let nap_ready = nap.is_some_and(|(nc, _)| carts[nc].and_then(|id| st(&run.world, id)).is_some_and(|p| p.3 < 0.002 && p.1 > m as f64 - 0.1 && (p.0 - (sx as f64 + 0.5)).abs() < 0.3));
        if nap_ready && nap_rested_at.is_none() {
            nap_rested_at = Some(t);
        }
        let guard_on = if so.guard_det {
            matches!(run.world.get(BlockPos::new(lx, m, mz + 2)), Block::DetectorRail { powered: true, .. })
        } else {
            so.guard && matches!(run.world.get(BlockPos::new(lx - 1, m + 1, mz + 1)), Block::TripwireHook { powered: true, .. })
        };
        if let Some((nc, pred)) = nap
            && released_at.is_none()
            && nap_ready
            && !guard_on
            && let Some(pp) = last_pass[pred]
            && t >= pp + so.k
            && (!so.early_only || nap_rested_at.is_some_and(|r| pp >= r))
        {
            let succ = drops.iter().position(|d| d.1 == nc).and_then(|i| drops.get(i + 1)).map(|d| d.1);
            let succ_after = succ.and_then(|s| last_pass[s]).is_some_and(|sp| sp > pp && sp >= nap_since);
            if succ_after {
                if !lap_wait {
                    lap_wait = true;
                }
            } else {
                s_until = t + so.nap_pulse;
                released_at = Some(t);
                pred_pass = Some(pp);
            }
        }
        let ph = (t - warm - o.corner).rem_euclid(o.period);
        if t >= warm && t < rel_end && ph == 0 {
            trap_until = t + o.drop_w;
            per_release.push(0);
            restage_from = Some(t);
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
            run.world.set_block(BlockPos::new(sx - 1, rest_y, rest_z), if want_s { rblock.clone() } else { air.clone() }, redsim::world::flags::ALL);
        }
        // (returns: on the branch at z mz + 2.5 moving -z, or a napper onto the siding slope at rest;
        // nothing arrives while the bench is frozen)
        let due: Vec<(usize, bool)> = if frozen32.is_some() { Vec::new() } else { pending.iter().filter(|p| p.0 <= t).map(|p| (p.1, p.2)).collect() };
        for (c, is_nap) in due {
            if is_nap {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [sx as f64 + 0.5, rest_y as f64 + 0.5 + 0.0625, rest_z as f64 + 0.5], Some(&nbt0))?;
                carts[c] = Some(id);
                pending.retain(|p| p.1 != c);
                continue;
            }
            let zs = mz as f64 + 2.5;
            let clear = carts.iter().all(|id| id.and_then(|id| st(&run.world, id)).is_none_or(|(x, y, z, _)| !(y > m as f64 - 0.5 && (z - zs).abs() < 1.3 && (x - zc).abs() < 0.9)));
            if clear {
                carts[c] = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [zc, m as f64 + 0.0625, zs], Some(&nbt_in))?);
                pending.retain(|p| p.1 != c);
            }
        }
        run.tick();
        let now: Vec<Option<(f64, f64, f64, f64)>> = carts.iter().map(|id| id.and_then(|id| st(&run.world, id))).collect();
        // (the riders: the block at every rider's eye (air, string, hooks and rails are fine; anything
        // else is a wall or a side the eye is inside) and the lowest health seen)
        for id in carts.iter().flatten() {
            let Some(i) = redsim::entity::index_of(&run.world, *id) else { continue };
            for rid in run.world.entities.list[i].passengers.clone() {
                let Some(ri) = redsim::entity::index_of(&run.world, rid) else { continue };
                let e = &run.world.entities.list[ri];
                if let redsim::entity::EntityKind::Mob(mb) = &e.kind {
                    min_health = min_health.min(mb.health);
                }
                let (ex, ey, ez) = (e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z);
                let eye = BlockPos::new(ex.floor() as i32, ey.floor() as i32, ez.floor() as i32);
                let blk = run.world.get(eye);
                // (inside the block's collider: the eye point within one of its boxes)
                let f = |v: f64| (v - v.floor()) * 16.0;
                let inside = redsim::behavior::collision_boxes(&blk).iter().any(|bx| f(ex) > bx[0] as f64 && f(ex) < bx[1] as f64 && f(ey) > bx[2] as f64 && f(ey) < bx[3] as f64 && f(ez) > bx[4] as f64 && f(ez) < bx[5] as f64);
                if inside {
                    let name = format!("{blk:?}");
                    let name = name.split([' ', '{', '(']).next().unwrap_or("?").to_string();
                    *eye_bad.entry(format!("{name} at {},{},{}", eye.x, eye.y, eye.z)).or_default() += 1;
                }
            }
        }
        if let Ok(tr) = std::env::var("MCTAS_SIDING_TRACE") {
            let p: Vec<i64> = tr.split(',').filter_map(|x| x.parse().ok()).collect();
            if p.len() == 2 && t >= p[0] && t < p[1] {
                let xs: Vec<String> = (0..n).filter_map(|c| now[c].map(|s| format!("{c}:{:.2},{:.2},{:.2}/{:.2}", s.0, s.1, s.2, s.3))).collect();
                eprintln!("t {t}: trap {} R {} S {} hold {} merge {} : {}", trap_is as u8, r_is as u8, s_is as u8, hh as u8, mh as u8, xs.join(" "));
            }
        }
        // (the merge's occupants: entry and clear)
        for c in 0..n {
            let on = now[c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz));
            let open = merge_occ.iter().position(|mo| mo.0 == c && mo.2.is_none());
            match (on, open) {
                (true, None) => merge_occ.push((c, t, None)),
                (false, Some(i)) => merge_occ[i].2 = Some(t),
                _ => {}
            }
        }
        let on_merge = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))).count();
        if on_merge > 1 {
            touching += 1;
            if touch_note.len() < 4 {
                let who: Vec<String> = (0..n).filter(|c| now[*c].is_some_and(|p| in_cell((p.0, p.1, p.2), lx, m, mz))).map(|c| { let p = now[c].unwrap(); format!("cart {c} at {:.2},{:.2},{:.2}/{:.2}", p.0, p.1, p.2, p.3) }).collect();
                touch_note.push(format!("t {t}: {}", who.join(", ")));
            }
        }
        for c in 0..n {
            let (Some((x, y, z, v)), Some((_, _, _, pv))) = (now[c], prev[c]) else { continue };
            // (the napper waiting: its creep while braked; the wrong way: south past the merge on the branch)
            if let Some((nc, _)) = nap
                && nc == c
            {
                if released_at.is_none() {
                    // (the creep, measured only once the napper rests on the siding)
                    match nap_rest {
                        None if v < 0.002 && y > m as f64 - 0.1 && (x - (sx as f64 + 0.5)).abs() < 0.3 => nap_rest = Some((x, z)),
                        Some((x0, z0)) => {
                            creep = creep.max(((x - x0).powi(2) + (z - z0).powi(2)).sqrt());
                            // (knocked loose: moving with no release after it had come to rest)
                            if v > 0.02 && !knocked_this {
                                knocked_this = true;
                                knocked += 1;
                            }
                        }
                        _ => {}
                    }
                } else if (x - zc).abs() < 0.6 && z > mz as f64 + 1.2 && y > m as f64 - 0.5 {
                    if wrong_way.len() < 4 {
                        wrong_way.push(format!("t {t}: the napper (cart {c}) at {x:.2},{y:.2},{z:.2} heading up the branch"));
                    }
                }
            }
            // (contacts in the lane: a stored speed dropping by 0.04 or more with a cart within 1.3 ahead
            // (toward -z) in the lane's column, the cart on the slopes' rails (not in the drop: a wall hit
            // there is no contact); q = the carts on the slopes behind R_1's cart at that moment, not
            // counting the arrival (the lane before the arrival: D + R_1's cart + q))
            let on_rails = |yy: f64, zz: f64| zz < mz as f64 && yy < slope_y(zz) + 0.3;
            if pv - v >= 0.04
                && (x - zc).abs() < 0.6
                && on_rails(y, z)
                && (0..n).any(|j| j != c && now[j].is_some_and(|(xj, _, zj, _)| (xj - zc).abs() < 0.6 && zj < z && z - zj < 1.3))
            {
                let q = (0..n).filter(|j| *j != c && now[*j].is_some_and(|(xj, yj, zj, _)| (xj - zc).abs() < 0.6 && zj > 1.9 && yj > 16.5 && on_rails(yj, zj))).count();
                // (an arrival's contact is its first since it entered the lane; later ones are the
                // restage's shoves inside the queue)
                let arrival = !had_contact[c];
                had_contact[c] = true;
                contacts.push((c, pv, z, q as f64, arrival));
                // (order: the cart right ahead must be the one that entered the lane just before)
                let ahead = (0..n).filter(|j| *j != c && now[*j].is_some_and(|(xj, _, zj, _)| (xj - zc).abs() < 0.6 && zj < z)).max_by(|a, b2| now[*a].unwrap().2.total_cmp(&now[*b2].unwrap().2));
                let expect = entered.iter().position(|e| *e == c).and_then(|i| i.checked_sub(1)).map(|i| entered[i]);
                if let (Some(a), Some(e)) = (ahead, expect)
                    && a != e
                    && order_note.len() < 4
                {
                    order_note.push(format!("t {t}: cart {c} landed behind cart {a}, entered after cart {e}"));
                }
                if let (Some(a), Some(e)) = (ahead, expect)
                    && a != e
                {
                    order_broken += 1;
                }
            }
            // (the lane's entry order: a cart's first tick on the slopes' rails past the merge)
            if (x - zc).abs() < 0.6 && on_rails(y, z) && z > 1.9 && !entered.contains(&c) {
                entered.push(c);
                // (the carts laid in the lane at the start are no arrivals)
                had_contact[c] = t < 5;
            }
            // (a cart resting in the drop above the top slope (never landed), or resting off the rails)
            if (x - zc).abs() < 0.6 && z > 1.9 && z < mz as f64 && v < 0.002 && (y - slope_y(z)).abs() > 0.15 {
                off_rest[c] += 1;
                if off_rest[c] == 20 {
                    off_rail += 1;
                    if off_note.len() < 4 {
                        off_note.push(format!("t {t}: cart {c} at rest off the rails at {x:.2},{y:.2},{z:.2} (the rail's surface {:.2})", slope_y(z)));
                    }
                }
            } else {
                off_rest[c] = 0;
            }
            // (through the floor)
            if y < 16.0 - 1.5 && z < 1.0 {
                entered.retain(|e| *e != c);
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
                        nap_since = t;
                        lap_wait = false;
                        nap_rested_at = None;
                        knocked_this = false;
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
                && z < mz as f64
                && v < 0.002
                && t > ra + 5
                && (x - zc).abs() < 0.6
            {
                let m_pred = merge_occ.iter().rev().find(|mo| mo.0 == pred && mo.1 <= ra).and_then(|mo| mo.2);
                let m_nap = merge_occ.iter().rev().find(|mo| mo.0 == c).map(|mo| mo.1);
                let curve_rule = match (m_pred, m_nap) { (Some(cl), Some(en)) => format!("{}", en - cl), _ => "-".to_string() };
                let c_nap = contacts.iter().filter(|k| k.0 == c && k.4).map(|k| k.1).fold(0.0f64, f64::max);
                let q_nap = contacts.iter().rev().find(|k| k.0 == c && k.4).map_or("-".to_string(), |k| format!("{}", k.3 as usize));
                nap_rows.push(format!(
                    "napper cart {c} (predecessor {pred}){}: the predecessor over the merge string t {}, released t {ra}, at the lane's back t {t} (z {z:.2}); the merge's curve rule (its entry minus the predecessor's clear) {curve_rule} gt; its contact {c_nap:.3} at q {q_nap}; braked creep {creep:.4}",
                    if lap_wait { " (late: its successor passed first, it waited a lane-lap)" } else { "" },
                    pred_pass.map_or("-".to_string(), |p| p.to_string())
                ));
                if lap_wait {
                    late_naps += 1;
                }
                nap = None;
            }
        }
        prev = now;
    }
    let mut ph: std::collections::BTreeMap<usize, usize> = Default::default();
    for r in &per_release {
        *ph.entry(*r).or_default() += 1;
    }
    let arr: Vec<&(usize, f64, f64, f64, bool)> = contacts.iter().filter(|k| k.4).collect();
    let cmax = arr.iter().map(|k| k.1).fold(0.0f64, f64::max);
    let imax = contacts.iter().filter(|k| !k.4).map(|k| k.1).fold(0.0f64, f64::max);
    let min_behind = arr.iter().map(|k| k.3 as usize).min();
    let worst_at_min = min_behind.map(|mb| arr.iter().filter(|k| k.3 as usize == mb).map(|k| k.1).fold(0.0f64, f64::max));
    let mut by_count: std::collections::BTreeMap<usize, f64> = Default::default();
    for k in &arr {
        let e = by_count.entry(k.3 as usize).or_insert(0.0);
        *e = e.max(k.1);
    }
    let empty_releases = per_release.iter().skip(1).filter(|r| **r == 0).count();
    let mut lines = vec![format!(
        "siding bench: lane x {lx} in the budget's cells (the hold (x,16,0), R_1 (x,17,1), {ns} slopes, the merge (x,{m},{mz}) with its string, the siding (x-1,{m},{}) and its curve{}), {} carts ({}), the R line {} gt, the trapdoor {} gt every {} gt (corner {:+}), returns on the branch after {} gt at {:.2} b/t, a napper every {} drops back on the siding after {} gt, released {} gt after its predecessor's merge pass for {} gt; {} cycles{}",
        mz + 1,
        if so.drop_entry { format!("; the drop entry: the merge one level up, its north end over the drop (x,{m}..{},{}) onto the top slope, the wall (x,{m},{}) — one level and one wall block a lane", m - 1, mz - 1, mz - 2) } else { String::new() },
        o.per_lane, if o.riders { "ridden" } else { "empty" }, o.r_pulse, o.drop_w, o.period, o.corner, o.ret, o.v_in, so.nap_every, so.nap_ret, so.k, so.nap_pulse, o.cycles,
        if o.login.is_empty() && o.freeze32.is_empty() { String::new() } else { format!(", logins {:?} {:?}", o.login, o.freeze32) }
    )];
    lines.push(format!("carts per release (exactly 1): {}; empty releases after the first (golems lost: a batch of 2 can't summon): {empty_releases}; late nappers (a lane-lap wait): {late_naps}", ph.iter().map(|(a, b2)| format!("{a}: {b2}")).collect::<Vec<_>>().join(", ")));
    lines.push(format!("nappers through the siding: {}", if nap_rows.is_empty() { "none".to_string() } else { nap_rows.join(" | ") }));
    lines.push(format!("the napper heading the wrong way at the merge (the bar: 0): {}{}", wrong_way.len(), if wrong_way.is_empty() { String::new() } else { format!(" ({})", wrong_way.join("; ")) }));
    lines.push(format!("nappers knocked loose on the siding (moving with no release; the bar: 0): {knocked}; rails the bench restored to their laid shape after placement: {reshaped}"));
    lines.push(format!("ticks with two carts over the merge (the curve rule's touching; the bar: 0): {touching}{}", if touch_note.is_empty() { String::new() } else { format!(" ({})", touch_note.join("; ")) }));
    lines.push(format!(
        "arrivals at the lanes' backs (each arrival's first contact in the lane): {}, {} above 0.46, the hardest {cmax:.3}; the fewest carts queued on the slopes behind R_1's cart at an arrival (q: the lane before it is D + R_1's cart + q) {} (the hardest there {}); the hardest by q: {}; the restage's shoves inside the queue (R's own, not arrivals) up to {imax:.3}",
        arr.len(),
        arr.iter().filter(|k| k.1 > 0.46).count(),
        min_behind.map_or("-".to_string(), |v| v.to_string()),
        worst_at_min.map_or("-".to_string(), |v| format!("{v:.3}")),
        by_count.iter().map(|(k, v)| format!("{k}: {v:.3}")).collect::<Vec<_>>().join(", ")
    ));
    lines.push(format!("arrivals landing out of order (the cart ahead is not the one that entered before; the bar: 0): {order_broken}{}", if order_note.is_empty() { String::new() } else { format!(" ({})", order_note.join("; ")) }));
    lines.push(format!("carts at rest off the rails (in the drop or beside a rail, 20 gt; the bar: 0): {off_rail}{}", if off_note.is_empty() { String::new() } else { format!(" ({})", off_note.join("; ")) }));
    lines.push(format!(
        "riders' eyes inside a block's collider (rider-ticks; the bar: 0): {}{}; the riders' lowest health {}",
        eye_bad.values().sum::<usize>(),
        if eye_bad.is_empty() { String::new() } else { format!(" ({})", eye_bad.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")) },
        if min_health == f32::MAX { "-".to_string() } else { format!("{min_health:.1}") }
    ));
    lines.push(format!(
        "the restage (a release to the hold's string rising again): the slowest {} gt of the {}-gt beat over {} cycles",
        restage.iter().max().map_or("-".to_string(), |v| v.to_string()),
        o.period,
        restage.len()
    ));
    if let Some((c, _)) = nap {
        lines.push(format!("a napper still waiting at the end: cart {c} (predecessor pass {:?}, released {:?})", pred_pass, released_at));
    }
    Ok(lines.join("\n"))
}

/// One siding run's numbers, for the matrix.
#[derive(Clone, Debug, Default)]
pub struct SidingRow {
    pub empty_releases: usize,
    pub not_one: usize,
    pub late_naps: usize,
    pub nap_contacts: Vec<f64>,
    pub nap_q: Vec<i64>,
    pub wrong: usize,
    pub knocked: usize,
    pub touching: usize,
    pub arrivals_over: usize,
    pub arrival_max: f64,
    pub by_q: std::collections::BTreeMap<usize, f64>,
    pub order_broken: usize,
    pub off_rail: usize,
    pub eyes: usize,
    pub min_health: f32,
    pub restage_max: i64,
    pub stuck: bool,
    pub text: String,
}

/// The bench's numbers read back from its own text (one source of truth: the printed lines).
pub fn siding_row(o: &DropOpts, so: &SidingOpts) -> Result<SidingRow, String> {
    let text = siding_bench(o, so)?;
    let mut r = SidingRow { min_health: f32::MAX, ..Default::default() };
    let num_after = |s: &str, key: &str| -> Option<f64> {
        let i = s.find(key)? + key.len();
        let t: String = s[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
        t.parse().ok()
    };
    for l in text.lines() {
        if l.starts_with("carts per release (exactly 1): ") {
            let body = l.trim_start_matches("carts per release (exactly 1): ");
            let counts = body.split(';').next().unwrap_or("");
            for part in counts.split(", ") {
                let mut it = part.split(": ");
                if let (Some(a), Some(b2)) = (it.next(), it.next())
                    && a.trim() != "1"
                {
                    r.not_one += b2.trim().parse::<usize>().unwrap_or(0);
                }
            }
            r.empty_releases = num_after(l, "golems lost: a batch of 2 can't summon): ").unwrap_or(0.0) as usize;
            r.late_naps = num_after(l, "late nappers (a lane-lap wait): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("nappers through the siding: ") {
            for seg in l.split(" | ") {
                if let Some(v) = num_after(seg, "its contact ") {
                    r.nap_contacts.push(v);
                    r.nap_q.push(num_after(seg, " at q ").map_or(-1, |q| q as i64));
                }
            }
        } else if l.starts_with("the napper heading the wrong way") {
            r.wrong = num_after(l, "(the bar: 0): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("nappers knocked loose") {
            r.knocked = num_after(l, "(moving with no release; the bar: 0): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("ticks with two carts over the merge") {
            r.touching = num_after(l, "the bar: 0): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("arrivals at the lanes' backs") {
            r.arrivals_over = num_after(l, "first contact in the lane): ").and_then(|_| l.split(", ").nth(1)).and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok()).unwrap_or(0);
            r.arrival_max = num_after(l, "the hardest ").unwrap_or(0.0);
            if let Some(i) = l.find("the hardest by q: ") {
                let body = &l[i + "the hardest by q: ".len()..];
                let body = body.split(';').next().unwrap_or("");
                for part in body.split(", ") {
                    let mut it = part.split(": ");
                    if let (Some(a), Some(b2)) = (it.next(), it.next())
                        && let (Ok(q), Ok(v)) = (a.trim().parse::<usize>(), b2.trim().parse::<f64>())
                    {
                        r.by_q.insert(q, v);
                    }
                }
            }
        } else if l.starts_with("arrivals landing out of order") {
            r.order_broken = num_after(l, "(the bar: 0): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("carts at rest off the rails") {
            r.off_rail = num_after(l, "the bar: 0): ").unwrap_or(0.0) as usize;
        } else if l.starts_with("riders' eyes inside") {
            r.eyes = num_after(l, "(rider-ticks; the bar: 0): ").unwrap_or(0.0) as usize;
            r.min_health = num_after(l, "the riders' lowest health ").map(|v| v as f32).unwrap_or(f32::MAX);
        } else if l.starts_with("the restage") {
            r.restage_max = num_after(l, "the slowest ").unwrap_or(-1.0) as i64;
        } else if l.starts_with("a napper still waiting") {
            r.stuck = true;
        }
    }
    r.text = text;
    Ok(r)
}
