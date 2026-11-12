//! The skip arm's drop latch as blocks (day-1, Nov 12 2026; ironspec 03:27): a fall of >= 3 between two
//! daylight-detector readings sets an RS latch (tick-exact: the sun and every weather ramp change the
//! level by <= 2 between readings, so 3 is a skip). The detector is read by two comparators (LIVE1 and
//! LIVE2, the real detector on two faces; the bench feeds both from identical source lines); LIVE1 ->
//! DLY1 -> DLY2 (4 gt) -> SUB's rear, LIVE2 straight into SUB's side, so SUB (subtract) shows the
//! previous reading minus the new one at an update. The window is 4 gt: with one DLY (2 gt) SUB's own
//! 2-gt tick lands when the rear has dropped too and the difference is swallowed (the control `--dly
//! 1`). SUB -> 3 dust (the k-th carries S - (k - 1)) -> the threshold repeater on the 3rd dust (fires
//! iff S >= 3; on the 2nd it fires at 2, the natural maximum: the control `--thresh-dust 2`) at delay 3
//! -> the set side (block A) of a two-torch RS NOR latch whose cross-coupling is dust (a 4-gt loop: two
//! torch delays). Resets (trigger 2's acceptance, RISE, the window's opening, ORed) power block B;
//! LATCH is read off A through a repeater.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

/// SRC1: the first source line's comparator end (LIVE1 reads it; the line runs +z).
const SRC1: P = [1, 1, 0];

const DUST_NS: &str = "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]";
const DUST_EW: &str = "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]";

/// The piece's geometry: `dly` delay comparators between LIVE1 and SUB (2 = the design, 1 = the control
/// whose 2-gt window is swallowed), the threshold repeater on dust `td` (3 = the design, 2 = the
/// control that sets at a fall of 2). Every other cell follows from them, along -x at y 1.
#[derive(Clone, Copy, Debug)]
pub struct Geom {
    pub dly: i32,
    pub td: i32,
}

impl Geom {
    pub const DESIGN: Geom = Geom { dly: 2, td: 3 };
    fn sub_x(&self) -> i32 {
        -(1 + self.dly)
    }
    /// SRC2: the second source line's comparator end (LIVE2 reads it; the line runs -z).
    fn src2(&self) -> P {
        [self.sub_x(), 1, -2]
    }
    pub fn sub_out(&self) -> P {
        [self.sub_x() - 1, 1, 0]
    }
    pub fn thresh(&self) -> P {
        [self.sub_x() - 1 - self.td, 1, 0]
    }
    pub fn a(&self) -> P {
        [self.thresh()[0] - 1, 1, 0]
    }
    pub fn b(&self) -> P {
        [self.a()[0] - 3, 1, 0]
    }
    pub fn reset_s_drive(&self) -> P {
        [self.b()[0], 1, 2]
    }
    pub fn reset_w_drive(&self) -> P {
        [self.b()[0] - 2, 1, 0]
    }
    pub fn latch_out(&self) -> P {
        [self.a()[0], 1, 2]
    }
    /// The piece's own blocks (the bench's source lines and floors aside): LIVE1, the DLYs, SUB, LIVE2;
    /// the threshold dust + repeater; the latch (2 blocks, 2 torches, 1 + 5 dust); 2 reset repeaters; the
    /// LATCH repeater.
    pub fn blocks(&self) -> usize {
        (3 + self.dly + self.td + 1 + 10 + 2 + 1) as usize
    }
}

/// The piece and the bench's two source lines (16 dust each: a redstone block on the cell 16 - s from
/// the comparator's end gives that end s).
pub fn design() -> Design {
    design_with(Geom::DESIGN)
}

pub fn design_with(g: Geom) -> Design {
    let mut d = Design { name: format!("drop-latch-dly{}-td{}", g.dly, g.td), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut put = |p: P, s: &str| {
        d.blocks.insert(p, s.to_string());
    };
    let sx = g.sub_x();
    // (a diode's `facing` points at its input, as in the game)
    // LIVE1 (input SRC1 at its east), the DLYs, SUB (subtract; side from LIVE2 at its north)
    for x in sx + 1..=0 {
        put([x, 1, 0], "minecraft:comparator[facing=east,mode=compare,powered=false]");
    }
    put([sx, 1, 0], "minecraft:comparator[facing=east,mode=subtract,powered=false]");
    put([sx, 1, -1], "minecraft:comparator[facing=north,mode=compare,powered=false]");
    // the threshold: td dust, the repeater on the last (delay 3)
    for k in 1..=g.td {
        put([sx - k, 1, 0], DUST_EW);
    }
    put(g.thresh(), "minecraft:repeater[delay=3,facing=east,locked=false,powered=false]");
    // the RS NOR latch: A, its wall torch on the west face, dust into B; B's wall torch on its north
    // face, the return dust west-to-east at z -2 and down into A from the north
    let (ax, bx) = (g.a()[0], g.b()[0]);
    put(g.a(), "minecraft:stone");
    put([ax - 1, 1, 0], "minecraft:redstone_wall_torch[facing=west,lit=true]");
    put([ax - 2, 1, 0], DUST_EW);
    put(g.b(), "minecraft:stone");
    put([bx, 1, -1], "minecraft:redstone_wall_torch[facing=north,lit=true]");
    for x in bx..ax {
        put([x, 1, -2], DUST_EW);
    }
    put([ax, 1, -2], DUST_NS);
    put([ax, 1, -1], DUST_NS);
    // the resets into B (south and west; the third input ORs into either drive)
    put([bx, 1, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
    put([bx - 1, 1, 0], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]");
    // LATCH read off A through a repeater (input A at its north)
    put([ax, 1, 1], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
    // the bench's source lines (16 dust each); SRC1's end connects west into LIVE1 (SRC2's end is N-S:
    // LIVE2 is south of it)
    let s2 = g.src2();
    for k in 0..16 {
        put([SRC1[0], 1, SRC1[2] + k], DUST_NS);
        put([s2[0], 1, s2[2] - k], DUST_NS);
    }
    put(SRC1, "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    // stone under everything
    let cells: Vec<P> = d.blocks.keys().copied().collect();
    for p in cells {
        d.blocks.entry([p[0], 0, p[2]]).or_insert_with(|| "minecraft:stone".to_string());
    }
    d.ports.insert("output".into(), g.latch_out());
    d
}

/// The bench: at every gt offset 0..19 (relative to a 20-gt detector update grid), each case moves both
/// sources from level `from` to `to` in one tick (a detector update), with the latch reset before:
/// LATCH must end set iff the fall is >= 3. Then each reset path clears a set latch. Prints the
/// failures, SUB's pulse widths, the threshold repeater's output width per case, and the source's
/// self-check (the level LIVE1 reads is the case's).
pub fn bench(spec: &FarmSpec, g: Geom) -> Result<Vec<String>, String> {
    let cases: [(i32, i32); 9] = [(9, 8), (9, 7), (9, 6), (15, 4), (2, 0), (3, 0), (7, 9), (8, 6), (8, 5)];
    let offsets = 20i64;
    let per = 120i64; // gt per trial
    let horizon = cases.len() as i64 * offsets * per + 400;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let dust = Block::parse(DUST_NS).map_err(|e| e.to_string())?;
    let all = redsim::world::flags::ALL;
    let bp = |p: P| BlockPos::new(p[0], p[1], p[2]);
    // (a source at level s: its block on the cell 16 - s from the comparator's end, so the end dust
    // reads 15 - (16 - s - 1) = s; 0 = no block)
    let src_cell = |line: P, dir: i32, s: i32| -> Option<BlockPos> { (s > 0).then(|| BlockPos::new(line[0], 1, line[2] + dir * (16 - s))) };
    let s2 = g.src2();
    let set_src = |run: &mut Run, prev: i32, s: i32| {
        for (line, dir) in [(SRC1, 1), (s2, -1)] {
            if let Some(p) = src_cell(line, dir, prev) {
                run.world.set_block(p, dust.clone(), all);
            }
            if let Some(p) = src_cell(line, dir, s) {
                run.world.set_block(p, on.clone(), all);
            }
        }
    };
    let latch_out = g.latch_out();
    let latch = |run: &Run| run.world.best_neighbor_signal(bp(latch_out)) > 0;
    let (sub_out, thresh) = (g.sub_out(), g.thresh());
    let mut fails: Vec<String> = Vec::new();
    let mut src_bad: Vec<String> = Vec::new();
    let mut sub_widths: std::collections::BTreeMap<(i32, i32), (i64, i64)> = Default::default();
    let mut thr_widths: std::collections::BTreeMap<(i32, i32), (i64, i64)> = Default::default();
    let (mut trials, mut ok) = (0u32, 0u32);
    let mut level = 0i32;
    let mut t = 0i64;
    let tick = |run: &mut Run, t: &mut i64| {
        run.tick();
        *t += 1;
    };
    // (the level LIVE1 sends into the cell west of it: the source's self-check)
    let probe = |run: &Run| run.world.best_neighbor_signal(BlockPos::new(-1, 1, 0));
    for &(from, to) in &cases {
        for off in 0..offsets {
            // reset the latch (a 6-gt pulse on the south reset), settle at `from`
            set_src(&mut run, level, from);
            level = from;
            run.world.set_block(bp(g.reset_s_drive()), on.clone(), all);
            for _ in 0..6 {
                tick(&mut run, &mut t);
            }
            run.world.set_block(bp(g.reset_s_drive()), air.clone(), all);
            // (wait until t = off mod 20, then 40 gt more: the update lands at off on a 20-gt grid)
            while t.rem_euclid(20) != off {
                tick(&mut run, &mut t);
            }
            for _ in 0..40 {
                tick(&mut run, &mut t);
            }
            let pre = latch(&run);
            if probe(&run) != from {
                src_bad.push(format!("{from}->{to} at {off}: before {}", probe(&run)));
            }
            // the update: both sources from `from` to `to` in one tick
            set_src(&mut run, level, to);
            level = to;
            let (mut sub_on, mut thr_on) = (0i64, 0i64);
            for _ in 0..(per - 46) {
                tick(&mut run, &mut t);
                if matches!(run.world.get(bp(sub_out)), Block::Wire { power, .. } if power > 0) {
                    sub_on += 1;
                }
                if run.world.get(bp(thresh)).to_string().contains("powered=true") {
                    thr_on += 1;
                }
            }
            if probe(&run) != to {
                src_bad.push(format!("{from}->{to} at {off}: after {}", probe(&run)));
            }
            let want = from - to >= 3;
            let got = latch(&run);
            trials += 1;
            if got == want && !pre {
                ok += 1;
            } else if fails.len() < 8 {
                fails.push(format!("{from}->{to} at offset {off}: latch {got} (want {want}, before the update {pre})"));
            }
            let e = sub_widths.entry((from, to)).or_insert((i64::MAX, i64::MIN));
            *e = (e.0.min(sub_on), e.1.max(sub_on));
            let e = thr_widths.entry((from, to)).or_insert((i64::MAX, i64::MIN));
            *e = (e.0.min(thr_on), e.1.max(thr_on));
        }
    }
    // each reset clears a set latch (set it with a 9 -> 6 fall first)
    let mut reset_rows = Vec::new();
    for (name, drive) in [("the south reset", g.reset_s_drive()), ("the west reset", g.reset_w_drive())] {
        set_src(&mut run, level, 9);
        level = 9;
        for _ in 0..40 {
            tick(&mut run, &mut t);
        }
        set_src(&mut run, level, 6);
        level = 6;
        for _ in 0..40 {
            tick(&mut run, &mut t);
        }
        let was = latch(&run);
        run.world.set_block(bp(drive), on.clone(), all);
        for _ in 0..6 {
            tick(&mut run, &mut t);
        }
        run.world.set_block(bp(drive), air.clone(), all);
        for _ in 0..30 {
            tick(&mut run, &mut t);
        }
        reset_rows.push(format!("{name}: set before {was}, after {}", latch(&run)));
    }
    let w = |m: &std::collections::BTreeMap<(i32, i32), (i64, i64)>| -> String {
        m.iter().map(|((f, to), (lo, hi))| format!("{f}->{to}: {lo}..{hi}")).collect::<Vec<_>>().join(", ")
    };
    let role = match (g.dly, g.td) {
        (2, 3) => "the design".to_string(),
        (1, _) => "the control with one DLY (a 2-gt window): must fail".to_string(),
        (_, 2) => "the control with the threshold on the 2nd dust: must fail (sets at a fall of 2)".to_string(),
        _ => format!("dly {} td {}", g.dly, g.td),
    };
    Ok(vec![
        format!(
            "drop latch, {role} ({} blocks: {} comparators (LIVE1, {} DLY, SUB, LIVE2), {} dust + the threshold repeater at delay 3, the RS NOR latch's 2 blocks + 2 torches + 6 dust, 2 reset repeaters, the LATCH repeater): every case at gt offsets 0..19, the latch reset before: {ok} of {trials} right (set iff the fall >= 3){}",
            g.blocks(),
            3 + g.dly,
            g.dly,
            g.td,
            if fails.is_empty() { String::new() } else { format!(" (FAIL: {})", fails.join("; ")) }
        ),
        format!("  SUB's pulse (gt powered, min..max over the offsets): {}", w(&sub_widths)),
        format!("  the threshold repeater's output (gt): {}", w(&thr_widths)),
        format!("  resets: {}", reset_rows.join("; ")),
        format!(
            "  the source's self-check (LIVE1's level = the case's from / to): {}",
            if src_bad.is_empty() { "every trial".to_string() } else { format!("FAIL {} ({})", src_bad.len(), src_bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
        ),
    ])
}
