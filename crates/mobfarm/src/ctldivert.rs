//! The divert's single atomic read as blocks (day-1, Nov 12 2026; ironspec 03:27): the daylight
//! inhibit sampled once into one latch at the divert decision, so a same-tick change gives a whole pick
//! (held at the queue) or a whole non-pick, never a junction flipping under a cart. DECIDE = DIV - DAY
//! (a subtract comparator: 15 at night with the lane armed, 0 by day); a repeater H passes it to the
//! junction line only while its side lock L is off, and holds it otherwise. L is fed by a torch whose
//! block is powered by E, a rising-edge pulse of D2 (the split's arrival detector): E (subtract) reads
//! D2's dust in its rear and D2 delayed 4 gt on its side, so it pulses 4 gt at D2's rise and never at
//! its fall, whatever D2's length (a detector rail stays up >= 20 gt). The FIFO push stays the cart's
//! own pass over the 4th route's detector D3, so it follows the cart's real route. Controls: `level`
//! (L fed by NOT D2: the sample at D2's fall, 20+ gt late) and `none` (no hold: the junction follows
//! DAY live).

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

pub const DIV_DRIVE: P = [2, 1, 0];
pub const DAY_DRIVE: P = [0, 1, -2];
pub const D2_DRIVE: P = [-1, 1, -6];
/// H's output cell: the junction's control line.
pub const JUNCTION: P = [-2, 1, 0];
pub const H: P = [-1, 1, 0];
pub const L: P = [-1, 1, -1];
/// cc767b78's junction bar: a junction switched within 14 gt of its detector.
pub const SWITCH_BAR: i64 = 14;

const DUST: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";

/// The hold's trigger: E's rising-edge pulse (the design), NOT D2 (`level`, the sample at D2's fall) or
/// none (H never locked).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Edge,
    Level,
    None,
}

pub fn design() -> Design {
    design_with(Mode::Edge)
}

pub fn design_with(mode: Mode) -> Design {
    let mut d = Design { name: format!("divert-hold-{mode:?}").to_lowercase(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    // (a diode's `facing` points at its input, as in the game)
    // DECIDE: comparator X (subtract), rear DIV through a repeater, side DAY through a repeater
    put([0, 1, 0], "minecraft:comparator[facing=east,mode=subtract,powered=false]");
    put([1, 1, 0], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
    put([0, 1, -1], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
    put(H, "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
    if mode != Mode::None {
        // L (input from the torch at its north, into H's north side); the torch at (-1,1,-2) on the
        // south face of block (-1,1,-3)
        put(L, "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
        put([-1, 1, -2], "minecraft:redstone_wall_torch[facing=south,lit=true]");
        put([-1, 1, -3], "minecraft:stone");
        put([-1, 1, -5], DUST);
    }
    match mode {
        Mode::Edge => {
            // E: subtract, rear D2's dust at (-1,1,-5), side D2 delayed by R2 (delay 2 = 4 gt) at its west
            put([-1, 1, -4], "minecraft:comparator[facing=north,mode=subtract,powered=false]");
            for p in [[-2, 1, -5], [-3, 1, -5], [-3, 1, -4]] {
                put(p, DUST);
            }
            put([-2, 1, -4], "minecraft:repeater[delay=2,facing=west,locked=false,powered=false]");
        }
        Mode::Level => {
            // the torch's block powered by D2 itself through a repeater
            put([-1, 1, -4], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
        }
        Mode::None => {}
    }
    let cells: Vec<P> = d.blocks.keys().copied().collect();
    for p in cells {
        d.blocks.entry([p[0], 0, p[2]]).or_insert_with(|| "minecraft:stone".to_string());
    }
    for p in [DIV_DRIVE, DAY_DRIVE, D2_DRIVE, JUNCTION] {
        d.blocks.entry([p[0], 0, p[2]]).or_insert_with(|| "minecraft:stone".to_string());
    }
    d.ports.insert("output".into(), JUNCTION);
    d
}

/// The piece's own blocks (floors aside): X, its 2 input repeaters, H; L, the torch, its block and D2's
/// dust; E, R2 and E's 3 more dust (the design).
pub fn piece_blocks(mode: Mode) -> usize {
    match mode {
        Mode::Edge => 4 + 4 + 5,
        Mode::Level => 4 + 4 + 1,
        Mode::None => 4,
    }
}

struct Trial {
    delta: i64,
    rising: bool,
    pick: bool,
    changes: Vec<i64>,
    hold: Option<i64>,
    open: Option<i64>,
}

/// The bench: DIV armed; the D2 pulse (`width` gt) at t0; DAY's edge (rising: night -> day, falling:
/// day -> night) at t0 + delta for every delta in -12..=16. Whole = the junction line changes at most
/// once and never after D2's rise + 14 (cc767b78's switching bar: the cart crosses the junction after
/// that); atomic = across delta each direction switches its decision once, at the same delta both ways
/// (one sampling instant: the decision is NOT DAY as it stood then). Prints the decisions, the sampling
/// instant, H's open span and the switching time (D2's rise to the junction line's change).
pub fn bench(spec: &FarmSpec, width: i64, mode: Mode) -> Result<Vec<String>, String> {
    let deltas: Vec<i64> = (-12..=16).collect();
    let per = 80i64 + width;
    let horizon = 2 * deltas.len() as i64 * (per + 60) + 200;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| BlockPos::new(p[0], p[1], p[2]);
    let junction = |run: &Run| run.world.best_neighbor_signal(bp(JUNCTION)) > 0;
    let l_on = |run: &Run| run.world.get(bp(L)).to_string().contains("powered=true");
    run.world.set_block(bp(DIV_DRIVE), on.clone(), all);
    let mut trials: Vec<Trial> = Vec::new();
    for rising in [true, false] {
        for &delta in &deltas {
            // settle with DAY at its starting level, the hold re-sampled once (a D2 pulse) so it starts
            // equal to NOT that level
            let day0 = !rising;
            run.world.set_block(bp(DAY_DRIVE), if day0 { on.clone() } else { air.clone() }, all);
            for _ in 0..10 {
                run.tick();
            }
            run.world.set_block(bp(D2_DRIVE), on.clone(), all);
            for _ in 0..width {
                run.tick();
            }
            run.world.set_block(bp(D2_DRIVE), air.clone(), all);
            for _ in 0..30 {
                run.tick();
            }
            // the trial: t = 0 at D2's rise
            let mut last = junction(&run);
            let mut tr = Trial { delta, rising, pick: last, changes: Vec::new(), hold: None, open: None };
            for t in -20i64..per {
                if t == delta {
                    run.world.set_block(bp(DAY_DRIVE), if rising { on.clone() } else { air.clone() }, all);
                }
                if t == 0 {
                    run.world.set_block(bp(D2_DRIVE), on.clone(), all);
                }
                if t == width {
                    run.world.set_block(bp(D2_DRIVE), air.clone(), all);
                }
                run.tick();
                if mode != Mode::None {
                    let lo = l_on(&run);
                    if t >= 0 && !lo && tr.open.is_none() {
                        tr.open = Some(t);
                    }
                    if tr.open.is_some() && lo && tr.hold.is_none() {
                        tr.hold = Some(t);
                    }
                }
                let j = junction(&run);
                if j != last {
                    tr.changes.push(t);
                    last = j;
                }
            }
            tr.pick = last;
            trials.push(tr);
        }
    }
    // per trial: at most one change, none after the switching bar
    let mut bad = Vec::new();
    for tr in &trials {
        if tr.changes.len() > 1 || tr.changes.iter().any(|c| *c > SWITCH_BAR) {
            bad.push(format!("DAY {} at {:+}: changes at {:?}", if tr.rising { "rises" } else { "falls" }, tr.delta, tr.changes));
        }
    }
    // across delta: one switch per direction, at the same delta both ways
    let boundary = |rising: bool| -> Result<i64, String> {
        let row: Vec<&Trial> = trials.iter().filter(|t| t.rising == rising).collect();
        // (rising: night -> day: pick while DAY's edge comes after the sample, no before it)
        let start_pick = rising;
        let switches: Vec<i64> = row.windows(2).filter(|w| w[0].pick != w[1].pick).map(|w| w[1].delta).collect();
        if switches.len() != 1 {
            return Err(format!("{} switches", switches.len()));
        }
        let b = switches[0];
        // (edges before b were seen: the decision is NOT the new level; from b on, the starting level's)
        let ok = row.iter().all(|t| if t.delta < b { t.pick != start_pick } else { t.pick == start_pick });
        if ok { Ok(b) } else { Err(format!("the switch at {b:+} is the wrong way round")) }
    };
    let (br, bf) = (boundary(true), boundary(false));
    let atomic_cross = matches!((&br, &bf), (Ok(a), Ok(b)) if a == b);
    let opens: Vec<i64> = trials.iter().filter_map(|t| t.open).collect();
    let holds: Vec<i64> = trials.iter().filter_map(|t| t.hold).collect();
    let switch_max = trials.iter().filter_map(|t| t.changes.first().copied()).max();
    let span = |v: &[i64]| -> String {
        match (v.iter().min(), v.iter().max()) {
            (Some(a), Some(b)) if a == b => a.to_string(),
            (Some(a), Some(b)) => format!("{a}..{b}"),
            _ => "-".into(),
        }
    };
    let row = |rising: bool| -> String {
        trials.iter().filter(|t| t.rising == rising).map(|t| format!("{:+}:{}", t.delta, if t.pick { "pick" } else { "no" })).collect::<Vec<_>>().join(" ")
    };
    let role = match mode {
        Mode::Edge => "the design (H unlocked by E's 4-gt pulse at D2's rise)",
        Mode::Level => "the control `level` (H unlocked while D2 is up: the sample at D2's fall)",
        Mode::None => "the control `none` (no hold: the junction follows DECIDE live)",
    };
    Ok(vec![
        format!(
            "divert hold, {role} ({} blocks; D2 {width} gt): {} of {} trials whole (the junction line changes at most once, never after D2's rise + {SWITCH_BAR}){}; one sampling instant both ways: {} (rising switches at {:?}, falling at {:?}){}; H open from D2's rise + {} to the hold at + {}; the junction line settles by D2's rise + {}",
            piece_blocks(mode),
            trials.len() - bad.len(),
            trials.len(),
            if bad.is_empty() { String::new() } else { format!(" (FAIL: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
            if atomic_cross { "yes" } else { "NO" },
            br,
            bf,
            match (&br, atomic_cross) {
                (Ok(b), true) => format!(" -> the decision is NOT DAY as it stood at D2's rise + {} (an edge at <= {:+} is seen, at >= {b:+} not)", b - 1, b - 1),
                _ => String::new(),
            },
            span(&opens),
            span(&holds),
            switch_max.map_or("- (no change seen)".into(), |v| v.to_string()),
        ),
        format!("  DAY rises (night -> day) at D2's rise + delta: {}", row(true)),
        format!("  DAY falls (day -> night) at D2's rise + delta: {}", row(false)),
    ])
}
