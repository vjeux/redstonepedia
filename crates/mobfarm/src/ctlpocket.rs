//! The pocket drive (day-1, Nov 12 2026; ironspec 15:11 / 16:03: (b) M16bw, the side drive, final once its
//! power rise matches (a)'s at every pocket). Each lane's pocket rail P (x,0,0) (a powered rail, north-south,
//! the pocket's slime back (x,0,-1)) is driven by the station's switch:
//! - (a) the slime support (x,-1,0) under P, strongly powered by a delay-1 repeater (x,-1,-1) facing north
//!   (input (x,-1,-2), the switch's cell) on glass (x,-2,-1);
//! - (b) M16bw: the support (x,-1,0) glass; a delay-1 repeater (x-1,0,0) facing west into P, on glass
//!   (x-1,-1,0), fed by the riser dust (x-2,0,0) on glass (x-2,-1,0), the step dust (x-3,-1,0) on glass
//!   (x-3,-2,0) and the feed dust: lanes 3 and 7 from the north (x-3,-1,-1) on glass (x-3,-2,-1), the
//!   switch's cell (x-3,-1,-2); lane -1 from the west (-5,-1,0) on glass (-5,-2,0) (9fc3155f: S1's ray), the
//!   switch's cell (-6,-1,0).
//! (A diode's `facing` names its input side, as in the game.)
//!
//! The bench lays all three pockets of a variant at once, places a redstone block on one lane's switch
//! cell for 30 gt at 20 phases, and prints that lane's P rise and fall after the switch's (the block placed
//! before tick t0, P read after each tick: P powered after tick t0 + k is k + 1 gt after the switch) and
//! the other pockets' rails (any rise is a leak); then the same with a restart at every tick around both
//! edges in every reload order. The control: (b)'s cells with lane -1's support a redstone block (the
//! emulation dfb89499 found leaking into lane 3's step dust, which touches it) must power lane 3's rail.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

const XS: [i32; 3] = [-1, 3, 7];
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const GLASS: &str = "minecraft:glass";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    A,
    B,
    /// (b)'s cells with lane -1's support a redstone block: the leak's positive control.
    LeakControl,
}

/// The variant's blocks and each lane's switch cell.
fn cells(v: Variant) -> (Vec<(P, String)>, [P; 3]) {
    let mut b: Vec<(P, String)> = Vec::new();
    let mut put = |p: P, s: &str| b.push((p, s.to_string()));
    let mut sw = [[0; 3]; 3];
    for (i, &x) in XS.iter().enumerate() {
        put([x, 0, 0], "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]");
        put([x, 0, -1], "minecraft:slime_block");
        match v {
            Variant::A => {
                put([x, -1, 0], "minecraft:slime_block");
                put([x, -1, -1], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
                put([x, -2, -1], GLASS);
                sw[i] = [x, -1, -2];
            }
            Variant::B | Variant::LeakControl => {
                if v == Variant::LeakControl && x == -1 {
                    put([x, -1, 0], "minecraft:redstone_block");
                } else {
                    put([x, -1, 0], GLASS);
                }
                put([x - 1, 0, 0], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]");
                put([x - 1, -1, 0], GLASS);
                put([x - 2, 0, 0], DUST);
                put([x - 2, -1, 0], GLASS);
                put([x - 3, -1, 0], DUST);
                put([x - 3, -2, 0], GLASS);
                if x == -1 {
                    put([-5, -1, 0], DUST);
                    put([-5, -2, 0], GLASS);
                    sw[i] = [-6, -1, 0];
                } else {
                    put([x - 3, -1, -1], DUST);
                    put([x - 3, -2, -1], GLASS);
                    sw[i] = [x - 3, -1, -2];
                }
            }
        }
    }
    (b, sw)
}

fn spec(v: Variant) -> Result<(FarmSpec, [P; 3]), String> {
    let (cs, sw) = cells(v);
    let mut blocks = Vec::new();
    for (p, s) in cs {
        blocks.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&s).map_err(|e| e.to_string())?));
    }
    let sp = FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(9, 1, 3),
        lo: BlockPos::new(-8, -3, -3),
        hi: BlockPos::new(10, 2, 3),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok((sp, sw))
}

/// One run: lane `lane`'s switch on from t0 for `width` gt; a restart between ticks; each pocket rail's
/// first rise and fall (ticks after which it reads powered / unpowered).
fn run_once(v: Variant, lane: usize, t0: i64, width: i64, restart: Option<(i64, crate::liveness::Restart)>) -> Result<[(Option<i64>, Option<i64>); 3], String> {
    let (sp, sw) = spec(v)?;
    let sp = crate::mirror::checked(&sp)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let mut out = [(None, None); 3];
    let mut was = [false; 3];
    // (the rails as settled, before any switch: the control's lane 3 is already powered by the leak)
    for (i, &x) in XS.iter().enumerate() {
        was[i] = run.world.get(bp([x, 0, 0])).to_string().contains("powered=true");
        if was[i] {
            out[i].0 = Some(-1);
        }
    }
    for t in 0..(t0 + width + 30) {
        if let Some((rt, how)) = restart
            && rt == t
        {
            crate::liveness::restart(&mut run.world, how)?;
        }
        if t == t0 {
            run.world.set_block(bp(sw[lane]), on.clone(), redsim::world::flags::ALL);
        }
        if t == t0 + width {
            run.world.set_block(bp(sw[lane]), air.clone(), redsim::world::flags::ALL);
        }
        run.tick();
        for (i, &x) in XS.iter().enumerate() {
            let now = run.world.get(bp([x, 0, 0])).to_string().contains("powered=true");
            if now && !was[i] && out[i].0.is_none() {
                out[i].0 = Some(t);
            }
            if !now && was[i] && out[i].1.is_none() {
                out[i].1 = Some(t);
            }
            was[i] = now;
        }
    }
    Ok(out)
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    if m.is_empty() {
        return "never".to_string();
    }
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// The bench (see the module's doc).
pub fn bench() -> Result<Vec<String>, String> {
    let width = 30i64;
    let mut lines = vec![format!(
        "the pocket drive (ironspec 16:03: (b) M16bw final once its power rise matches (a)'s at every pocket): all three pockets of a variant laid at once, one lane's switch cell driven {width} gt at 20 phases; P = the pocket rail (x,0,0); every run's spec checked: 0 cells outside its world's box{}",
        crate::mirror::tag()
    )];
    let mut rises: std::collections::BTreeMap<(usize, &str), Hist> = Default::default();
    for (v, vname) in [(Variant::A, "(a) the slime support (x,-1,0) under P, its delay-1 repeater (x,-1,-1)"), (Variant::B, "(b) M16bw: the side repeater (x-1,0,0) into P, the riser (x-2,0,0), the step (x-3,-1,0), the feed (lane -1 from the west)")] {
        lines.push(format!("  {vname}:"));
        for (i, &x) in XS.iter().enumerate() {
            let (mut rise, mut fall, mut leaks) = (Hist::new(), Hist::new(), 0u32);
            let mut base: Option<(i64, i64)> = None;
            for t0 in 40..60 {
                let o = run_once(v, i, t0, width, None)?;
                if let (Some(r), Some(f)) = o[i] {
                    *rise.entry(r - t0 + 1).or_default() += 1;
                    *fall.entry(f - (t0 + width) + 1).or_default() += 1;
                    base.get_or_insert((r - t0, f - (t0 + width)));
                }
                leaks += o.iter().enumerate().filter(|(j, s)| *j != i && s.0.is_some()).count() as u32;
            }
            rises.insert((i, if v == Variant::A { "a" } else { "b" }), rise.clone());
            let t0 = 40;
            let (mut n, mut same, mut diff) = (0u32, 0u32, Vec::<String>::new());
            for rt in ((t0 - 1)..=(t0 + 6)).chain((t0 + width - 1)..=(t0 + width + 6)) {
                for how in crate::liveness::Restart::rows() {
                    let o = run_once(v, i, t0, width, Some((rt, how)))?;
                    n += 1;
                    let got = (o[i].0.map(|r| r - t0), o[i].1.map(|f| f - (t0 + width)));
                    let leak = o.iter().enumerate().any(|(j, s)| j != i && s.0.is_some());
                    if Some((got.0.unwrap_or(-99), got.1.unwrap_or(-99))) == base && !leak {
                        same += 1;
                    } else {
                        diff.push(format!("restart at t0 + {} ({}): P rise {:?}, fall {:?}, leak {leak}", rt - t0, how.label(), got.0, got.1));
                    }
                }
            }
            lines.push(format!(
                "    lane {x}: P powered (gt after the switch, gt:count) {}, unpowered after the switch's removal {}; the other pockets' rails rose {leaks} times (a leak); with a restart at every tick around both edges in every reload order: {same} of {n} the same{}",
                row(&rise),
                row(&fall),
                if diff.is_empty() { String::new() } else { format!(" (DIFFERENT: {})", diff.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) }
            ));
        }
    }
    let verdict: Vec<String> = XS
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let (a, b) = (rises.get(&(i, "a")), rises.get(&(i, "b")));
            format!("lane {x} {}", if a.is_some() && a == b { "the same" } else { "DIFFERENT" })
        })
        .collect();
    lines.push(format!("  (b)'s rise against (a)'s, phase by phase: {}", verdict.join(", ")));
    // the control: lane -1's support a redstone block (no switch driven: lane 0's run with an idle cell)
    let o = run_once(Variant::LeakControl, 0, 1_000, width, None)?;
    lines.push(format!(
        "  the control ((b)'s cells, lane -1's support (-1,-1,0) a redstone block, no switch on): lane 3's rail {} (lane 3's step dust (0,-1,0) touches that support), lane 7's {}: {}",
        if o[1].0.is_some() { "powered" } else { "unpowered" },
        if o[2].0.is_some() { "powered" } else { "unpowered" },
        if o[1].0.is_some() { "the leak shows (the bench sees it)" } else { "NO LEAK SEEN (the control fails)" }
    ));
    Ok(lines)
}
