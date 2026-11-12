//! The siding feeds (day-1, Nov 12 2026; ironspec 11:22: (a), the feed from below, for lanes -1 and 3,
//! lane 7 as ruled). Each lane's S-bend R (x-2,24,7), a powered rail on a slime block (x-2,23,7), is
//! powered by ironfarm's driver (x-2,23,8), a delay-1 repeater facing south reading the feed dust
//! (x-2,23,9). The ruled feed (lane 7) reaches that dust from the east: day-1's switched cell (x,23,9), a
//! delay-2 repeater whose input is (x+1,23,9), into ironfarm's delay-1 repeater (x-1,23,9) facing east,
//! into the dust. Lanes -1 and 3 cannot be fed from the east without a cell in a kept seat candidate's
//! head cell ((0,23,9), (4,23,9)), so their dust stands on a slime block (x-2,22,9), strongly powered from
//! the side by day-1's delay-3 repeater: lane -1's at (-4,22,9) facing west (input the dust (-5,22,9)),
//! lane 3's at (2,22,9) facing east (input the dust (3,22,9), under glass (3,23,9)). Both paths take 8 gt
//! from the line's input to R (4 + 2 + 2 against 6 + 0 + 2), so R switches at D + 6 in every lane.
//! (A diode's `facing` names its input side, as in the game.)
//!
//! The bench lays each lane's cells from the composed module (onechunk's rev 3, 213bda2bf) with the
//! context that touches them (the row's support glass at y 24, D's slope (x,24,8) on its glass, the
//! candidates' floors), drives the line's input with a redstone block for 30 gt at 20 phases, and prints
//! R's rise and fall after the drive's; then the same with a restart (every chunk saved and loaded again
//! between two ticks) at every tick around both edges in every reload order (save, reverse, seed 1..8),
//! against the run without one; and the reach facts: each powered slime's six neighbours (no rail), each
//! feed dust's connections (only its driver), the candidate cells left empty.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Feed {
    /// ironspec 11:22 (a): the feed dust on a slime block powered from the side.
    Below,
    /// The ruled feed: the switched cell (x,23,9) into ironfarm's repeater (x-1,23,9).
    East,
}

pub struct Lane {
    pub x: i32,
    pub feed: Feed,
}

pub const LANES: [Lane; 3] = [Lane { x: -1, feed: Feed::Below }, Lane { x: 3, feed: Feed::Below }, Lane { x: 7, feed: Feed::East }];

const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const GLASS: &str = "minecraft:glass";

/// One lane's cells: (the blocks, the drive cell, the powered slime under the feed dust if any, the
/// candidate cells that must stay empty).
fn lane_cells(l: &Lane) -> (Vec<(P, String)>, P, Option<P>, Vec<P>) {
    let x = l.x;
    let mut b: Vec<(P, String)> = Vec::new();
    let mut put = |p: P, s: &str| b.push((p, s.to_string()));
    // R, its slime, the glass it leans on (an ascending rail is removed without a sturdy block in its
    // ascending direction), ironfarm's driver on its glass, the feed dust
    put([x - 2, 24, 7], "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]");
    put([x - 2, 23, 7], "minecraft:slime_block");
    put([x - 2, 24, 8], GLASS);
    put([x - 2, 23, 8], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put([x - 2, 22, 8], GLASS);
    put([x - 2, 23, 9], DUST);
    // context: the row's support glass at y 24 over z 9, D's slope (x,24,8) on its glass (x,23,8)
    for xx in (x - 4)..=(x + 2) {
        put([xx, 24, 9], GLASS);
    }
    put([x, 23, 8], GLASS);
    put([x, 24, 8], "minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]");
    let (drive, slime, empty) = match (x, l.feed) {
        (-1, Feed::Below) => {
            // the slime under the feed dust; day-1's delay-3 repeater (-4,22,9) facing west on glass,
            // its input dust (-5,22,9) on glass; the exit rail's glass (-5,23,9), (-4,23,9); the floors
            // (-1..0,21,9) (glowstone; F3's cfd6fd621 dropped (-3,21,9) and (-2,21,9), G1 on 9c30d6dc3)
            put([-3, 22, 9], "minecraft:slime_block");
            put([-4, 22, 9], "minecraft:repeater[delay=3,facing=west,locked=false,powered=false]");
            put([-4, 21, 9], GLASS);
            put([-5, 22, 9], DUST);
            put([-5, 21, 9], GLASS);
            put([-5, 23, 9], GLASS);
            put([-4, 23, 9], GLASS);
            for xx in -1..=0 {
                put([xx, 21, 9], "minecraft:glowstone");
            }
            ([-6, 22, 9], Some([-3, 22, 9]), vec![[-1, 22, 9], [-1, 23, 9], [0, 22, 9], [0, 23, 9]])
        }
        (3, Feed::Below) => {
            // the slime under the feed dust; day-1's delay-3 repeater (2,22,9) facing east on glass, its
            // input dust (3,22,9) on glass under glass (3,23,9); the floors (0,21,9), (4,21,9)
            put([1, 22, 9], "minecraft:slime_block");
            put([2, 22, 9], "minecraft:repeater[delay=3,facing=east,locked=false,powered=false]");
            put([2, 21, 9], GLASS);
            put([3, 22, 9], DUST);
            put([3, 21, 9], GLASS);
            put([3, 23, 9], GLASS);
            put([0, 21, 9], "minecraft:glowstone");
            put([4, 21, 9], "minecraft:glowstone");
            ([3, 22, 8], Some([1, 22, 9]), vec![[0, 22, 9], [0, 23, 9], [4, 22, 9], [4, 23, 9]])
        }
        _ => {
            // the ruled feed: the dust on glass, ironfarm's delay-1 repeater (x-1,23,9) facing east on
            // glass, day-1's switched cell (x,23,9) a delay-2 repeater facing east on glass, its input dust
            // (x+1,23,9) on glass
            put([x - 2, 22, 9], GLASS);
            put([x - 1, 23, 9], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
            put([x - 1, 22, 9], GLASS);
            put([x, 23, 9], "minecraft:repeater[delay=2,facing=east,locked=false,powered=false]");
            put([x, 22, 9], GLASS);
            put([x + 1, 23, 9], DUST);
            put([x + 1, 22, 9], GLASS);
            ([x + 2, 23, 9], None, vec![[4, 22, 9], [4, 23, 9]])
        }
    };
    (b, drive, slime, empty)
}

fn spec(l: &Lane) -> Result<(FarmSpec, P, Option<P>, Vec<P>), String> {
    let (cells, drive, slime, empty) = lane_cells(l);
    let mut blocks = Vec::new();
    for (p, s) in cells {
        blocks.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&s).map_err(|e| e.to_string())?));
    }
    let sp = FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(l.x + 3, 26, 11),
        lo: BlockPos::new(l.x - 6, 19, 5),
        hi: BlockPos::new(l.x + 4, 27, 11),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok((sp, drive, slime, empty))
}

/// One run: the drive block at the drive cell from t0 for `width` gt; a restart (`restart` = (tick,
/// order)) between ticks; R's rise and fall ticks (after each tick). Also the reach facts read with the
/// drive on (t0 + 14): the slime's six neighbours' blocks, the feed dust's state, the candidate cells.
struct Out {
    rise: Option<i64>,
    fall: Option<i64>,
    slime_nb: Vec<String>,
    dust: String,
    r_state: String,
    empty_ok: bool,
}

fn run_once(l: &Lane, t0: i64, width: i64, restart: Option<(i64, crate::liveness::Restart)>) -> Result<Out, String> {
    let (sp, drive, slime, empty) = spec(l)?;
    let sp = crate::mirror::checked(&sp)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let r = [l.x - 2, 24, 7];
    let mut out = Out { rise: None, fall: None, slime_nb: Vec::new(), dust: String::new(), r_state: String::new(), empty_ok: true };
    let mut was = false;
    for t in 0..(t0 + width + 40) {
        if let Some((rt, how)) = restart
            && rt == t
        {
            crate::liveness::restart(&mut run.world, how)?;
        }
        if t == t0 {
            run.world.set_block(bp(drive), on.clone(), redsim::world::flags::ALL);
        }
        if t == t0 + width {
            run.world.set_block(bp(drive), air.clone(), redsim::world::flags::ALL);
        }
        run.tick();
        let now = run.world.get(bp(r)).to_string().contains("powered=true");
        if now && !was && out.rise.is_none() {
            out.rise = Some(t);
        }
        if !now && was && out.fall.is_none() {
            out.fall = Some(t);
        }
        was = now;
        if t == t0 + 14 {
            if let Some(s) = slime {
                for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    let q = [s[0] + d[0], s[1] + d[1], s[2] + d[2]];
                    let name = run.world.get(bp(q)).to_string();
                    out.slime_nb.push(format!("({},{},{}) {}", q[0], q[1], q[2], name.trim_start_matches("minecraft:")));
                }
            }
            out.dust = run.world.get(bp([l.x - 2, 23, 9])).to_string();
            out.r_state = run.world.get(bp(r)).to_string();
            out.empty_ok = empty.iter().all(|&p| run.world.get(bp(p)).to_string() == "minecraft:air");
        }
    }
    Ok(out)
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// The bench: per lane, R's rise and fall after the drive's at 20 phases; with a restart at every tick
/// t0 - 1 .. t0 + 10 and t1 - 1 .. t1 + 10 in every reload order; the reach facts.
pub fn bench(width: i64) -> Result<Vec<String>, String> {
    let mut lines = vec![format!(
        "the siding feeds (ironspec 11:22: lanes -1 and 3 fed from below, lane 7 as ruled), each lane's cells from rev 3 with the context touching them, the line's input driven {width} gt; R = the S-bend's powered rail (x-2,24,7); every run's spec checked: 0 cells outside its world's box{}",
        crate::mirror::tag()
    )];
    for l in &LANES {
        let (mut rise, mut fall) = (Hist::new(), Hist::new());
        let mut base: Option<(i64, i64)> = None;
        for t0 in 40..60 {
            let o = run_once(l, t0, width, None)?;
            if let (Some(r), Some(f)) = (o.rise, o.fall) {
                // (G1 on 9c30d6dc3: one reference, one number: the drive is placed before tick t0 and R is
                // read after each tick, so R powered after tick t0 + k is k + 1 gt after the drive)
                *rise.entry(r - t0 + 1).or_default() += 1;
                *fall.entry(f - (t0 + width) + 1).or_default() += 1;
                base.get_or_insert((r - t0, f - (t0 + width)));
            }
        }
        let t0 = 40;
        let (mut n, mut same, mut diff) = (0u32, 0u32, Vec::<String>::new());
        for rt in ((t0 - 1)..=(t0 + 10)).chain((t0 + width - 1)..=(t0 + width + 10)) {
            for how in crate::liveness::Restart::rows() {
                let o = run_once(l, t0, width, Some((rt, how)))?;
                n += 1;
                let got = (o.rise.map(|r| r - t0), o.fall.map(|f| f - (t0 + width)));
                if Some((got.0.unwrap_or(-1), got.1.unwrap_or(-1))) == base {
                    same += 1;
                } else {
                    diff.push(format!("restart at t0 + {} ({}): R rise {:?}, fall {:?}", rt - t0, how.label(), got.0, got.1));
                }
            }
        }
        let facts = run_once(l, t0, width, None)?;
        // (from below: the dust connects only to its driver (north); the ruled feed: to ironfarm's
        // repeater (east) and its driver)
        let dust_ok = match l.feed {
            Feed::Below => facts.dust.contains("east=none") && facts.dust.contains("west=none"),
            Feed::East => facts.dust.contains("east=side") && facts.dust.contains("west=none"),
        };
        let slime_ok = facts.slime_nb.iter().all(|s| !s.contains("rail"));
        lines.push(format!(
            "  lane {} ({}): R powered (gt after the drive, gt:count; the drive placed before tick t0, R read after each tick) {}, unpowered after the drive's removal {}; with a restart at every tick around both edges in every reload order: {same} of {n} the same{}; R with the drive on: {}; the feed dust (x-2,23,9) {} ({}); {}; the candidate cells {}",
            l.x,
            match l.feed {
                Feed::Below => "fed from below: day-1's delay-3 repeater into the slime under the feed dust",
                Feed::East => "the ruled feed: day-1's delay-2 switched cell, ironfarm's delay-1 repeater, the dust on glass",
            },
            row(&rise),
            row(&fall),
            if diff.is_empty() { String::new() } else { format!(" (DIFFERENT: {})", diff.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            facts.r_state.trim_start_matches("minecraft:"),
            facts.dust.trim_start_matches("minecraft:"),
            match (l.feed, dust_ok) {
                (Feed::Below, true) => "pointing only into its driver: no east or west connection",
                (Feed::East, true) => "fed by ironfarm's repeater from the east, into its driver",
                _ => "UNEXPECTED connections",
            },
            if facts.slime_nb.is_empty() { "no powered slime under the dust".to_string() } else { format!("the powered slime's neighbours: {} ({})", facts.slime_nb.join(", "), if slime_ok { "no rail" } else { "A RAIL" }) },
            if facts.empty_ok { "empty" } else { "NOT EMPTY" }
        ));
    }
    Ok(lines)
}
