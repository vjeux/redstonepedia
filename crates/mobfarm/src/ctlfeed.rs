//! The siding feeds (day-1, Nov 12 2026; ironspec 11:22: (a), the feed from below, for lanes -1 and 3,
//! lane 7 as ruled). Each lane's S-bend R (x-2,24,7), a powered rail on a slime block (x-2,23,7), is
//! powered by ironfarm's driver (x-2,23,8), a delay-1 repeater facing south reading the feed dust
//! (x-2,23,9). The ruled feed (lane 7) reaches that dust from the east: day-1's switched cell (x,23,9), a
//! delay-2 repeater whose input is (x+1,23,9), into ironfarm's delay-1 repeater (x-1,23,9) facing east,
//! into the dust. Lanes -1 and 3 cannot be fed from the east without a cell in a kept seat candidate's
//! head cell ((0,23,9), (4,23,9)), so their dust stands on a slime block (x-2,22,9), strongly powered from
//! the side by day-1's delay-3 repeater: lane -1's at (-4,22,9) facing west (input the dust (-5,22,9)),
//! lane 3's at (2,22,9) facing east (input the dust (3,22,9), under glass (3,23,9)). Both paths take 8 gt
//! from the line's input to R: lanes -1 / 3 6 + 0 + 2 (day-1's delay-3 repeater, the slime-supported dust,
//! ironfarm's delay-1 driver), lane 7 4 + 2 + 2 (day-1's delay-2 switched cell, ironfarm's delay-1 repeater,
//! its delay-1 driver): R powered 8 gt after the line's input in every lane, the bench's judged bar.
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
/// candidate cells that must stay empty). `slow`: the control lane (the chain fix), day-1's own repeater one tick
/// slower (delay 3 -> 4 from below, 2 -> 3 for the ruled feed), which the 8-gt bar must catch.
fn lane_cells(l: &Lane, slow: bool) -> (Vec<(P, String)>, P, Option<P>, Vec<P>) {
    let x = l.x;
    let mut b: Vec<(P, String)> = Vec::new();
    let mut put = |p: P, s: &str| b.push((p, s.to_string()));
    let below_rep = if slow { 4 } else { 3 };
    let east_rep = if slow { 3 } else { 2 };
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
            put([-4, 22, 9], &format!("minecraft:repeater[delay={below_rep},facing=west,locked=false,powered=false]"));
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
            put([2, 22, 9], &format!("minecraft:repeater[delay={below_rep},facing=east,locked=false,powered=false]"));
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
            put([x, 23, 9], &format!("minecraft:repeater[delay={east_rep},facing=east,locked=false,powered=false]"));
            put([x, 22, 9], GLASS);
            put([x + 1, 23, 9], DUST);
            put([x + 1, 22, 9], GLASS);
            ([x + 2, 23, 9], None, vec![[4, 22, 9], [4, 23, 9]])
        }
    };
    (b, drive, slime, empty)
}

fn spec(l: &Lane, slow: bool) -> Result<(FarmSpec, P, Option<P>, Vec<P>), String> {
    let (cells, drive, slime, empty) = lane_cells(l, slow);
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
    /// (G1 on 7ccd7737c point 5) every edge, so an extra pulse from a reload is seen
    rises: Vec<i64>,
    falls: Vec<i64>,
    slime_nb: Vec<(P, String)>,
    dust: String,
    r_state: String,
    empty_ok: bool,
    /// (the override rule) the drive cell's state before -> after the drive is placed.
    drive_before: String,
    drive_after: String,
}

fn run_once(l: &Lane, t0: i64, width: i64, restart: Option<(i64, crate::liveness::Restart)>, slow: bool) -> Result<Out, String> {
    let (sp, drive, slime, empty) = spec(l, slow)?;
    let sp = crate::mirror::checked(&sp)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let r = [l.x - 2, 24, 7];
    let mut out = Out { rise: None, fall: None, rises: Vec::new(), falls: Vec::new(), slime_nb: Vec::new(), dust: String::new(), r_state: String::new(), empty_ok: true, drive_before: String::new(), drive_after: String::new() };
    let mut was = false;
    for t in 0..(t0 + width + 40) {
        if let Some((rt, how)) = restart
            && rt == t
        {
            crate::liveness::restart(&mut run.world, how)?;
        }
        if t == t0 {
            out.drive_before = run.world.get(bp(drive)).to_string();
            run.world.set_block(bp(drive), on.clone(), redsim::world::flags::ALL);
            out.drive_after = run.world.get(bp(drive)).to_string();
        }
        if t == t0 + width {
            run.world.set_block(bp(drive), air.clone(), redsim::world::flags::ALL);
        }
        run.tick();
        let now = run.world.get(bp(r)).to_string().contains("powered=true");
        if now && !was {
            out.rises.push(t);
            if out.rise.is_none() {
                out.rise = Some(t);
            }
        }
        if !now && was {
            out.falls.push(t);
            if out.fall.is_none() {
                out.fall = Some(t);
            }
        }
        was = now;
        if t == t0 + 14 {
            if let Some(s) = slime {
                for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    let q = [s[0] + d[0], s[1] + d[1], s[2] + d[2]];
                    out.slime_nb.push((q, run.world.get(bp(q)).to_string()));
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
/// t0 - 1 .. t0 + 10 and t1 - 1 .. t1 + 10 in every reload order; the reach facts. (The chain fix) every bar
/// through `v`: R powered exactly 8 gt after the drive (the PRESENT bar's 14 = k_sw + 8, ironspec 05:03 PT), one
/// rise and one fall per phase, every restart row the same, the dust's connections, the slime's neighbours, the
/// candidate cells; the control: the lane with day-1's repeater one tick slower must break the 8-gt bar.
pub fn bench(width: i64, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    // (G1's audit, hole 16: no silent clamp; a drive of 30 gt is the design's, f1q2-run's siding input)
    if width < 1 {
        return Err(format!("--width {width}: at least 1 gt"));
    }
    const R_DELAY: i64 = 8;
    let mut lines = vec![format!(
        "the siding feeds (ironspec 11:22: lanes -1 and 3 fed from below, lane 7 as ruled), each lane's cells from rev 3 with the context touching them, the line's input driven {width} gt; R = the S-bend's powered rail (x-2,24,7); every run's spec checked: 0 cells outside its world's box{}",
        crate::mirror::tag()
    )];
    for l in &LANES {
        let (mut rise, mut fall) = (Hist::new(), Hist::new());
        let mut base: Option<(i64, i64)> = None;
        // (G1 on 7ccd7737c point 5: a phase with no edge, or more than one, is a failure, never a drop)
        let mut edge_bad: Vec<String> = Vec::new();
        let mut phases = 0usize;
        let mut drive_over = String::new();
        for t0 in 40..60 {
            let o = run_once(l, t0, width, None, false)?;
            phases += 1;
            if drive_over.is_empty() {
                drive_over = v.over(&format!("lane {}'s drive cell", l.x), &o.drive_before, &o.drive_after);
            }
            if o.rises.len() != 1 || o.falls.len() != 1 {
                edge_bad.push(format!("drive at t0 {t0}: R rises {:?}, falls {:?}", o.rises, o.falls));
            }
            if let (Some(r), Some(f)) = (o.rise, o.fall) {
                // (G1 on 9c30d6dc3: one reference, one number: the drive is placed before tick t0 and R is
                // read after each tick, so R powered after tick t0 + k is k + 1 gt after the drive)
                *rise.entry(r - t0 + 1).or_default() += 1;
                *fall.entry(f - (t0 + width) + 1).or_default() += 1;
                base.get_or_insert((r - t0, f - (t0 + width)));
            }
        }
        let edges = v.bar_n(&format!("lane {}: one rise and one fall at every phase", l.x), phases, edge_bad.is_empty());
        let delay_ok = !rise.is_empty() && rise.keys().all(|&k| k == R_DELAY);
        let delay = v.bar_n(&format!("lane {}: R powered {R_DELAY} gt after the drive at every phase", l.x), rise.values().sum::<u32>() as usize, delay_ok);
        let t0 = 40;
        let (mut n, mut same, mut diff) = (0u32, 0u32, Vec::<String>::new());
        for rt in ((t0 - 1)..=(t0 + 10)).chain((t0 + width - 1)..=(t0 + width + 10)) {
            for how in crate::liveness::Restart::rows() {
                let o = run_once(l, t0, width, Some((rt, how)), false)?;
                n += 1;
                // (every edge compared: exactly the base's one rise and one fall)
                let rises: Vec<i64> = o.rises.iter().map(|r| r - t0).collect();
                let falls: Vec<i64> = o.falls.iter().map(|f| f - (t0 + width)).collect();
                if base.is_some_and(|(br, bf)| rises == vec![br] && falls == vec![bf]) {
                    same += 1;
                } else {
                    diff.push(format!("restart at t0 + {} ({}): R rises {rises:?}, falls {falls:?}", rt - t0, how.label()));
                }
            }
        }
        let restarts = v.bar_n(&format!("lane {}: every restart row the same as without one", l.x), n as usize, diff.is_empty() && base.is_some());
        let facts = run_once(l, t0, width, None, false)?;
        // (from below: the dust connects only to its driver (north); the ruled feed: to ironfarm's
        // repeater (east) and its driver)
        let dust_ok = match l.feed {
            Feed::Below => facts.dust.contains("east=none") && facts.dust.contains("west=none"),
            Feed::East => facts.dust.contains("east=side") && facts.dust.contains("west=none"),
        };
        let dust_w = v.bar(&format!("lane {}: the feed dust's connections", l.x), dust_ok);
        // (G1's audit, hole 16: the slime's neighbours checked against the designed set, not by a "rail" substring: the
        // feed dust above it, day-1's repeater beside it, and air, glass or glowstone otherwise)
        let feed_dust = [l.x - 2, 23, 9];
        let slime_ok = facts.slime_nb.iter().all(|(q, s)| {
            *q == feed_dust && s.contains("redstone_wire") || s.contains("minecraft:repeater") && (q[0] == -4 || q[0] == 2) && q[1] == 22 || s == "minecraft:air" || s == "minecraft:glass" || s == "minecraft:glowstone"
        });
        let slime_w = match l.feed {
            Feed::Below => v.bar_n(&format!("lane {}: the powered slime's neighbours are the designed set", l.x), facts.slime_nb.len(), slime_ok),
            Feed::East => v.scope(&format!("lane {}'s powered slime", l.x), "the ruled feed has none"),
        };
        let empty_w = v.bar(&format!("lane {}: the candidate cells empty", l.x), facts.empty_ok);
        // (the control: the same lane with day-1's repeater one tick slower, through the same run)
        let c = run_once(l, t0, width, None, true)?;
        let c_rise = c.rise.map(|r| r - t0 + 1);
        let ctl = v.control(&format!("lane {}'s slower repeater breaks the {R_DELAY}-gt bar", l.x), usize::from(c_rise.is_some()), c_rise.is_some_and(|k| k != R_DELAY));
        lines.push(format!(
            "  lane {} ({}): {drive_over}; R powered (gt after the drive, gt:count; the drive placed before tick t0, R read after each tick) {} (bar {R_DELAY}: {delay}), unpowered after the drive's removal {}; phases without exactly one rise and one fall {} ({edges}){}; with a restart at every tick around both edges in every reload order, every edge compared: {same} of {n} the same ({restarts}){}; R with the drive on: {}; the feed dust (x-2,23,9) {} ({dust_w}); {}; the candidate cells {} ({empty_w}); CONTROL (the repeater one tick slower): R powered {} gt after the drive ({ctl})",
            l.x,
            match l.feed {
                Feed::Below => "fed from below: day-1's delay-3 repeater into the slime under the feed dust",
                Feed::East => "the ruled feed: day-1's delay-2 switched cell, ironfarm's delay-1 repeater, the dust on glass",
            },
            row(&rise),
            row(&fall),
            edge_bad.len(),
            if edge_bad.is_empty() { String::new() } else { format!(" ({})", edge_bad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            if diff.is_empty() { String::new() } else { format!(" (the differing rows: {})", diff.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            facts.r_state.trim_start_matches("minecraft:"),
            facts.dust.trim_start_matches("minecraft:"),
            if facts.slime_nb.is_empty() {
                slime_w
            } else {
                format!("the powered slime's neighbours: {} ({slime_w})", facts.slime_nb.iter().map(|(q, s)| format!("({},{},{}) {}", q[0], q[1], q[2], s.trim_start_matches("minecraft:"))).collect::<Vec<_>>().join(", "))
            },
            if facts.empty_ok { "empty" } else { "NOT EMPTY" },
            c_rise.map_or("NEVER".to_string(), |k| k.to_string())
        ));
    }
    Ok(lines)
}
