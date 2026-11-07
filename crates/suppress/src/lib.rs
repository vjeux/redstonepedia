//! Update suppressor study (designs/briefs/update-suppressor.md, designs/update-suppressor/NOTES.md).
//!
//! A flood generator is a field of blocks that, on one trigger edge, makes a single
//! neighbor-updater cascade add as many counted calls as possible
//! (`CollectingNeighborUpdater.addAndRun`, cap 1,000,000 in 26.3). `probe` builds a
//! parameterized family in redsim, settles it the way survival placement would (rails and
//! wires placed one by one with updates, in a fixed order, so rest states are the game's),
//! fires the trigger, and reports the cascade statistics redsim now records
//! (`World.nu_history`): counted calls, executions, dropped calls, and how many times each
//! cell changed state (`World.writes`).

pub mod cli;

use anyhow::{Result, bail};
use redsim::block::{Block, Face, RailShape, Simple};
use redsim::pos::{BlockPos, Dir};
use redsim::world::World;
use std::collections::HashMap;
use std::time::Instant;

pub fn bp(x: i32, y: i32, z: i32) -> BlockPos {
    BlockPos::new(x, y, z)
}

pub fn stone() -> Block {
    Block::Simple(Simple::Stone)
}

/// One probe run's measurements.
#[derive(Clone, Debug, Default)]
pub struct ProbeReport {
    pub family: String,
    pub size: i32,
    /// Blocks placed (supports included, the trigger lever/rail excluded).
    pub blocks: usize,
    pub build_peak: u64,
    /// The trigger's cascades, largest first: (counted, executions, dropped).
    pub cascades: Vec<(u64, u64, u64)>,
    pub counted: u64,
    pub executions: u64,
    pub dropped: u64,
    /// Cells whose state changed during the trigger, and the largest number of changes of
    /// one cell (a value > 2 on a mesh cell is the non-monotone signature the brief hunts).
    pub cells_flipped: usize,
    pub flips_max: u32,
    pub flips_total: u64,
    pub errors: Vec<String>,
    pub wall_ms: u128,
}

/// A world sized to `blocks` plus padding, with statistics on from the start.
fn world_for(blocks: &[(BlockPos, Block)], at: BlockPos) -> World {
    let mut lo = bp(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = bp(i32::MIN, i32::MIN, i32::MIN);
    for (p, _) in blocks {
        lo = bp(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = bp(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let min = bp(lo.x - 3 + at.x, lo.y - 2 + at.y, lo.z - 3 + at.z);
    let size = [hi.x - lo.x + 7, hi.y - lo.y + 5, hi.z - lo.z + 7];
    let mut w = World::new(min, size);
    w.nu_stats_on = true;
    w
}

fn place_player_like(w: &mut World, blocks: &[(BlockPos, Block)], at: BlockPos) {
    // Supports and statics raw-put first (their own placement cascades cannot touch the
    // not-yet-placed field), then rails/wires one by one with updates, in file order: rest
    // shapes and powers are whatever the game's placement code settles to.
    for pass in 0..2 {
        for (p, b) in blocks {
            let q = bp(p.x + at.x, p.y + at.y, p.z + at.z);
            let live = matches!(b, Block::Wire { .. } | Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. });
            if pass == 0 && !live {
                w.put(q, *b);
            } else if pass == 1 && live {
                w.set_block_and_update(q, *b);
            }
        }
    }
}

fn summarize(w: &World, family: &str, size: i32, blocks: usize, build_peak: u64, started: Instant) -> ProbeReport {
    let mut flips: HashMap<BlockPos, u32> = HashMap::new();
    for p in &w.writes {
        *flips.entry(*p).or_default() += 1;
    }
    let mut cascades: Vec<(u64, u64, u64)> = w.nu_history.iter().map(|s| (s.counted, s.executions, s.dropped)).collect();
    cascades.sort_by_key(|c| std::cmp::Reverse(c.0));
    ProbeReport {
        family: family.to_string(),
        size,
        blocks,
        build_peak,
        counted: cascades.first().map(|c| c.0).unwrap_or(0),
        executions: cascades.iter().map(|c| c.1).sum(),
        dropped: cascades.iter().map(|c| c.2).sum(),
        cascades,
        cells_flipped: flips.len(),
        flips_max: flips.values().copied().max().unwrap_or(0),
        flips_total: w.writes.len() as u64,
        errors: w.errors.clone(),
        wall_ms: started.elapsed().as_millis(),
    }
}

/// Rail patterns as cell sets with a kind rule (`cells` + `kind(x,z)`), laid over geometries
/// (a height per cell). The lever stands south of the center cell, adjacent at rail height;
/// `place`/`break` triggers act at the first hole / the center rail.
/// Patterns: `full`, `comb` (holes x%3==2 && z%3==2), `t` (track z=0 + stubs z=-1 at even
/// x), `t1` (stubs at every x), `ladder` (tracks at z%3==0, rungs between), `grid3` (rails
/// where x%2==0 or z%2==0). Kinds: `checker` (plain/detector by parity), `plain`,
/// `detector`, `jp` (plain at exact-3-neighbour cells of the pattern, detector elsewhere).
fn rail_mesh(n: i32, pattern: &str, kinds: &str, geom: &str) -> Result<(Vec<(BlockPos, Block)>, BlockPos, BlockPos, BlockPos)> {
    let in_pat = |x: i32, z: i32| -> bool {
        match pattern {
            "full" => (0..n).contains(&x) && (0..n).contains(&z),
            "comb" => (0..n).contains(&x) && (0..n).contains(&z) && !(x % 3 == 2 && z % 3 == 2),
            "t" => (-2..=n).contains(&x) && (z == 0 || (z == -1 && x % 2 == 0)),
            "t1" => (-2..=n).contains(&x) && (z == 0 || z == -1),
            "ladder" => (0..n).contains(&x) && (0..n).contains(&z) && (z % 3 != 1 || x % 4 == 0),
            "grid3" => (0..n).contains(&x) && (0..n).contains(&z) && (x % 2 == 0 || z % 2 == 0),
            _ => false,
        }
    };
    if !matches!(pattern, "full" | "comb" | "t" | "t1" | "ladder" | "grid3") {
        bail!("unknown rail pattern {pattern} (full|comb|t|t1|ladder|grid3)");
    }
    let height = |x: i32, z: i32| -> i32 {
        match geom {
            "slope" => 1 + (x + z).rem_euclid(2),
            "stepx" => 1 + x.rem_euclid(2),
            _ => 1,
        }
    };
    let plain = Block::PlainRail { shape: RailShape::NorthSouth };
    let detector = Block::DetectorRail { shape: RailShape::NorthSouth, powered: false };
    let kind = |x: i32, z: i32| -> Result<Block> {
        Ok(match kinds {
            "plain" => plain,
            "detector" => detector,
            "checker" => {
                if (x + z) % 2 == 0 {
                    plain
                } else {
                    detector
                }
            }
            "jp" => {
                let c = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| in_pat(x + dx, z + dz)).count();
                if c == 3 {
                    plain
                } else {
                    detector
                }
            }
            other => bail!("unknown kinds {other} (checker|plain|detector|jp)"),
        })
    };
    let mut v: Vec<(BlockPos, Block)> = Vec::new();
    let (cx, mut cz) = (n / 2, n / 2);
    if !in_pat(cx, cz) {
        // The center rail for t/t1 lives on the track line.
        cz = 0;
    }
    for x in -2..=n {
        for z in -2..=n {
            if !in_pat(x, z) {
                continue;
            }
            let y = height(x, z);
            for sy in 0..y {
                v.push((bp(x, sy, z), stone()));
            }
            v.push((bp(x, y, z), kind(x, z)?));
        }
    }
    let cy = height(cx, cz);
    v.push((bp(cx, cy - 1, cz + 1), stone()));
    let lever = bp(cx, cy, cz + 1);
    if in_pat(cx, cz + 1) {
        // Don't let the lever cell collide with a rail of the pattern: drop that rail.
        v.retain(|(p, _)| *p != bp(cx, height(cx, cz + 1), cz + 1));
    }
    let mut hole = bp(cx, cy, cz);
    'outer: for x in 0..n {
        for z in 0..n {
            if !in_pat(x, z) {
                hole = bp(x, height(x, z), z);
                break 'outer;
            }
        }
    }
    Ok((v, lever, hole, bp(cx, cy, cz)))
}

/// A flat dust field: stone at y=0, cross wires at y=1, `n` x `n`; the lever sits on the
/// center support (replacing its wire), so its first cascade feeds the wires around it.
fn dust_field(n: i32) -> (Vec<(BlockPos, Block)>, BlockPos) {
    let mut v = Vec::new();
    let c = n / 2;
    for x in 0..n {
        for z in 0..n {
            v.push((bp(x, 0, z), stone()));
            if !(x == c && z == c) {
                v.push((bp(x, 1, z), redsim::redstone::wire::CROSS));
            }
        }
    }
    (v, bp(c, 1, c))
}

/// An ASCII plane at height `y` over the world's non-air extent (rails by shape: lower case
/// plain, upper case detector/powered; wire = power digit; L lever, # other solid).
pub fn plane(w: &World, y: i32) -> String {
    // Plain rails lower case, detector/powered upper case (i/e = straight NS/EW,
    // r/t/f/g = ascending E/W/N/S, a/b/c/d = curves SE/SW/NW/NE).
    let shape_ch = |s: RailShape| match s {
        RailShape::NorthSouth => 'i',
        RailShape::EastWest => 'e',
        RailShape::AscendingEast => 'r',
        RailShape::AscendingWest => 't',
        RailShape::AscendingNorth => 'f',
        RailShape::AscendingSouth => 'g',
        RailShape::SouthEast => 'a',
        RailShape::SouthWest => 'b',
        RailShape::NorthWest => 'c',
        RailShape::NorthEast => 'd',
    };
    let mut out = String::new();
    for z in 0..w.size[2] {
        let mut row = String::new();
        for x in 0..w.size[0] {
            let p = bp(w.min.x + x, y, w.min.z + z);
            row.push(match w.get(p) {
                Block::Air => '.',
                Block::PlainRail { shape } => shape_ch(shape).to_ascii_lowercase(),
                Block::DetectorRail { shape, .. } | Block::Rail { shape, .. } => shape_ch(shape).to_ascii_uppercase(),
                Block::Wire { power, .. } => char::from_digit(power.min(9) as u32, 10).unwrap_or('9'),
                Block::Lever { .. } => 'L',
                _ => '#',
            });
        }
        if row.trim_matches('.').is_empty() {
            continue;
        }
        out.push_str(row.trim_end_matches('.'));
        out.push('\n');
    }
    out
}

/// Probe one family/size at world offset `at`.
/// Family: `dust`, or `rail:<pattern>:<kinds>:<geom>` (see `rail_mesh`; short forms
/// `rail-comb` = rail:comb:checker:flat, `rail-t` = rail:t:jp:flat, `rail` = rail:full).
/// Trigger: `lever` | `lever-off` (build with the lever on) | `place` | `break`.
pub fn probe(family: &str, size: i32, at: BlockPos, trigger: &str) -> Result<ProbeReport> {
    let started = Instant::now();
    let lever = |on: bool| Block::Lever { face: Face::Floor, facing: Dir::East, powered: on };
    let fam = match family {
        "rail" => "rail:full:checker:flat",
        "rail-comb" => "rail:comb:checker:flat",
        "rail-t" => "rail:t:jp:flat",
        f => f,
    };
    let (mut blocks, lever_rel, place_rel, break_rel) = match fam {
        "dust" => {
            let (b, l) = dust_field(size);
            (b, l, bp(0, -99, 0), l)
        }
        f if f.starts_with("rail:") => {
            let p: Vec<&str> = f.split(':').collect();
            anyhow::ensure!(p.len() == 4, "family must be rail:<pattern>:<kinds>:<geom>");
            rail_mesh(size, p[1], p[2], p[3])?
        }
        other => bail!("unknown family {other} (dust|rail:<pattern>:<kinds>:<geom>)"),
    };
    let lever_on = trigger == "lever-off";
    blocks.push((lever_rel, lever(lever_on)));
    let n_blocks = blocks.len() - 1;
    let mut w = world_for(&blocks, at);
    place_player_like(&mut w, &blocks, at);
    w.run_until_idle(100);
    if std::env::var_os("SUPPRESS_DUMP").is_some() {
        eprintln!("--- rest plane y={} ---\n{}", 1 + at.y, plane(&w, 1 + at.y));
    }
    let build_peak = w.nu_history.iter().map(|s| s.counted).max().unwrap_or(0);
    w.nu_stats_reset();
    w.log_writes = true;
    w.writes.clear();
    let lever_pos = bp(lever_rel.x + at.x, lever_rel.y + at.y, lever_rel.z + at.z);
    match trigger {
        "lever" | "lever-off" => {
            if !w.use_block(lever_pos) {
                bail!("lever use failed at {lever_pos}");
            }
        }
        "place" => {
            let q = bp(place_rel.x + at.x, place_rel.y + at.y, place_rel.z + at.z);
            // The support under the placed rail must exist (the generator only puts
            // supports under pattern cells).
            if w.get(q.below()).is_air() {
                w.put(q.below(), stone());
            }
            w.set_block_and_update(q, Block::PlainRail { shape: RailShape::NorthSouth });
        }
        "break" => {
            let q = bp(break_rel.x + at.x, break_rel.y + at.y, break_rel.z + at.z);
            w.remove_block(q, false);
        }
        other => bail!("unknown trigger {other} (lever|lever-off|place|break)"),
    }
    w.run_until_idle(100);
    if std::env::var_os("SUPPRESS_DUMP").is_some() {
        eprintln!("--- after trigger plane y={} ---\n{}", 1 + at.y, plane(&w, 1 + at.y));
    }
    Ok(summarize(&w, family, size, n_blocks, build_peak, started))
}

/// Random rail-mesh hunt (the rail-wave question, closed by sampling instead of patterns):
/// `seeds` patches of `n` x `n` cells (density/kind/support-height drawn from the seed),
/// built player-like in a shuffled order, then up to three triggers (lever south of the
/// center, place a plain rail in the first hole, break the center rail), each on a fresh
/// clone. Returns (best counted, seed, trigger) and the counted quantiles.
pub fn probe_rand(seeds: i64, n: i32, at: BlockPos) -> Result<(Vec<String>, u64)> {
    use redsim::rng::LegacyRandom;
    let mut lines = Vec::new();
    let mut all: Vec<u64> = Vec::new();
    let (mut best, mut best_desc) = (0u64, String::new());
    for seed in 1..=seeds {
        let mut rng = LegacyRandom::new(seed.wrapping_mul(0x5DEECE66D));
        let density = 55 + rng.next_int(45); // percent of cells holding a rail
        let mut supports: Vec<(BlockPos, Block)> = Vec::new();
        let mut rails: Vec<(BlockPos, Block)> = Vec::new();
        for x in 0..n {
            for z in 0..n {
                if rng.next_int(100) >= density {
                    continue;
                }
                let y = 1 + rng.next_int(2);
                for sy in 0..y {
                    supports.push((bp(x, sy, z), stone()));
                }
                let b = if rng.next_int(2) == 0 {
                    Block::PlainRail { shape: RailShape::NorthSouth }
                } else {
                    Block::DetectorRail { shape: RailShape::NorthSouth, powered: false }
                };
                rails.push((bp(x, y, z), b));
            }
        }
        if rails.len() < 8 {
            continue;
        }
        let c = rails[rails.len() / 2].0;
        // Build the base world (supports put raw, rails placed shuffled with updates).
        let lever_rel = bp(c.x, c.y, c.z + 1);
        let mut blocks: Vec<(BlockPos, Block)> = supports.clone();
        blocks.extend(rails.iter().copied());
        blocks.retain(|(p, _)| *p != lever_rel);
        let mut order: Vec<usize> = (0..rails.len()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.next_int(i as i32 + 1) as usize;
            order.swap(i, j);
        }
        let mut base = world_for(&blocks, at);
        for (p, b) in &supports {
            base.put(bp(p.x + at.x, p.y + at.y, p.z + at.z), *b);
        }
        base.put(bp(lever_rel.x + at.x, lever_rel.y - 1 + at.y, lever_rel.z + at.z), stone());
        base.put(bp(lever_rel.x + at.x, lever_rel.y + at.y, lever_rel.z + at.z), Block::Lever { face: Face::Floor, facing: Dir::East, powered: false });
        for &i in &order {
            let (p, b) = rails[i];
            if p != lever_rel {
                base.set_block_and_update(bp(p.x + at.x, p.y + at.y, p.z + at.z), b);
            }
        }
        base.run_until_idle(100);
        let cc = bp(c.x + at.x, c.y + at.y, c.z + at.z);
        let lever_pos = bp(lever_rel.x + at.x, lever_rel.y + at.y, lever_rel.z + at.z);
        for trig in ["lever", "break"] {
            let mut w = base.clone();
            w.nu_stats_reset();
            if trig == "lever" {
                let _ = w.use_block(lever_pos);
            } else {
                w.remove_block(cc, false);
            }
            w.run_until_idle(100);
            let counted = w.nu_history.iter().map(|s| s.counted).max().unwrap_or(0);
            all.push(counted);
            if counted > best {
                best = counted;
                best_desc = format!("seed={seed} trig={trig} rails={} density={density}", rails.len());
            }
        }
    }
    all.sort_unstable();
    let q = |p: usize| all.get(all.len() * p / 100).copied().unwrap_or(0);
    lines.push(format!("rail-rand n={n} patches={} triggers=2 counted: p50={} p90={} p99={} max={best} ({best_desc})", all.len() / 2, q(50), q(90), q(99)));
    Ok((lines, best))
}

/// One line per probe report (the CLI's table).
pub fn report_line(r: &ProbeReport) -> String {
    format!(
        "{:<10} n={:<5} blocks={:<7} build_peak={:<8} counted={:<9} exec={:<9} dropped={:<7} cells_flipped={:<7} flips_max={:<3} counted/block={:.2} wall={}ms err={}",
        r.family,
        r.size,
        r.blocks,
        r.build_peak,
        r.counted,
        r.executions,
        r.dropped,
        r.cells_flipped,
        r.flips_max,
        r.counted as f64 / r.blocks.max(1) as f64,
        r.wall_ms,
        r.errors.len(),
    )
}
