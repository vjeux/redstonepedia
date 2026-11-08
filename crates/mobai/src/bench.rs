//! Benchmarks: the game's cost of mob AI (the bar the redsim model is measured against).

use anyhow::{Result, anyhow};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, WorkerConfig};
use std::time::Instant;

/// Build the closed arena: a stone floor at y = 0 over the box, a stone roof at y = 5 (dark: no
/// sunburn), walls around it; mobs walk on y = 1.
fn arena(w: &mut mctas::sim::Worker, sb: u32, half: i32) -> Result<()> {
    let lo = [-half, 0, -half];
    let hi = [half - 1, 5, half - 1];
    w.fill(sb, [lo[0], 0, lo[2]], [hi[0], 0, hi[2]], "minecraft:stone", PlaceMode::Raw)?;
    w.fill(sb, [lo[0], 5, lo[2]], [hi[0], 5, hi[2]], "minecraft:stone", PlaceMode::Raw)?;
    for (a, b) in [([lo[0], 1, lo[2]], [hi[0], 4, lo[2]]), ([lo[0], 1, hi[2]], [hi[0], 4, hi[2]]), ([lo[0], 1, lo[2]], [lo[0], 4, hi[2]]), ([hi[0], 1, lo[2]], [hi[0], 4, hi[2]])] {
        w.fill(sb, a, b, "minecraft:stone", PlaceMode::Raw)?;
    }
    for cmd in ["gamerule spawn_mobs false", "difficulty normal", "time set 6000"] {
        w.exec(sb, cmd).map_err(|e| anyhow!("{cmd}: {e:#}"))?;
    }
    Ok(())
}

fn time_steps(w: &mut mctas::sim::Worker, sb: u32, ticks: u32) -> Result<f64> {
    // warm up (JIT, chunk loads) before the timed steps
    w.step(sb, 100)?;
    let t0 = Instant::now();
    let mut left = ticks;
    while left > 0 {
        let n = left.min(100);
        w.step(sb, n)?;
        left -= n;
    }
    Ok(t0.elapsed().as_secs_f64())
}

/// `mctas mobai bench-game`.
pub fn bench_game(version: &str, mob: &str, count: u32, ticks: u32, half: i32, seed: i64) -> Result<()> {
    let pool = Pool::spawn(1, &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let mut w = pool.worker(0);
    let c = half.div_euclid(16) + 1;
    let chunks = [-c, -c, c - 1, c - 1];
    // the baseline: the same arena, no mobs
    let base_sb = w.create(&SandboxOptions { chunks, seed })?;
    arena(&mut w, base_sb, half)?;
    let base = time_steps(&mut w, base_sb, ticks)?;
    w.drop_sandbox(base_sb)?;
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    arena(&mut w, sb, half)?;
    let mut r = redsim::rng::LegacyRandom::new(seed);
    for _ in 0..count {
        let x = -half as f64 + 2.0 + r.next_double() * (2 * half - 4) as f64;
        let z = -half as f64 + 2.0 + r.next_double() * (2 * half - 4) as f64;
        let cmd = format!("summon minecraft:{mob} {x:.3} 1 {z:.3} {{PersistenceRequired:1b}}");
        w.exec(sb, &cmd).map_err(|e| anyhow!("{cmd}: {e:#}"))?;
    }
    let with = time_steps(&mut w, sb, ticks)?;
    let alive = w.entities(sb)?.iter().filter(|e| e.kind.ends_with(mob)).count();
    w.drop_sandbox(sb)?;
    let per = (with - base) / (count as f64 * ticks as f64) * 1.0e6;
    println!("game: {count} {mob} ({alive} alive at the end), {ticks} ticks: {:.3} s with mobs, {:.3} s without: {per:.1} us per mob per tick", with, base);
    Ok(())
}

/// The same arena in redsim: the floor, roof and walls as blocks of a [`redsim::world::World`].
fn redsim_arena(half: i32) -> redsim::world::World {
    use redsim::block::{Block, Simple};
    use redsim::pos::BlockPos;
    let mut w = redsim::world::World::new(BlockPos::new(-half, -2, -half), [2 * half, 10, 2 * half]);
    let mut blocks = Vec::new();
    for z in -half..half {
        for x in -half..half {
            for y in [0, 5] {
                blocks.push((BlockPos::new(x, y, z), Block::Simple(Simple::Stone)));
            }
            if x == -half || z == -half || x == half - 1 || z == half - 1 {
                for y in 1..5 {
                    blocks.push((BlockPos::new(x, y, z), Block::Simple(Simple::Stone)));
                }
            }
        }
    }
    w.place_raw(&blocks);
    w
}

/// `mctas mobai bench-redsim`: the redsim mob layer's cost per mob per tick in the same arena
/// (the mobs as the layer runs them today: physics and the villager brain, no walking).
pub fn bench_redsim(mob: &str, count: u32, ticks: u32, half: i32, seed: i64) -> Result<()> {
    use redsim::entity::Vec3;
    use redsim::entity::mob::ai::path::navigation::{SEARCHES, SEARCH_NANOS};
    use redsim::entity::mob::types::MobType;
    use std::sync::atomic::Ordering::Relaxed;
    let ty = MobType::from_id(mob).ok_or_else(|| anyhow!("{mob}: not a modeled mob type"))?;
    // the mob AI on, as the exact compares run it (the bridge's seeding, the mirrored level random)
    let time = |with: bool, ai: bool| -> (f64, u64, u64) {
        let mut w = redsim_arena(half);
        w.mob.seed(seed, 6000);
        if ai {
            w.mob.ai.on = true;
            w.mob.ai.difficulty = redsim::entity::mob::natural::Difficulty::Normal;
            let c = half.div_euclid(16) + 1;
            w.mirror_level_random(seed, [-c, -c, c - 1, c - 1]);
        }
        if with {
            let mut r = redsim::rng::LegacyRandom::new(seed);
            for _ in 0..count {
                let x = -half as f64 + 2.0 + r.next_double() * (2 * half - 4) as f64;
                let z = -half as f64 + 2.0 + r.next_double() * (2 * half - 4) as f64;
                redsim::entity::mob::add(&mut w, ty, Vec3::new(x, 1.0, z), true);
            }
        }
        for _ in 0..100 {
            w.tick();
        }
        let (s0, n0) = (SEARCHES.load(Relaxed), SEARCH_NANOS.load(Relaxed));
        let t0 = Instant::now();
        for _ in 0..ticks {
            w.tick();
        }
        let el = t0.elapsed().as_secs_f64();
        if !w.errors.is_empty() {
            eprintln!("model errors: {:?}", w.errors);
        }
        (el, SEARCHES.load(Relaxed) - s0, SEARCH_NANOS.load(Relaxed) - n0)
    };
    for ai in [false, true] {
        let (base, _, _) = time(false, ai);
        let (with, searches, nanos) = time(true, ai);
        let per = (with - base) / (count as f64 * ticks as f64) * 1.0e6;
        let rate = if nanos > 0 { searches as f64 / (nanos as f64 * 1e-9) } else { 0.0 };
        println!(
            "redsim{}: {count} {mob}, {ticks} ticks: {with:.3} s with mobs, {base:.3} s without: {per:.2} us per mob per tick; {searches} path searches ({:.1} per mob per 100 ticks), {rate:.0} searches per second, {:.1}% of the time",
            if ai { " (mob AI)" } else { " (no AI)" },
            searches as f64 / count as f64 / ticks as f64 * 100.0,
            nanos as f64 * 1e-9 / with.max(1e-9) * 100.0
        );
    }
    Ok(())
}
